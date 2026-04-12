mod display;
#[macro_use]
mod macros;
mod buffer;
mod compositor;
mod keyboard;
mod layout;
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
mod zwlr_layer_shell_v1;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub use output::DisplayGeometry;
pub use types::WaylandPacket;

use crate::{
	VideoBuffer,
	cursor::CursorEvent,
	events::wayland::WaylandEvent,
	keyboard::KeyEvent,
	wayland::{
		pointer::{ButtonCode, ButtonEvent, ButtonState},
		types::Client,
	},
};

pub struct WaylandCompositor {
	pub layout: Rc<RefCell<Box<dyn layout::Layout>>>,
	pub clients: HashMap<u32, types::Client>,
	pub display_geometry: DisplayGeometry,
	pub hovered_window: Option<(u32, u32)>,
	pub active_window: Option<(u32, u32)>,
	pub keyboard: crate::keyboard::Keyboard,
	pub serial: u32,
}

impl WaylandCompositor {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self {
			layout: Rc::new(RefCell::new(Box::new(layout::FloatingLayout::new(
				display_geometry.clone(),
			)))),
			clients: HashMap::new(),
			display_geometry,
			hovered_window: None,
			active_window: None,
			keyboard: crate::keyboard::Keyboard::new(),
			serial: 1,
		}
	}

	pub fn repaint(&mut self, framebuffer: &mut VideoBuffer) {
		for client in self.clients.values_mut() {
			client.repaint_background_bottom(framebuffer);
		}
		for client in self.clients.values_mut() {
			client.repaint_xdg(framebuffer);
		}
		for client in self.clients.values_mut() {
			client.repaint_top_overlay(framebuffer);
		}
	}

	pub fn handle_key_event(&mut self, event: KeyEvent) {
		self.keyboard.handle_input(event);
		if let Some((client_id, _)) = self.active_window
			&& let Some(client) = self.clients.get_mut(&client_id)
		{
			client.handle_key_event(self.serial, event, &self.keyboard).ok();
			self.serial += 1;
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
						prev_client.send_leave_event(self.serial, prev_surface_id).ok();
						self.serial += 1;
					}

					if let Some((new_client_id, new_surface_id)) = hovered_window
						&& let Some(new_client) = self.clients.get_mut(&new_client_id)
					{
						new_client.send_enter_event(self.serial, new_surface_id, x, y).ok();
						self.serial += 1;
					}

					self.hovered_window = hovered_window;
				}

				if let Some((client_id, surface_id)) = self.hovered_window
					&& let Some(client) = self.clients.get_mut(&client_id)
				{
					client.send_move_event(surface_id, x, y).ok();
				}
			}
			CursorEvent::ButtonDown(button) => {
				if let Some((client_id, _)) = self.hovered_window
					&& let Some(client) = self.clients.get_mut(&client_id)
				{
					let button = ButtonCode::try_from(button).unwrap_or(ButtonCode::Left);
					let time = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC).expect("clock");
					let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
					let button_event = ButtonEvent {
						serial: self.serial,
						time: ms as u32,
						button: u32::from(button),
						state: u32::from(ButtonState::Pressed),
					};
					client.send_button_event(button_event).ok();

					// Move the focus to the clicked window
					if self.active_window != self.hovered_window {
						if let Some((prev_client_id, prev_surface_id)) = self.active_window
							&& let Some(prev_client) = self.clients.get_mut(&prev_client_id)
						{
							prev_client.handle_focus_leave(self.serial, prev_surface_id).ok();
							self.serial += 1;
						}

						if let Some((new_client_id, new_surface_id)) = self.hovered_window
							&& let Some(new_client) = self.clients.get_mut(&new_client_id)
						{
							new_client
								.handle_focus_enter(self.serial, new_surface_id, &self.keyboard)
								.ok();
							self.serial += 1;
						}

						self.active_window = self.hovered_window;
					}
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
					let button = ButtonCode::try_from(button).unwrap_or(ButtonCode::Left);
					let time = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC).expect("clock");
					let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
					let button_event = ButtonEvent {
						serial: self.serial,
						time: ms as u32,
						button: u32::from(button),
						state: u32::from(ButtonState::Released),
					};
					client.send_button_event(button_event).ok();
				}
			}
		}
	}

	pub fn handle_event(&mut self, event: WaylandEvent) {
		let client = self
			.clients
			.entry(event.client_id)
			.or_insert_with(|| Client::new(event.client.clone(), self.display_geometry.clone(), self.layout.clone()));
		if let Err(e) = client.handle_event(event.packet, event.fds) {
			match e {
				types::WaylandError::IOError(e) => eprintln!("Wayland IO error: {}", e),
				types::WaylandError::NixError(e) => eprintln!("Wayland Nix error: {}", e),
				types::WaylandError::UnrecognisedObject => eprintln!("Wayland: unrecognised object"),
			}
		}
	}
}
