use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::output::ReleaseRequest;
use wayland::types::WaylandEncodedString;

pub use wayland::output::{DoneEvent, GeometryEvent, ModeEvent};

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Output;
impl SubSystem for Output {
	type Request = OutputCommand;
	const NAME: &'static str = "wl_output";
	const VERSION: u32 = 3;
}

wayland_interface!(Output, OutputCommand {
  ReleaseRequest::OPCODE => Release(ReleaseRequest),
});

impl Command<Output> for ReleaseRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _output: &mut Output) -> WaylandResult<Option<ClientEffect>> {
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

	pub fn geometry_event(&self) -> GeometryEvent {
		GeometryEvent {
			x: self.x,
			y: self.y,
			physical_width: self.physical_width,
			physical_height: self.physical_height,
			subpixel: self.subpixel as i32,
			make: WaylandEncodedString(self.make.clone()),
			model: WaylandEncodedString(self.model.clone()),
			transform: self.transform as i32,
		}
	}

	pub fn mode_event(&self) -> ModeEvent {
		ModeEvent {
			flags: 0x3,
			width: self.width,
			height: self.height,
			refresh_rate: self.refresh_rate,
		}
	}
}
