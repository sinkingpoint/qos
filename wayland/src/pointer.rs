use bytestruct::int_enum;
use bytestruct_derive::ByteStruct;

use crate::wayland_payload;

#[derive(Debug, ByteStruct)]
pub struct SetCursorRequest {
	pub serial: u32,
	pub surface_id: u32,
	pub hotspot_x: i32,
	pub hotspot_y: i32,
}

wayland_payload!(SetCursorRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct EnterEvent {
	pub serial: u32,
	pub surface_id: u32,
	pub x: i32,
	pub y: i32,
}

wayland_payload!(EnterEvent, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct LeaveEvent {
	pub serial: u32,
	pub surface_id: u32,
}

wayland_payload!(LeaveEvent, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct MoveEvent {
	pub time: u32,
	pub x: i32,
	pub y: i32,
}

wayland_payload!(MoveEvent, opcode = 2);

#[derive(Debug, ByteStruct)]
pub struct ButtonEvent {
	pub serial: u32,
	pub time: u32,
	pub button: u32,
	pub state: u32,
}

wayland_payload!(ButtonEvent, opcode = 3);

#[derive(Debug, ByteStruct)]
pub struct FrameEvent;

wayland_payload!(FrameEvent, opcode = 5);

crate::wayland_client_events!(PointerEvent {
	EnterEvent::OPCODE  => Enter(EnterEvent),
	LeaveEvent::OPCODE  => Leave(LeaveEvent),
	MoveEvent::OPCODE   => Move(MoveEvent),
	ButtonEvent::OPCODE => Button(ButtonEvent),
	FrameEvent::OPCODE  => Frame(FrameEvent),
});

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
