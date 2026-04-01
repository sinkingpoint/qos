use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Surface;

impl SubSystem for Surface {
	type Request = SurfaceRequest;
}

wayland_interface!(Surface, SurfaceRequest {
  0 => Destroy(DestroyCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Surface> for DestroyCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, _surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}
