use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::xdg_toplevel::{DestroyRequest, MoveRequest, SetTitleRequest};

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct XdgTopLevel {
	pub xdg_surface: u32,
	pub x: i32,
	pub y: i32,
	title: Option<String>,
}

impl XdgTopLevel {
	pub fn new(xdg_surface: u32) -> Self {
		Self {
			xdg_surface,
			x: 0,
			y: 0,
			title: None,
		}
	}
}

impl SubSystem for XdgTopLevel {
	type Request = XdgTopLevelRequest;
	const NAME: &'static str = "xdg_toplevel";
}

wayland_interface!(XdgTopLevel, XdgTopLevelRequest {
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
	SetTitleRequest::OPCODE => SetTitle(SetTitleRequest),
	MoveRequest::OPCODE => Move(MoveRequest),
});

impl Command<XdgTopLevel> for DestroyRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

impl Command<XdgTopLevel> for SetTitleRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		xdg_toplevel.title = Some(self.title.0);
		Ok(None)
	}
}

impl Command<XdgTopLevel> for MoveRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::StartDrag))
	}
}
