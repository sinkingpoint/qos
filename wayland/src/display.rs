use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct SyncRequest {
	pub callback_id: u32,
}

wayland_payload!(SyncRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct GetRegistryRequest {
	pub registry_id: u32,
}

wayland_payload!(GetRegistryRequest, opcode = 1);
