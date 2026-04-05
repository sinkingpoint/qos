use std::{
	os::{fd::AsRawFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::{LengthPrefixedVec, WriteToWithEndian};
use bytestruct_derive::ByteStruct;
use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};
use std::io::IoSlice;

use crate::types::{WaylandPacket, WithFd};
use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct KeyMapEvent {
	pub format: u32,
	pub size: u32,
}

wayland_payload!(KeyMapEvent, opcode = 0);

impl WithFd<KeyMapEvent> {
	pub fn write_as_packet(&self, object_id: u32, connection: &Arc<UnixStream>) -> std::io::Result<()> {
		let mut payload = Vec::new();
		self.cmd
			.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
		let packet = WaylandPacket::new(object_id, KeyMapEvent::OPCODE, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
		let iov = [IoSlice::new(&buf)];
		let cmsg = [ControlMessage::ScmRights(&[self.fd.as_raw_fd()])];
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

crate::wayland_client_events!(KeyboardEvent {
	KeyMapEvent::OPCODE    => KeyMap(WithFd<KeyMapEvent>),
	KeyEnterEvent::OPCODE  => Enter(KeyEnterEvent),
	KeyLeaveEvent::OPCODE  => Leave(KeyLeaveEvent),
	KeyEvent::OPCODE       => Key(KeyEvent),
	ModifiersEvent::OPCODE => Modifiers(ModifiersEvent),
});
