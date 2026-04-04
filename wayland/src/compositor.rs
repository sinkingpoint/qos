use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct CreateSurfaceRequest {
	pub new_id: u32,
}

wayland_payload!(CreateSurfaceRequest, opcode = 0);
