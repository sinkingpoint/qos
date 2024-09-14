use std::{
	fmt::Debug,
	io::{self, Cursor, ErrorKind, Read},
};

use bytestruct::{int_enum, Endian, ReadFromWithEndian, Size};
use nix::errno::Errno;
use thiserror::Error;

use crate::{new_string, new_u32, read_attribute, NetlinkMessageHeader, NetlinkSockType};

#[derive(Error, Debug)]
pub enum NetlinkError<T: NetlinkSockType, M: ReadFromWithEndian> {
	#[error("IOError Reading Response: {0}")]
	IOError(#[from] io::Error),

	#[error("Netlink Error ({0}): {1}")]
	NetlinkError(Errno, NetlinkErrorContents<T, M>),
}

pub type NetlinkResult<T, M> = Result<(), NetlinkError<T, M>>;

pub fn read_netlink_result<T: NetlinkSockType, M: ReadFromWithEndian, R: Read>(
	source: &mut R,
	endian: bytestruct::Endian,
) -> NetlinkResult<T, M> {
	let errno = i32::read_from_with_endian(source, endian)?.abs();
	if errno == 0 {
		return Ok(());
	}

	Err(NetlinkError::NetlinkError(
		Errno::from_i32(errno),
		NetlinkErrorContents::read_from_with_endian(source, endian)?,
	))
}

#[derive(Debug)]
pub struct NetlinkErrorContents<T: NetlinkSockType, M: ReadFromWithEndian> {
	pub header: NetlinkMessageHeader<T>,
	pub msg: M,
	pub affected_attribute: Option<(u16, Vec<u8>)>,
	pub attributes: ErrorAttributes,
}

impl<T: NetlinkSockType, M: ReadFromWithEndian> ReadFromWithEndian for NetlinkErrorContents<T, M> {
	fn read_from_with_endian<R: Read>(source: &mut R, endian: bytestruct::Endian) -> io::Result<Self> {
		let header = NetlinkMessageHeader::read_from_with_endian(source, endian)?;

		let mut internal_body = vec![0; header.length as usize - header.size()];
		source.read_exact(&mut internal_body)?;
		let msg = M::read_from_with_endian(&mut Cursor::new(&internal_body), endian)?;

		let attributes = ErrorAttributes::read_from_with_endian(source, endian)?;

		let affected_attribute = if let Some(offset) = attributes.attribute_offset {
			let offset = offset as usize - header.size();
			Some(read_attribute(&mut Cursor::new(&internal_body[offset..]), endian)?)
		} else {
			None
		};

		Ok(Self {
			header,
			msg,
			affected_attribute,
			attributes,
		})
	}
}

int_enum! {
	enum AttributeType: u16 {
	Msg = 1,
	AttributeOffset = 2,
	Cookie = 3,
	Policy = 4,
	MissType = 5,
	MissNest = 6,
  Unknown = 9999,
  }
}

#[derive(Debug, Default)]
pub struct ErrorAttributes {
	pub msg: Option<String>,
	pub attribute_offset: Option<u32>,
	pub unknowns: Vec<(u16, Vec<u8>)>,
}

impl ReadFromWithEndian for ErrorAttributes {
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

impl ErrorAttributes {
	pub(crate) fn read_attribute<T: Read>(&mut self, source: &mut T, endian: Endian) -> io::Result<()> {
		let (attr_type, data_buffer) = read_attribute(source, endian)?;
		match AttributeType::try_from(attr_type).unwrap_or(AttributeType::Unknown) {
			AttributeType::Msg => self.msg = Some(new_string(&data_buffer)?),
			AttributeType::AttributeOffset => self.attribute_offset = Some(new_u32(&data_buffer)?),
			_ => self.unknowns.push((attr_type, data_buffer)),
		}

		Ok(())
	}
}
