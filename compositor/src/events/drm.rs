use std::io::{self, Read};

use bytestruct::int_enum;

use super::event_threads::EventSource;

int_enum! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub enum DrmEventType: u32 {
		VBlank = 0x01,
		FlipComplete = 0x02,
		Sequence = 0x03,
	}
}

#[derive(Debug, Clone)]
pub struct DrmEvent {
	pub event_type: DrmEventType,
	pub _data: Vec<u8>,
}

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
			_data: buf[8..length.min(n)].to_vec(),
		})
	}
}

impl DrmEventSource {
	pub fn new(card: std::fs::File) -> Self {
		Self { card }
	}
}
