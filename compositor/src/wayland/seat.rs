use std::{
	fs::File,
	os::{fd::OwnedFd, unix::net::UnixStream},
	sync::Arc,
};

use wayland::seat::{GetKeyboardRequest, GetPointerRequest, ReleaseRequest};

pub use wayland::seat::SeatCapabilities;

use crate::wayland::{
	keyboard::{KeyMapCommand, Keyboard},
	pointer::Pointer,
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult},
};
use wayland::types::WithFd;

pub struct Seat;

impl SubSystem for Seat {
	type Request = WlSeatRequest;
	const NAME: &'static str = "wl_seat";
}

wayland_interface!(Seat, WlSeatRequest {
  GetPointerRequest::OPCODE => GetPointer(GetPointerRequest),
  GetKeyboardRequest::OPCODE => GetKeyboard(GetKeyboardRequest),
  ReleaseRequest::OPCODE => Release(ReleaseRequest),
});

impl Command<Seat> for GetPointerRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _seat: &mut Seat) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::Pointer(Pointer),
		)))
	}
}

impl Command<Seat> for GetKeyboardRequest {
	fn handle(self, connection: &Arc<UnixStream>, _seat: &mut Seat) -> WaylandResult<Option<ClientEffect>> {
		let file = File::open("/etc/xkb/qwerty")?;
		let size = file.metadata()?.len() as u32 + 1;
		let keymap = WithFd {
			cmd: KeyMapCommand { format: 1, size },
			fd: OwnedFd::from(file),
		};
		keymap.write_as_packet(self.new_id, connection)?;
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::Keyboard(Keyboard),
		)))
	}
}

impl Command<Seat> for ReleaseRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _seat: &mut Seat) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}
