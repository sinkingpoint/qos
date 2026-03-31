use bytestruct::{LengthPrefixedString, Padding};
use bytestruct_derive::ByteStruct;

use crate::wayland::types::{Command, SubSystem};

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
	fn handle(&self, client: &mut super::types::Client, registry: &mut Registry) -> super::types::WaylandResult<()> {
		match &*self.interface {
			_ => {
				// For now, just ignore unknown interfaces. In the future, we might want to send an error back to the client.
				eprintln!("Unknown interface requested: {}", self.interface.0);
			}
		}
		Ok(())
	}
}
