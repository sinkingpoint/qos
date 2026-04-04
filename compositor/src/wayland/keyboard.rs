use std::{
	fs::File,
	io::IoSlice,
	os::{fd::AsRawFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;
use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};

use crate::wayland::{
	WaylandPacket,
	types::{ClientEffect, Command, SubSystem, WaylandResult},
};

pub struct Keyboard;

impl SubSystem for Keyboard {
	type Request = KeyboardRequest;
	const NAME: &'static str = "wl_keyboard";
}

wayland_interface!(Keyboard, KeyboardRequest {
  0 => Destroy(DestroyCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Keyboard> for DestroyCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _keyboard: &mut Keyboard) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug)]
pub struct KeyMapCommand {
	pub keymap_path: String,
}

impl KeyMapCommand {
	pub fn new(keymap_path: String) -> Self {
		Self { keymap_path }
	}

	pub fn write_as_packet(&self, object_id: u32, connection: &Arc<UnixStream>) -> WaylandResult<()> {
		let file = File::open(&self.keymap_path)?;
		let size = file.metadata()?.len() as u32 + 1; // +1 for the null terminator

		let mut payload = Vec::new();
		1u32.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
		size.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

		let packet = WaylandPacket::new(object_id, 0, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;

		let raw_fd = file.as_raw_fd();
		let iov = [IoSlice::new(&buf)];
		let cmsg = [ControlMessage::ScmRights(&[raw_fd])];
		sendmsg::<()>(connection.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None)?;

		Ok(())
	}
}
