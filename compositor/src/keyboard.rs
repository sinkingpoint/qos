use std::io::Write;

use bitflags::bitflags;
use bytestruct::{ReadFromWithEndian, WriteToWithEndian};

use crate::events::input::{KeyCode, KeyState};

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq)]
	pub struct Modifiers: u32 {
		const SHIFT   = 0x01;
		const LOCK    = 0x02;
		const CONTROL = 0x04;
		const MOD1    = 0x08;
		const MOD2    = 0x10;
		const MOD3    = 0x20;
		const MOD4    = 0x40;
		const MOD5    = 0x80;
	}
}

impl WriteToWithEndian for Modifiers {
	fn write_to_with_endian<W: Write>(&self, writer: &mut W, endian: bytestruct::Endian) -> std::io::Result<()> {
		self.bits().write_to_with_endian(writer, endian)
	}
}

impl ReadFromWithEndian for Modifiers {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let bits = u32::read_from_with_endian(reader, endian)?;
		Modifiers::from_bits(bits).ok_or_else(|| {
			std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Invalid modifiers bitmask: {bits:#x}"),
			)
		})
	}
}

pub struct Keyboard {
	pub depressed: Modifiers,
	pub locked: Modifiers,
}

impl Keyboard {
	pub fn new() -> Self {
		Self {
			depressed: Modifiers::empty(),
			locked: Modifiers::empty(),
		}
	}

	pub fn handle_input(&mut self, event: KeyEvent) {
		match event {
			KeyEvent::KeyPress(code) => self.update_modifiers(code, KeyState::Pressed),
			KeyEvent::KeyRelease(code) => self.update_modifiers(code, KeyState::Released),
		}
	}

	fn update_modifiers(&mut self, code: KeyCode, state: KeyState) {
		// Update modifiers based on the keycode and state.
		// This is a simplified example; in a real implementation, you'd need to handle all relevant modifier keys.
		if code == KeyCode::KeyLeftCtrl || code == KeyCode::KeyRightCtrl {
			if state == KeyState::Pressed || state == KeyState::AutoRepeat {
				self.depressed.insert(Modifiers::CONTROL);
			} else {
				self.depressed.remove(Modifiers::CONTROL);
			}
		} else if code == KeyCode::KeyLeftShift || code == KeyCode::KeyRightShift {
			if state == KeyState::Pressed || state == KeyState::AutoRepeat {
				self.depressed.insert(Modifiers::SHIFT);
			} else {
				self.depressed.remove(Modifiers::SHIFT);
			}
		} else if code == KeyCode::KeyLeftAlt {
			if state == KeyState::Pressed || state == KeyState::AutoRepeat {
				self.depressed.insert(Modifiers::MOD1);
			} else {
				self.depressed.remove(Modifiers::MOD1);
			}
		} else if code == KeyCode::KeyLeftMeta || code == KeyCode::KeyRightMeta {
			if state == KeyState::Pressed || state == KeyState::AutoRepeat {
				self.depressed.insert(Modifiers::MOD4);
			} else {
				self.depressed.remove(Modifiers::MOD4);
			}
		} else if code == KeyCode::KeyRightAlt {
			if state == KeyState::Pressed || state == KeyState::AutoRepeat {
				self.depressed.insert(Modifiers::MOD5);
			} else {
				self.depressed.remove(Modifiers::MOD5);
			}
		} else if code == KeyCode::KeyCapsLock && state == KeyState::Pressed {
			self.locked.toggle(Modifiers::LOCK);
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub enum KeyEvent {
	KeyPress(KeyCode),
	KeyRelease(KeyCode),
}

impl KeyEvent {
	pub fn new(code: KeyCode, state: KeyState) -> Self {
		match state {
			KeyState::Pressed | KeyState::AutoRepeat => KeyEvent::KeyPress(code),
			KeyState::Released => KeyEvent::KeyRelease(code),
		}
	}
}
