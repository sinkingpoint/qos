use bitflags::bitflags;
use bytestruct_derive::{ByteStruct, Size};
use std::{
	fmt::Display,
	io::{self, ErrorKind, Read, Write},
};

use bytestruct::{int_enum, ReadFromWithEndian, Size, WriteToWithEndian};

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
	family: AddressFamily,
	prefix_length: u8,
	flags: AddressFlags,
	scope: AddressScope,
	index: u32,
}

impl InterfaceAddressMessage {
	pub fn empty() -> InterfaceAddressMessage {
		InterfaceAddressMessage {
			family: AddressFamily::Unspecified,
			prefix_length: 0,
			flags: AddressFlags::empty(),
			scope: AddressScope::Universe,
			index: 0,
		}
	}
}

pub enum IPAddress {
	IPv4([u8; 4]),
	IPv6([u8; 16]),
}

impl IPAddress {
	pub fn new(bytes: &[u8]) -> io::Result<Self> {
		match bytes.len() {
			4 => Ok(IPAddress::IPv4(bytes.try_into().unwrap())),
			8 => Ok(IPAddress::IPv6(bytes.try_into().unwrap())),
			_ => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid IP address length: {}", bytes.len()),
			)),
		}
	}
}

impl Display for IPAddress {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::IPv4(bytes) => f.write_fmt(format_args!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])),
			Self::IPv6(bytes) => f.write_fmt(
				format_args!(
					"{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}", 
					bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], 
					bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
				)
			), // TODO: Handle shortening here (i.e. replace runs of 0s with ::)
		}
	}
}

