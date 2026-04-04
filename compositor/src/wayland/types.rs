use std::{
	collections::{HashMap, VecDeque},
	os::{fd::OwnedFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::{Endian, ReadFromWithEndian};
use thiserror::Error;

pub use wayland::types::{WaylandEncodedString, WaylandPacket};

use crate::{
	VideoBuffer,
	keyboard::{KeyEvent, Modifiers},
	wayland::{
		buffer::Buffer,
		compositor::Compositor,
		display::Display,
		keyboard::{KeyEnterCommand, KeyEventPacket, KeyLeaveCommand, Keyboard, ModifiersCommand},
		output::{DisplayGeometry, Output},
		pointer::{ButtonEvent, Pointer},
		registry::Registry,
		seat::Seat,
		shm::{SharedMemory, SharedMemoryPool},
		surface::Surface,
		xdg_surface::XDGSurface,
		xdg_toplevel::XdgTopLevel,
		xdg_wm_base::XdgWmBase,
	},
};
use wayland::buffer::ReleaseEvent;
use wayland::{
	pointer::{EnterEvent, FrameEvent, LeaveEvent, MoveEvent},
	types::WaylandPayload,
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
		let mut blitted_rects: Vec<(u32, i32, i32, i32, i32)> = Vec::new(); // (surface_id, x, y, width, height)
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

				if let Some((last_x, last_y, last_width, last_height)) = surface.last_blit_rect
					&& (blit_x != last_x
						|| blit_y != last_y
						|| buffer.width != last_width
						|| buffer.height != last_height)
				{
					let x0 = last_x.max(0);
					let y0 = last_y.max(0);
					let x1 = (last_x + last_width).min(framebuffer.width as i32);
					let y1 = (last_y + last_height).min(framebuffer.height as i32);
					framebuffer.clear_rect(x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32, 0);
				}
				blitted_rects.push((*surface_id, blit_x, blit_y, buffer.width, buffer.height));

				mem_pool.blit_onto(buffer, framebuffer, blit_x, blit_y);
				blitted.push((*surface_id, buffer_id));
			}
		}

		for (surface_id, buffer_id) in blitted {
			if let Some(SubsystemType::Surface(surface)) = self.objects.get_mut(&surface_id) {
				surface.mark_blitted(&self.connection);
			}
			if let Err(e) = ReleaseEvent.write_as_packet(buffer_id, &self.connection) {
				eprintln!("Failed to send wl_buffer.release packet: {}", e);
				continue;
			}
		}

		for (surface_id, x, y, width, height) in blitted_rects {
			if let Some(SubsystemType::Surface(surface)) = self.objects.get_mut(&surface_id) {
				surface.last_blit_rect = Some((x, y, width, height));
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

		if let Some(drag_state) = &self.dragging
			&& let Some(surface_id) = self.derive_surface_id_from_top_level_id(drag_state.top_level_id)
			&& let Some(SubsystemType::Surface(surface)) = self.objects.get_mut(&surface_id)
		{
			surface.blitted = false; // mark the surface as needing to be repainted
		}

		Ok(())
	}

	pub fn end_drag(&mut self) {
		self.dragging = None;
	}

	pub fn handle_focus_enter(
		&mut self,
		serial: u32,
		top_level_id: u32,
		keyboard: &crate::keyboard::Keyboard,
	) -> WaylandResult<()> {
		let keyboard_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Keyboard(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface_id = self
			.derive_surface_id_from_top_level_id(top_level_id)
			.ok_or(WaylandError::UnrecognisedObject)?;

		let enter_event = KeyEnterCommand {
			serial,
			surface_id,
			keys: bytestruct::LengthPrefixedVec::new(vec![]),
		};
		enter_event.write_as_packet(keyboard_id, &self.connection)?;

		let modifiers_event = ModifiersCommand {
			serial,
			depressed: keyboard.depressed.bits(),
			latched: Modifiers::empty().bits(),
			locked: keyboard.locked.bits(),
			group: 0,
		};
		Ok(modifiers_event.write_as_packet(keyboard_id, &self.connection)?)
	}

	pub fn handle_focus_leave(&mut self, serial: u32, top_level_id: u32) -> WaylandResult<()> {
		let keyboard_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Keyboard(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		let surface_id = self
			.derive_surface_id_from_top_level_id(top_level_id)
			.ok_or(WaylandError::UnrecognisedObject)?;

		let leave_event = KeyLeaveCommand { serial, surface_id };
		Ok(leave_event.write_as_packet(keyboard_id, &self.connection)?)
	}

	pub fn handle_key_event(
		&mut self,
		serial: u32,
		key_event: KeyEvent,
		keyboard: &crate::keyboard::Keyboard,
	) -> WaylandResult<()> {
		let keyboard_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Keyboard(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		let (code, state) = match key_event {
			KeyEvent::KeyPress(code) => (code, 1),
			KeyEvent::KeyRelease(code) => (code, 0),
		};

		let raw_key_code: u16 = code.into();
		let time = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC)?;
		let time_ms = time.tv_sec() as u64 * 1000 + time.tv_nsec() as u64 / 1_000_000;
		let event = KeyEventPacket {
			serial,
			time: time_ms as u32,
			key: raw_key_code as u32,
			state,
		};
		event.write_as_packet(keyboard_id, &self.connection)?;

		if code.is_modifier() {
			let modifiers_event = ModifiersCommand {
				serial,
				depressed: keyboard.depressed.bits(),
				latched: Modifiers::empty().bits(),
				locked: keyboard.locked.bits(),
				group: 0,
			};
			modifiers_event.write_as_packet(keyboard_id, &self.connection)?;
		}

		Ok(())
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

		let enter_event = EnterEvent {
			serial,
			surface_id,
			x: relative_x * 256,
			y: relative_y * 256,
		};
		enter_event.write_as_packet(pointer_id, &self.connection)?;
		FrameEvent.write_as_packet(pointer_id, &self.connection)?;

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
		leave_event.write_as_packet(pointer_id, &self.connection)?;
		FrameEvent.write_as_packet(pointer_id, &self.connection)?;

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

		let time = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC)?;
		let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
		MoveEvent {
			time: ms as u32,
			x: relative_x * 256,
			y: relative_y * 256,
		}
		.write_as_packet(pointer_id, &self.connection)?;
		FrameEvent.write_as_packet(pointer_id, &self.connection)?;
		Ok(())
	}

	pub fn send_button_event(&self, event: ButtonEvent) -> WaylandResult<()> {
		let pointer_id = self
			.objects
			.iter()
			.find_map(|(id, s)| matches!(s, SubsystemType::Pointer(_)).then_some(*id))
			.ok_or(WaylandError::UnrecognisedObject)?;

		event.write_as_packet(pointer_id, &self.connection)?;
		FrameEvent.write_as_packet(pointer_id, &self.connection)?;
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
