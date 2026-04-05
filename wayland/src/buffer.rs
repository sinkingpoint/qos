use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct ReleaseEvent;

wayland_payload!(ReleaseEvent, opcode = 0);

crate::wayland_client_events!(BufferEvent {
	ReleaseEvent::OPCODE => Release(ReleaseEvent),
});
