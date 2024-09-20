use bitflags::bitflags;
use bytestruct_derive::{ByteStruct, Size};
use std::{
	fmt::{Display, Write as _},
	io::{self, Cursor, ErrorKind, Read, Write},
	num::ParseIntError,
};

use bytestruct::{int_enum, Endian, NullTerminatedString, ReadFromWithEndian, Size, WriteToWithEndian};

use crate::{new_string, new_u32, read_attribute, write_attribute};

#[derive(Debug, Clone, PartialEq)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
	pub fn new(bytes: [u8; 6]) -> Self {
		Self(bytes)
	}
}

impl TryFrom<&str> for MacAddress {
	type Error = String;
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		let parts: Result<Vec<u8>, ParseIntError> = value.splitn(6, ":").map(|s| u8::from_str_radix(s, 16)).collect();
		let parts = parts.map_err(|e| format!("failed to parse MAC address: {}", e))?;

		if parts.len() != 6 {
			return Err(format!("expected 6 parts for mac address, got: {:?}", parts));
		}

		Ok(MacAddress(
			parts.try_into().expect("BUG: wrong number of parts after validation"),
		))
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

#[derive(Debug, Clone, PartialEq)]
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

	// Returns true if this the first `prefix_len` bits of this address matches the same
	// bits in the given address.
	pub fn is_in_subnet(&self, address: &IPAddress, prefix_len: u8) -> bool {
		match (self, address) {
			(Self::IPv4(self_bytes), Self::IPv4(other_bytes)) => first_bits_match(self_bytes, other_bytes, prefix_len),
			(Self::IPv6(self_bytes), Self::IPv6(other_bytes)) => first_bits_match(self_bytes, other_bytes, prefix_len),
			(_, _) => false,
		}
	}

	// Return true if the address is in the "host" block, i.e. 127/8 for IPv4 or ::1/128 for IPv6.
	pub fn is_host(&self) -> bool {
		match self {
			Self::IPv4(_) => self.is_in_subnet(&IPAddress::IPv4([127, 0, 0, 0]), 8),
			Self::IPv6(_) => self.is_in_subnet(&IPAddress::IPv6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]), 128),
		}
	}

	// Returns true if the address is in the "local" blocks, i.e. 10/8, 172.16/12, or 192.168/16 for IPv4
	// or fd00::/8 for IPv4.
	pub fn is_local(&self) -> bool {
		match self {
			Self::IPv4(_) => {
				self.is_in_subnet(&IPAddress::IPv4([10, 0, 0, 0]), 8)
					|| self.is_in_subnet(&IPAddress::IPv4([172, 16, 0, 0]), 12)
					|| self.is_in_subnet(&IPAddress::IPv4([192, 168, 0, 0]), 16)
			}
			Self::IPv6(_) => {
				self.is_in_subnet(&IPAddress::IPv6([0xFD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), 8)
			}
		}
	}
}

impl TryFrom<&str> for IPAddress {
	type Error = String;
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		let try_ipv4: Vec<&str> = value.splitn(4, ".").collect();
		if try_ipv4.len() == 4 {
			let ipv4_bytes: Result<Vec<u8>, ParseIntError> = try_ipv4.into_iter().map(|s| s.parse()).collect();
			let ipv4_bytes = ipv4_bytes.map_err(|s| s.to_string())?;

			return Ok(IPAddress::IPv4(
				ipv4_bytes
					.try_into()
					.expect("BUG: Incorrect number of bytes after validation"),
			));
		}

		let mut try_ipv6: Vec<&str> = value.split(":").collect();

		if try_ipv6[0].is_empty() {
			// Special case: for `::n`, we end up with two empty bits of the array (["", "", "n"]).
			// Remove the first one so that run calculations below work properly.
			try_ipv6.remove(0);
		} else if try_ipv6[try_ipv6.len() - 1].is_empty() {
			// And similarly for n::
			try_ipv6.pop();
		}

		let mut ipv6_bytes = [0_u8; 16];
		let mut current_byte = 0_usize;
		let mut found_run = false;
		for (i, byte_str) in try_ipv6.iter().enumerate() {
			if byte_str.is_empty() && !found_run {
				found_run = true;
				current_byte = 16 - (try_ipv6.len() - i - 1) * 2;
			} else if byte_str.is_empty() {
				return Err(format!("found more than one compressed run in {:?}", try_ipv6));
			} else {
				let byte = u16::from_str_radix(byte_str, 16).map_err(|s| format!("failed to parse byte: {}", s))?;
				if current_byte % 2 == 0 {
					ipv6_bytes[current_byte] = ((byte & 0xFF00) >> 8) as u8;
					ipv6_bytes[current_byte + 1] = (byte & 0x00FF) as u8;
					current_byte += 2;
				} else {
					ipv6_bytes[current_byte] = (byte & 0xFF) as u8;
					current_byte += 1;
				}
			}
		}

		if current_byte == 16 {
			return Ok(Self::IPv6(ipv6_bytes));
		}

		Err(String::from("Invalid IPAddress"))
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

				// If we have something in the form n::, mark the run as ending at the end of the bytes.
				if let Some(r) = run_start {
					if longest_run.0.is_none() {
						longest_run = (Some(r), Some(16));
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

// Returns true if the first n bits of each of the arrays matches
fn first_bits_match(a: &[u8], b: &[u8], n: u8) -> bool {
	let total_bytes = (n / 8) as usize;
	let extra_bits = (n % 8) as usize;
	let extra_bits_mask = (2_u8.pow(extra_bits as u32) - 1).reverse_bits(); // Creates a mask where the first `extra_bits` bits are 1.
	let needed_bytes = if extra_bits == 0 { total_bytes } else { total_bytes + 1 };

	if a.len() < needed_bytes || b.len() < needed_bytes {
		return false;
	}

	let first_bytes_match = a[0..total_bytes] == b[0..total_bytes];
	let extra_bits_match = if extra_bits > 0 {
		(a[total_bytes] & extra_bits_mask) == (b[total_bytes] & extra_bits_mask)
	} else {
		true
	};

	first_bytes_match && extra_bits_match
}

#[cfg(test)]
mod test {
	use super::IPAddress;

	#[test]
	fn test_ipv4_parse_success() {
		let tests = [
			("127.0.0.1", IPAddress::IPv4([127, 0, 0, 1])),
			("172.19.0.1", IPAddress::IPv4([172, 19, 0, 1])),
			("192.168.178.79", IPAddress::IPv4([192, 168, 178, 79])),
			("172.18.0.1", IPAddress::IPv4([172, 18, 0, 1])),
		];

		for (test, expected) in tests {
			let address = IPAddress::try_from(test).unwrap();
			assert_eq!(address, expected);
			assert_eq!(address.to_string(), test);
		}
	}

	#[test]
	fn test_ipv4_parse_fail() {
		let tests = ["289.0.0.1", "172.19.0.", "198.1", "0", "ff.ff.ff.ff"];

		for test in tests {
			let err = IPAddress::try_from(test);
			assert!(err.is_err(), "{:?}", err);
		}
	}

	#[test]
	fn test_ipv6_parse_success() {
		let tests = [
			(
				"fe80::42:eeff:fed7:8c85",
				IPAddress::IPv6([
					0xFE, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0xEE, 0xFF, 0xFE, 0xD7, 0x8C, 0x85,
				]),
			),
			(
				"::1",
				IPAddress::IPv6([
					0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
				]),
			),
			(
				"0001::",
				IPAddress::IPv6([
					0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
				]),
			),
			(
				"2404:440c:1a35:5000:e772:6fdd:ed15:cb59",
				IPAddress::IPv6([
					0x24, 0x04, 0x44, 0x0C, 0x1A, 0x35, 0x50, 0x00, 0xE7, 0x72, 0x6F, 0xDD, 0xED, 0x15, 0xCB, 0x59,
				]),
			),
		];

		for (test, expected) in tests {
			let address = IPAddress::try_from(test).unwrap();
			assert_eq!(address, expected);
			assert_eq!(address.to_string(), test);
		}
	}

	#[test]
	fn test_ipv6_parse_fail() {
		let tests = ["foo", "fe::5::01", "9g::"];

		for test in tests {
			let err = IPAddress::try_from(test);
			assert!(err.is_err(), "{:?}", err);
		}
	}

	#[test]
	fn test_ipaddress_is_host() {
		let tests = [
			(IPAddress::try_from("127.0.0.1"), true),
			(IPAddress::try_from("127.255.255.255"), true),
			(IPAddress::try_from("128.0.0.0"), false),
			(IPAddress::try_from("::1"), true),
			(IPAddress::try_from("::2"), false),
		];

		for (addr, local) in tests {
			let addr = addr.unwrap();
			assert_eq!(addr.is_host(), local, "{} is host: {}", addr, local);
		}
	}

	#[test]
	fn test_ipaddress_is_local() {
		let tests = [
			(IPAddress::try_from("10.0.0.0"), true),
			(IPAddress::try_from("10.255.255.255"), true),
			(IPAddress::try_from("11.255.255.255"), false),
			(IPAddress::try_from("172.16.0.0"), true),
			(IPAddress::try_from("172.31.255.255"), true),
			(IPAddress::try_from("172.32.0.0"), false),
			(IPAddress::try_from("fd00::"), true),
			(IPAddress::try_from("::1"), false),
		];

		for (addr, local) in tests {
			let addr = addr.unwrap();
			assert_eq!(addr.is_local(), local, "{} is local: {}", addr, local);
		}
	}
}
