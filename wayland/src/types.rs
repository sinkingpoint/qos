use std::{collections::VecDeque, io::Write, ops::Deref, os::{fd::OwnedFd, unix::net::UnixStream}};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

#[derive(Debug)]
pub struct WaylandEncodedString(pub String);

impl Deref for WaylandEncodedString {
	type Target = String;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl WriteToWithEndian for WaylandEncodedString {
	fn write_to_with_endian<W: std::io::Write>(&self, writer: &mut W, _endian: Endian) -> std::io::Result<()> {
		writer.write_all(&(self.0.len() as u32 + 1).to_le_bytes())?;
		writer.write_all(self.0.as_bytes())?;
		writer.write_all(&[0])?; // null terminator
		let padding = (4 - (self.0.len() + 1) % 4) % 4;
		writer.write_all(&vec![0; padding])?; // padding to 4 bytes
		Ok(())
	}
}

impl ReadFromWithEndian for WaylandEncodedString {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, _endian: Endian) -> std::io::Result<Self> {
		let mut len_bytes = [0u8; 4];
		reader.read_exact(&mut len_bytes)?;
		let len = u32::from_le_bytes(len_bytes);
		let mut string_bytes = vec![0u8; len as usize];
		reader.read_exact(&mut string_bytes)?;
		// Strip the null byte
		if string_bytes.last() == Some(&0) {
			string_bytes.pop();
		} else {
			return Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				"Wayland string is not null-terminated",
			));
		}

		let padding = (4 - (len % 4)) % 4;
		let mut padding_bytes = vec![0u8; padding as usize];
		reader.read_exact(&mut padding_bytes)?;

		let string = String::from_utf8(string_bytes).map_err(|e| {
			std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				format!("Invalid UTF-8 in Wayland string: {}", e),
			)
		})?;
		Ok(Self(string))
	}
}

pub trait WaylandPayload {
	const OPCODE: u16;
	fn write_as_packet(&self, object_id: u32, connection: &std::sync::Arc<UnixStream>) -> std::io::Result<()>;
}

/// Wraps a parsed payload struct with an `OwnedFd` received via SCM_RIGHTS.
/// Used for Wayland messages that carry a file descriptor alongside payload bytes.
pub struct WithFd<T> {
	pub cmd: T,
	pub fd: OwnedFd,
}

/// Parses a value from a Wayland event payload, with access to any received
/// file descriptors. Normal types ignore `fds`; `WithFd<T>` pops one from it.
pub trait FromPayload: Sized {
	fn from_payload(payload: &[u8], fds: &mut VecDeque<OwnedFd>) -> Option<Self>;
}

impl<T: ReadFromWithEndian> FromPayload for T {
	fn from_payload(payload: &[u8], _fds: &mut VecDeque<OwnedFd>) -> Option<Self> {
		T::read_from_with_endian(&mut std::io::Cursor::new(payload), Endian::Little).ok()
	}
}

impl<T: ReadFromWithEndian> FromPayload for WithFd<T> {
	fn from_payload(payload: &[u8], fds: &mut VecDeque<OwnedFd>) -> Option<Self> {
		let cmd = T::read_from_with_endian(&mut std::io::Cursor::new(payload), Endian::Little).ok()?;
		let fd = fds.pop_front()?;
		Some(Self { cmd, fd })
	}
}

#[derive(Debug, ByteStruct)]
pub struct WaylandHeader {
	pub object_id: u32,
	pub opcode: u16,
	pub data_length: u16,
}

#[derive(Debug)]
pub struct WaylandPacket {
	pub object_id: u32,
	pub opcode: u16,
	pub payload: Vec<u8>,
}

impl WaylandPacket {
	pub fn new(object_id: u32, opcode: u16, payload: Vec<u8>) -> Self {
		Self {
			object_id,
			opcode,
			payload,
		}
	}
}

impl WriteToWithEndian for WaylandPacket {
	fn write_to_with_endian<W: Write>(&self, writer: &mut W, endian: Endian) -> std::io::Result<()> {
		let header = WaylandHeader {
			object_id: self.object_id,
			opcode: self.opcode,
			data_length: self.payload.len() as u16 + 8,
		};
		header.write_to_with_endian(writer, endian)?;
		writer.write_all(&self.payload)
	}
}

/// Generates a client-side event enum for a Wayland object type.
/// Each variant wraps a typed event struct. The enum exposes a
/// `parse(opcode, payload) -> Option<Self>` method for use in `poll()` loops.
///
/// Usage:
/// ```
/// wayland_client_events!(PointerEvent {
///     EnterEvent::OPCODE => Enter(EnterEvent),
///     LeaveEvent::OPCODE => Leave(LeaveEvent),
/// });
/// ```
#[macro_export]
macro_rules! wayland_client_events {
	($enum_name:ident { $($opcode:pat => $variant:ident($ty:ty)),* $(,)? }) => {
		pub enum $enum_name {
			$($variant($ty),)*
		}

		impl $enum_name {
			pub fn parse(
				opcode: u16,
				payload: &[u8],
				fds: &mut ::std::collections::VecDeque<::std::os::fd::OwnedFd>,
			) -> Option<Self> {
				use $crate::types::FromPayload;
				match opcode {
					$($opcode => Some(Self::$variant(<$ty>::from_payload(payload, fds)?)),)*
					_ => None,
				}
			}
		}
	};
}

/// Implements `WaylandPayload` for a struct, associating it with a fixed opcode,
/// and exposes `<Type>::OPCODE: u16` for matching.
/// The struct must implement `WriteToWithEndian`.
///
/// Usage: `wayland_payload!(MyStruct, opcode = 3);`
#[macro_export]
macro_rules! wayland_payload {
	($ty:ty, opcode = $opcode:expr) => {
		impl $ty {
			pub const OPCODE: u16 = $opcode;
		}
		impl $crate::types::WaylandPayload for $ty {
			const OPCODE: u16 = $opcode;
			fn write_as_packet(
				&self,
				object_id: u32,
				connection: &std::sync::Arc<std::os::unix::net::UnixStream>,
			) -> std::io::Result<()> {
				use bytestruct::WriteToWithEndian;
				use std::io::Write;
				let mut payload = Vec::new();
				self.write_to_with_endian(&mut payload, bytestruct::Endian::Little)?;
				let packet = $crate::types::WaylandPacket::new(object_id, $opcode, payload);
				let mut buf = Vec::new();
				packet.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
				connection.as_ref().write_all(&buf)
			}
		}
	};
}
