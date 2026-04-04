use std::{io, os::unix::net::UnixStream, sync::Arc};

use bitflags::bitflags;
use bytestruct::{ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

use crate::wayland::{
	keyboard::{KeyMapCommand, Keyboard},
	pointer::Pointer,
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult},
};

pub struct Seat;

impl SubSystem for Seat {
	type Request = WlSeatRequest;
	const NAME: &'static str = "wl_seat";
}

wayland_interface!(Seat, WlSeatRequest {
  0 => GetPointer(GetPointerCommand),
  1 => GetKeyboard(GetKeyboardCommand),
  3 => Release(ReleaseCommand),
});

#[derive(Debug, ByteStruct)]
pub struct GetPointerCommand {
	pub new_id: u32,
}

impl Command<Seat> for GetPointerCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _seat: &mut Seat) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::Pointer(Pointer),
		)))
	}
}

#[derive(Debug, ByteStruct)]
pub struct GetKeyboardCommand {
	pub new_id: u32,
}

impl Command<Seat> for GetKeyboardCommand {
	fn handle(self, connection: &Arc<UnixStream>, _seat: &mut Seat) -> WaylandResult<Option<ClientEffect>> {
		let keymap = KeyMapCommand::new("/etc/xkb/qwerty".to_string());
		keymap.write_as_packet(self.new_id, connection)?;
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::Keyboard(Keyboard),
		)))
	}
}

#[derive(Debug, ByteStruct)]
pub struct ReleaseCommand;

impl Command<Seat> for ReleaseCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _seat: &mut Seat) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

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

#[derive(Debug, ByteStruct)]
pub struct CapabilitiesCommand {
	pub capabilities: SeatCapabilities,
}

impl CapabilitiesCommand {
	pub fn new(capabilities: SeatCapabilities) -> Self {
		Self { capabilities }
	}
}
