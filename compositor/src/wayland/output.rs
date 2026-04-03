use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;

use crate::wayland::{
	WaylandPacket,
	types::{ClientEffect, Command, SubSystem, WaylandEncodedString, WaylandResult},
};

pub struct Output;
impl SubSystem for Output {
	type Request = OutputCommand;
	const NAME: &'static str = "wl_output";
	const VERSION: u32 = 3;
}

wayland_interface!(Output, OutputCommand {
  0 => Release(ReleaseCommand),
});

#[derive(Debug, ByteStruct)]
pub struct ReleaseCommand;

impl Command<Output> for ReleaseCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, _output: &mut Output) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

// The display geometry that the compositor is initialised with.
#[derive(Debug, Clone)]
pub struct DisplayGeometry {
	pub x: i32,
	pub y: i32,
	pub width: i32,
	pub height: i32,
	pub refresh_rate: i32,
	pub physical_width: i32,
	pub physical_height: i32,
	pub subpixel: u32,
	pub make: String,
	pub model: String,
	pub transform: u32,
}

impl DisplayGeometry {
	pub fn new(width: u16, height: u16, refresh_hz: u32, physical_width: u32, physical_height: u32) -> Self {
		Self {
			x: 0,
			y: 0,
			width: width as i32,
			height: height as i32,
			refresh_rate: (refresh_hz * 1000) as i32,
			physical_width: physical_width as i32,
			physical_height: physical_height as i32,
			subpixel: 0,
			make: "Unknown".to_string(),
			model: "Unknown".to_string(),
			transform: 0,
		}
	}
}

#[derive(Debug, ByteStruct)]
struct GeometryCommand {
	x: i32,
	y: i32,
	physical_width: i32,
	physical_height: i32,
	subpixel: u32,
	make: WaylandEncodedString,
	model: WaylandEncodedString,
	transform: u32,
}

impl GeometryCommand {
	pub fn from_display_geometry(display_geometry: &DisplayGeometry) -> Self {
		Self {
			x: display_geometry.x,
			y: display_geometry.y,
			physical_width: display_geometry.physical_width,
			physical_height: display_geometry.physical_height,
			subpixel: display_geometry.subpixel,
			make: WaylandEncodedString(display_geometry.make.clone()),
			model: WaylandEncodedString(display_geometry.model.clone()),
			transform: display_geometry.transform,
		}
	}
}

pub fn geometry_command_packet(display_geometry: &DisplayGeometry, output_id: u32) -> WaylandResult<WaylandPacket> {
	let geometry_command = GeometryCommand::from_display_geometry(display_geometry);
	let mut payload = Vec::new();
	geometry_command.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
	Ok(WaylandPacket::new(output_id, 0, payload))
}

#[derive(Debug, ByteStruct)]
struct ModeCommand {
	flags: u32,
	width: i32,
	height: i32,
	refresh_rate: i32,
}

impl ModeCommand {
	pub fn from_display_geometry(display_geometry: &DisplayGeometry) -> Self {
		Self {
			flags: 0x3,
			width: display_geometry.width,
			height: display_geometry.height,
			refresh_rate: display_geometry.refresh_rate,
		}
	}
}

pub fn mode_command_packet(display_geometry: &DisplayGeometry, output_id: u32) -> WaylandResult<WaylandPacket> {
	let mode_command = ModeCommand::from_display_geometry(display_geometry);
	let mut payload = Vec::new();
	mode_command.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
	Ok(WaylandPacket::new(output_id, 1, payload))
}
