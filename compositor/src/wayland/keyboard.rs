use std::{
	io::IoSlice,
	os::{fd::AsRawFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;
use nix::sys::{
	memfd::{MemFdCreateFlag, memfd_create},
	socket::{ControlMessage, MsgFlags, sendmsg},
};

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
	fn handle(&self, _connection: &Arc<UnixStream>, _keyboard: &mut Keyboard) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug)]
pub struct KeyMapCommand {
	pub keymap_path: String,
}

impl KeyMapCommand {
	pub fn new_no_keymap() -> Self {
		Self {
			keymap_path: String::new(),
		}
	}

	pub fn write_as_packet(&self, object_id: u32, connection: &Arc<UnixStream>) -> WaylandResult<()> {
		// TODO: If we have a keyboard_path, we should mmap it and send the fd instead of sending an empty keymap
		let memfd = memfd_create(c"wl-keymap", MemFdCreateFlag::empty())
			.map_err(crate::wayland::types::WaylandError::NixError)?;

		nix::unistd::ftruncate(&memfd, 1).map_err(crate::wayland::types::WaylandError::NixError)?;

		let mut payload = Vec::new();
		// format: 0 = no keymap
		0u32.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
		// size: 1 (minimal memfd)
		1u32.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

		let packet = WaylandPacket::new(object_id, 0, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;

		let raw_fd = memfd.as_raw_fd();
		let iov = [IoSlice::new(&buf)];
		let cmsg = [ControlMessage::ScmRights(&[raw_fd])];
		sendmsg::<()>(connection.as_raw_fd(), &iov, &cmsg, MsgFlags::empty(), None)
			.map_err(crate::wayland::types::WaylandError::NixError)?;

		Ok(())
	}
}
