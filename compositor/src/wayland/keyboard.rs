use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::keyboard::DestroyRequest;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub use wayland::keyboard::{
	KeyEnterEvent as KeyEnterCommand, KeyEvent as KeyEventPacket, KeyLeaveEvent as KeyLeaveCommand,
	KeyMapEvent as KeyMapCommand, ModifiersEvent as ModifiersCommand,
};

pub struct Keyboard;

impl SubSystem for Keyboard {
	type Request = KeyboardRequest;
	const NAME: &'static str = "wl_keyboard";
}

wayland_interface!(Keyboard, KeyboardRequest {
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
});

impl Command<Keyboard> for DestroyRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _keyboard: &mut Keyboard) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}
