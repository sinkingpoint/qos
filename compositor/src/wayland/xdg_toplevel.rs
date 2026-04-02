use bytestruct_derive::ByteStruct;

use crate::wayland::types::{Command, SubSystem};

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
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_toplevel: &mut XdgTopLevel,
	) -> crate::wayland::types::WaylandResult<Option<crate::wayland::types::ClientEffect>> {
		Ok(Some(crate::wayland::types::ClientEffect::DestroySelf))
	}
}
