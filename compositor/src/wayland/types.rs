use std::{
	collections::{HashMap, VecDeque},
	io::Write,
	ops::Deref,
	os::{fd::OwnedFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;
use thiserror::Error;

use crate::{
	VideoBuffer,
	wayland::{
		buffer::Buffer,
		compositor::Compositor,
		display::Display,
		keyboard::Keyboard,
		output::{DisplayGeometry, Output},
		pointer::Pointer,
		registry::Registry,
		seat::Seat,
		shm::{SharedMemory, SharedMemoryPool},
		surface::Surface,
		xdg_surface::XDGSurface,
		xdg_toplevel::XdgTopLevel,
		xdg_wm_base::XdgWmBase,
	},
};

pub trait SubSystem {
	type Request: CommandRegistry;
	const NAME: &'static str;
	const VERSION: u32 = 1;
	fn parse_command(&self, command: WaylandPacket, fds: &mut VecDeque<OwnedFd>) -> Option<Self::Request> {
		Self::Request::parse(command, fds)
	}
}

#[derive(Debug, Error)]
pub enum WaylandError {
	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
	#[error("Nix error: {0}")]
	NixError(#[from] nix::Error),
	#[error("Unrecognised object")]
	UnrecognisedObject,
}

pub type WaylandResult<T> = Result<T, WaylandError>;

pub enum ClientEffect {
	Register(u32, SubsystemType),
	Unregister(u32),
	DestroySelf,
}

pub trait Command<T: SubSystem>
where
	Self: Sized,
{
	fn handle(&self, connection: &Arc<UnixStream>, subsystem: &mut T) -> WaylandResult<Option<ClientEffect>>;
}

pub trait CommandRegistry {
	fn parse(command: WaylandPacket, fds: &mut VecDeque<OwnedFd>) -> Option<Self>
	where
		Self: std::marker::Sized;
}

pub trait FromPacket: Sized {
	fn from_packet(packet: WaylandPacket, fds: &mut VecDeque<OwnedFd>) -> Option<Self>;
}

impl<T: ReadFromWithEndian> FromPacket for T {
	fn from_packet(packet: WaylandPacket, _fds: &mut VecDeque<OwnedFd>) -> Option<Self> {
		T::read_from_with_endian(&mut std::io::Cursor::new(packet.payload), Endian::Little).ok()
	}
}

pub struct WithFd<T> {
	pub cmd: T,
	pub fd: OwnedFd,
}

impl<T: ReadFromWithEndian> FromPacket for WithFd<T> {
	fn from_packet(packet: WaylandPacket, fds: &mut VecDeque<OwnedFd>) -> Option<Self> {
		let cmd = T::read_from_with_endian(&mut std::io::Cursor::new(packet.payload), Endian::Little).ok()?;
		let fd = fds.pop_front()?;
		Some(Self { cmd, fd })
	}
}

pub struct Client {
	pub connection: Arc<UnixStream>,
	pub objects: HashMap<u32, SubsystemType>,
	fds: VecDeque<OwnedFd>,
}

impl Client {
	pub fn new(connection: Arc<UnixStream>, display_geometry: DisplayGeometry) -> Self {
		let mut objects = HashMap::new();
		objects.insert(1, SubsystemType::Display(Display::new(display_geometry)));
		Self {
			connection,
			objects,
			fds: VecDeque::new(),
		}
	}

	pub fn repaint(&mut self, framebuffer: &mut VideoBuffer) {
		let mut blitted: Vec<(u32, u32)> = Vec::new(); // (surface_id, buffer_id)
		// For each surface with a committed buffer, blit the buffer to the framebuffer.
		for (surface_id, subsystem) in self.objects.iter() {
			if let SubsystemType::Surface(surface) = subsystem
				&& surface.committed
				&& let Some((buffer_id, _, _)) = surface.attached_buffer
			{
				if let Some(xdg_surface) = self
					.objects
					.values()
					.find(|v| matches!(v, SubsystemType::XdgSurface(x) if x.surface_id == *surface_id))
					&& let SubsystemType::XdgSurface(xdg_surface) = xdg_surface
					&& !xdg_surface.configured
				{
					continue; // skip if the surface isn't configured yet
				}

				let buffer = match self.objects.get(&buffer_id) {
					Some(SubsystemType::Buffer(buffer)) => buffer,
					_ => continue, // skip if attached buffer doesn't exist or isn't a buffer
				};

				let mem_pool = match self.objects.get(&buffer.pool_id) {
					Some(SubsystemType::SharedMemoryPool(pool)) => pool,
					_ => continue, // skip if pool doesn't exist or isn't a shared memory pool
				};

				if surface.blitted {
					continue; // skip if we've already blitted this surface since the last commit
				}

				mem_pool.blit_onto(buffer, framebuffer);
				blitted.push((*surface_id, buffer_id));
			}
		}

		for (surface_id, buffer_id) in blitted {
			if let Some(SubsystemType::Surface(surface)) = self.objects.get_mut(&surface_id) {
				surface.blitted = true;
			}
			// wl_buffer.release — opcode 0, no payload
			let packet = WaylandPacket::new(buffer_id, 0, vec![]);
			let mut buf = Vec::new();
			if let Err(e) = packet.write_to_with_endian(&mut buf, Endian::Little) {
				eprintln!("Failed to write wl_buffer.release packet: {}", e);
				continue;
			}

			if let Err(e) = self.connection.as_ref().write_all(&buf) {
				eprintln!("Failed to send wl_buffer.release packet: {}", e);
				continue;
			}
		}
	}

	pub fn handle_event(&mut self, command: WaylandPacket, fds: Vec<OwnedFd>) -> WaylandResult<()> {
		self.fds.extend(fds);
		let object_id = command.object_id;
		let Some(subsystem) = self.objects.get_mut(&object_id) else {
			return Err(WaylandError::UnrecognisedObject);
		};
		match subsystem.handle_command(&self.connection, command, &mut self.fds)? {
			Some(ClientEffect::Register(id, obj)) => {
				self.objects.insert(id, obj);
			}
			Some(ClientEffect::Unregister(id)) => {
				self.objects.remove(&id);
			}
			Some(ClientEffect::DestroySelf) => {
				self.objects.remove(&object_id);
			}
			None => {}
		}
		Ok(())
	}
}

subsystem_type! {
	Display(Display),
	Registry(Registry),
	Compositor(Compositor),
	Surface(Surface),
	SharedMemory(SharedMemory),
	SharedMemoryPool(SharedMemoryPool),
	Buffer(Buffer),
	XdgWmBase(XdgWmBase),
	XdgSurface(XDGSurface),
	XdgTopLevel(XdgTopLevel),
	Seat(Seat),
	Pointer(Pointer),
	Keyboard(Keyboard),
	Output(Output),
}

#[derive(Debug, ByteStruct)]
pub struct WaylandHeader {
	pub object_id: u32,
	pub opcode: u16,
	pub data_length: u16,
}

#[derive(Debug)]
pub struct WaylandPacket {
	pub object_id: u32,
	pub opcode: u16,
	pub payload: Vec<u8>,
}

impl WaylandPacket {
	pub fn new(object_id: u32, opcode: u16, payload: Vec<u8>) -> Self {
		Self {
			object_id,
			opcode,
			payload,
		}
	}
}

impl WriteToWithEndian for WaylandPacket {
	fn write_to_with_endian<W: std::io::Write>(&self, writer: &mut W, endian: Endian) -> std::io::Result<()> {
		let header = WaylandHeader {
			object_id: self.object_id,
			opcode: self.opcode,
			data_length: self.payload.len() as u16 + 8,
		};
		header.write_to_with_endian(writer, endian)?;
		writer.write_all(&self.payload)?;
		Ok(())
	}
}

impl ReadFromWithEndian for WaylandPacket {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: Endian) -> std::io::Result<Self> {
		let header = WaylandHeader::read_from_with_endian(reader, endian)?;
		let mut payload = vec![0u8; header.data_length as usize - 8];
		reader.read_exact(&mut payload)?;
		Ok(Self::new(header.object_id, header.opcode, payload))
	}
}

#[derive(Debug)]
pub struct WaylandEncodedString(pub String);

impl Deref for WaylandEncodedString {
	type Target = String;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl WriteToWithEndian for WaylandEncodedString {
	fn write_to_with_endian<W: std::io::Write>(&self, writer: &mut W, _endian: Endian) -> std::io::Result<()> {
		writer.write_all(&(self.0.len() as u32 + 1).to_le_bytes())?;
		writer.write_all(self.0.as_bytes())?;
		writer.write_all(&[0])?; // null terminator
		let padding = (4 - (self.0.len() + 1) % 4) % 4;
		writer.write_all(&vec![0; padding])?; // padding to 4 bytes
		Ok(())
	}
}

impl ReadFromWithEndian for WaylandEncodedString {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, _endian: Endian) -> std::io::Result<Self> {
		let mut len_bytes = [0u8; 4];
		reader.read_exact(&mut len_bytes)?;
		let len = u32::from_le_bytes(len_bytes);
		let mut string_bytes = vec![0u8; len as usize];
		reader.read_exact(&mut string_bytes)?;
		// Strip the null byte
		if string_bytes.last() == Some(&0) {
			string_bytes.pop();
		} else {
			return Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				"Wayland string is not null-terminated",
			));
		}

		let padding = (4 - (len % 4)) % 4;
		let mut padding_bytes = vec![0u8; padding as usize];
		reader.read_exact(&mut padding_bytes)?;

		let string = String::from_utf8(string_bytes).map_err(|e| {
			std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Invalid UTF-8 in Wayland string: {}", e),
			)
		})?;
		Ok(Self(string))
	}
}
