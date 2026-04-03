use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Pointer;

impl SubSystem for Pointer {
	type Request = PointerRequest;
	const NAME: &'static str = "wl_pointer";
}

wayland_interface!(Pointer, PointerRequest {
  0 => SetCursor(SetCursorCommand),
  1 => Destroy(DestroyCommand),
});

#[derive(Debug, ByteStruct)]
pub struct SetCursorCommand {
	pub serial: u32,
	pub surface_id: u32,
	pub hotspot_x: i32,
	pub hotspot_y: i32,
}

impl Command<Pointer> for SetCursorCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _pointer: &mut Pointer) -> WaylandResult<Option<ClientEffect>> {
		Ok(None) // TODO: implement cursor setting
	}
}

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Pointer> for DestroyCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _pointer: &mut Pointer) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}
