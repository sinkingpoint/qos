use crate::{AppEvent, canvas::Canvas, widgets::Widget};

pub struct Button {
	pub width: i32,
	pub height: i32,
	pub label: String,

	hovered: bool,
	pressed: bool,
	action: Option<Box<dyn Fn()>>,
}

impl Button {
	pub fn new<F: Fn() + 'static>(label: String, action: Option<F>) -> Self {
		Self {
			width: 100,
			height: 30,
			label,
			hovered: false,
			pressed: false,
			action: action.map(|f| Box::new(f) as Box<dyn Fn()>),
		}
	}
}

impl Widget for Button {
	fn handle_event(&mut self, event: &AppEvent) -> bool {
		if let AppEvent::PointerMotion { x, y } = event {
			self.hovered = *x >= 0 && *x < self.width && *y >= 0 && *y < self.height;
		}

		if let AppEvent::PointerButton { button, pressed, x, y } = event
			&& *button == 0x110
		{
			self.pressed = *pressed && *x >= 0 && *x < self.width && *y >= 0 && *y < self.height;
			if self.pressed
				&& let Some(action) = &self.action
			{
				action();
			}
			return true;
		}

		false
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
