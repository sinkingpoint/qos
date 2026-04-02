use std::{
	collections::HashMap,
	io::Write,
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
		registry::Registry,
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
	fn parse_command(&self, command: WaylandPacket) -> Option<Self::Request> {
		Self::Request::parse(command)
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
	fn parse(command: WaylandPacket) -> Option<Self>
	where
		Self: std::marker::Sized;
}

pub trait FromPacket: Sized {
	fn from_packet(packet: WaylandPacket) -> Option<Self>;
}

impl<T: ReadFromWithEndian> FromPacket for T {
	fn from_packet(packet: WaylandPacket) -> Option<Self> {
		T::read_from_with_endian(&mut std::io::Cursor::new(packet.payload), Endian::Little).ok()
	}
}

pub struct WithFd<T> {
	pub cmd: T,
	pub fd: OwnedFd,
}

impl<T: ReadFromWithEndian> FromPacket for WithFd<T> {
	fn from_packet(mut packet: WaylandPacket) -> Option<Self> {
		let cmd = T::read_from_with_endian(&mut std::io::Cursor::new(packet.payload), Endian::Little).ok()?;
		let fd = packet.fds.drain(..1).next()?;
		Some(Self { cmd, fd })
	}
}

pub struct Client {
	pub connection: Arc<UnixStream>,
	pub objects: HashMap<u32, SubsystemType>,
}

impl Client {
	pub fn new(connection: Arc<UnixStream>) -> Self {
		let mut objects = HashMap::new();
		objects.insert(1, SubsystemType::Display(Display {}));
		Self { connection, objects }
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

	pub fn handle_command(&mut self, command: WaylandPacket) -> WaylandResult<()> {
		let object_id = command.object_id;
		let Some(subsystem) = self.objects.get_mut(&object_id) else {
			return Err(WaylandError::UnrecognisedObject);
		};
		match subsystem.handle_command(&self.connection, command)? {
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

pub enum SubsystemType {
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
}

// TODO: Macro this
impl SubsystemType {
	pub fn name(&self) -> &'static str {
		match self {
			Self::Display(_) => Display::NAME,
			Self::Registry(_) => Registry::NAME,
			Self::Compositor(_) => Compositor::NAME,
			Self::Surface(_) => Surface::NAME,
			Self::SharedMemory(_) => SharedMemory::NAME,
			Self::SharedMemoryPool(_) => SharedMemoryPool::NAME,
			Self::Buffer(_) => Buffer::NAME,
			Self::XdgWmBase(_) => XdgWmBase::NAME,
			Self::XdgSurface(_) => XDGSurface::NAME,
			Self::XdgTopLevel(_) => XdgTopLevel::NAME,
		}
	}

	pub fn version(&self) -> u32 {
		match self {
			Self::Display(_) => Display::VERSION,
			Self::Registry(_) => Registry::VERSION,
			Self::Compositor(_) => Compositor::VERSION,
			Self::Surface(_) => Surface::VERSION,
			Self::SharedMemory(_) => SharedMemory::VERSION,
			Self::SharedMemoryPool(_) => SharedMemoryPool::VERSION,
			Self::Buffer(_) => Buffer::VERSION,
			Self::XdgWmBase(_) => XdgWmBase::VERSION,
			Self::XdgSurface(_) => XDGSurface::VERSION,
			Self::XdgTopLevel(_) => XdgTopLevel::VERSION,
		}
	}

	fn handle_command(
		&mut self,
		connection: &Arc<UnixStream>,
		command: WaylandPacket,
	) -> WaylandResult<Option<ClientEffect>> {
		match self {
			SubsystemType::Display(display) => {
				if let Some(cmd) = display.parse_command(command) {
					cmd.handle(connection, display)
				} else {
					Ok(None)
				}
			}
			SubsystemType::Registry(registry) => {
				if let Some(cmd) = registry.parse_command(command) {
					cmd.handle(connection, registry)
				} else {
					Ok(None)
				}
			}
			SubsystemType::Compositor(compositor) => {
				if let Some(cmd) = compositor.parse_command(command) {
					cmd.handle(connection, compositor)
				} else {
					Ok(None)
				}
			}
			SubsystemType::Surface(surface) => {
				if let Some(cmd) = surface.parse_command(command) {
					cmd.handle(connection, surface)
				} else {
					Ok(None)
				}
			}
			SubsystemType::SharedMemory(shared_memory) => {
				if let Some(cmd) = shared_memory.parse_command(command) {
					cmd.handle(connection, shared_memory)
				} else {
					Ok(None)
				}
			}
			SubsystemType::SharedMemoryPool(shared_memory_pool) => {
				if let Some(cmd) = shared_memory_pool.parse_command(command) {
					cmd.handle(connection, shared_memory_pool)
				} else {
					Ok(None)
				}
			}
			SubsystemType::Buffer(buffer) => {
				if let Some(cmd) = buffer.parse_command(command) {
					cmd.handle(connection, buffer)
				} else {
					Ok(None)
				}
			}
			SubsystemType::XdgWmBase(xdg_wm_base) => {
				if let Some(cmd) = xdg_wm_base.parse_command(command) {
					cmd.handle(connection, xdg_wm_base)
				} else {
					Ok(None)
				}
			}
			SubsystemType::XdgSurface(xdg_surface) => {
				if let Some(cmd) = xdg_surface.parse_command(command) {
					cmd.handle(connection, xdg_surface)
				} else {
					Ok(None)
				}
			}
			SubsystemType::XdgTopLevel(xdg_toplevel) => {
				if let Some(cmd) = xdg_toplevel.parse_command(command) {
					cmd.handle(connection, xdg_toplevel)
				} else {
					Ok(None)
				}
			}
		}
	}
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
	pub fds: Vec<OwnedFd>,
}

impl WaylandPacket {
	pub fn new(object_id: u32, opcode: u16, payload: Vec<u8>) -> Self {
		Self {
			object_id,
			opcode,
			payload,
			fds: Vec::new(),
		}
	}

	pub fn new_with_fds(object_id: u32, opcode: u16, payload: Vec<u8>, fds: Vec<OwnedFd>) -> Self {
		Self {
			object_id,
			opcode,
			payload,
			fds,
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
