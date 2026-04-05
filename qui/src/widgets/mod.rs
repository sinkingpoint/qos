use std::any::Any;

use crate::{AppEvent, canvas::Canvas};

mod button;
pub use button::{Button, ButtonEvent};

pub trait Widget {
	type Event: 'static;
	fn handle_event(&mut self, event: &AppEvent) -> Option<Self::Event>;
	fn render(&mut self, canvas: &mut Canvas);
	fn size_hint(&self) -> (i32, i32);
}

/// Type-erased wrapper so Scene can hold heterogeneous widgets in a Vec.
pub(crate) trait AnyWidget {
	fn handle_event_any(&mut self, event: &AppEvent) -> Option<Box<dyn Any>>;
	fn render(&mut self, canvas: &mut Canvas);
}

impl<W: Widget> AnyWidget for W {
	fn handle_event_any(&mut self, event: &AppEvent) -> Option<Box<dyn Any>> {
		self.handle_event(event).map(|e| Box::new(e) as Box<dyn Any>)
	}
	fn render(&mut self, canvas: &mut Canvas) {
		<Self as Widget>::render(self, canvas);
	}
}
