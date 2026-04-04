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

use crate::{
	VideoBuffer,
	cursor::CursorEvent,
	events::wayland::WaylandEvent,
	wayland::{
		pointer::{ButtonCode, ButtonEvent, ButtonState},
		types::Client,
	},
};

pub struct WaylandCompositor {
	pub clients: HashMap<u32, types::Client>,
	pub display_geometry: DisplayGeometry,
	pub hovered_window: Option<(u32, u32)>,
	pub serial: u32,
}

impl WaylandCompositor {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self {
			clients: HashMap::new(),
			display_geometry,
			hovered_window: None,
			serial: 1,
		}
	}

	pub fn repaint(&mut self, framebuffer: &mut VideoBuffer) {
		for client in self.clients.values_mut() {
			client.repaint(framebuffer);
		}
	}

	pub fn handle_cursor_event(&mut self, event: CursorEvent) {
		match event {
			CursorEvent::Move(x, y) => {
				let dragging_window = self
					.clients
					.iter()
					.find(|(_, client)| client.dragging.is_some())
					.map(|(client_id, _)| *client_id);

				if let Some(dragging_window_id) = dragging_window {
					if let Some(client) = self.clients.get_mut(&dragging_window_id) {
						client.handle_drag(x, y).unwrap();
					}
					return;
				}

				let hovered_window = self
					.clients
					.iter()
					.find_map(|(client_id, client)| client.surface_at(x, y).map(|surface_id| (*client_id, surface_id)));

				if hovered_window != self.hovered_window {
					if let Some((prev_client_id, prev_surface_id)) = self.hovered_window
						&& let Some(prev_client) = self.clients.get_mut(&prev_client_id)
					{
						prev_client.send_leave_event(self.serial, prev_surface_id).unwrap();
						self.serial += 1;
					}

					if let Some((new_client_id, new_surface_id)) = hovered_window
						&& let Some(new_client) = self.clients.get_mut(&new_client_id)
					{
						new_client.send_enter_event(self.serial, new_surface_id, x, y).unwrap();
						self.serial += 1;
					}

					self.hovered_window = hovered_window;
				}

				if let Some((client_id, surface_id)) = self.hovered_window
					&& let Some(client) = self.clients.get_mut(&client_id)
				{
					client.send_move_event(surface_id, x, y).unwrap();
				}
			}
			CursorEvent::ButtonDown(button) => {
				if let Some((client_id, _)) = self.hovered_window
					&& let Some(client) = self.clients.get_mut(&client_id)
				{
					let button = ButtonCode::try_from(button).unwrap_or(ButtonCode::Left); // TODO: handle this properly instead of just defaulting to ButtonLeft
					let button_event = ButtonEvent::new(self.serial, button, ButtonState::Pressed);
					client.send_button_event(button_event).unwrap();
				}
			}
			CursorEvent::ButtonUp(button) => {
				let dragging_window = self
					.clients
					.iter()
					.find(|(_, client)| client.dragging.is_some())
					.map(|(client_id, _)| *client_id);

				if let Some(dragging_window_id) = dragging_window
					&& let Some(client) = self.clients.get_mut(&dragging_window_id)
				{
					client.end_drag();
				}

				if let Some((client_id, _)) = self.hovered_window
					&& let Some(client) = self.clients.get_mut(&client_id)
				{
					let button = ButtonCode::try_from(button).unwrap_or(ButtonCode::Left); // TODO: handle this properly instead of just defaulting to ButtonLeft
					let button_event = ButtonEvent::new(self.serial, button, ButtonState::Released);
					client.send_button_event(button_event).unwrap();
				}
			}
		}
	}

	pub fn handle_event(&mut self, event: WaylandEvent) {
		let client = self
			.clients
			.entry(event.client_id)
			.or_insert_with(|| Client::new(event.client.clone(), self.display_geometry.clone()));
		if let Err(e) = client.handle_event(event.packet, event.fds) {
			match e {
				types::WaylandError::IOError(e) => eprintln!("Wayland IO error: {}", e),
				types::WaylandError::NixError(e) => eprintln!("Wayland Nix error: {}", e),
				types::WaylandError::UnrecognisedObject => eprintln!("Wayland: unrecognised object"),
			}
		}
	}
}
