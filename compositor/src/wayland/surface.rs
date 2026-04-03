use std::{io::Write, os::unix::net::UnixStream, sync::Arc};

use bytestruct::WriteToWithEndian;
use bytestruct_derive::ByteStruct;
use nix::time::{ClockId, clock_gettime};

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Surface {
	pub attached_buffer: Option<(u32, i32, i32)>,
	pub committed: bool,
	pub blitted: bool,
	pub pending_callbacks: Vec<u32>,
}

impl Surface {
	pub fn new() -> Self {
		Self {
			attached_buffer: None,
			committed: false,
			blitted: false,
			pending_callbacks: Vec::new(),
		}
	}

	pub fn mark_blitted(&mut self, connection: &Arc<UnixStream>) {
		self.blitted = true;

		for callback_id in self.pending_callbacks.drain(..) {
			let response = FrameCallbackResponse::new();
			let mut payload = Vec::new();
			response
				.write_to_with_endian(&mut payload, bytestruct::Endian::Little)
				.unwrap();
			let packet = crate::wayland::WaylandPacket::new(callback_id, 0, payload);
			let mut packet_bytes = Vec::new();
			packet
				.write_to_with_endian(&mut packet_bytes, bytestruct::Endian::Little)
				.unwrap();
			connection.as_ref().write_all(&packet_bytes).unwrap();
		}
	}
}

impl SubSystem for Surface {
	type Request = SurfaceRequest;
	const NAME: &'static str = "wl_surface";
}

wayland_interface!(Surface, SurfaceRequest {
  0 => Destroy(DestroyCommand),
	1 => Attach(AttachCommand),
	3 => Frame(FrameCommand),
	6 => Commit(CommitCommand),
});

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Surface> for DestroyCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, _surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug, ByteStruct)]
pub struct AttachCommand {
	pub buffer_id: u32,
	pub x: i32,
	pub y: i32,
}

impl Command<Surface> for AttachCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		surface.attached_buffer = Some((self.buffer_id, self.x, self.y));
		surface.committed = false;
		surface.blitted = false;
		Ok(None)
	}
}

#[derive(Debug, ByteStruct)]
pub struct CommitCommand;

impl Command<Surface> for CommitCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		if surface.attached_buffer.is_some() {
			surface.committed = true;
			surface.blitted = false;
		}

		Ok(None)
	}
}

#[derive(Debug, ByteStruct)]
pub struct FrameCommand {
	pub callback_id: u32,
}

impl Command<Surface> for FrameCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		surface.pending_callbacks.push(self.callback_id);
		Ok(None)
	}
}
// Returned in response to a frame callback when the surface is blitted, to notify the client that it can start drawing the next frame.
#[derive(Debug, ByteStruct)]
struct FrameCallbackResponse {
	time_msec: u32,
}

impl FrameCallbackResponse {
	fn new() -> Self {
		let time = clock_gettime(ClockId::CLOCK_MONOTONIC).expect("Failed to get time");

		let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
		Self { time_msec: ms as u32 }
	}
}
