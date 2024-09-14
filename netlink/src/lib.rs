#![feature(buf_read_has_data_left)]

#[cfg(feature = "async")]
mod async_socket;
#[cfg(feature = "async")]
pub use async_socket::*;

mod error;
pub use error::*;

pub mod rtnetlink;

use std::{
	io::{self, BufReader, Cursor, ErrorKind, Read, Write},
	marker::PhantomData,
	os::fd::{AsRawFd, OwnedFd},
	sync::Mutex,
};

use bitflags::{bitflags, Flags};
use bytestruct::{int_enum, Endian, ReadFromWithEndian, Size, WriteToWithEndian};
use bytestruct_derive::{ByteStruct, Size};
use nix::{
	libc::{setsockopt, NETLINK_EXT_ACK, SOL_NETLINK},
	sys::socket::{self, AddressFamily, NetlinkAddr, SockFlag, SockProtocol, SockType},
	unistd::{getpid, write},
};

use common::{io::RawFdReader, rand::rand_u32};

/// A socket for communicating with the kernel over Netlink.
pub struct NetlinkSocket<T: NetlinkSockType> {
	socket_fd: OwnedFd,

	/// A BufReader over the socket connection.
	reader: Mutex<BufReader<RawFdReader>>,

	_phantom: PhantomData<T>,
}

impl<T: NetlinkSockType> NetlinkSocket<T> {
	/// Creates a new Netlink socket with the specified multicast groups.
	pub fn new(groups: T::SockGroups) -> io::Result<Self> {
		let socket_fd = socket::socket(
			AddressFamily::Netlink,
			SockType::Raw,
			SockFlag::empty(),
			T::SOCK_PROTOCOL,
		)?;

		let address = NetlinkAddr::new(getpid().as_raw() as u32, groups.bits());

		socket::bind(socket_fd.as_raw_fd(), &address)?;

		// Set NETLINK_EXT_ACK, which gives us nicer error messages on failures.
		let errcode = unsafe {
			let val: u32 = 1;
			setsockopt(
				socket_fd.as_raw_fd(),
				SOL_NETLINK,
				NETLINK_EXT_ACK,
				(&val as *const u32).cast(),
				4,
			)
		};

		if errcode != 0 {
			return Err(io::Error::new(
				ErrorKind::Other,
				format!("failed to set NETLINK_EXT_ACK: {}", errcode),
			));
		}

		Ok(Self {
			// We have to use a BufReader here because Linux is very silly. Even though we _request_ a SOCK_RAW
			// socket,
			reader: Mutex::new(BufReader::new(RawFdReader::new(socket_fd.as_raw_fd()))),
			socket_fd,
			_phantom: PhantomData,
		})
	}

	pub fn write_netlink_message<M: WriteToWithEndian>(
		&self,
		mut header: NetlinkMessageHeader<T>,
		msg: M,
	) -> io::Result<usize> {
		let mut body = Vec::new();
		msg.write_to_with_endian(&mut body, bytestruct::Endian::Little)?;

		header.length = (header.size() + body.len()) as u32;
		let mut buf = Vec::new();
		header.write_to_with_endian(&mut buf, bytestruct::Endian::Little)?;
		buf.extend(body);

		self.uwrite(&buf)
	}

	pub fn read_netlink_message(&self) -> io::Result<(NetlinkMessageHeader<T>, Vec<u8>)> {
		let mut header = [0; 16];
		let n = self.uread(&mut header)?;
		if n != 16 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid read for header"));
		}

		let header =
			NetlinkMessageHeader::read_from_with_endian(&mut Cursor::new(&header), bytestruct::Endian::Little)?;
		let mut body = vec![0; header.length as usize - header.size()];
		if self.uread(&mut body)? != body.len() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid read for body"));
		}

		Ok((header, body))
	}

	fn uread(&self, buf: &mut [u8]) -> io::Result<usize> {
		let mut reader = self.reader.lock().unwrap();
		reader.read(buf)
	}

	fn uwrite(&self, buf: &[u8]) -> io::Result<usize> {
		write(self.as_raw_fd(), buf).map_err(io::Error::from)
	}
}

impl<T: NetlinkSockType> Read for NetlinkSocket<T> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.uread(buf)
	}
}

impl<T: NetlinkSockType> Write for NetlinkSocket<T> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.uwrite(buf)
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}

impl<T: NetlinkSockType> AsRawFd for NetlinkSocket<T> {
	fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
		self.socket_fd.as_raw_fd()
	}
}

/// The Netlink socket type for receiving kernel uevents.
pub struct NetlinkKObjectUEvent;

bitflags! {
	pub struct UEventNetlinkGroups: u32 {
		const None = 0;
	}
}

impl NetlinkSockType for NetlinkKObjectUEvent {
	const SOCK_PROTOCOL: SockProtocol = SockProtocol::NetlinkKObjectUEvent;
	type SockGroups = UEventNetlinkGroups;
	type MessageType = BaseNetlinkMessageType;
}

/// A trait for types that can be used as the message type for a Netlink socket.
pub trait NetlinkSockType {
	const SOCK_PROTOCOL: SockProtocol;

	type SockGroups: Flags<Bits = u32>;
	type MessageType: ReadFromWithEndian + WriteToWithEndian + Size + std::fmt::Debug;
}

#[derive(Debug, ByteStruct, Size)]
pub struct NetlinkMessageHeader<T: NetlinkSockType> {
	pub length: u32,
	pub message_type: T::MessageType,
	pub flags: NetlinkFlags,
	pub sequence_number: u32,
	pub pid: u32,
}

impl<T: NetlinkSockType> NetlinkMessageHeader<T> {
	fn new(message_type: T::MessageType, flags: NetlinkFlags) -> NetlinkMessageHeader<T> {
		Self {
			length: 0,
			message_type,
			flags,
			sequence_number: rand_u32().expect("random sequence number"),
			pid: getpid().as_raw() as u32,
		}
	}
}

bitflags! {
	/// Flags for Netlink messages.
	#[derive(Debug)]
	pub struct NetlinkFlags: u16 {
		const NLM_F_REQUEST = 0x1;
		const NLM_F_MULTI = 0x2;
		const NLM_F_ACK = 0x4;
		const NLM_F_ECHO = 0x8;
		const NLM_F_DUMP_INTR = 0x10;

		// Modifiers to GET request */
		const NLM_F_ROOT = 0x100;
		const NLM_F_MATCH = 0x200;
		const NLM_F_ATOMIC = 0x400;
		const NLM_F_DUMP = (Self::NLM_F_ROOT.bits() | Self::NLM_F_MATCH.bits());

		// Modifiers to NEW request
		const NLM_F_REPLACE = 0x100;
		const NLM_F_EXCL = 0x200;
		const NLM_F_CREATE = 0x400;
		const NLM_F_APPEND = 0x800;
	}
}

impl WriteToWithEndian for NetlinkFlags {
	fn write_to_with_endian<T: std::io::Write>(
		&self,
		writer: &mut T,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		self.bits().write_to_with_endian(writer, endian)
	}
}

impl ReadFromWithEndian for NetlinkFlags {
	fn read_from_with_endian<T: std::io::Read>(reader: &mut T, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let bits = u16::read_from_with_endian(reader, endian)?;
		Ok(Self::from_bits(bits).unwrap())
	}
}

impl Size for NetlinkFlags {
	fn size(&self) -> usize {
		2
	}
}

int_enum! {
	/// The available base message types which are common to all Netlink sockets.
	#[derive(Debug)]
	pub enum BaseNetlinkMessageType: u16 {
		NoOp = 0x1,
		Error = 0x2,
		Done = 0x3,
		Overrun = 0x4,
	}
}

bitflags! {
	/// The available MultiCast groups for Netlink sockets.
	pub struct NetLinkGroups: u32 {
			const RTMGRP_LINK = 0x1;
			const RTMGRP_NOTIFY = 0x2;
			const RTMGRP_NEIGH = 0x4;
			const RTMGRP_TC = 0x8;
			const RTMGRP_IPV4_IFADDR = 0x10;
			const RTMGRP_IPV4_MROUTE = 0x20;
			const RTMGRP_IPV4_ROUTE = 0x40;
			const RTMGRP_IPV4_RULE = 0x80;
			const RTMGRP_IPV6_IFADDR = 0x100;
			const RTMGRP_IPV6_MROUTE = 0x200;
			const RTMGRP_IPV6_ROUTE = 0x400;
			const RTMGRP_IPV6_IFINFO = 0x800;
			const RTMGRP_DECNET_IFADDR = 0x1000;
			const RTMGRP_DECNET_ROUTE = 0x4000;
			const RTMGRP_IPV6_PREFIX = 0x20000;
	}
}

const ATTRIBUTE_SIZE: usize = 4;
const ATTRIBUTE_ALIGN_TO: usize = 4;

pub(crate) fn read_attribute<T: Read>(source: &mut T, endian: Endian) -> io::Result<(u16, Vec<u8>)> {
	let length = u16::read_from_with_endian(source, endian)? as usize;
	if length < ATTRIBUTE_SIZE {
		return Err(io::Error::new(
			ErrorKind::InvalidInput,
			format!(
				"error reading attribute, got length {} which is less than {}",
				length, ATTRIBUTE_SIZE
			),
		));
	}

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

pub(crate) fn new_string(buffer: &[u8]) -> io::Result<String> {
	Ok(std::str::from_utf8(&buffer[0..buffer.len() - 1])
		.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
		.to_owned())
}

pub(crate) fn new_u32(buffer: &[u8]) -> io::Result<u32> {
	Ok(u32::from_le_bytes(buffer.try_into().map_err(|e| {
		io::Error::new(io::ErrorKind::InvalidData, format!("expected 4 bytes, got {:?}", e))
	})?))
}
