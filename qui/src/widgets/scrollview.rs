use std::cmp;

use crate::{AppEvent, Widget};

pub enum ScrollBarEvent {
	ScrolledTo(f32), // Scroll position as a fraction between 0.0 and 1.0
}

pub struct Scrollbar {
	pub thumb_height: i32,
	pub height: i32,
	pub scroll_position: f32, // Fraction between 0.0 and 1.
	pub start_drag_y: Option<i32>,
}

impl Scrollbar {
	pub fn new() -> Self {
		Self {
			thumb_height: 0,
			height: 0,
			scroll_position: 0.,
			start_drag_y: None,
		}
	}

	pub fn recalculate_thumb_size(&mut self, container_height: u32) {
		// Container_height is the large one, self.height is the visible height
		self.thumb_height = ((self.height as f64 / container_height as f64) * self.height as f64) as i32;
	}
}

impl Widget for Scrollbar {
	type Event = ScrollBarEvent;

	fn handle_event(&mut self, event: &crate::AppEvent) -> Vec<Self::Event> {
		let target_bounds = (
			0,
			((self.height - self.thumb_height) as f32 * self.scroll_position) as i32,
			20,
			self.thumb_height,
		);
		match event {
			AppEvent::Resize { height, .. } => {
				self.height = *height;
			}
			AppEvent::PointerButton {
				x, y, button, pressed, ..
			} if *button == 0
				&& *pressed && *x >= target_bounds.0
				&& *x <= target_bounds.0 + target_bounds.2
				&& *y >= target_bounds.1
				&& *y <= target_bounds.1 + target_bounds.3 =>
			{
				self.start_drag_y = Some(*y);
				return vec![ScrollBarEvent::ScrolledTo(self.scroll_position)];
			}
			AppEvent::PointerMotion { x, y } if let Some(start_y) = self.start_drag_y => {
				let delta_y = *y - start_y;
				self.start_drag_y = Some(*y);
				let new_scroll_position =
					(self.scroll_position + delta_y as f32 / (self.height - self.thumb_height) as f32).clamp(0.0, 1.0);
				if (new_scroll_position - self.scroll_position).abs() > f32::EPSILON {
					self.scroll_position = new_scroll_position;
					return vec![ScrollBarEvent::ScrolledTo(self.scroll_position)];
				}
			}
			_ => {}
		}
		Vec::new()
	}

	fn render(&mut self, canvas: &mut crate::canvas::Canvas) {
		// Render the scrollbar background
		canvas.fill_rect(0, 0, 20, self.height, 0xFFCCCCCC);
		let thumb_y = ((self.height - self.thumb_height) as f32 * self.scroll_position) as i32;
		if self.thumb_height > 0 && self.thumb_height < self.height {
			canvas.fill_rect(0, thumb_y, 20, self.thumb_height, 0xFF888888);
		}
	}

	fn size_hint(&self) -> (i32, i32) {
		(20, self.height)
	}
}

pub trait ScrollTarget {
	fn total_size(&self) -> (i32, i32);
}

pub struct ScrollView<W: Widget + ScrollTarget + 'static> {
	widget: Box<W>,
	scrollbar: Scrollbar,
	width: i32,
	height: i32,
}

impl<W: Widget + ScrollTarget + 'static> ScrollView<W> {
	pub fn new(w: W) -> Self {
		Self {
			widget: Box::new(w),
			scrollbar: Scrollbar::new(),
			width: 0,
			height: 0,
		}
	}
}

impl<W: Widget + ScrollTarget + 'static> Widget for ScrollView<W> {
	type Event = W::Event;
	fn handle_event(&mut self, event: &AppEvent) -> Vec<Self::Event> {
		if let AppEvent::Resize { width, height } = event {
			self.width = *width;
			self.height = *height;
		}

		self.scrollbar.handle_event(event);
		self.widget.handle_event(event)
	}

	fn render(&mut self, canvas: &mut crate::Canvas) {
		let (_, total_height) = self.widget.total_size();
		self.scrollbar.recalculate_thumb_size(total_height as u32);
		self.widget.render(&mut canvas.sub(0, 0, self.width - 20, self.height));

		self.scrollbar
			.render(&mut canvas.sub(self.width - 20, 0, 20, self.height));
	}

	fn size_hint(&self) -> (i32, i32) {
		let container_size = self.widget.size_hint();
		let scroll_bar_size = self.scrollbar.size_hint();

		(
			container_size.0 + scroll_bar_size.0,
			cmp::max(container_size.1, scroll_bar_size.1),
		)
	}
}
