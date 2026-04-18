use std::{
	cell::RefCell,
	io::{self, Cursor},
	rc::Rc,
};

use bytestruct::{Endian, ReadFromWithEndian};
use wayland::{
	compositor::CreateSurfaceRequest,
	keyboard::KeyboardEvent,
	pointer::PointerEvent,
	surface::{AttachRequest, CommitRequest, DamageRequest, FrameCallbackEvent, FrameRequest},
	types::{WaylandEncodedString, WaylandPayload},
	xdg_surface::{AckConfigureRequest, ConfigureEvent, GetTopLevelSurfaceRequest},
	xdg_toplevel::{CloseEvent, MoveRequest, SetTitleRequest},
	xdg_wm_base::GetXdgSurfaceRequest,
};

use crate::{
	buffer::Buffer,
	canvas::Canvas,
	context::{ContextEvent, WaylandContext},
};

pub struct App<'a> {
	width: i32,
	height: i32,

	context: Rc<RefCell<WaylandContext>>,
	surface_id: u32,
	xdg_surface_id: u32,
	xdg_toplevel_id: u32,
	buffers: Vec<Buffer<'a>>,
	frame_callback_id: u32,
	last_interaction_serial: Option<u32>,
	damage: Vec<(i32, i32, i32, i32)>,

	last_pointer_position: Option<(i32, i32)>,
}

impl App<'_> {
	pub fn new(title: String, width: i32, height: i32) -> io::Result<Self> {
		let mut context = WaylandContext::connect()?;
		let surface_id = context.next_object_id.next();
		CreateSurfaceRequest { new_id: surface_id }
			.write_as_packet(context.globals.compositor.unwrap(), &context.conn.stream)?;

		let xdg_surface_id = context.next_object_id.next();
		GetXdgSurfaceRequest {
			new_id: xdg_surface_id,
			surface_id,
		}
		.write_as_packet(context.globals.xdg_wm_base.unwrap(), &context.conn.stream)?;

		let xdg_toplevel_id = context.next_object_id.next();
		GetTopLevelSurfaceRequest {
			new_id: xdg_toplevel_id,
		}
		.write_as_packet(xdg_surface_id, &context.conn.stream)?;

		// Initial commit with no buffer: required by xdg_shell to signal that
		// setup is complete and prompt the compositor to send xdg_surface.configure.
		CommitRequest.write_as_packet(surface_id, &context.conn.stream)?;

		// Wait for xdg_surface.configure, then ack
		loop {
			let packet = context.conn.recv_packet()?;
			context.conn.drain_fds(); // discard any fds (e.g. keyboard keymap) arriving before poll loop
			if packet.object_id == xdg_surface_id && packet.opcode == ConfigureEvent::OPCODE {
				let event = ConfigureEvent::read_from_with_endian(&mut Cursor::new(&packet.payload), Endian::Little)?;
				AckConfigureRequest { serial: event.serial }.write_as_packet(xdg_surface_id, &context.conn.stream)?;
				break;
			}
		}

		SetTitleRequest {
			title: WaylandEncodedString(title.clone()),
		}
		.write_as_packet(xdg_toplevel_id, &context.conn.stream)?;
		let buffer = Buffer::new(&mut context, width, height)?;
		AttachRequest {
			buffer_id: buffer.id,
			x: 0,
			y: 0,
		}
		.write_as_packet(surface_id, &context.conn.stream)?;
		CommitRequest.write_as_packet(surface_id, &context.conn.stream)?;
		Ok(Self {
			width,
			height,
			surface_id,
			xdg_surface_id,
			xdg_toplevel_id,
			frame_callback_id: context.next_object_id.next(),
			last_interaction_serial: None,
			damage: Vec::new(),
			last_pointer_position: None,
			context: Rc::new(RefCell::new(context)),
			buffers: vec![buffer],
		})
	}

	pub fn canvas(&mut self) -> Canvas<'_> {
		Canvas::new(
			self.buffers[0].pixels,
			self.width,
			self.height,
			self.width,
			0,
			0,
			&mut self.damage,
		)
	}

	pub fn start_move(&mut self) -> io::Result<()> {
		if let Some(serial) = self.last_interaction_serial {
			MoveRequest {
				serial,
				seat_id: self.context.borrow().globals.seat.unwrap(),
			}
			.write_as_packet(self.xdg_toplevel_id, &self.context.borrow().conn.stream)
		} else {
			Ok(())
		}
	}

	pub fn poll(&mut self) -> io::Result<AppEvent> {
		loop {
			let (object_id, event) = self.context.borrow_mut().poll(&[
				self.surface_id,
				self.xdg_surface_id,
				self.xdg_toplevel_id,
				self.frame_callback_id,
			])?;
			if let Some(app_event) = self.interpret_event(object_id, event)? {
				return Ok(app_event);
			}
		}
	}

	pub fn try_poll(&mut self) -> io::Result<Option<AppEvent>> {
		loop {
			let maybe = self.context.borrow_mut().try_poll(&[
				self.surface_id,
				self.xdg_surface_id,
				self.xdg_toplevel_id,
				self.frame_callback_id,
			])?;
			match maybe {
				None => return Ok(None),
				Some((object_id, event)) => {
					if let Some(app_event) = self.interpret_event(object_id, event)? {
						return Ok(Some(app_event));
					}
				}
			}
		}
	}

	fn interpret_event(&mut self, object_id: u32, event: ContextEvent) -> io::Result<Option<AppEvent>> {
		if let ContextEvent::Pointer(pointer_event) = event {
			match pointer_event {
				PointerEvent::Move(event) => {
					self.last_pointer_position = Some((event.x / 256, event.y / 256));
					return Ok(Some(AppEvent::PointerMotion {
						x: event.x / 256,
						y: event.y / 256,
					}));
				}
				PointerEvent::Button(event) => {
					self.last_interaction_serial = Some(event.serial);
					return Ok(Some(AppEvent::PointerButton {
						button: event.button,
						pressed: event.state != 0,
						x: self.last_pointer_position.map(|(x, _)| x).unwrap_or(0),
						y: self.last_pointer_position.map(|(_, y)| y).unwrap_or(0),
					}));
				}
				_ => {}
			}
		} else if let ContextEvent::Keyboard(keyboard_event) = event {
			if let KeyboardEvent::Key(event) = keyboard_event {
				self.last_interaction_serial = Some(event.serial);
				return Ok(Some(AppEvent::Keyboard {
					keycode: event.key,
					pressed: event.state != 0,
				}));
			}
		} else if matches!(event, ContextEvent::Unknown { opcode: ConfigureEvent::OPCODE, .. } if object_id == self.xdg_surface_id)
			&& let ContextEvent::Unknown { payload, .. } = event
		{
			let configure = ConfigureEvent::read_from_with_endian(&mut Cursor::new(&payload), Endian::Little)?;
			AckConfigureRequest {
				serial: configure.serial,
			}
			.write_as_packet(self.xdg_surface_id, &self.context.borrow().conn.stream)?;
		} else if matches!(event, ContextEvent::Unknown { opcode: CloseEvent::OPCODE, .. } if object_id == self.xdg_toplevel_id)
		{
			return Ok(Some(AppEvent::Close));
		} else if matches!(event, ContextEvent::Unknown { opcode: FrameCallbackEvent::OPCODE, .. } if object_id == self.frame_callback_id)
		{
			return Ok(Some(AppEvent::Frame));
		}
		Ok(None)
	}

	pub fn commit_frame(&mut self) -> io::Result<()> {
		FrameRequest {
			callback_id: self.frame_callback_id,
		}
		.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		AttachRequest {
			buffer_id: self.buffers[0].id,
			x: 0,
			y: 0,
		}
		.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		for damage in self.damage.drain(..) {
			DamageRequest {
				x: damage.0,
				y: damage.1,
				width: damage.2,
				height: damage.3,
			}
			.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		}
		CommitRequest.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)
	}
}

#[derive(Clone)]
pub enum AppEvent {
	Frame,
	Keyboard { keycode: u32, pressed: bool },
	PointerMotion { x: i32, y: i32 },
	PointerButton { button: u32, pressed: bool, x: i32, y: i32 },
	Close,
}

impl TryFrom<PointerEvent> for AppEvent {
	type Error = ();

	fn try_from(event: PointerEvent) -> Result<Self, Self::Error> {
		match event {
			PointerEvent::Move(event) => Ok(AppEvent::PointerMotion {
				x: event.x / 256,
				y: event.y / 256,
			}),
			PointerEvent::Button(event) => Ok(AppEvent::PointerButton {
				button: event.button,
				pressed: event.state != 0,
				x: 0,
				y: 0,
			}),
			_ => Err(()),
		}
	}
}

impl TryFrom<KeyboardEvent> for AppEvent {
	type Error = ();

	fn try_from(event: KeyboardEvent) -> Result<Self, Self::Error> {
		match event {
			KeyboardEvent::Key(event) => Ok(AppEvent::Keyboard {
				keycode: event.key,
				pressed: event.state != 0,
			}),
			_ => Err(()),
		}
	}
}
