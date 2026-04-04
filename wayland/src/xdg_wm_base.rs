use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct CreatePositionerRequest {
	pub positioner_id: u32,
}

wayland_payload!(CreatePositionerRequest, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct GetXdgSurfaceRequest {
	pub new_id: u32,
	pub surface_id: u32,
}

wayland_payload!(GetXdgSurfaceRequest, opcode = 2);

#[derive(Debug, ByteStruct)]
pub struct PongRequest {
	pub callback_id: u32,
}

wayland_payload!(PongRequest, opcode = 3);
