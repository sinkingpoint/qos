mod display;
#[macro_use]
mod macros;
mod buffer;
mod compositor;
mod keyboard;
mod output;
mod pointer;
mod registry;
mod seat;
mod shm;
mod surface;
mod types;
mod xdg_surface;
mod xdg_toplevel;
mod xdg_wm_base;

use std::collections::HashMap;

pub use output::DisplayGeometry;
pub use types::WaylandPacket;

use crate::{VideoBuffer, events::wayland::WaylandEvent, wayland::types::Client};

pub struct WaylandCompositor {
	pub clients: HashMap<u32, types::Client>,
	pub display_geometry: DisplayGeometry,
}

impl WaylandCompositor {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self {
			clients: HashMap::new(),
			display_geometry,
		}
	}

	pub fn repaint(&mut self, framebuffer: &mut VideoBuffer) {
		for client in self.clients.values_mut() {
			client.repaint(framebuffer);
		}
	}

	pub fn handle_event(&mut self, event: WaylandEvent) {
		let client = self
			.clients
			.entry(event.client_id)
			.or_insert_with(|| Client::new(event.client.clone(), self.display_geometry.clone()));
		if let Err(e) = client.handle_command(event.packet) {
			match e {
				types::WaylandError::IOError(e) => eprintln!("Wayland IO error: {}", e),
				types::WaylandError::NixError(e) => eprintln!("Wayland Nix error: {}", e),
				types::WaylandError::UnrecognisedObject => eprintln!("Wayland: unrecognised object"),
			}
		}
	}
}
