use wayland::xdg_wm_base::{CreatePositionerRequest, DestroyRequest, GetXdgSurfaceRequest, PongRequest};

use crate::wayland::{
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult},
	xdg_surface::XDGSurface,
};

pub struct XdgWmBase;

impl SubSystem for XdgWmBase {
	type Request = XdgWmBaseRequest;
	const NAME: &'static str = "xdg_wm_base";
	const VERSION: u32 = 1;
}

wayland_interface!(XdgWmBase, XdgWmBaseRequest {
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
  CreatePositionerRequest::OPCODE => CreatePositioner(CreatePositionerRequest),
  GetXdgSurfaceRequest::OPCODE => GetXdgSurface(GetXdgSurfaceRequest),
  PongRequest::OPCODE => Pong(PongRequest),
});

impl Command<XdgWmBase> for DestroyRequest {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

impl Command<XdgWmBase> for CreatePositionerRequest {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(None)
	}
}

impl Command<XdgWmBase> for GetXdgSurfaceRequest {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::XdgSurface(XDGSurface::new(self.new_id, self.surface_id)),
		)))
	}
}

impl Command<XdgWmBase> for PongRequest {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(None)
	}
}
