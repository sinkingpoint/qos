use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;

use crate::{
	wayland::types::{Command, SubSystem, SubsystemType, WaylandError, WaylandPacket, WaylandResult},
	wayland_interface,
};

pub struct Display {}

impl SubSystem for Display {
	type Request = DisplayRequest;
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
	fn handle(
		&self,
		connection: &Arc<UnixStream>,
		_display: &mut Display,
	) -> WaylandResult<Option<(u32, SubsystemType)>> {
		let mut payload = Vec::new();
		0u32.write_to_with_endian(&mut payload, bytestruct::Endian::Little)
			.map_err(WaylandError::IOError)?;
		let packet = WaylandPacket::new(self.callback_id, 0, payload);
		let mut buf = Vec::new();
		packet
			.write_to_with_endian(&mut buf, bytestruct::Endian::Little)
			.map_err(WaylandError::IOError)?;
		connection.as_ref().write_all(&buf).map_err(WaylandError::IOError)?;
		Ok(None)
	}
}

#[derive(Debug, ByteStruct)]
pub struct GetRegistry {
	pub registry_id: u32,
}

impl Command<Display> for GetRegistry {
	fn handle(
		&self,
		_connection: &Arc<UnixStream>,
		_display: &mut Display,
	) -> WaylandResult<Option<(u32, SubsystemType)>> {
		Ok(Some((
			self.registry_id,
			SubsystemType::Registry(super::registry::Registry),
		)))
	}
}
