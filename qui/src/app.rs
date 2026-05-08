use std::io;

use crate::{AppEvent, Scene, TopBar, TopBarEvent, Widget, WidgetHandle, Window};

pub struct App {
	window: Window,
	scene: Scene,
	top_bar: WidgetHandle<TopBarEvent>,
}

impl App {
	pub fn new(title: impl Into<String>, width: i32, height: i32) -> io::Result<Self> {
		let window = Window::new(width, height)?;
		let mut scene = Scene::new(width, height);
		let top_bar = scene.add_widget(TopBar::new(title.into()), 0, 0);
		Ok(Self { window, scene, top_bar })
	}

	pub fn set_content<W: Widget + 'static>(&mut self, widget: W) -> WidgetHandle<W::Event> {
		self.scene.add_widget(widget, 0, TopBar::HEIGHT)
	}

	pub fn poll(&mut self) -> io::Result<AppEvent> {
		let event = self.window.poll()?;
		self.handle_event(&event)?;
		Ok(event)
	}

	fn handle_event(&mut self, event: &AppEvent) -> io::Result<()> {
		self.scene.handle_event(event);
		// drain topbar events and handle drag internally
		while let Some(ev) = self.scene.poll() {
			if let Some(TopBarEvent::DragStarted) = self.top_bar.extract(&ev) {
				self.window.start_move()?;
			}
		}
		Ok(())
	}

	pub fn render(&mut self) -> io::Result<()> {
		if let Some(mut canvas) = self.window.canvas() {
			self.scene.render(&mut canvas);
			self.window.commit_frame()?;
		}

		Ok(())
	}
}
