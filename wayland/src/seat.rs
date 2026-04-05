use bytestruct_derive::ByteStruct;
use std::io;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct GetPointerRequest {
	pub new_id: u32,
}

wayland_payload!(GetPointerRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct GetKeyboardRequest {
	pub new_id: u32,
}

wayland_payload!(GetKeyboardRequest, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct ReleaseRequest;

wayland_payload!(ReleaseRequest, opcode = 3);

#[derive(Debug, ByteStruct)]
pub struct CapabilitiesEvent {
	pub capabilities: u32,
}

wayland_payload!(CapabilitiesEvent, opcode = 0);

crate::wayland_client_events!(SeatEvent {
	CapabilitiesEvent::OPCODE => Capabilities(CapabilitiesEvent),
});

use bitflags::bitflags;
use bytestruct::{ReadFromWithEndian, WriteToWithEndian};

bitflags! {
  #[derive(Debug)]
  pub struct SeatCapabilities: u32 {
	const POINTER = 0x1;
	const KEYBOARD = 0x2;
	const TOUCH = 0x4;
  }
}

impl WriteToWithEndian for SeatCapabilities {
	fn write_to_with_endian<W: io::Write>(&self, writer: &mut W, endian: bytestruct::Endian) -> io::Result<()> {
		self.bits().write_to_with_endian(writer, endian)
	}
}

impl ReadFromWithEndian for SeatCapabilities {
	fn read_from_with_endian<R: io::Read>(reader: &mut R, endian: bytestruct::Endian) -> io::Result<Self> {
		let bits = u32::read_from_with_endian(reader, endian)?;
		Ok(Self::from_bits_truncate(bits))
	}
}
