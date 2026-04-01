use std::thread;

pub mod drm;
mod event_threads;
pub mod input;
mod scm_bufreader;
pub mod wayland;

#[derive(Debug)]
pub enum CompositorEvent {
	Drm(crate::events::drm::DrmEvent),
	Input(crate::events::input::Event),
	Wayland(crate::events::wayland::WaylandEvent),
}

impl From<crate::events::drm::DrmEvent> for CompositorEvent {
	fn from(event: crate::events::drm::DrmEvent) -> Self {
		Self::Drm(event)
	}
}

impl From<crate::events::input::Event> for CompositorEvent {
	fn from(event: crate::events::input::Event) -> Self {
		Self::Input(event)
	}
}

impl From<crate::events::wayland::WaylandEvent> for CompositorEvent {
	fn from(event: crate::events::wayland::WaylandEvent) -> Self {
		Self::Wayland(event)
	}
}

pub fn input_watcher_event_thread(
	sender: std::sync::mpsc::Sender<CompositorEvent>,
) -> event_threads::EventThreadHandle {
	let watcher = input::InputWatcher::new().expect("Failed to initialize input watcher");
	let (mut input_thread, input_thread_handle) =
		event_threads::EventThread::new(watcher).expect("Failed to create input event thread");
	thread::spawn(move || {
		if let Err(err) = input_thread.start_watching(sender) {
			eprintln!("Input watcher error: {}", err);
		}
	});

	input_thread_handle
}

pub fn drm_event_thread(
	card: std::fs::File,
	sender: std::sync::mpsc::Sender<CompositorEvent>,
) -> event_threads::EventThreadHandle {
	let source = drm::DrmEventSource::new(card);
	let (mut drm_thread, drm_thread_handle) =
		event_threads::EventThread::new(source).expect("Failed to create DRM event thread");
	thread::spawn(move || {
		if let Err(err) = drm_thread.start_watching(sender) {
			eprintln!("DRM watcher error: {}", err);
		}
	});

	drm_thread_handle
}

pub fn wayland_event_thread(
	display_name: String,
	sender: std::sync::mpsc::Sender<CompositorEvent>,
) -> event_threads::EventThreadHandle {
	let socket = wayland::WaylandSocket::new(display_name).expect("Failed to create Wayland socket");
	socket.start(sender).expect("Failed to start Wayland event thread")
}
