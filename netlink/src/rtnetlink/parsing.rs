use std::io::{self, Read, Write};

use bytestruct::{Endian, ReadFromWithEndian, WriteToWithEndian};

use super::address::MacAddress;

const ATTRIBUTE_SIZE: usize = 4;
const ATTRIBUTE_ALIGN_TO: usize = 4;

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
	let length = u16::read_from_with_endian(source, endian)? as usize;
	let attr_type = u16::read_from_with_endian(source, endian)?;
	let padding_length = ((length + ATTRIBUTE_ALIGN_TO - 1) & !(ATTRIBUTE_ALIGN_TO - 1)) - length;

	let mut data_buffer = vec![0; length - ATTRIBUTE_SIZE];
	source.read_exact(&mut data_buffer)?;

	let mut _padding_buffer = vec![0; padding_length];
	source.read_exact(&mut _padding_buffer)?;

	Ok((attr_type, data_buffer))
}

pub(crate) fn write_attribute<W: Write, T: Into<u16>, D: WriteToWithEndian>(
	dest: &mut W,
	endian: Endian,
	ty: T,
	data: &Option<D>,
) -> io::Result<()> {
	if data.is_none() {
		return Ok(());
	}

	let data = data.as_ref().unwrap();
	let mut data_bytes = Vec::new();
	data.write_to_with_endian(&mut data_bytes, endian)?;

	let length = ATTRIBUTE_SIZE + data_bytes.len();
	let padding_length = ((length + ATTRIBUTE_ALIGN_TO - 1) & !(ATTRIBUTE_ALIGN_TO - 1)) - length;

	let mut output = Vec::new();
	(length as u16).write_to_with_endian(&mut output, endian)?;
	ty.into().write_to_with_endian(&mut output, endian)?;
	output.extend(data_bytes);
	output.extend(vec![0_u8; padding_length]);

	dest.write_all(&output)?;

	Ok(())
}
