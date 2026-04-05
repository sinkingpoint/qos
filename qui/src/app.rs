use std::{
	collections::VecDeque,
	env,
	io::{self, Cursor},
	os::{
		fd::{AsRawFd, OwnedFd},
		unix::net::UnixStream,
	},
};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};
use nix::sys::mman::{MapFlags, ProtFlags, mmap};
use wayland::{
	compositor::CreateSurfaceRequest,
	connection::WaylandConnection,
	display::{GetRegistryRequest, SyncRequest, WL_DISPLAY_OBJECT_ID},
	keyboard::KeyboardEvent,
	pointer::PointerEvent,
	registry::{BindRequest, GlobalEvent},
	seat::{GetKeyboardRequest, GetPointerRequest},
	shm::CreatePoolRequest,
	shm_pool::CreateBufferRequest,
	surface::{AttachRequest, CommitRequest, DamageRequest, FrameCallbackEvent, FrameRequest},
	types::{WaylandEncodedString, WaylandPayload},
	xdg_surface::{AckConfigureRequest, ConfigureEvent, GetTopLevelSurfaceRequest},
	xdg_toplevel::{CloseEvent, MoveRequest, SetTitleRequest},
	xdg_wm_base::{GetXdgSurfaceRequest, PingEvent, PongRequest},
};

use crate::canvas::Canvas;

pub struct App<'a> {
	title: String,
	width: i32,
	height: i32,

	conn: WaylandConnection,
	globals: BoundGlobals,
	pointer_id: u32,
	keyboard_id: u32,
	surface_id: u32,
	xdg_surface_id: u32,
	xdg_toplevel_id: u32,
	buffer_id: u32,
	seat_id: u32,
	pool_id: u32,
	pixels: &'a mut [u32],
	pixel_count: usize,
	frame_callback_id: u32,
	next_object_id: ObjectId,
	fds: VecDeque<OwnedFd>,
	last_interaction_serial: Option<u32>,
	damage: Vec<(i32, i32, i32, i32)>,
}

impl App<'_> {
	pub fn new(title: String, width: i32, height: i32) -> io::Result<Self> {
		let uid = 0;
		let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", uid));
		let display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "0".to_string());
		let socket_path = if display.starts_with("/") {
			display
		} else {
			format!("{}/{}", runtime_dir, display)
		};

		println!("Connecting to Wayland display at {}", socket_path);
		let socket = UnixStream::connect(&socket_path)?;
		let mut conn = WaylandConnection::new(socket);
		let registry_id: u32 = 2;
		let mut next_obj_id = ObjectId(3);
		GetRegistryRequest { registry_id }.write_as_packet(WL_DISPLAY_OBJECT_ID, &conn.stream)?;

		let globals = bind_globals(&mut conn, registry_id, &mut next_obj_id)?;

		let compositor_id = globals
			.compositor
			.ok_or_else(|| io::Error::other("no wl_compositor advertised"))?;
		let shm_id = globals.shm.ok_or_else(|| io::Error::other("no wl_shm advertised"))?;
		let xdg_wm_base_id = globals
			.xdg_wm_base
			.ok_or_else(|| io::Error::other("no xdg_wm_base advertised"))?;
		let seat_id = globals.seat.ok_or_else(|| io::Error::other("no wl_seat advertised"))?;

		// wl_seat.get_pointer
		let pointer_id = next_obj_id.next();
		GetPointerRequest { new_id: pointer_id }.write_as_packet(seat_id, &conn.stream)?;

		let surface_id = next_obj_id.next();
		CreateSurfaceRequest { new_id: surface_id }.write_as_packet(compositor_id, &conn.stream)?;

		let xdg_surface_id = next_obj_id.next();
		GetXdgSurfaceRequest {
			new_id: xdg_surface_id,
			surface_id,
		}
		.write_as_packet(xdg_wm_base_id, &conn.stream)?;

		let xdg_toplevel_id = next_obj_id.next();
		GetTopLevelSurfaceRequest {
			new_id: xdg_toplevel_id,
		}
		.write_as_packet(xdg_surface_id, &conn.stream)?;

		// Initial commit with no buffer: required by xdg_shell to signal that
		// setup is complete and prompt the compositor to send xdg_surface.configure.
		CommitRequest.write_as_packet(surface_id, &conn.stream)?;

		// Wait for xdg_surface.configure, then ack
		loop {
			let packet = conn.recv_packet()?;
			if packet.object_id == xdg_surface_id && packet.opcode == ConfigureEvent::OPCODE {
				let event = ConfigureEvent::read_from_with_endian(&mut Cursor::new(&packet.payload), Endian::Little)?;
				AckConfigureRequest { serial: event.serial }.write_as_packet(xdg_surface_id, &conn.stream)?;
				break;
			}
		}

		// Send get_keyboard now so the keymap event arrives in the main event loop.
		let keyboard_id = next_obj_id.next();
		GetKeyboardRequest { new_id: keyboard_id }.write_as_packet(seat_id, &conn.stream)?;

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

		let pool_id = next_obj_id.next();
		let mut pool_payload = Vec::new();
		CreatePoolRequest {
			pool_id,
			size: pool_size as u32,
		}
		.write_to_with_endian(&mut pool_payload, Endian::Little)?;
		conn.send_with_fd(shm_id, CreatePoolRequest::OPCODE, &pool_payload, memfd.as_raw_fd())?;

		let buffer_id = next_obj_id.next();
		CreateBufferRequest {
			buffer_id,
			offset: 0,
			width,
			height,
			stride,
			format: 1,
		}
		.write_as_packet(pool_id, &conn.stream)?;

		SetTitleRequest {
			title: WaylandEncodedString(title.clone()),
		}
		.write_as_packet(xdg_toplevel_id, &conn.stream)?;
		AttachRequest { buffer_id, x: 0, y: 0 }.write_as_packet(surface_id, &conn.stream)?;
		CommitRequest.write_as_packet(surface_id, &conn.stream)?;
		Ok(Self {
			title,
			width,
			height,
			conn,
			globals,
			pointer_id,
			keyboard_id,
			surface_id,
			xdg_surface_id,
			xdg_toplevel_id,
			buffer_id,
			seat_id,
			pool_id,
			pixels: unsafe { std::slice::from_raw_parts_mut(ptr as *mut u32, (width * height) as usize) },
			pixel_count: (width * height) as usize,
			frame_callback_id: next_obj_id.next(),
			next_object_id: next_obj_id,
			fds: VecDeque::new(),
			last_interaction_serial: None,
			damage: Vec::new(),
		})
	}

	pub fn canvas(&mut self) -> Canvas<'_> {
		Canvas::new(self.pixels, self.width, self.height, self.width, 0, 0, &mut self.damage)
	}

	pub fn start_move(&mut self) -> io::Result<()> {
		if let Some(serial) = self.last_interaction_serial {
			MoveRequest {
				serial,
				seat_id: self.seat_id,
			}
			.write_as_packet(self.xdg_toplevel_id, &self.conn.stream)
		} else {
			Ok(())
		}
	}

	pub fn poll(&mut self) -> io::Result<AppEvent> {
		loop {
			let packet = self.conn.recv_packet()?;
			self.fds.extend(self.conn.drain_fds());
			if packet.object_id == self.pointer_id {
				let event = PointerEvent::parse(packet.opcode, &packet.payload, &mut self.fds);

				if let Some(PointerEvent::Button(event)) = &event {
					self.last_interaction_serial = Some(event.serial);
				}

				if let Some(event) = event
					&& let Ok(app_event) = event.try_into()
				{
					return Ok(app_event);
				}
			} else if packet.object_id == self.keyboard_id {
				let event = KeyboardEvent::parse(packet.opcode, &packet.payload, &mut self.fds);
				if let Some(event) = event
					&& let Ok(app_event) = event.try_into()
				{
					return Ok(app_event);
				}
			} else if packet.object_id == self.frame_callback_id && packet.opcode == FrameCallbackEvent::OPCODE {
				return Ok(AppEvent::Frame);
			} else if packet.object_id == self.globals.xdg_wm_base.unwrap() && packet.opcode == PingEvent::OPCODE {
				let ping_event = PingEvent::read_from_with_endian(&mut Cursor::new(&packet.payload), Endian::Little)?;
				PongRequest {
					callback_id: ping_event.callback_id,
				}
				.write_as_packet(self.globals.xdg_wm_base.unwrap(), &self.conn.stream)?;
			} else if packet.object_id == self.xdg_toplevel_id && packet.opcode == CloseEvent::OPCODE {
				return Ok(AppEvent::Close);
			}
		}
	}

	pub fn commit_frame(&mut self) -> io::Result<()> {
		FrameRequest {
			callback_id: self.frame_callback_id,
		}
		.write_as_packet(self.surface_id, &self.conn.stream)?;
		AttachRequest {
			buffer_id: self.buffer_id,
			x: 0,
			y: 0,
		}
		.write_as_packet(self.surface_id, &self.conn.stream)?;
		for damage in self.damage.drain(..) {
			DamageRequest {
				x: damage.0,
				y: damage.1,
				width: damage.2,
				height: damage.3,
			}
			.write_as_packet(self.surface_id, &self.conn.stream)?;
		}
		CommitRequest.write_as_packet(self.surface_id, &self.conn.stream)
	}
}

impl Drop for App<'_> {
	fn drop(&mut self) {
		unsafe {
			nix::sys::mman::munmap(self.pixels.as_mut_ptr() as *mut _, self.pixel_count * 4).ok();
		}
	}
}

pub enum AppEvent {
	Frame,
	Keyboard { keycode: u32, pressed: bool },
	PointerMotion { x: i32, y: i32 },
	PointerButton { button: u32, pressed: bool },
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

fn get_globals(
	conn: &mut WaylandConnection,
	registry_id: u32,
	callback_id: u32,
) -> io::Result<Vec<(u32, String, u32)>> {
	let mut globals: Vec<(u32, String, u32)> = Vec::new();
	loop {
		let packet = conn.recv_packet()?;
		if packet.object_id == callback_id && packet.opcode == 0 {
			break;
		} else if packet.object_id == registry_id && packet.opcode == GlobalEvent::OPCODE {
			let event = GlobalEvent::read_from_with_endian(&mut Cursor::new(&packet.payload), Endian::Little)?;
			globals.push((event.name, event.interface.0, event.version));
		}
	}
	Ok(globals)
}

fn bind_globals(
	conn: &mut WaylandConnection,
	registry_id: u32,
	next_object_id: &mut ObjectId,
) -> io::Result<BoundGlobals> {
	let callback_id = next_object_id.next();
	SyncRequest { callback_id }.write_as_packet(WL_DISPLAY_OBJECT_ID, &conn.stream)?;

	let globals = get_globals(conn, registry_id, callback_id)?;
	let mut bound = BoundGlobals::new();

	for (name, iface, version) in &globals {
		let bind_version = match iface.as_str() {
			"wl_compositor" | "wl_shm" | "xdg_wm_base" | "wl_seat" => *version,
			_ => continue,
		};
		let id = next_object_id.next();
		BindRequest {
			name: *name,
			interface: WaylandEncodedString(iface.clone()),
			version: bind_version,
			new_id: id,
		}
		.write_as_packet(registry_id, &conn.stream)?;

		match iface.as_str() {
			"wl_compositor" => bound.compositor = Some(id),
			"wl_shm" => bound.shm = Some(id),
			"xdg_wm_base" => bound.xdg_wm_base = Some(id),
			"wl_seat" => bound.seat = Some(id),
			_ => {}
		}
	}

	Ok(bound)
}

struct BoundGlobals {
	compositor: Option<u32>,
	shm: Option<u32>,
	xdg_wm_base: Option<u32>,
	seat: Option<u32>,
}

impl BoundGlobals {
	fn new() -> Self {
		Self {
			compositor: None,
			shm: None,
			xdg_wm_base: None,
			seat: None,
		}
	}
}

struct ObjectId(u32);
impl ObjectId {
	fn next(&mut self) -> u32 {
		let id = self.0;
		self.0 += 1;
		id
	}
}
