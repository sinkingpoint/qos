use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::{LengthPrefixedString, Padding, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

use crate::wayland::{
	WaylandPacket,
	types::{ClientEffect, Command, SubSystem, WaylandError, WaylandResult},
};

pub struct Registry;

impl SubSystem for Registry {
	type Request = RegistryRequest;
	const NAME: &'static str = "wl_registry";
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
	fn handle(&self, connection: &Arc<UnixStream>, _registry: &mut Registry) -> WaylandResult<Option<ClientEffect>> {
		match self.interface.as_ref() {
			"wl_compositor" => Ok(Some(ClientEffect::Register(
				self.new_id,
				crate::wayland::types::SubsystemType::Compositor(crate::wayland::compositor::Compositor),
			))),
			"wl_shm" => {
				// Write the wl_shm.format event immediately, since the client expects it to be sent as part of the bind request.
				let packet = WaylandPacket::new(self.new_id, 0, 0_u32.to_le_bytes().to_vec());
				let mut buf = Vec::new();
				packet
					.write_to_with_endian(&mut buf, bytestruct::Endian::Little)
					.map_err(WaylandError::IOError)?;
				connection.as_ref().write_all(&buf).map_err(WaylandError::IOError)?;

				Ok(Some(ClientEffect::Register(
					self.new_id,
					crate::wayland::types::SubsystemType::SharedMemory(crate::wayland::shm::SharedMemory),
				)))
			}
			"xdg_wm_base" => Ok(Some(ClientEffect::Register(
				self.new_id,
				crate::wayland::types::SubsystemType::XdgWmBase(crate::wayland::xdg_wm_base::XdgWmBase),
			))),
			_ => Ok(None), // unrecognised interface, ignore for now
		}
	}
}
