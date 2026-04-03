use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::{Endian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

use crate::wayland::{
	DisplayGeometry, WaylandPacket,
	compositor::Compositor,
	output::{Output, geometry_command_packet, mode_command_packet},
	seat::{CapabilitiesCommand, Seat, SeatCapabilities},
	shm::SharedMemory,
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandEncodedString, WaylandError, WaylandResult},
	xdg_wm_base::XdgWmBase,
};

pub struct Registry {
	display_geometry: DisplayGeometry,
}

impl Registry {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self { display_geometry }
	}
}

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
	pub interface: WaylandEncodedString,
	pub version: u32,
	pub new_id: u32,
}

impl Command<Registry> for BindCommand {
	fn handle(&self, connection: &Arc<UnixStream>, registry: &mut Registry) -> WaylandResult<Option<ClientEffect>> {
		match self.interface.as_ref() {
			"wl_compositor" => Ok(Some(ClientEffect::Register(
				self.new_id,
				SubsystemType::Compositor(Compositor),
			))),
			"wl_shm" => {
				println!("Client bound to wl_shm, sending supported formats");
				// Write the wl_shm.format event immediately, since the client expects it to be sent as part of the bind request.
				let argb_packet = WaylandPacket::new(self.new_id, 0, 0u32.to_le_bytes().to_vec());
				let xrgb_packet = WaylandPacket::new(self.new_id, 0, 1u32.to_le_bytes().to_vec());
				let mut buf = Vec::new();
				argb_packet
					.write_to_with_endian(&mut buf, Endian::Little)
					.map_err(WaylandError::IOError)?;
				xrgb_packet
					.write_to_with_endian(&mut buf, Endian::Little)
					.map_err(WaylandError::IOError)?;
				connection.as_ref().write_all(&buf).map_err(WaylandError::IOError)?;

				Ok(Some(ClientEffect::Register(
					self.new_id,
					SubsystemType::SharedMemory(SharedMemory),
				)))
			}
			"xdg_wm_base" => Ok(Some(ClientEffect::Register(
				self.new_id,
				SubsystemType::XdgWmBase(XdgWmBase),
			))),
			"wl_seat" => {
				let capabilities = CapabilitiesCommand::new(SeatCapabilities::KEYBOARD | SeatCapabilities::POINTER);
				let mut bytes = Vec::new();
				capabilities.write_to_with_endian(&mut bytes, Endian::Little)?;

				let packet = WaylandPacket::new(self.new_id, 0, bytes);
				let mut bytes = Vec::new();
				packet.write_to_with_endian(&mut bytes, Endian::Little)?;
				connection.as_ref().write_all(&bytes)?;
				Ok(Some(ClientEffect::Register(self.new_id, SubsystemType::Seat(Seat))))
			}
			"wl_output" => {
				let geometry_packet = geometry_command_packet(&registry.display_geometry, self.new_id)?;
				let mode_packet = mode_command_packet(&registry.display_geometry, self.new_id)?;
				let mut bytes = Vec::new();
				geometry_packet.write_to_with_endian(&mut bytes, Endian::Little)?;
				mode_packet.write_to_with_endian(&mut bytes, Endian::Little)?;
				WaylandPacket::new(self.new_id, 2, vec![]).write_to_with_endian(&mut bytes, Endian::Little)?;
				connection.as_ref().write_all(&bytes)?;
				Ok(Some(ClientEffect::Register(self.new_id, SubsystemType::Output(Output))))
			}
			_ => Ok(None), // unrecognised interface, ignore for now
		}
	}
}
