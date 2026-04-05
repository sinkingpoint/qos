use bytestruct_derive::ByteStruct;

use crate::types::WaylandEncodedString;
use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct BindRequest {
	pub name: u32,
	pub interface: WaylandEncodedString,
	pub version: u32,
	pub new_id: u32,
}

wayland_payload!(BindRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct GlobalEvent {
	pub name: u32,
	pub interface: WaylandEncodedString,
	pub version: u32,
}

wayland_payload!(GlobalEvent, opcode = 0);

crate::wayland_client_events!(RegistryEvent {
	GlobalEvent::OPCODE => Global(GlobalEvent),
});
