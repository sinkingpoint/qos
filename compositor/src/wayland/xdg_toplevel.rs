use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandEncodedString, WaylandResult};

pub struct XdgTopLevel {
	pub xdg_surface: u32,
	pub x: i32,
	pub y: i32,
	pub dragging: bool,
	title: Option<String>,
}

impl XdgTopLevel {
	pub fn new(xdg_surface: u32) -> Self {
		Self {
			xdg_surface,
			x: 0,
			y: 0,
			dragging: false,
			title: None,
		}
	}
}

impl SubSystem for XdgTopLevel {
	type Request = XdgTopLevelRequest;
	const NAME: &'static str = "xdg_toplevel";
}

wayland_interface!(XdgTopLevel, XdgTopLevelRequest {
  0 => Destroy(DestroyCommand),
	2 => SetTitle(SetTitleCommand),
	6 => Move(MoveCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<XdgTopLevel> for DestroyCommand {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug, ByteStruct)]
pub struct SetTitleCommand {
	pub title: WaylandEncodedString,
}

impl Command<XdgTopLevel> for SetTitleCommand {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		xdg_toplevel.title = Some(self.title.0);
		Ok(None)
	}
}

#[derive(Debug, ByteStruct)]
pub struct MoveCommand {
	pub seat_id: u32,
	pub serial: u32,
}

impl Command<XdgTopLevel> for MoveCommand {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		xdg_toplevel: &mut XdgTopLevel,
	) -> WaylandResult<Option<ClientEffect>> {
		xdg_toplevel.dragging = true;
		Ok(None)
	}
}
