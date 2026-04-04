use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct CreateBufferRequest {
	pub buffer_id: u32,
	pub offset: i32,
	pub width: i32,
	pub height: i32,
	pub stride: i32,
	pub format: u32,
}

wayland_payload!(CreateBufferRequest, opcode = 0);
