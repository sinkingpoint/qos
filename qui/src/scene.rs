use crate::{AppEvent, canvas::Canvas, widgets::Widget};

pub struct WidgetContainer {
	pub widget: Box<dyn Widget>,
	boundary: Rect,
}

pub struct Scene {
	pub widgets: Vec<WidgetContainer>,
	width: i32,
	height: i32,
}

impl Scene {
	pub fn new(width: i32, height: i32) -> Self {
		Self {
			widgets: Vec::new(),
			width,
			height,
		}
	}

	pub fn add_widget<W: Widget + 'static>(&mut self, widget: W, x: i32, y: i32) {
		let (width, height) = widget.size_hint();
		self.widgets.push(WidgetContainer {
			widget: Box::new(widget),
			boundary: Rect { x, y, width, height },
		});
	}
}

impl Widget for Scene {
	fn handle_event(&mut self, event: &AppEvent) -> bool {
		for widget in &mut self.widgets {
			// For mouse events, we need to adjust the coordinates relative to the widget's position.
			if let AppEvent::PointerMotion { x, y } = event {
				let local_x = *x - widget.boundary.x;
				let local_y = *y - widget.boundary.y;
				if widget
					.widget
					.handle_event(&AppEvent::PointerMotion { x: local_x, y: local_y })
				{
					return true;
				}
			} else if let AppEvent::PointerButton { x, y, button, pressed } = event {
				if !widget.boundary.contains(*x, *y) {
					continue;
				}
				let local_x = *x - widget.boundary.x;
				let local_y = *y - widget.boundary.y;
				if widget.widget.handle_event(&AppEvent::PointerButton {
					x: local_x,
					y: local_y,
					button: *button,
					pressed: *pressed,
				}) {
					return true;
				}
			} else {
				if widget.widget.handle_event(event) {
					return true;
				}
			}
		}
		false
	}

	fn render(&mut self, canvas: &mut Canvas) {
		for widget in &mut self.widgets {
			let mut canvas = canvas.sub(
				widget.boundary.x,
				widget.boundary.y,
				widget.boundary.width,
				widget.boundary.height,
			);
			widget.widget.render(&mut canvas);
		}
	}

	fn size_hint(&self) -> (i32, i32) {
		(self.width, self.height)
	}
}

struct Rect {
	x: i32,
	y: i32,
	width: i32,
	height: i32,
}

impl Rect {
	fn contains(&self, x: i32, y: i32) -> bool {
		x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
	}
}
