use std::{
	fs::File,
	io::IoSlice,
	os::{fd::AsRawFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::{Endian, LengthPrefixedVec, WriteToWithEndian};
use bytestruct_derive::ByteStruct;
use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};

use crate::types::{WaylandPacket, WaylandPayload};
use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug)]
pub struct KeyMapEvent {
	pub keymap_path: String,
}

impl KeyMapEvent {
	pub fn new(keymap_path: String) -> Self {
		Self { keymap_path }
	}
}

impl WaylandPayload for KeyMapEvent {
	const OPCODE: u16 = 0;

	fn write_as_packet(&self, object_id: u32, connection: &Arc<UnixStream>) -> std::io::Result<()> {
		let file = File::open(&self.keymap_path)?;
		let size = file.metadata()?.len() as u32 + 1;

		let mut payload = Vec::new();
		1u32.write_to_with_endian(&mut payload, Endian::Little)?;
		size.write_to_with_endian(&mut payload, Endian::Little)?;

		let packet = WaylandPacket::new(object_id, Self::OPCODE, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, Endian::Little)?;

		let raw_fd = file.as_raw_fd();
		let iov = [IoSlice::new(&buf)];
		let cmsg = [ControlMessage::ScmRights(&[raw_fd])];
		sendmsg::<()>(connection.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None)?;
		Ok(())
	}
}

#[derive(Debug, ByteStruct)]
pub struct KeyEnterEvent {
	pub serial: u32,
	pub surface_id: u32,
	pub keys: LengthPrefixedVec<u32, u32>,
}

wayland_payload!(KeyEnterEvent, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct KeyLeaveEvent {
	pub serial: u32,
	pub surface_id: u32,
}

wayland_payload!(KeyLeaveEvent, opcode = 2);

#[derive(Debug, ByteStruct)]
pub struct KeyEvent {
	pub serial: u32,
	pub time: u32,
	pub key: u32,
	pub state: u32,
}

wayland_payload!(KeyEvent, opcode = 3);

#[derive(Debug, ByteStruct)]
pub struct ModifiersEvent {
	pub serial: u32,
	pub depressed: u32,
	pub latched: u32,
	pub locked: u32,
	pub group: u32,
}

wayland_payload!(ModifiersEvent, opcode = 4);
