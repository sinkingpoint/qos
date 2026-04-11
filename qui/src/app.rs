use std::{
	cell::RefCell,
	io::{self, Cursor},
	os::fd::AsRawFd,
	rc::Rc,
};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};
use nix::sys::mman::{MapFlags, ProtFlags, mmap};
use wayland::{
	compositor::CreateSurfaceRequest,
	keyboard::KeyboardEvent,
	pointer::PointerEvent,
	shm::CreatePoolRequest,
	shm_pool::CreateBufferRequest,
	surface::{AttachRequest, CommitRequest, DamageRequest, FrameCallbackEvent, FrameRequest},
	types::{WaylandEncodedString, WaylandPayload},
	xdg_surface::{AckConfigureRequest, ConfigureEvent, GetTopLevelSurfaceRequest},
	xdg_toplevel::{CloseEvent, MoveRequest, SetTitleRequest},
	xdg_wm_base::GetXdgSurfaceRequest,
};

use crate::{
	canvas::Canvas,
	context::{ContextEvent, WaylandContext},
};

pub struct App<'a> {
	width: i32,
	height: i32,

	context: Rc<RefCell<WaylandContext>>,
	surface_id: u32,
	xdg_toplevel_id: u32,
	buffer_id: u32,
	pixels: &'a mut [u32],
	pixel_count: usize,
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
			if packet.object_id == xdg_surface_id && packet.opcode == ConfigureEvent::OPCODE {
				let event = ConfigureEvent::read_from_with_endian(&mut Cursor::new(&packet.payload), Endian::Little)?;
				AckConfigureRequest { serial: event.serial }.write_as_packet(xdg_surface_id, &context.conn.stream)?;
				break;
			}
		}

		let shm_id = context
			.globals
			.shm
			.ok_or_else(|| io::Error::other("no wl_shm advertised"))?;
		let stride: i32 = width * 4;
		let pool_size: usize = (stride * height) as usize;

		let memfd = nix::sys::memfd::memfd_create(c"qui-shm", nix::sys::memfd::MemFdCreateFlag::empty())
			.map_err(io::Error::other)?;
		nix::unistd::ftruncate(&memfd, pool_size as i64).map_err(io::Error::other)?;
		let ptr = unsafe {
			mmap(
				None,
				std::num::NonZeroUsize::new(pool_size).unwrap(),
				ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
				MapFlags::MAP_SHARED,
				Some(&memfd),
				0,
			)
			.map_err(io::Error::other)?
		};

		let pool_id = context.next_object_id.next();
		let mut pool_payload = Vec::new();
		CreatePoolRequest {
			pool_id,
			size: pool_size as u32,
		}
		.write_to_with_endian(&mut pool_payload, Endian::Little)?;
		context
			.conn
			.send_with_fd(shm_id, CreatePoolRequest::OPCODE, &pool_payload, memfd.as_raw_fd())?;

		let buffer_id = context.next_object_id.next();
		CreateBufferRequest {
			buffer_id,
			offset: 0,
			width,
			height,
			stride,
			format: 1,
		}
		.write_as_packet(pool_id, &context.conn.stream)?;

		SetTitleRequest {
			title: WaylandEncodedString(title.clone()),
		}
		.write_as_packet(xdg_toplevel_id, &context.conn.stream)?;
		AttachRequest { buffer_id, x: 0, y: 0 }.write_as_packet(surface_id, &context.conn.stream)?;
		CommitRequest.write_as_packet(surface_id, &context.conn.stream)?;
		Ok(Self {
			width,
			height,
			surface_id,
			xdg_toplevel_id,
			buffer_id,
			pixels: unsafe { std::slice::from_raw_parts_mut(ptr as *mut u32, (width * height) as usize) },
			pixel_count: (width * height) as usize,
			frame_callback_id: context.next_object_id.next(),
			last_interaction_serial: None,
			damage: Vec::new(),
			last_pointer_position: None,
			context: Rc::new(RefCell::new(context)),
		})
	}

	pub fn canvas(&mut self) -> Canvas<'_> {
		Canvas::new(self.pixels, self.width, self.height, self.width, 0, 0, &mut self.damage)
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
			let (object_id, event) =
				self.context
					.borrow_mut()
					.poll(&[self.surface_id, self.xdg_toplevel_id, self.frame_callback_id])?;

			if object_id != self.frame_callback_id {
				println!("Received event for object {}: {:?}", object_id, event);
			}
			if let ContextEvent::Pointer(pointer_event) = event {
				match pointer_event {
					PointerEvent::Move(event) => {
						self.last_pointer_position = Some((event.x / 256, event.y / 256));
						return Ok(AppEvent::PointerMotion {
							x: event.x / 256,
							y: event.y / 256,
						});
					}
					PointerEvent::Button(event) => {
						self.last_interaction_serial = Some(event.serial);
						return Ok(AppEvent::PointerButton {
							button: event.button,
							pressed: event.state != 0,
							x: self.last_pointer_position.map(|(x, _)| x).unwrap_or(0),
							y: self.last_pointer_position.map(|(_, y)| y).unwrap_or(0),
						});
					}
					_ => {}
				}
			} else if let ContextEvent::Keyboard(keyboard_event) = event {
				if let KeyboardEvent::Key(event) = keyboard_event {
					self.last_interaction_serial = Some(event.serial);
					return Ok(AppEvent::Keyboard {
						keycode: event.key,
						pressed: event.state != 0,
					});
				}
			} else if matches!(event, ContextEvent::Unknown { opcode: CloseEvent::OPCODE, .. } if object_id == self.xdg_toplevel_id)
			{
				return Ok(AppEvent::Close);
			} else if matches!(event, ContextEvent::Unknown { opcode: FrameCallbackEvent::OPCODE, .. } if object_id == self.frame_callback_id)
			{
				eprintln!("returning Frame");
				return Ok(AppEvent::Frame);
			}
		}
	}

	pub fn commit_frame(&mut self) -> io::Result<()> {
		FrameRequest {
			callback_id: self.frame_callback_id,
		}
		.write_as_packet(self.surface_id, &self.context.borrow().conn.stream)?;
		AttachRequest {
			buffer_id: self.buffer_id,
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

impl Drop for App<'_> {
	fn drop(&mut self) {
		unsafe {
			nix::sys::mman::munmap(self.pixels.as_mut_ptr() as *mut _, self.pixel_count * 4).ok();
		}
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
