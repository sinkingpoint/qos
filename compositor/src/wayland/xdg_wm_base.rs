use bytestruct_derive::ByteStruct;

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
  0 => Destroy(DestroyCommand),
  1 => CreatePositioner(CreatePositionerCommand),
  2 => GetXdgSurface(GetXdgSurfaceCommand),
  3 => Pong(PongCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<XdgWmBase> for DestroyCommand {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug, ByteStruct)]
pub struct CreatePositionerCommand {
	pub positioner_id: u32,
}

impl Command<XdgWmBase> for CreatePositionerCommand {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(None)
	}
}

#[derive(Debug, ByteStruct)]
pub struct GetXdgSurfaceCommand {
	pub new_id: u32,
	pub surface_id: u32,
}

impl Command<XdgWmBase> for GetXdgSurfaceCommand {
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

#[derive(Debug, ByteStruct)]
pub struct PongCommand {
	pub callback_id: u32,
}

impl Command<XdgWmBase> for PongCommand {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_xdg_wm_base: &mut XdgWmBase,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(None)
	}
}
