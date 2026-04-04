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
		pointer::{ButtonEvent, EnterEvent, LeaveEvent, MoveEvent, Pointer},
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
	StartDrag,
	DestroySelf,
}

pub trait Command<T: SubSystem>
where
	Self: Sized,
{
	fn handle(self, connection: &Arc<UnixStream>, subsystem: &mut T) -> WaylandResult<Option<ClientEffect>>;
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

pub struct DragState {
	top_level_id: u32,
	initial_pointer: Option<(i32, i32)>,
}

pub struct Client {
	pub connection: Arc<UnixStream>,
	pub objects: HashMap<u32, SubsystemType>,
	fds: VecDeque<OwnedFd>,
	pub dragging: Option<DragState>,
}

impl Client {
	pub fn new(connection: Arc<UnixStream>, display_geometry: DisplayGeometry) -> Self {
		let mut objects = HashMap::new();
		objects.insert(1, SubsystemType::Display(Display::new(display_geometry)));
		Self {
			connection,
			objects,
			fds: VecDeque::new(),
			dragging: None,
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

				// Find the XdgTopLevel for this surface to get its position.
				let (blit_x, blit_y) = self
					.objects
					.values()
					.find_map(|obj| {
						if let SubsystemType::XdgTopLevel(toplevel) = obj {
							let xdg_surface = self.objects.get(&toplevel.xdg_surface)?;
							if let SubsystemType::XdgSurface(xdg_surface) = xdg_surface
								&& xdg_surface.surface_id == *surface_id
							{
								return Some((toplevel.x, toplevel.y));
							}
						}
						None
					})
					.unwrap_or((0, 0));

				mem_pool.blit_onto(buffer, framebuffer, blit_x, blit_y);
				blitted.push((*surface_id, buffer_id));
			}
		}

		for (surface_id, buffer_id) in blitted {
			if let Some(SubsystemType::Surface(surface)) = self.objects.get_mut(&surface_id) {
				surface.mark_blitted(&self.connection);
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

	pub fn handle_drag(&mut self, x: i32, y: i32) -> WaylandResult<()> {
		if let Some(drag_state) = &mut self.dragging {
			if drag_state.initial_pointer.is_none() {
				drag_state.initial_pointer = Some((x, y));
			} else {
				let (initial_x, initial_y) = drag_state.initial_pointer.unwrap();
				let delta_x = x - initial_x;
				let delta_y = y - initial_y;

				if let Some(SubsystemType::XdgTopLevel(top_level)) = self.objects.get_mut(&drag_state.top_level_id) {
					top_level.x += delta_x;
					top_level.y += delta_y;
				}

				drag_state.initial_pointer = Some((x, y));
			}
		}
		Ok(())
	}

	pub fn end_drag(&mut self) {
		self.dragging = None;
	}

	// Returns the ID of the surface at the given coordinates, if any.
	pub fn surface_at(&self, x: i32, y: i32) -> Option<u32> {
		for (obj_id, obj) in self.objects.iter() {
			if let SubsystemType::XdgTopLevel(xdg_toplevel) = obj
				&& let Some(SubsystemType::XdgSurface(xdg_surface)) = self.objects.get(&xdg_toplevel.xdg_surface)
				&& let Some(SubsystemType::Surface(surface)) = self.objects.get(&xdg_surface.surface_id)
				&& let Some((buffer_id, subsurface_x, subsurface_y)) = surface.attached_buffer
				&& let Some(SubsystemType::Buffer(buffer)) = self.objects.get(&buffer_id)
			{
				let surface_x = subsurface_x + xdg_toplevel.x;
				let surface_y = subsurface_y + xdg_toplevel.y;
				if x >= surface_x && x < surface_x + buffer.width && y >= surface_y && y < surface_y + buffer.height {
					return Some(*obj_id);
				}
			}
		}
		None
	}

	// send_enter_event needs to be its own thing, because it needs to transform the global
	// coordinates of the pointer into surface-local coordinates, which requires looking up the
	// position of the surface and the position of the buffer attached to the surface.
	pub fn send_enter_event(&self, serial: u32, top_level_id: u32, x: i32, y: i32) -> WaylandResult<()> {
		let pointer_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Pointer(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		let top_level = self
			.objects
			.get(&top_level_id)
			.and_then(|s| {
				if let SubsystemType::XdgTopLevel(xdg_toplevel) = s {
					Some(xdg_toplevel)
				} else {
					None
				}
			})
			.ok_or(WaylandError::UnrecognisedObject)?;

		// Make the enter_event relative to the surface's position
		let surface_id = self
			.derive_surface_id_from_top_level_id(top_level_id)
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface = self
			.objects
			.get(&surface_id)
			.and_then(|s| {
				if let SubsystemType::Surface(surface) = s {
					Some(surface)
				} else {
					None
				}
			})
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface_x = surface
			.attached_buffer
			.map(|(_, subsurface_x, _)| subsurface_x)
			.unwrap_or(0)
			+ top_level.x;
		let surface_y = surface
			.attached_buffer
			.map(|(_, _, subsurface_y)| subsurface_y)
			.unwrap_or(0)
			+ top_level.y;
		let relative_x = x - surface_x;
		let relative_y = y - surface_y;

		let enter_event = EnterEvent::new(serial, surface_id, relative_x, relative_y);

		let mut event_bytes = Vec::new();
		enter_event.write_to_with_endian(&mut event_bytes, Endian::Little)?;

		let packet = WaylandPacket::new(pointer_id, 0, event_bytes); // opcode 0 is enter
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;

		self.connection.as_ref().write_all(&buf)?;

		// wl_pointer.frame — opcode 5, no payload
		let packet = WaylandPacket::new(pointer_id, 5, vec![]); // opcode 0 is enter
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;

		self.connection.as_ref().write_all(&buf)?;

		Ok(())
	}

	pub fn send_leave_event(&self, serial: u32, top_level_id: u32) -> WaylandResult<()> {
		let pointer_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Pointer(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface_id = self
			.derive_surface_id_from_top_level_id(top_level_id)
			.ok_or(WaylandError::UnrecognisedObject)?;
		let leave_event = LeaveEvent { serial, surface_id };
		let mut event_bytes = Vec::new();
		leave_event.write_to_with_endian(&mut event_bytes, Endian::Little)?;
		let packet = WaylandPacket::new(pointer_id, 1, event_bytes); // opcode 1 is leave
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;
		self.connection.as_ref().write_all(&buf)?;
		// wl_pointer.frame — opcode 5, no payload
		let packet = WaylandPacket::new(pointer_id, 5, vec![]); // opcode 0 is enter
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;
		self.connection.as_ref().write_all(&buf)?;

		Ok(())
	}

	pub fn send_move_event(&self, top_level_id: u32, x: i32, y: i32) -> WaylandResult<()> {
		let pointer_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Pointer(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;
		let top_level = self
			.objects
			.get(&top_level_id)
			.and_then(|s| {
				if let SubsystemType::XdgTopLevel(xdg_toplevel) = s {
					Some(xdg_toplevel)
				} else {
					None
				}
			})
			.ok_or(WaylandError::UnrecognisedObject)?;

		// Make the move_event relative to the surface's position
		let surface_id = self
			.derive_surface_id_from_top_level_id(top_level_id)
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface = self
			.objects
			.get(&surface_id)
			.and_then(|s| {
				if let SubsystemType::Surface(surface) = s {
					Some(surface)
				} else {
					None
				}
			})
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface_x = surface
			.attached_buffer
			.map(|(_, subsurface_x, _)| subsurface_x)
			.unwrap_or(0)
			+ top_level.x;
		let surface_y = surface
			.attached_buffer
			.map(|(_, _, subsurface_y)| subsurface_y)
			.unwrap_or(0)
			+ top_level.y;
		let relative_x = x - surface_x;
		let relative_y = y - surface_y;

		let move_event = MoveEvent::new(relative_x, relative_y);
		let mut event_bytes = Vec::new();
		move_event.write_to_with_endian(&mut event_bytes, Endian::Little)?;
		let packet = WaylandPacket::new(pointer_id, 2, event_bytes); // opcode 2 is move
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;
		self.connection.as_ref().write_all(&buf)?;
		// wl_pointer.frame — opcode 5, no payload
		let packet = WaylandPacket::new(pointer_id, 5, vec![]); // opcode 5 is frame
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;
		self.connection.as_ref().write_all(&buf)?;
		Ok(())
	}

	pub fn send_button_event(&self, event: ButtonEvent) -> WaylandResult<()> {
		let pointer_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Pointer(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		let mut event_bytes = Vec::new();
		event.write_to_with_endian(&mut event_bytes, Endian::Little)?;
		let packet = WaylandPacket::new(pointer_id, 3, event_bytes); // opcode 3 is button
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;
		self.connection.as_ref().write_all(&buf)?;
		// wl_pointer.frame — opcode 5, no payload
		let packet = WaylandPacket::new(pointer_id, 5, vec![]); // opcode 5 is frame
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;
		self.connection.as_ref().write_all(&buf)?;
		Ok(())
	}

	// Returns the surface ID associated with the given top level ID, if it exists.
	fn derive_surface_id_from_top_level_id(&self, top_level_id: u32) -> Option<u32> {
		if let Some(SubsystemType::XdgTopLevel(top_level)) = self.objects.get(&top_level_id)
			&& let Some(SubsystemType::XdgSurface(xdg_surface)) = self.objects.get(&top_level.xdg_surface)
			&& let Some(SubsystemType::Surface(_)) = self.objects.get(&xdg_surface.surface_id)
		{
			return Some(xdg_surface.surface_id);
		}
		None
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
			Some(ClientEffect::StartDrag) => {
				self.dragging = Some(DragState {
					top_level_id: object_id,
					initial_pointer: None,
				});
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
