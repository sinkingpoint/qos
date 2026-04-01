use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, Command, SubSystem, WaylandResult};

pub struct Compositor;

impl SubSystem for Compositor {
	type Request = CompositorRequest;
	const NAME: &'static str = "wl_compositor";
	const VERSION: u32 = 1;
}

wayland_interface!(Compositor, CompositorRequest {
  0 => CreateSurface(CreateSurfaceCommand),
});

#[derive(Debug, ByteStruct)]
pub struct CreateSurfaceCommand {
	pub new_id: u32,
}

impl Command<Compositor> for CreateSurfaceCommand {
	fn handle(
		&self,
		_connection: &Arc<UnixStream>,
		_compositor: &mut Compositor,
	) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::Register(
			self.new_id,
			crate::wayland::types::SubsystemType::Surface(crate::wayland::surface::Surface::new()),
		)))
	}
}
