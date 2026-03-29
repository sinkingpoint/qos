use std::{
	fs::File,
	io::{self, Read},
	os::fd::AsRawFd,
};

use bytestruct::{ReadFromWithEndian, int_enum};
use bytestruct_derive::ByteStruct;
use nix::ioctl_write_int;

use super::event_threads::EventSource;

ioctl_write_int!(eviocgrab, b'E', 0x90);

pub struct InputWatcher {
	devices: Vec<File>,
}

impl EventSource for InputWatcher {
	type Reader = std::fs::File;
	type EventType = InputEvent;

	fn get_fds(&self) -> &[Self::Reader] {
		&self.devices
	}

	fn read_event(&mut self, index: usize) -> std::io::Result<Self::EventType> {
		let mut buf = [0u8; 24]; // Size of input_event struct
		let reader = self
			.devices
			.get_mut(index)
			.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Device not found"))?;
		match reader.read_exact(&mut buf) {
			Ok(_) => InputEvent::read_from_with_endian(&mut &buf[..], bytestruct::Endian::Little),
			Err(e) => Err(e),
		}
	}
}

impl InputWatcher {
	pub fn new() -> io::Result<Self> {
		let mut devices = Vec::new();

		for entry in std::fs::read_dir("/dev/input")? {
			let entry = entry?;
			if entry.file_name().to_string_lossy().starts_with("event") {
				let fd = std::fs::OpenOptions::new().read(true).open(entry.path())?;
				unsafe { eviocgrab(fd.as_raw_fd(), 1) }?;

				devices.push(fd);
			}
		}

		Ok(Self { devices })
	}
}

impl Drop for InputWatcher {
	fn drop(&mut self) {
		for device in &self.devices {
			let _ = unsafe { eviocgrab(device.as_raw_fd(), 0) };
		}
	}
}

int_enum! {
  #[derive(Debug, Clone, PartialEq)]
  pub enum InputEventType: u16 {
	System = 0,
	Key = 1,
	Relative = 2,
	Absolute = 3,
	Misc = 4,
	Switch = 5,
  Led = 0x11,
  Sound = 0x12,
  Repeat = 0x14,
  ForceFeedback = 0x15,
  Power = 0x16,
  ForceFeedbackStatus = 0x17,
  }
}

#[derive(Debug, Clone, ByteStruct)]
pub struct InputEvent {
	pub secs: u64,
	pub usecs: u64,
	pub event_type: InputEventType,
	pub code: u16,
	pub value: i32,
}
