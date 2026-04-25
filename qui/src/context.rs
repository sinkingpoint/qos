use std::{
	cell::RefCell,
	collections::{HashMap, VecDeque},
	env,
	io::{self, Cursor, Read},
	os::{
		fd::{AsRawFd, FromRawFd, OwnedFd},
		unix::net::UnixStream,
	},
	rc::Rc,
};

use bytestruct::{Endian, ReadFromWithEndian};
use wayland::{
	connection::WaylandConnection,
	display::{GetRegistryRequest, SyncRequest, WL_DISPLAY_OBJECT_ID},
	keyboard::KeyboardEvent,
	pointer::PointerEvent,
	registry::{BindRequest, GlobalEvent},
	seat::{GetKeyboardRequest, GetPointerRequest},
	types::{WaylandEncodedString, WaylandPayload},
	xdg_wm_base::{PongRequest, XdgWmBaseEvent},
};

use crate::xkb::XkbKeyMap;

pub struct WaylandContext {
	pub(crate) conn: WaylandConnection,
	pub(crate) globals: BoundGlobals,
	pub(crate) pointer_id: u32,
	pub(crate) keyboard_id: u32,
	pub(crate) next_object_id: ObjectId,
	pub(crate) keyboard: Keyboard,
	fds: VecDeque<OwnedFd>,
	events: HashMap<u32, VecDeque<ContextEvent>>,
	keyboard_focus: Option<u32>,
	mouse_focus: Option<u32>,
	last_mouse_surface: Option<u32>,
}

impl WaylandContext {
	pub fn connect() -> io::Result<Rc<RefCell<Self>>> {
		let uid = 0;
		let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| format!("/run/user/{}", uid));
		let display = env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "0".to_string());
		let socket_path = if display.starts_with("/") {
			display
		} else {
			format!("{}/{}", runtime_dir, display)
		};

		let socket = UnixStream::connect(&socket_path)
			.map_err(|i| io::Error::new(i.kind(), format!("failed to connect to socket {}: {}", socket_path, i)))?;
		let mut conn = WaylandConnection::new(socket);
		let registry_id: u32 = 2;
		let mut next_obj_id = ObjectId(3);
		GetRegistryRequest { registry_id }.write_as_packet(WL_DISPLAY_OBJECT_ID, &conn.stream)?;

		let globals = bind_globals(&mut conn, registry_id, &mut next_obj_id)?;
		let seat_id = globals.seat.ok_or_else(|| io::Error::other("no wl_seat advertised"))?;

		// wl_seat.get_pointer
		let pointer_id = next_obj_id.next();
		GetPointerRequest { new_id: pointer_id }.write_as_packet(seat_id, &conn.stream)?;

		let keyboard_id = next_obj_id.next();
		GetKeyboardRequest { new_id: keyboard_id }.write_as_packet(seat_id, &conn.stream)?;

		Ok(Rc::new(RefCell::new(Self {
			conn,
			globals,
			pointer_id,
			keyboard_id,
			next_object_id: next_obj_id,
			fds: VecDeque::new(),
			events: HashMap::new(),
			keyboard_focus: None,
			mouse_focus: None,
			last_mouse_surface: None,
			keyboard: Keyboard::new(),
		})))
	}

	pub fn dispatch_one(&mut self) -> io::Result<()> {
		let packet = self.conn.recv_packet()?;
		self.route_packet(packet)
	}

	pub fn poll(&mut self, object_id: &[u32]) -> io::Result<(u32, ContextEvent)> {
		loop {
			if let Some(item) = self.try_poll(object_id)? {
				return Ok(item);
			}
			// No data in queues and nothing on socket — block until data arrives.
			let packet = self.conn.recv_packet()?;
			self.route_packet(packet)?;
		}
	}

	/// Non-blocking variant: returns `None` if no event is ready right now.
	pub fn try_poll(&mut self, object_id: &[u32]) -> io::Result<Option<(u32, ContextEvent)>> {
		// Drain in-memory queues first (free — no syscall).
		for id in object_id {
			if let Some(queue) = self.events.get_mut(id)
				&& let Some(event) = queue.pop_front()
			{
				return Ok(Some((*id, event)));
			}
		}
		// Read and route packets until the socket is empty or we get one for our objects.
		while self.conn.has_data() {
			let packet = self.conn.recv_packet()?;
			self.route_packet(packet)?;
			for id in object_id {
				if let Some(queue) = self.events.get_mut(id)
					&& let Some(event) = queue.pop_front()
				{
					return Ok(Some((*id, event)));
				}
			}
		}
		Ok(None)
	}

	fn route_packet(&mut self, packet: wayland::types::WaylandPacket) -> io::Result<()> {
		self.fds.extend(self.conn.drain_fds());
		if packet.object_id == self.keyboard_id
			&& let Some(event) = KeyboardEvent::parse(packet.opcode, &packet.payload, &mut self.fds)
		{
			match &event {
				KeyboardEvent::KeyMap(_) => {} // fd already consumed by parse, nothing to route
				KeyboardEvent::Enter(e) => self.keyboard_focus = Some(e.surface_id),
				KeyboardEvent::Leave(_) => self.keyboard_focus = None,
				_ => {}
			}

			if let Some(focus) = self.keyboard_focus {
				self.events
					.entry(focus)
					.or_default()
					.push_back(ContextEvent::Keyboard(event));
			}
		} else if packet.object_id == self.pointer_id
			&& let Some(event) = PointerEvent::parse(packet.opcode, &packet.payload, &mut self.fds)
		{
			match &event {
				PointerEvent::Enter(e) => {
					self.mouse_focus = Some(e.surface_id);
					self.last_mouse_surface = Some(e.surface_id);
				}
				PointerEvent::Leave(_) => {
					self.mouse_focus = None;
				}
				_ => {}
			}

			// Motion events require active focus; button events are routed to the
			// last entered surface so releases aren't dropped after a Leave.
			let route_to = match &event {
				PointerEvent::Button(_) => self.mouse_focus.or(self.last_mouse_surface),
				_ => self.mouse_focus,
			};
			if let Some(focus) = route_to {
				self.events
					.entry(focus)
					.or_default()
					.push_back(ContextEvent::Pointer(event));
			}
		} else if packet.object_id == self.globals.xdg_wm_base.unwrap()
			&& let Some(event) = XdgWmBaseEvent::parse(packet.opcode, &packet.payload, &mut self.fds)
		{
			let XdgWmBaseEvent::Ping(ping_event) = event;
			PongRequest {
				callback_id: ping_event.callback_id,
			}
			.write_as_packet(self.globals.xdg_wm_base.unwrap(), &self.conn.stream)?;
		} else {
			self.events
				.entry(packet.object_id)
				.or_default()
				.push_back(ContextEvent::Unknown {
					opcode: packet.opcode,
					payload: packet.payload,
				});
		}
		Ok(())
	}
}

#[derive(Debug)]
pub struct BoundGlobals {
	pub compositor: Option<u32>,
	pub shm: Option<u32>,
	pub xdg_wm_base: Option<u32>,
	pub seat: Option<u32>,
	pub zwlr_layer_shell_v1: Option<u32>,
}

impl BoundGlobals {
	fn new() -> Self {
		Self {
			compositor: None,
			shm: None,
			xdg_wm_base: None,
			seat: None,
			zwlr_layer_shell_v1: None,
		}
	}
}

pub struct ObjectId(u32);
impl ObjectId {
	pub fn next(&mut self) -> u32 {
		let id = self.0;
		self.0 += 1;
		id
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
			"wl_compositor" | "wl_shm" | "xdg_wm_base" | "wl_seat" | "zwlr_layer_shell_v1" => *version,
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
			"zwlr_layer_shell_v1" => bound.zwlr_layer_shell_v1 = Some(id),
			_ => {}
		}
	}

	Ok(bound)
}

#[derive(Debug)]
pub enum ContextEvent {
	Keyboard(KeyboardEvent),
	Pointer(PointerEvent),
	Unknown { opcode: u16, payload: Vec<u8> },
}

pub struct Keyboard {
	keymap: Option<XkbKeyMap>,
}

impl Keyboard {
	pub fn new() -> Self {
		Self { keymap: None }
	}

	pub fn set_keymap(&mut self, src_fd: &OwnedFd) -> io::Result<()> {
		let cloned_fd = src_fd.try_clone()?;
		let mut file = std::fs::File::from(cloned_fd);
		let mut contents = String::new();
		file.read_to_string(&mut contents)?;
		println!("Received keymap:\n{}", contents);
		self.keymap = Some(XkbKeyMap::from_str(&contents).map_err(io::Error::other)?);
		Ok(())
	}
}
