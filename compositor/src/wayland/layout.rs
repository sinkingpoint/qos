use std::collections::HashMap;

use wayland::zwlr_layer_shell_v1::Anchor;

use crate::wayland::DisplayGeometry;

#[derive(Debug, Clone, Copy)]
pub struct Rectangle {
	pub x: i32,
	pub y: i32,
	pub width: i32,
	pub height: i32,
}

impl Rectangle {
	pub fn intersect(&self, other: &Rectangle) -> Option<Rectangle> {
		let x1 = self.x.max(other.x);
		let y1 = self.y.max(other.y);
		let x2 = (self.x + self.width).min(other.x + other.width);
		let y2 = (self.y + self.height).min(other.y + other.height);

		if x1 < x2 && y1 < y2 {
			Some(Rectangle {
				x: x1,
				y: y1,
				width: (x2 - x1),
				height: (y2 - y1),
			})
		} else {
			None
		}
	}
}

pub trait Layout {
	// Add a new window to this layout, returning any updated size / positions.
	fn new_window(&mut self, id: u32, request: Rectangle) -> Vec<(u32, Rectangle)>;
	// Remove a window from this layout, returning any updated size / positions.
	fn remove_window(&mut self, id: u32) -> Vec<(u32, Rectangle)>;

	// Add a new exclusive zone to this layout, returning any updated size / positions.
	fn new_exclusive_zone(&mut self, id: u32, anchor: Anchor, size: i32) -> Vec<(u32, Rectangle)>;

	// Remove an exclusive zone from this layout, returning any updated size / positions.
	fn remove_exclusive_zone(&mut self, id: u32) -> Vec<(u32, Rectangle)>;
}

// Floating layout, where windows are placed according to their requests and can overlap.
// As long as they don't touch the exclusive zones, the compositor doesn't care.
pub struct FloatingLayout {
	display_geometry: DisplayGeometry,
	windows: HashMap<u32, Rectangle>,
	exclusive_zones: HashMap<u32, Rectangle>,
}

impl FloatingLayout {
	pub fn new(display_geometry: DisplayGeometry) -> Self {
		Self {
			display_geometry,
			windows: HashMap::new(),
			exclusive_zones: HashMap::new(),
		}
	}
}

impl Layout for FloatingLayout {
	fn new_window(&mut self, id: u32, mut request: Rectangle) -> Vec<(u32, Rectangle)> {
		for exclusive_zone in self.exclusive_zones.values() {
			if let Some(intersection) = request.intersect(exclusive_zone) {
				let x_shift = if intersection.x + intersection.width / 2 < exclusive_zone.x + exclusive_zone.width / 2 {
					-intersection.width
				} else {
					intersection.width
				};

				let y_shift = if intersection.y + intersection.height / 2 < exclusive_zone.y + exclusive_zone.height / 2
				{
					-intersection.height
				} else {
					intersection.height
				};

				if intersection.width < intersection.height {
					request.x += x_shift;
				} else {
					request.y += y_shift;
				}
			}
		}

		self.windows.insert(id, request);
		vec![(id, request)]
	}

	fn remove_window(&mut self, id: u32) -> Vec<(u32, Rectangle)> {
		self.windows.remove(&id);
		vec![]
	}

	fn new_exclusive_zone(&mut self, id: u32, anchor: Anchor, size: i32) -> Vec<(u32, Rectangle)> {
		let request = if anchor.contains(Anchor::Top) {
			Rectangle {
				x: 0,
				y: 0,
				width: self.display_geometry.width,
				height: size,
			}
		} else if anchor.contains(Anchor::Bottom) {
			Rectangle {
				x: 0,
				y: self.display_geometry.height - size,
				width: self.display_geometry.width,
				height: size,
			}
		} else if anchor.contains(Anchor::Left) {
			Rectangle {
				x: 0,
				y: 0,
				width: size,
				height: self.display_geometry.height,
			}
		} else if anchor.contains(Anchor::Right) {
			Rectangle {
				x: self.display_geometry.width - size,
				y: 0,
				width: size,
				height: self.display_geometry.height,
			}
		} else {
			return vec![];
		};

		self.exclusive_zones.insert(id, request);
		let mut shifts = Vec::new();
		// Iterate the existing windows and shift any that intersect with the new exclusive zone.
		for (window_id, window_rect) in self.windows.iter_mut() {
			if let Some(intersection) = window_rect.intersect(&request) {
				let x_shift = if intersection.x + intersection.width / 2 < request.x + request.width / 2 {
					-intersection.width
				} else {
					intersection.width
				};

				let y_shift = if intersection.y + intersection.height / 2 < request.y + request.height / 2 {
					-intersection.height
				} else {
					intersection.height
				};

				if intersection.width < intersection.height {
					window_rect.x += x_shift;
				} else {
					window_rect.y += y_shift;
				}

				shifts.push((*window_id, *window_rect));
			}
		}

		shifts
	}

	fn remove_exclusive_zone(&mut self, id: u32) -> Vec<(u32, Rectangle)> {
		self.exclusive_zones.remove(&id);
		vec![]
	}
}
