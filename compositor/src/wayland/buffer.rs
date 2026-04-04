use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Buffer {
	pub pool_id: u32,
	pub offset: i32,
	pub width: i32,
	pub height: i32,
	pub stride: i32,
	pub _format: u32,
}

impl Buffer {
	pub fn new(pool_id: u32, offset: i32, width: i32, height: i32, stride: i32, format: u32) -> Self {
		Self {
			pool_id,
			offset,
			width,
			height,
			stride,
			_format: format,
		}
	}
}

impl SubSystem for Buffer {
	type Request = BufferRequest;
	const NAME: &'static str = "wl_buffer";
}

wayland_interface!(Buffer, BufferRequest {
  0 => Destroy(DestroyCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Buffer> for DestroyCommand {
	fn handle(
		self,
		_connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
		_buffer: &mut Buffer,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}
