use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::pointer::{DestroyRequest, SetCursorRequest};

pub use wayland::pointer::{ButtonCode, ButtonState};

use crate::{
	events::input::KeyCode,
	wayland::types::{ClientEffect, Command, SubSystem, WaylandResult},
};

pub use wayland::pointer::ButtonEvent;

pub struct Pointer;

impl SubSystem for Pointer {
	type Request = PointerRequest;
	const NAME: &'static str = "wl_pointer";
}

wayland_interface!(Pointer, PointerRequest {
  SetCursorRequest::OPCODE => SetCursor(SetCursorRequest),
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
});

impl Command<Pointer> for SetCursorRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _pointer: &mut Pointer) -> WaylandResult<Option<ClientEffect>> {
		Ok(None) // TODO: implement cursor setting
	}
}

impl Command<Pointer> for DestroyRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _pointer: &mut Pointer) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

impl TryFrom<KeyCode> for ButtonCode {
	type Error = ();

	fn try_from(value: KeyCode) -> Result<Self, Self::Error> {
		match value {
			KeyCode::KeyLeft => Ok(ButtonCode::Left),
			KeyCode::KeyRight => Ok(ButtonCode::Right),
			_ => Err(()),
		}
	}
}
