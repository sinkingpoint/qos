use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct XdgTopLevel {}

impl XdgTopLevel {
	pub fn new() -> Self {
		Self {}
	}
}

impl SubSystem for XdgTopLevel {
	type Request = XdgTopLevelRequest;
	const NAME: &'static str = "xdg_toplevel";
}

wayland_interface!(XdgTopLevel, XdgTopLevelRequest {
  0 => Destroy(DestroyCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<XdgTopLevel> for DestroyCommand {
	fn handle(
		&self,
		_connection: &Arc<UnixStream>,
		_xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}
