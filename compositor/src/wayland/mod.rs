mod display;
#[macro_use]
mod macros;
mod registry;
mod types;

use std::collections::HashMap;

pub use types::WaylandPacket;

use crate::{events::wayland::WaylandEvent, wayland::types::Client};

pub struct WaylandCompositor {
	pub clients: HashMap<u32, types::Client>,
}

impl WaylandCompositor {
	pub fn new() -> Self {
		Self {
			clients: HashMap::new(),
		}
	}

	pub fn handle_event(&mut self, event: WaylandEvent) {
		let client = self
			.clients
			.entry(event.client_id)
			.or_insert_with(|| Client::new(event.client.clone()));
		if let Err(e) = client.handle_command(event.packet) {
			match e {
				types::WaylandError::IOError(e) => eprintln!("Wayland IO error: {}", e),
				types::WaylandError::UnrecognisedObject => eprintln!("Wayland: unrecognised object"),
			}
		}
	}
}
