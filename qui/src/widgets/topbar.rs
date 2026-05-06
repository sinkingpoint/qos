use crate::Widget;

#[derive(Debug)]
pub enum TopBarEvent {
	DragStarted,
}

pub struct TopBar {
	pub label: String,
	pub width: i32,
}

impl TopBar {
	pub fn new(label: String) -> Self {
		Self { label, width: 100 }
	}
}

impl Widget for TopBar {
	type Event = TopBarEvent;

	fn handle_event(&mut self, event: &crate::AppEvent) -> Option<Self::Event> {
		if let crate::AppEvent::PointerButton { button, pressed, .. } = event
			&& *button == 0x110
			&& *pressed
		{
			return Some(TopBarEvent::DragStarted);
		}

		if let crate::AppEvent::Resize { width, .. } = event {
			self.width = *width;
		}

		None
	}

	fn render(&mut self, canvas: &mut crate::Canvas) {
		canvas.fill_rect(0, 0, self.width, 30, 0xFF202020);
	}

	fn size_hint(&self) -> (i32, i32) {
		(self.width, 30)
	}
}
