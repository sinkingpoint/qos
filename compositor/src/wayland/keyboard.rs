use std::{
	fs::File,
	io::{IoSlice, Write},
	os::{fd::AsRawFd, unix::net::UnixStream},
	sync::Arc,
};

use bytestruct::{LengthPrefixedVec, WriteToWithEndian};
use bytestruct_derive::ByteStruct;
use nix::{
	sys::socket::{ControlMessage, MsgFlags, sendmsg},
	time::{ClockId, clock_gettime},
};

use crate::{
	keyboard::Modifiers,
	wayland::{
		WaylandPacket,
		types::{ClientEffect, Command, SubSystem, WaylandResult},
	},
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

#[derive(Debug, ByteStruct)]
pub struct KeyEnterCommand {
	serial: u32,
	surface_id: u32,
	keys: LengthPrefixedVec<u32, u32>,
}

impl KeyEnterCommand {
	pub fn new(serial: u32, surface_id: u32, keys: Vec<u32>) -> Self {
		Self {
			serial,
			surface_id,
			keys: LengthPrefixedVec::new(keys),
		}
	}

	pub fn write_as_packet(&self, keyboard_id: u32, connection: &Arc<UnixStream>) -> WaylandResult<()> {
		let mut payload = Vec::new();
		self.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

		let packet = WaylandPacket::new(keyboard_id, 1, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
		connection.as_ref().write_all(&buf)?;

		Ok(())
	}
}

#[derive(Debug, ByteStruct)]
pub struct KeyLeaveCommand {
	serial: u32,
	surface_id: u32,
}

impl KeyLeaveCommand {
	pub fn new(serial: u32, surface_id: u32) -> Self {
		Self { serial, surface_id }
	}

	pub fn write_as_packet(&self, keyboard_id: u32, connection: &Arc<UnixStream>) -> WaylandResult<()> {
		let mut payload = Vec::new();
		self.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

		let packet = WaylandPacket::new(keyboard_id, 2, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
		connection.as_ref().write_all(&buf)?;

		Ok(())
	}
}

#[derive(Debug, ByteStruct)]
pub struct KeyCommand {
	serial: u32,
	time: u32,
	key: u32,
	state: u32,
}

impl KeyCommand {
	pub fn new(serial: u32, key: u32, state: u32) -> WaylandResult<Self> {
		let time = clock_gettime(ClockId::CLOCK_MONOTONIC)?;
		let time_ms = time.tv_sec() as u64 * 1000 + (time.tv_nsec() as u64) / 1_000_000;
		Ok(Self {
			serial,
			time: time_ms as u32,
			key,
			state,
		})
	}

	pub fn write_as_packet(&self, keyboard_id: u32, connection: &Arc<UnixStream>) -> WaylandResult<()> {
		let mut payload = Vec::new();
		self.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

		let packet = WaylandPacket::new(keyboard_id, 3, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
		connection.as_ref().write_all(&buf)?;

		Ok(())
	}
}

#[derive(Debug, ByteStruct)]
pub struct ModifiersCommand {
	serial: u32,
	depressed: Modifiers,
	latched: Modifiers,
	locked: Modifiers,
	group: u32,
}

impl ModifiersCommand {
	pub fn new(serial: u32, depressed: Modifiers, latched: Modifiers, locked: Modifiers, group: u32) -> Self {
		Self {
			serial,
			depressed,
			latched,
			locked,
			group,
		}
	}

	pub fn write_as_packet(&self, keyboard_id: u32, connection: &Arc<UnixStream>) -> WaylandResult<()> {
		let mut payload = Vec::new();
		self.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

		let packet = WaylandPacket::new(keyboard_id, 4, payload);
		let mut buf = Vec::new();
		packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
		connection.as_ref().write_all(&buf)?;

		Ok(())
	}
}
