use std::thread;

pub mod drm;
mod event_threads;
pub mod input;

#[derive(Debug)]
pub enum CompositorEvent {
	DrmEvent(crate::drm::DrmEvent),
	InputEvent(crate::events::input::InputEvent),
}

impl From<crate::drm::DrmEvent> for CompositorEvent {
	fn from(event: crate::drm::DrmEvent) -> Self {
		Self::DrmEvent(event)
	}
}

impl From<crate::events::input::InputEvent> for CompositorEvent {
	fn from(event: crate::events::input::InputEvent) -> Self {
		Self::InputEvent(event)
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
