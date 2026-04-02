use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Surface {
	pub attached_buffer: Option<(u32, i32, i32)>,
	pub committed: bool,
	pub blitted: bool,
}

impl Surface {
	pub fn new() -> Self {
		Self {
			attached_buffer: None,
			committed: false,
			blitted: false,
		}
	}
}

impl SubSystem for Surface {
	type Request = SurfaceRequest;
	const NAME: &'static str = "wl_surface";
}

wayland_interface!(Surface, SurfaceRequest {
  0 => Destroy(DestroyCommand),
	1 => Attach(AttachCommand),
	6 => Commit(CommitCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Surface> for DestroyCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, _surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug, ByteStruct)]
pub struct AttachCommand {
	pub buffer_id: u32,
	pub x: i32,
	pub y: i32,
}

impl Command<Surface> for AttachCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		surface.attached_buffer = Some((self.buffer_id, self.x, self.y));
		surface.committed = false;
		surface.blitted = false;
		Ok(None)
	}
}

#[derive(Debug, ByteStruct)]
pub struct CommitCommand;

impl Command<Surface> for CommitCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		if surface.attached_buffer.is_some() {
			surface.committed = true;
			surface.blitted = false;
		}

		Ok(None)
	}
}
