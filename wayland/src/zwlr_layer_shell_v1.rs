use std::io;

use bytestruct::{ReadFromWithEndian, WriteToWithEndian, int_enum};
use bytestruct_derive::ByteStruct;

use crate::{types::WaylandEncodedString, wayland_payload};

#[derive(Debug, ByteStruct)]
pub struct GetLayerSurfaceRequest {
	pub new_id: u32,
	pub wl_surface_id: u32,
	pub output_id: u32,
	pub layer: Layer,
	pub namespace: WaylandEncodedString,
}

int_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Layer: u32 {
	Background = 0,
	Bottom = 1,
	Top = 2,
	Overlay = 3,
  }
}

wayland_payload!(GetLayerSurfaceRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct SetSizeRequest {
	pub width: i32,
	pub height: i32,
}

wayland_payload!(SetSizeRequest, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct SetAnchorRequest {
	pub anchor: Anchor,
}

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct Anchor: u32 {
  const Top = 1 << 0;
  const Bottom = 1 << 1;
  const Left = 1 << 2;
  const Right = 1 << 3;
  }
}

impl ReadFromWithEndian for Anchor {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: bytestruct::Endian) -> io::Result<Self> {
		let bits = u32::read_from_with_endian(reader, endian)?;
		Ok(Anchor::from_bits_truncate(bits))
	}
}

impl WriteToWithEndian for Anchor {
	fn write_to_with_endian<W: std::io::Write>(&self, writer: &mut W, endian: bytestruct::Endian) -> io::Result<()> {
		self.bits().write_to_with_endian(writer, endian)
	}
}

wayland_payload!(SetAnchorRequest, opcode = 1);

#[derive(Debug, ByteStruct)]
pub struct SetExclusiveZoneRequest {
	pub zone: i32,
}

wayland_payload!(SetExclusiveZoneRequest, opcode = 2);

#[derive(Debug, ByteStruct)]
pub struct SetMarginRequest {
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
	pub left: i32,
}

wayland_payload!(SetMarginRequest, opcode = 3);

#[derive(Debug, ByteStruct)]
pub struct SetKeyboardInteractivityRequest {
	pub interactivity: KeyboardInteractivity,
}

int_enum! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum KeyboardInteractivity: u32 {
  None = 0,
  OnDemand = 1,
  Full = 2,
  }
}

wayland_payload!(SetKeyboardInteractivityRequest, opcode = 4);

#[derive(Debug, ByteStruct)]
pub struct GetPopupRequest {
	pub popup_id: u32,
}

wayland_payload!(GetPopupRequest, opcode = 5);

#[derive(Debug, ByteStruct)]
pub struct AckConfigureRequest {
	pub serial: u32,
}

wayland_payload!(AckConfigureRequest, opcode = 6);

#[derive(Debug, ByteStruct)]
pub struct DestroyRequest;

wayland_payload!(DestroyRequest, opcode = 7);

#[derive(Debug, ByteStruct)]
pub struct SetLayerRequest {
	pub layer: Layer,
}

wayland_payload!(SetLayerRequest, opcode = 8);

#[derive(Debug, ByteStruct)]
pub struct SetExclusiveEdgeRequest {
	pub edge: Anchor,
}

wayland_payload!(SetExclusiveEdgeRequest, opcode = 9);

#[derive(Debug, ByteStruct)]
pub struct ConfigureEvent {
	pub serial: u32,
	pub width: u32,
	pub height: u32,
}

wayland_payload!(ConfigureEvent, opcode = 0);

#[derive(Debug, ByteStruct)]
pub struct ClosedEvent;

wayland_payload!(ClosedEvent, opcode = 1);
