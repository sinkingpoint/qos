use bytestruct_derive::ByteStruct;

use crate::types::WaylandEncodedString;
use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct SetTitleRequest {
	pub title: WaylandEncodedString,
}

wayland_payload!(SetTitleRequest, opcode = 2);

#[derive(Debug, ByteStruct)]
pub struct MoveRequest {
	pub seat_id: u32,
	pub serial: u32,
}

wayland_payload!(MoveRequest, opcode = 5);

#[derive(Debug, ByteStruct)]
pub struct ConfigureEvent {
	pub width: i32,
	pub height: i32,
	pub states: u32,
}

wayland_payload!(ConfigureEvent, opcode = 0);
