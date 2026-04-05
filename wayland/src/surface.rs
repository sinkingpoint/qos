use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct AttachRequest {
	pub buffer_id: u32,
	pub x: i32,
	pub y: i32,
}

wayland_payload!(AttachRequest, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct DamageRequest {
	pub x: i32,
	pub y: i32,
	pub width: i32,
	pub height: i32,
}

wayland_payload!(DamageRequest, opcode = 2);

#[derive(Debug, ByteStruct)]
pub struct FrameRequest {
	pub callback_id: u32,
}

wayland_payload!(FrameRequest, opcode = 3);

#[derive(Debug, ByteStruct)]
pub struct CommitRequest;

wayland_payload!(CommitRequest, opcode = 6);

// Sent on wl_callback objects created by wl_surface.frame
#[derive(Debug, ByteStruct)]
pub struct FrameCallbackEvent {
	pub time_msec: u32,
}

wayland_payload!(FrameCallbackEvent, opcode = 0);

crate::wayland_client_events!(FrameCallbackObjectEvent {
	FrameCallbackEvent::OPCODE => Done(FrameCallbackEvent),
});
