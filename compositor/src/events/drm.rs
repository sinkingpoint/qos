use std::io::{self, Read};

use super::event_threads::EventSource;
use crate::drm::{DrmEvent, DrmEventType};

pub struct DrmEventSource {
	card: std::fs::File,
}

impl EventSource for DrmEventSource {
	type Reader = std::fs::File;
	type EventType = DrmEvent;

	fn get_fds(&self) -> &[Self::Reader] {
		std::slice::from_ref(&self.card)
	}

	fn read_event(&mut self, _index: usize) -> io::Result<Self::EventType> {
		let mut buf = [0u8; 64];
		let n = self.card.read(&mut buf)?;
		let event_type = u32::from_le_bytes(buf[0..4].try_into().unwrap());
		let length = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;
		let event_type = DrmEventType::try_from(event_type)
			.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Unknown DRM event type: {}", e)))?;
		Ok(DrmEvent {
			event_type,
			data: buf[8..length.min(n)].to_vec(),
		})
	}
}

impl DrmEventSource {
	pub fn new(card: std::fs::File) -> Self {
		Self { card }
	}
}
