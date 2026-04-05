use crate::{AppEvent, canvas::Canvas, widgets::Widget};

pub enum ButtonEvent {
	Clicked,
}

pub struct Button {
	pub width: i32,
	pub height: i32,
	pub label: String,
	hovered: bool,
	pressed: bool,
}

impl Button {
	pub fn new(label: String) -> Self {
		Self {
			width: 100,
			height: 30,
			label,
			hovered: false,
			pressed: false,
		}
	}
}

impl Widget for Button {
	type Event = ButtonEvent;

	fn handle_event(&mut self, event: &AppEvent) -> Option<ButtonEvent> {
		if let AppEvent::PointerMotion { x, y } = event {
			self.hovered = *x >= 0 && *x < self.width && *y >= 0 && *y < self.height;
		}
		if let AppEvent::PointerButton { button, pressed, x, y } = event
			&& *button == 0x110
		{
			self.pressed = *pressed && *x >= 0 && *x < self.width && *y >= 0 && *y < self.height;
			if self.pressed {
				return Some(ButtonEvent::Clicked);
			}
		}
		None
	}

	fn render(&mut self, canvas: &mut Canvas) {
		let color = if self.pressed {
			0xFF5555AA
		} else if self.hovered {
			0xFF5555FF
		} else {
			0xFF3333AA
		};
		canvas.fill_rect(0, 0, self.width, self.height, color);
	}

	fn size_hint(&self) -> (i32, i32) {
		(self.width, self.height)
	}
}
