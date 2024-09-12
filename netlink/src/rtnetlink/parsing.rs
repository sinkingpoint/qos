use std::io::{self, Read};

use bytestruct::{Endian, ReadFromWithEndian};

use super::address::MacAddress;

pub(crate) fn new_mac_address(buffer: &[u8]) -> io::Result<MacAddress> {
	Ok(MacAddress::new(buffer.try_into().map_err(|e| {
		io::Error::new(io::ErrorKind::InvalidData, format!("expected 6 bytes, got {:?}", e))
	})?))
}

pub(crate) fn new_string(buffer: &[u8]) -> io::Result<String> {
	Ok(std::str::from_utf8(buffer)
		.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
		.to_owned())
}

pub(crate) fn new_u32(buffer: &[u8]) -> io::Result<u32> {
	Ok(u32::from_le_bytes(buffer.try_into().map_err(|e| {
		io::Error::new(io::ErrorKind::InvalidData, format!("expected 4 bytes, got {:?}", e))
	})?))
}

pub(crate) fn read_attribute<T: Read>(source: &mut T, endian: Endian) -> io::Result<(u16, Vec<u8>)> {
	const SIZE: usize = 4;
	const ALIGN_TO: usize = 4;
	let length = u16::read_from_with_endian(source, endian)? as usize;
	let attr_type = u16::read_from_with_endian(source, endian)?;
	let padding_length = ((length + ALIGN_TO - 1) & !(ALIGN_TO - 1)) - length;

	let mut data_buffer = vec![0; length - SIZE];
	source.read_exact(&mut data_buffer)?;

	let mut _padding_buffer = vec![0; padding_length];
	source.read_exact(&mut _padding_buffer)?;

	Ok((attr_type, data_buffer))
}
