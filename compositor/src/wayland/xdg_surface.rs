use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;

use crate::wayland::{
	WaylandPacket,
	types::{Command, SubSystem, SubsystemType},
	xdg_toplevel::XdgTopLevel,
};

pub struct XDGSurface {
	pub id: u32,
	pub surface_id: u32,
	pub pending_configure: Option<u32>,
	pub configured: bool,
	pub next_configure_serial: u32,
}

impl XDGSurface {
	pub fn new(id: u32, surface_id: u32) -> Self {
		Self {
			id,
			surface_id,
			pending_configure: None,
			configured: false,
			next_configure_serial: 1,
		}
	}
}

impl SubSystem for XDGSurface {
	const NAME: &'static str = "xdg_surface";
	type Request = XDGSurfaceRequest;
}

wayland_interface!(XDGSurface, XDGSurfaceRequest {
  0 => Destroy(DestroyRequest),
  1 => GetTopLevelSurface(GetTopLevelSurfaceCommand),
  4 => AckConfigure(AckConfigureCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

impl Command<XDGSurface> for DestroyRequest {
	fn handle(
		&self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_surface: &mut XDGSurface,
	) -> crate::wayland::types::WaylandResult<Option<crate::wayland::types::ClientEffect>> {
		Ok(Some(crate::wayland::types::ClientEffect::DestroySelf))
	}
}

#[derive(Debug, ByteStruct)]
pub struct GetTopLevelSurfaceCommand {
	pub new_id: u32,
}

impl Command<XDGSurface> for GetTopLevelSurfaceCommand {
	fn handle(
		&self,
		connection: &Arc<UnixStream>,
		xdg_surface: &mut XDGSurface,
	) -> crate::wayland::types::WaylandResult<Option<crate::wayland::types::ClientEffect>> {
		let new_surface = SubsystemType::XdgTopLevel(XdgTopLevel::new());

		// xdg_toplevel.configure
		let mut configure_args = Vec::new();
		0i32.write_to_with_endian(&mut configure_args, bytestruct::Endian::Little)?; // width
		0i32.write_to_with_endian(&mut configure_args, bytestruct::Endian::Little)?; // height
		0u32.write_to_with_endian(&mut configure_args, bytestruct::Endian::Little)?; // states (none for now)

		let toplevel_configure_event = WaylandPacket::new(self.new_id, 0, configure_args); // opcode 0 is configure
		let mut configure_event_bytes = Vec::new();
		toplevel_configure_event.write_to_with_endian(&mut configure_event_bytes, bytestruct::Endian::Little)?;
		connection.as_ref().write_all(&configure_event_bytes)?;

		xdg_surface.configured = false;
		let configure_serial = xdg_surface.next_configure_serial;
		xdg_surface.next_configure_serial += 1;
		xdg_surface.pending_configure = Some(configure_serial);
		let surface_configure_event = WaylandPacket::new(xdg_surface.id, 0, configure_serial.to_le_bytes().to_vec()); // opcode 0 is configure
		let mut surface_configure_event_bytes = Vec::new();
		surface_configure_event.write_to_with_endian(&mut surface_configure_event_bytes, bytestruct::Endian::Little)?;
		connection.as_ref().write_all(&surface_configure_event_bytes)?;

		Ok(Some(crate::wayland::types::ClientEffect::Register(
			self.new_id,
			new_surface,
		)))
	}
}

#[derive(Debug, ByteStruct)]
pub struct AckConfigureCommand {
	pub serial: u32,
}

impl Command<XDGSurface> for AckConfigureCommand {
	fn handle(
		&self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		xdg_surface: &mut XDGSurface,
	) -> crate::wayland::types::WaylandResult<Option<crate::wayland::types::ClientEffect>> {
		if Some(self.serial) == xdg_surface.pending_configure {
			xdg_surface.pending_configure = None;
			xdg_surface.configured = true;
		}
		Ok(None)
	}
}
