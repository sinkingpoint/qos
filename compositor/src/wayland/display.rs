use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;

use crate::{
	wayland::{
		compositor::Compositor,
		output::{DisplayGeometry, Output},
		registry::Registry,
		seat::Seat,
		shm::SharedMemory,
		types::{
			ClientEffect, Command, SubSystem, SubsystemType, WaylandEncodedString, WaylandError, WaylandPacket,
			WaylandResult,
		},
		xdg_wm_base::XdgWmBase,
	},
	wayland_interface,
};
pub struct Display {
	globals: Vec<SubsystemType>,
	display_geometry: DisplayGeometry,
}

impl Display {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self {
			globals: vec![
				SubsystemType::Compositor(Compositor),
				SubsystemType::SharedMemory(SharedMemory),
				SubsystemType::XdgWmBase(XdgWmBase),
				SubsystemType::Seat(Seat),
				SubsystemType::Output(Output),
			],
			display_geometry,
		}
	}
}

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
	fn handle(self, connection: &Arc<UnixStream>, _display: &mut Display) -> WaylandResult<Option<ClientEffect>> {
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
	fn handle(self, connection: &Arc<UnixStream>, display: &mut Display) -> WaylandResult<Option<ClientEffect>> {
		for (i, global) in display.globals.iter().enumerate() {
			let name = global.name();
			let version = global.version();
			let mut payload = Vec::new();

			(i as u32).write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
			WaylandEncodedString(name.to_string()).write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
			version.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;

			let packet = WaylandPacket::new(self.registry_id, 0, payload);
			let mut buf = Vec::new();
			packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
			connection.as_ref().write_all(&buf).map_err(WaylandError::IOError)?;
		}

		Ok(Some(ClientEffect::Register(
			self.registry_id,
			SubsystemType::Registry(Registry::new(display.display_geometry.clone())),
		)))
	}
}
