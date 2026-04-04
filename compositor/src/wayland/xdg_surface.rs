use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::types::WaylandPayload;
use wayland::xdg_surface::{
	AckConfigureRequest, ConfigureEvent as XdgSurfaceConfigureEvent, DestroyRequest, GetTopLevelSurfaceRequest,
};
use wayland::xdg_toplevel::ConfigureEvent as TopLevelConfigureEvent;

use crate::wayland::types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult};
use crate::wayland::xdg_toplevel::XdgTopLevel;

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
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
  GetTopLevelSurfaceRequest::OPCODE => GetTopLevelSurface(GetTopLevelSurfaceRequest),
  AckConfigureRequest::OPCODE => AckConfigure(AckConfigureRequest),
});

impl Command<XDGSurface> for DestroyRequest {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_surface: &mut XDGSurface,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

impl Command<XDGSurface> for GetTopLevelSurfaceRequest {
	fn handle(self, connection: &Arc<UnixStream>, xdg_surface: &mut XDGSurface) -> WaylandResult<Option<ClientEffect>> {
		let new_surface = SubsystemType::XdgTopLevel(XdgTopLevel::new(xdg_surface.id));

		TopLevelConfigureEvent {
			width: 0,
			height: 0,
			states: 0,
		}
		.write_as_packet(self.new_id, connection)?;

		xdg_surface.configured = false;
		let configure_serial = xdg_surface.next_configure_serial;
		xdg_surface.next_configure_serial += 1;
		xdg_surface.pending_configure = Some(configure_serial);
		XdgSurfaceConfigureEvent {
			serial: configure_serial,
		}
		.write_as_packet(xdg_surface.id, connection)?;

		Ok(Some(ClientEffect::Register(self.new_id, new_surface)))
	}
}

impl Command<XDGSurface> for AckConfigureRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		xdg_surface: &mut XDGSurface,
	) -> WaylandResult<Option<ClientEffect>> {
		if Some(self.serial) == xdg_surface.pending_configure {
			xdg_surface.pending_configure = None;
			xdg_surface.configured = true;
		}
		Ok(None)
	}
}
