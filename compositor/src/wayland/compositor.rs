use std::{os::unix::net::UnixStream, sync::Arc};

use wayland::compositor::CreateSurfaceRequest;

use crate::wayland::{
	surface::Surface,
	types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult},
};

pub struct Compositor;

impl SubSystem for Compositor {
	type Request = CompositorRequest;
	const NAME: &'static str = "wl_compositor";
	const VERSION: u32 = 1;
}

wayland_interface!(Compositor, CompositorRequest {
  CreateSurfaceRequest::OPCODE => CreateSurface(CreateSurfaceRequest),
});

impl Command<Compositor> for CreateSurfaceRequest {
	fn handle(
		self,
		_connection: &Arc<UnixStream>,
		_compositor: &mut Compositor,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::Register(
			self.new_id,
			SubsystemType::Surface(Surface::new()),
		)))
	}
}
