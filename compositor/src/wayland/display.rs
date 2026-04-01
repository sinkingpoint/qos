use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;

use crate::{
	wayland::types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandError, WaylandPacket, WaylandResult},
	wayland_interface,
};

const GLOBALS_TO_ADVERTISE: &[SubsystemType] = &[
	SubsystemType::Compositor(crate::wayland::compositor::Compositor),
	SubsystemType::SharedMemory(crate::wayland::shm::SharedMemory),
];

pub struct Display {}

impl SubSystem for Display {
	type Request = DisplayRequest;
	const NAME: &'static str = "wl_display";
}

wayland_interface!(Display, DisplayRequest {
  0 => Sync(SyncCommand),
  1 => GetRegistry(GetRegistry),
});

#[derive(Debug, ByteStruct)]
pub struct SyncCommand {
	pub callback_id: u32,
}

impl Command<Display> for SyncCommand {
	fn handle(&self, connection: &Arc<UnixStream>, _display: &mut Display) -> WaylandResult<Option<ClientEffect>> {
		let mut payload = Vec::new();
		0u32.write_to_with_endian(&mut payload, bytestruct::Endian::Little)
			.map_err(WaylandError::IOError)?;
		let packet = WaylandPacket::new(self.callback_id, 0, payload);
		let mut buf = Vec::new();
		packet
			.write_to_with_endian(&mut buf, bytestruct::Endian::Little)
			.map_err(WaylandError::IOError)?;
		connection.as_ref().write_all(&buf).map_err(WaylandError::IOError)?;
		Ok(Some(ClientEffect::Unregister(self.callback_id)))
	}
}

#[derive(Debug, ByteStruct)]
pub struct GetRegistry {
	pub registry_id: u32,
}

impl Command<Display> for GetRegistry {
	fn handle(&self, connection: &Arc<UnixStream>, _display: &mut Display) -> WaylandResult<Option<ClientEffect>> {
		for (i, global) in GLOBALS_TO_ADVERTISE.iter().enumerate() {
			let name = global.name();
			let version = global.version();
			let mut payload = Vec::new();

			(i as u32).write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
			(name.len() as u32 + 1).write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
			payload.extend_from_slice(name.as_bytes());
			// Null Terminator
			0u8.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
			// Pad to 4 bytes
			let padding = (4 - ((name.len() as u32 + 1) % 4)) % 4; // calculate padding needed to align to 4 bytes
			for _ in 0..padding {
				0u8.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
			}
			version.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

			let packet = WaylandPacket::new(self.registry_id, 0, payload);
			let mut buf = Vec::new();
			packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
			connection.as_ref().write_all(&buf).map_err(WaylandError::IOError)?;
		}

		Ok(Some(ClientEffect::Register(
			self.registry_id,
			SubsystemType::Registry(super::registry::Registry),
		)))
	}
}
