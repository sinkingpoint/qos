use crate::{AppEvent, canvas::Canvas};

mod button;
pub use button::Button;

pub trait Widget {
	fn handle_event(&mut self, event: &AppEvent) -> bool;
	fn render(&mut self, canvas: &mut Canvas);
	fn size_hint(&self) -> (i32, i32);
}
