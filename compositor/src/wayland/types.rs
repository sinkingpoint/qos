use std::{collections::HashMap, os::unix::net::UnixStream, sync::Arc};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

use crate::wayland::{display::Display, registry::Registry};

pub trait SubSystem {
	type Request: CommandRegistry;
	fn parse_command(&self, command: WaylandPacket) -> Option<Self::Request> {
		Self::Request::parse(command)
	}
}

pub enum WaylandError {
	IOError(std::io::Error),
	UnrecognisedObject,
}

pub type WaylandResult<T> = Result<T, WaylandError>;

pub trait Command<T: SubSystem>
where
	Self: Sized,
{
	fn handle(&self, connection: &Arc<UnixStream>, subsystem: &mut T) -> WaylandResult<Option<(u32, SubsystemType)>>;
}

pub trait CommandRegistry {
	fn parse(command: WaylandPacket) -> Option<Self>
	where
		Self: std::marker::Sized;
}

pub struct Client {
	pub connection: Arc<UnixStream>,
	pub objects: HashMap<u32, SubsystemType>,
}

impl Client {
	pub fn new(connection: Arc<UnixStream>) -> Self {
		let mut objects = HashMap::new();
		objects.insert(1, SubsystemType::Display(Display {}));
		Self { connection, objects }
	}

	pub fn register_object(&mut self, object_id: u32, subsystem: SubsystemType) {
		self.objects.insert(object_id, subsystem);
	}

	pub fn handle_command(&mut self, command: WaylandPacket) -> WaylandResult<()> {
		let object_id = command.object_id;
		let Some(subsystem) = self.objects.get_mut(&object_id) else {
			return Err(WaylandError::UnrecognisedObject);
		};
		if let Some((new_id, new_obj)) = subsystem.handle_command(&self.connection, command)? {
			self.objects.insert(new_id, new_obj);
		}
		Ok(())
	}
}

pub enum SubsystemType {
	Display(Display),
	Registry(Registry),
}

// TODO: Macro this
impl SubsystemType {
	fn handle_command(
		&mut self,
		connection: &Arc<UnixStream>,
		command: WaylandPacket,
	) -> WaylandResult<Option<(u32, SubsystemType)>> {
		match self {
			SubsystemType::Display(display) => {
				if let Some(cmd) = display.parse_command(command) {
					cmd.handle(connection, display)
				} else {
					Ok(None)
				}
			}
			SubsystemType::Registry(registry) => {
				if let Some(cmd) = registry.parse_command(command) {
					cmd.handle(connection, registry)
				} else {
					Ok(None)
				}
			}
		}
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
	fn write_to_with_endian<W: std::io::Write>(&self, writer: &mut W, endian: Endian) -> std::io::Result<()> {
		let header = WaylandHeader {
			object_id: self.object_id,
			opcode: self.opcode,
			data_length: self.payload.len() as u16 + 8,
		};
		header.write_to_with_endian(writer, endian)?;
		writer.write_all(&self.payload)?;
		Ok(())
	}
}

impl ReadFromWithEndian for WaylandPacket {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: Endian) -> std::io::Result<Self> {
		let header = WaylandHeader::read_from_with_endian(reader, endian)?;
		let mut payload = vec![0u8; header.data_length as usize - 8];
		reader.read_exact(&mut payload)?;
		Ok(Self {
			object_id: header.object_id,
			opcode: header.opcode,
			payload,
		})
	}
}
