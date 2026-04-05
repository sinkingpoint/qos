use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct GetTopLevelSurfaceRequest {
	pub new_id: u32,
}

wayland_payload!(GetTopLevelSurfaceRequest, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct AckConfigureRequest {
	pub serial: u32,
}

wayland_payload!(AckConfigureRequest, opcode = 4);

#[derive(Debug, ByteStruct)]
pub struct ConfigureEvent {
	pub serial: u32,
}

wayland_payload!(ConfigureEvent, opcode = 0);

crate::wayland_client_events!(XdgSurfaceEvent {
	ConfigureEvent::OPCODE => Configure(ConfigureEvent),
});
