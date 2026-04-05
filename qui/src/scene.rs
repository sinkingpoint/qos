use std::{any::Any, marker::PhantomData};

use crate::{
	AppEvent,
	canvas::Canvas,
	widgets::{AnyWidget, Widget},
};

/// Opaque fired event from the scene. Use `WidgetHandle::extract` to get a typed value.
pub struct SceneEvent(usize, Box<dyn Any>);

pub struct WidgetHandle<E: 'static> {
	index: usize,
	_phantom: PhantomData<E>,
}

impl<E: 'static> WidgetHandle<E> {
	pub fn extract<'a>(&self, event: &'a SceneEvent) -> Option<&'a E> {
		if event.0 == self.index {
			event.1.downcast_ref::<E>()
		} else {
			None
		}
	}
}

struct WidgetContainer {
	widget: Box<dyn AnyWidget>,
	boundary: Rect,
}

pub struct Scene {
	widgets: Vec<WidgetContainer>,
	pending: Vec<SceneEvent>,
	width: i32,
	height: i32,
}

impl Scene {
	pub fn new(width: i32, height: i32) -> Self {
		Self {
			widgets: Vec::new(),
			pending: Vec::new(),
			width,
			height,
		}
	}

	pub fn add_widget<W: Widget + 'static>(&mut self, widget: W, x: i32, y: i32) -> WidgetHandle<W::Event> {
		let index = self.widgets.len();
		let (width, height) = widget.size_hint();
		self.widgets.push(WidgetContainer {
			widget: Box::new(widget),
			boundary: Rect { x, y, width, height },
		});
		WidgetHandle {
			index,
			_phantom: PhantomData,
		}
	}

	pub fn handle_event(&mut self, event: &AppEvent) {
		for (i, container) in self.widgets.iter_mut().enumerate() {
			let translated = translate_event(event, &container.boundary);
			if let Some(ev) = container.widget.handle_event_any(&translated) {
				self.pending.push(SceneEvent(i, ev));
			}
		}
	}

	pub fn poll(&mut self) -> Option<SceneEvent> {
		self.pending.pop()
	}

	pub fn render(&mut self, canvas: &mut Canvas) {
		let mut canvas = canvas.sub(0, 0, self.width, self.height);
		for container in &mut self.widgets {
			let mut sub = canvas.sub(
				container.boundary.x,
				container.boundary.y,
				container.boundary.width,
				container.boundary.height,
			);
			container.widget.render(&mut sub);
		}
	}
}

fn translate_event(event: &AppEvent, rect: &Rect) -> AppEvent {
	match event {
		AppEvent::PointerMotion { x, y } => AppEvent::PointerMotion {
			x: x - rect.x,
			y: y - rect.y,
		},
		AppEvent::PointerButton { x, y, button, pressed } => AppEvent::PointerButton {
			x: x - rect.x,
			y: y - rect.y,
			button: *button,
			pressed: *pressed,
		},
		other => other.clone(),
	}
}

struct Rect {
	x: i32,
	y: i32,
	width: i32,
	height: i32,
}
