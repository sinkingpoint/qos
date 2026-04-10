use std::{os::unix::net::UnixStream, sync::Arc};

use nix::time::{ClockId, clock_gettime};
use wayland::surface::FrameCallbackEvent;
use wayland::surface::{AttachRequest, CommitRequest, DestroyRequest, FrameRequest};
use wayland::types::WaylandPayload;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Surface {
	pub attached_buffer: Option<(u32, i32, i32)>,
	pub last_blit_rect: Option<(i32, i32, i32, i32)>,
	pub committed: bool,
	pub blitted: bool,
	pub pending_callbacks: Vec<u32>,
	pub role_id: Option<u32>, // ID of the role object (e.g., xdg_surface) associated with this surface
}

impl Surface {
	pub fn new() -> Self {
		Self {
			attached_buffer: None,
			last_blit_rect: None,
			committed: false,
			blitted: false,
			pending_callbacks: Vec::new(),
			role_id: None,
		}
	}

	pub fn mark_blitted(&mut self, connection: &Arc<UnixStream>) {
		self.blitted = true;

		for callback_id in self.pending_callbacks.drain(..) {
			let time = clock_gettime(ClockId::CLOCK_MONOTONIC).expect("Failed to get time");
			let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
			let event = FrameCallbackEvent { time_msec: ms as u32 };
			event.write_as_packet(callback_id, connection).unwrap();
		}
	}
}

impl SubSystem for Surface {
	type Request = SurfaceRequest;
	const NAME: &'static str = "wl_surface";
}

wayland_interface!(Surface, SurfaceRequest {
  DestroyRequest::OPCODE => Destroy(DestroyRequest),
	AttachRequest::OPCODE => Attach(AttachRequest),
	FrameRequest::OPCODE => Frame(FrameRequest),
	CommitRequest::OPCODE => Commit(CommitRequest),
});

impl Command<Surface> for DestroyRequest {
	fn handle(self, _connection: &Arc<UnixStream>, _surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

impl Command<Surface> for AttachRequest {
	fn handle(self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		surface.attached_buffer = Some((self.buffer_id, self.x, self.y));
		surface.committed = false;
		surface.blitted = false;
		Ok(None)
	}
}

impl Command<Surface> for CommitRequest {
	fn handle(self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		if surface.attached_buffer.is_some() {
			surface.committed = true;
			surface.blitted = false;
		}

		Ok(None)
	}
}

impl Command<Surface> for FrameRequest {
	fn handle(self, _connection: &Arc<UnixStream>, surface: &mut Surface) -> WaylandResult<Option<ClientEffect>> {
		surface.pending_callbacks.push(self.callback_id);
		Ok(None)
	}
}
