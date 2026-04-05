use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct CreatePoolRequest {
	pub pool_id: u32,
	pub size: u32,
}

wayland_payload!(CreatePoolRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct FormatEvent {
	pub format: u32,
}

wayland_payload!(FormatEvent, opcode = 0);

crate::wayland_client_events!(ShmEvent {
	FormatEvent::OPCODE => Format(FormatEvent),
});
