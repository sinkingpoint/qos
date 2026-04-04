use bytestruct_derive::ByteStruct;

use crate::types::WaylandEncodedString;
use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct ReleaseRequest;

wayland_payload!(ReleaseRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct GeometryEvent {
	pub x: i32,
	pub y: i32,
	pub physical_width: i32,
	pub physical_height: i32,
	pub subpixel: i32,
	pub make: WaylandEncodedString,
	pub model: WaylandEncodedString,
	pub transform: i32,
}

wayland_payload!(GeometryEvent, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct ModeEvent {
	pub flags: u32,
	pub width: i32,
	pub height: i32,
	pub refresh_rate: i32,
}

wayland_payload!(ModeEvent, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct DoneEvent;

wayland_payload!(DoneEvent, opcode = 2);
