use std::{collections::HashMap, os::unix::net::UnixStream};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

use crate::wayland::{display::Display, registry::Registry};

pub trait SubSystem {
	type Request: CommandRegistry;
	fn parse_command(&self, opcode: u16, arguments: &[u8]) -> Option<Self::Request> {
		Self::Request::parse(opcode, arguments)
	}
}

pub enum WaylandError {
	IOError(std::io::Error),
}

pub type WaylandResult<T> = Result<T, WaylandError>;

pub trait Command<T: SubSystem>
where
	Self: Sized,
{
	fn handle(&self, client: &mut Client, subsystem: &mut T) -> WaylandResult<()>;
}

pub trait CommandRegistry {
	fn parse(opcode: u16, args: &[u8]) -> Option<Self>
	where
		Self: std::marker::Sized;
}

pub struct Client {
	pub connection: UnixStream,
	pub objects: HashMap<u32, SubsystemType>,
}

impl Client {
	pub fn new(connection: UnixStream) -> Self {
		let mut objects = HashMap::new();
		objects.insert(1, SubsystemType::Display(Display {}));
		Self { connection, objects }
	}

	pub fn register_object(&mut self, object_id: u32, subsystem: SubsystemType) {
		self.objects.insert(object_id, subsystem);
	}
}

pub enum SubsystemType {
	Display(Display),
	Registry(Registry),
}

#[derive(Debug, ByteStruct)]
pub struct WaylandHeader {
	pub object_id: u32,
	pub opcode: u16,
	pub data_length: u16,
}

#[derive(Debug)]
pub struct WaylandPacket<T> {
	pub object_id: u32,
	pub opcode: u16,
	pub payload: T,
}

impl<T> WaylandPacket<T> {
	pub fn new(object_id: u32, opcode: u16, payload: T) -> Self {
		Self {
			object_id,
			opcode,
			payload,
		}
	}
}

impl<T: WriteToWithEndian> WriteToWithEndian for WaylandPacket<T> {
	fn write_to_with_endian<W: std::io::Write>(&self, writer: &mut W, endian: Endian) -> std::io::Result<()> {
		let mut data = Vec::new();
		self.payload.write_to_with_endian(&mut data, endian)?;
		let header = WaylandHeader {
			object_id: self.object_id,
			opcode: self.opcode,
			data_length: data.len() as u16 + 8, // Total message size: payload + 8-byte header
		};
		header.write_to_with_endian(writer, endian)?;
		writer.write_all(&data)?;
		Ok(())
	}
}

impl<T: ReadFromWithEndian> ReadFromWithEndian for WaylandPacket<T> {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: Endian) -> std::io::Result<Self> {
		let header = WaylandHeader::read_from_with_endian(reader, endian)?;
		let mut payload_data = vec![0u8; header.data_length as usize - 8]; // data_length includes 8-byte header
		reader.read_exact(&mut payload_data)?;
		let payload = T::read_from_with_endian(&mut &payload_data[..], endian)?;
		Ok(Self {
			object_id: header.object_id,
			opcode: header.opcode,
			payload,
		})
	}
}
