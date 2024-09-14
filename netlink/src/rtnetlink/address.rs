use bitflags::bitflags;
use bytestruct_derive::{ByteStruct, Size};
use std::{
	fmt::{Display, Write as _},
	io::{self, Cursor, ErrorKind, Read, Write},
};

use bytestruct::{int_enum, Endian, NullTerminatedString, ReadFromWithEndian, Size, WriteToWithEndian};

use crate::{new_string, new_u32, read_attribute, write_attribute};

#[derive(Debug, Clone)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
	pub fn new(bytes: [u8; 6]) -> Self {
		Self(bytes)
	}
}

impl Display for MacAddress {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!(
			"{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
			self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
		))
	}
}

impl WriteToWithEndian for MacAddress {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: bytestruct::Endian) -> io::Result<()> {
		self.0.write_to_with_endian(target, endian)
	}
}

int_enum! {
	#[derive(Debug)]
	pub enum AddressFamily: u8 {
		Unspecified = 0,
		IPv4 = 2,
		IPv6 = 10,
	}
}

bitflags! {
	#[derive(Debug)]
	pub struct AddressFlags : u8 {
		const IFA_F_SECONDARY = 0x01;
		const IFA_F_NODAD = 0x02;
		const IFA_F_OPTIMISTIC = 0x04;
		const IFA_F_DADFAILED = 0x08;
		const IFA_F_HOMEADDRESS = 0x10;
		const IFA_F_DEPRECATED = 0x20;
		const IFA_F_TENTATIVE = 0x40;
		const IFA_F_PERMANENT = 0x80;
	}
}

impl Display for AddressFlags {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		bitflags::parser::to_writer_strict(self, f)
	}
}

impl WriteToWithEndian for AddressFlags {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: bytestruct::Endian) -> io::Result<()> {
		self.bits().write_to_with_endian(target, endian)
	}
}

impl ReadFromWithEndian for AddressFlags {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: bytestruct::Endian) -> io::Result<Self> {
		let val = u8::read_from_with_endian(source, endian)?;
		Ok(Self::from_bits_retain(val))
	}
}

impl Size for AddressFlags {
	fn size(&self) -> usize {
		1
	}
}

int_enum! {
	#[derive(Debug)]
	pub enum AddressScope: u8 {
		Universe = 0,
		Site = 200,
		Link = 253,
		Host = 254,
		Nowhere = 255,
	}
}

#[derive(Debug, ByteStruct, Size)]
pub struct InterfaceAddressMessage {
	pub family: AddressFamily,
	pub prefix_length: u8,
	pub flags: AddressFlags,
	pub scope: AddressScope,
	pub interface_index: u32,
}

impl InterfaceAddressMessage {
	pub fn empty() -> InterfaceAddressMessage {
		InterfaceAddressMessage {
			family: AddressFamily::Unspecified,
			prefix_length: 0,
			flags: AddressFlags::empty(),
			scope: AddressScope::Universe,
			interface_index: 0,
		}
	}
}

#[derive(Debug, Clone)]
pub enum IPAddress {
	IPv4([u8; 4]),
	IPv6([u8; 16]),
}

impl IPAddress {
	pub fn new(bytes: &[u8]) -> io::Result<Self> {
		match bytes.len() {
			4 => Ok(IPAddress::IPv4(bytes.try_into().unwrap())),
			16 => Ok(IPAddress::IPv6(bytes.try_into().unwrap())),
			_ => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid IP address length: {}", bytes.len()),
			)),
		}
	}
}

impl WriteToWithEndian for IPAddress {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match self {
			Self::IPv4(bytes) => <[u8; 4]>::write_to_with_endian(bytes, target, endian),
			Self::IPv6(bytes) => <[u8; 16]>::write_to_with_endian(bytes, target, endian),
		}
	}
}

impl Display for IPAddress {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::IPv4(bytes) => f.write_fmt(format_args!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])),
			Self::IPv6(bytes) => {
				// IPv6 Addresses compress the longest run of 0 bytes into `::`. First, we find that:
				let mut run_start = None;
				let mut longest_run = (None, None);

				for (i, &byte) in bytes.iter().enumerate() {
					if byte == 0 {
						// If we hit a zero bytes and we're not in a run, start one.
						if run_start.is_none() {
							run_start = Some(i);
						}
						continue;
					}

					// If we are run a run, and we encountered a non-zero byte, end the run.
					if let Some(start) = run_start {
						let run_length = i - start;
						if run_length <= 1 {
							// Only compress runs that are more than one zero byte.
							run_start = None;
							continue;
						}

						if longest_run == (None, None) {
							// If we don't have a run yet, store it.
							longest_run = (Some(start), Some(i));
						} else if let (Some(start), Some(end)) = longest_run {
							// Otherwise, only store it if it's longer than a previous one.
							if end - start < run_length {
								longest_run = (Some(start), Some(i));
							}
						}

						run_start = None;
					}
				}

				let run_start = longest_run.0.unwrap_or(18); // 18 is arbitrary here, so long as it's >= 16
				let run_end = longest_run.1.unwrap_or(18);

				for (i, &byte) in bytes.iter().enumerate() {
					if i > run_start && i < run_end {
						continue;
					} else if i == run_start {
						f.write_str("::")?;
					} else {
						if run_end == i && i % 2 == 1 {
							// This handles the case where the first byte of a pair is in the run.
							// In that instance, we don't both padding. e.g. `0x00 0x01` where the 0x00 is in a run - output that as `::1`.
							f.write_fmt(format_args!("{:x}", byte))?;
						} else {
							// Otherwise, if we're not in a run then print out the byte as hex, padded to two hex-gits.
							f.write_fmt(format_args!("{:02x}", byte))?;
						}

						// Print a `:` every two pairs of bytes.
						if i != bytes.len() - 1 && i % 2 == 1 && i + 1 != run_start {
							f.write_char(':')?;
						}
					}
				}

				Ok(())
			} // TODO: Handle shortening here (i.e. replace runs of 0s with ::)
		}
	}
}

int_enum! {
	enum AttributeType: u16 {
		Address = 1,
		Local = 2,
		Label = 3,
		Broadcast = 4,
		Anycast = 5,
		CacheInfo = 6,
		Multicast = 7,
		Flags = 8,
		RoutePriority = 9,
		TargetNewNetNamespaceID = 10,
		Protocol = 11,
		Unknown = 9999,
	}
}

#[derive(Debug, Default)]
pub struct AddressAttributes {
	pub address: Option<IPAddress>,
	pub local_address: Option<IPAddress>,
	pub label: Option<String>,
	pub broadcast_address: Option<IPAddress>,
	pub anycast_address: Option<IPAddress>,
	pub cache_info: Option<CacheInfo>,
	pub multicast: Option<IPAddress>,
	pub flags: Option<AddressFlags>,
	pub priority: Option<u32>,
	pub new_net_namespace_id: Option<u32>,
	pub protocol: Option<AddressProtocol>,
	pub unknown: Vec<(u16, Vec<u8>)>,
}

impl ReadFromWithEndian for AddressAttributes {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut attributes = Self::default();
		loop {
			match attributes.read_attribute(source, endian) {
				Ok(_) => {}
				Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
				Err(e) => return Err(e),
			}
		}
		Ok(attributes)
	}
}

impl WriteToWithEndian for AddressAttributes {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		write_attribute(target, endian, AttributeType::Address, &self.address)?;
		write_attribute(target, endian, AttributeType::Local, &self.local_address)?;
		write_attribute(
			target,
			endian,
			AttributeType::Label,
			&self.label.clone().map(NullTerminatedString::<0>),
		)?;
		write_attribute(target, endian, AttributeType::Broadcast, &self.broadcast_address)?;
		write_attribute(target, endian, AttributeType::Anycast, &self.anycast_address)?;
		write_attribute(target, endian, AttributeType::Multicast, &self.multicast)?;
		write_attribute(target, endian, AttributeType::Flags, &self.flags)?;
		write_attribute(target, endian, AttributeType::RoutePriority, &self.priority)?;
		write_attribute(target, endian, AttributeType::Protocol, &self.protocol)?;

		Ok(())
	}
}

impl AddressAttributes {
	pub(crate) fn read_attribute<T: Read>(&mut self, source: &mut T, endian: Endian) -> io::Result<()> {
		let (attr_type, data_buffer) = read_attribute(source, endian)?;

		match AttributeType::try_from(attr_type).unwrap_or(AttributeType::Unknown) {
			AttributeType::Address => self.address = Some(IPAddress::new(&data_buffer)?),
			AttributeType::Local => self.local_address = Some(IPAddress::new(&data_buffer)?),
			AttributeType::Label => self.label = Some(new_string(&data_buffer)?),
			AttributeType::Broadcast => self.broadcast_address = Some(IPAddress::new(&data_buffer)?),
			AttributeType::Anycast => self.anycast_address = Some(IPAddress::new(&data_buffer)?),
			AttributeType::CacheInfo => {
				self.cache_info = Some(CacheInfo::read_from_with_endian(
					&mut Cursor::new(&data_buffer),
					endian,
				)?)
			}
			AttributeType::Multicast => self.multicast = Some(IPAddress::new(&data_buffer)?),
			AttributeType::Flags => {
				self.flags = Some(AddressFlags::read_from_with_endian(
					&mut Cursor::new(&data_buffer),
					endian,
				)?)
			}
			AttributeType::RoutePriority => self.priority = Some(new_u32(&data_buffer)?),
			AttributeType::TargetNewNetNamespaceID => self.new_net_namespace_id = Some(new_u32(&data_buffer)?),
			AttributeType::Protocol => {
				self.protocol = Some(AddressProtocol::read_from_with_endian(
					&mut Cursor::new(&data_buffer),
					endian,
				)?)
			}
			_ => self.unknown.push((attr_type, data_buffer)),
		}

		Ok(())
	}
}

#[derive(Debug, ByteStruct)]
pub struct CacheInfo {
	preferred: u32,
	valid: u32,
	created_time: u32,
	updated_time: u32,
}

int_enum! {
	#[derive(Debug)]
	pub enum AddressProtocol: u8 {
		Loopback = 1,
		RouterAnnouncement = 2,
		LinkLocal = 3,
	}
}
