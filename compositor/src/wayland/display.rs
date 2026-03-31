use std::io::Write;

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;

use crate::{
	wayland::types::{Command, SubSystem, WaylandError, WaylandPacket, WaylandResult},
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
	fn handle(&self, client: &mut super::types::Client, display: &mut Display) -> WaylandResult<()> {
		let mut data = Vec::new();
		WaylandPacket::new(self.callback_id, 0, 0)
			.write_to_with_endian(&mut data, bytestruct::Endian::Little)
			.map_err(WaylandError::IOError)?;

		client.connection.write_all(&data).map_err(WaylandError::IOError)
	}
}

#[derive(Debug, ByteStruct)]
pub struct GetRegistry {
	pub registry_id: u32,
}

impl Command<Display> for GetRegistry {
	fn handle(&self, client: &mut super::types::Client, display: &mut Display) -> WaylandResult<()> {
		client.register_object(
			self.registry_id,
			super::types::SubsystemType::Registry(super::registry::Registry),
		);

		Ok(())
	}
}
