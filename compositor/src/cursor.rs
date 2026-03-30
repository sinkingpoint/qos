use std::os::fd::AsFd;

use crate::{
	drm::move_cursor,
	events::input::{Event, KeyCode, KeyState, MouseCode},
};

pub struct Cursor {
	// The current position of the cursor
	x: i32,
	y: i32,

	// The maximum allowed position of the cursor, based on the screen resolution
	max_x: i32,
	max_y: i32,

	// A flag to indicate if we were just touched, which helps us ignore the first absolute event that follows a touch event.
	was_just_touched: (bool, bool),

	// The last absolute position received from the input events, used to calculate movement deltas for absolute events.
	last_abs_x: Option<i32>,
	last_abs_y: Option<i32>,
}

impl Cursor {
	pub fn new(max_x: i32, max_y: i32) -> Self {
		Self {
			x: max_x / 2, // Start in the middle of the screen
			y: max_y / 2,
			max_x,
			max_y,
			was_just_touched: (false, false),
			last_abs_x: None,
			last_abs_y: None,
		}
	}

	// Update the cursor position in the kernel using the DRM API.
	// This should be called after handling input events to reflect
	// the new cursor position on the screen.
	pub fn update_kernel(&self, fd: impl AsFd, crtc_id: u32) {
		move_cursor(fd, crtc_id, self.x, self.y).expect("Failed to move cursor");
	}

	// Handle an input event to update the cursor position.
	pub fn handle_input_event(&mut self, event: &Event) {
		match event {
			Event::Relative(code, value) => match code {
				// For relative events, we simply add the value to the current position and clamp it within the screen bounds.
				MouseCode::X => self.x = (self.x + *value).clamp(0, self.max_x),
				MouseCode::Y => self.y = (self.y + *value).clamp(0, self.max_y),
			},
			Event::Absolute(code, value) => match code {
				// For absolute events, we need to calculate the movement delta from the last
				// absolute position, and then apply that delta to the current position. This
				// is because absolute events give us the absolute position of the input
				// (like a touch), but we want to move the cursor relative to its current
				// position based on how much the input has moved since the last event.
				MouseCode::X => {
					// If we were just touched, ignore this event.
					if self.was_just_touched.0 {
						self.was_just_touched.0 = false;
						self.last_abs_x = Some(*value);
						return;
					}

					self.x = self
						.last_abs_x
						.map_or(self.x, |last| (self.x + (*value - last)).clamp(0, self.max_x));
					self.last_abs_x = Some(*value);
				}
				MouseCode::Y => {
					// If we were just touched, ignore this event.
					if self.was_just_touched.1 {
						self.was_just_touched.1 = false;
						self.last_abs_y = Some(*value);
						return;
					}

					self.y = self
						.last_abs_y
						.map_or(self.y, |last| (self.y + (*value - last)).clamp(0, self.max_y));
					self.last_abs_y = Some(*value);
				}
			},
			Event::Key(KeyCode::BtnTouch, KeyState::Pressed) => {
				// Store that we were just touched, so we should ignore the first absolute event that follows this,
				// which is an ABS call with the position of the touch, which we don't want to use as a movement event,
				// since it can be very far from the current cursor position.
				self.was_just_touched = (true, true);
			}
			_ => {}
		}
	}
}
