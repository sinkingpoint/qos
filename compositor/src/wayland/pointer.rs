use std::{os::unix::net::UnixStream, sync::Arc};

use bytestruct::int_enum;
use bytestruct_derive::ByteStruct;
use nix::time::{ClockId, clock_gettime};

use crate::{
	events::input::KeyCode,
	wayland::types::{ClientEffect, Command, SubSystem, WaylandResult},
};

pub struct Pointer;

impl SubSystem for Pointer {
	type Request = PointerRequest;
	const NAME: &'static str = "wl_pointer";
}

wayland_interface!(Pointer, PointerRequest {
  0 => SetCursor(SetCursorCommand),
  1 => Destroy(DestroyCommand),
});

#[derive(Debug, ByteStruct)]
pub struct SetCursorCommand {
	pub serial: u32,
	pub surface_id: u32,
	pub hotspot_x: i32,
	pub hotspot_y: i32,
}

impl Command<Pointer> for SetCursorCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _pointer: &mut Pointer) -> WaylandResult<Option<ClientEffect>> {
		Ok(None) // TODO: implement cursor setting
	}
}

#[derive(Debug, ByteStruct)]
pub struct DestroyCommand;

impl Command<Pointer> for DestroyCommand {
	fn handle(self, _connection: &Arc<UnixStream>, _pointer: &mut Pointer) -> WaylandResult<Option<ClientEffect>> {
		Ok(Some(ClientEffect::DestroySelf))
	}
}

#[derive(Debug, ByteStruct)]
pub struct EnterEvent {
	pub serial: u32,
	pub surface_id: u32,
	pub x: i32,
	pub y: i32,
}

impl EnterEvent {
	pub fn new(serial: u32, surface_id: u32, x: i32, y: i32) -> Self {
		Self {
			serial,
			surface_id,
			x: x * 256,
			y: y * 256,
		}
	}
}

#[derive(Debug, ByteStruct)]
pub struct LeaveEvent {
	pub serial: u32,
	pub surface_id: u32,
}

#[derive(Debug, ByteStruct)]
pub struct MoveEvent {
	pub time: u32,
	pub x: i32,
	pub y: i32,
}

impl MoveEvent {
	pub fn new(x: i32, y: i32) -> Self {
		let time = clock_gettime(ClockId::CLOCK_MONOTONIC).expect("Failed to get time");
		let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
		Self {
			time: ms as u32,
			x: x * 256,
			y: y * 256,
		}
	}
}

int_enum! {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState: u32 {
	Released = 0,
	Pressed = 1,
}
}

int_enum! {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonCode: u32 {
	Left = 0x110,
	Right = 0x111,
	Middle = 0x112,
	Button4 = 0x113,
	Button5 = 0x114,
}
}

impl TryFrom<KeyCode> for ButtonCode {
	type Error = ();

	fn try_from(value: KeyCode) -> Result<Self, Self::Error> {
		match value {
			KeyCode::KeyLeft => Ok(ButtonCode::Left),
			KeyCode::KeyRight => Ok(ButtonCode::Right),
			_ => Err(()),
		}
	}
}

#[derive(Debug, ByteStruct)]
pub struct ButtonEvent {
	pub serial: u32,
	pub time: u32,
	pub button: ButtonCode,
	pub state: ButtonState,
}

impl ButtonEvent {
	pub fn new(serial: u32, button: ButtonCode, state: ButtonState) -> Self {
		let time = clock_gettime(ClockId::CLOCK_MONOTONIC).expect("Failed to get time");
		let ms = time.tv_sec() * 1000 + time.tv_nsec() / 1_000_000;
		Self {
			serial,
			time: ms as u32,
			button,
			state,
		}
	}
}
