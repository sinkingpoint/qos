use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct::{LengthPrefixedString, Padding};
use bytestruct_derive::ByteStruct;

use crate::wayland::types::{Command, SubSystem, SubsystemType, WaylandResult};

pub struct Registry;

impl SubSystem for Registry {
	type Request = RegistryRequest;
}

wayland_interface!(Registry, RegistryRequest {
  0 => Bind(BindCommand),
});

#[derive(Debug, ByteStruct)]
pub struct BindCommand {
	pub name: u32,
	pub interface: LengthPrefixedString<u32>,
	pub padding: Padding<4>,
	pub version: u32,
	pub new_id: u32,
}

impl Command<Registry> for BindCommand {
	fn handle(
		&self,
		_connection: &Arc<UnixStream>,
		_registry: &mut Registry,
	) -> WaylandResult<Option<(u32, SubsystemType)>> {
		eprintln!("Unknown interface requested: {}", self.interface.0);
		Ok(None)
	}
}
