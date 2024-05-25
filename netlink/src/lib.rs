use std::{
	io::{self, Read, Write},
	marker::PhantomData,
	os::fd::{AsRawFd, OwnedFd},
};

use bitflags::{bitflags, Flags};
use bytestruct::{ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;
use nix::{
	sys::socket::{self, recv, send, AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType},
	unistd::getpid,
};

/// A socket for communicating with the kernel over Netlink.
pub struct NetlinkSocket<T: NetlinkSockType> {
	socket_fd: OwnedFd,

	_phantom: PhantomData<T>,
}

impl<T: NetlinkSockType> NetlinkSocket<T> {
	/// Creates a new Netlink socket with the specified multicast groups.
	pub fn new(groups: u32) -> std::io::Result<Self> {
		let socket_fd = socket::socket(
			AddressFamily::Netlink,
			SockType::Raw,
			SockFlag::empty(),
			T::SOCK_PROTOCOL,
		)?;

		let address = NetlinkAddr::new(getpid().as_raw() as u32, groups);

		socket::bind(socket_fd.as_raw_fd(), &address)?;

		Ok(Self {
			socket_fd,
			_phantom: PhantomData,
		})
	}
}

impl<T: NetlinkSockType> Read for NetlinkSocket<T> {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		recv(self.socket_fd.as_raw_fd(), buf, MsgFlags::empty()).map_err(io::Error::from)
	}
}

impl<T: NetlinkSockType> Write for NetlinkSocket<T> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		send(self.socket_fd.as_raw_fd(), buf, MsgFlags::empty()).map_err(io::Error::from)
	}

	fn flush(&mut self) -> std::io::Result<()> {
		Ok(())
	}
}

/// The Netlink socket type for receiving kernel uevents.
pub struct NetlinkKObjectUEvent;

impl NetlinkSockType for NetlinkKObjectUEvent {
	const SOCK_PROTOCOL: SockProtocol = SockProtocol::NetlinkKObjectUEvent;
	type MessageType = BaseNetlinkMessageType;
}

/// A trait for types that can be used as the message type for a Netlink socket.
pub trait NetlinkSockType {
	const SOCK_PROTOCOL: SockProtocol;
	type MessageType: Flags<Bits = u16>;
}

#[derive(ByteStruct)]
pub struct NetlinkMessageHeader<T: NetlinkSockType> {
	pub length: u32,
	pub message_type: NetlinkMessageType<T::MessageType>,
	pub flags: NetlinkFlags,
	pub sequence_number: u32,
	pub port_id: u32,
}

bitflags! {
	/// Flags for Netlink messages.
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

bitflags! {
	/// The available base message types which are common to all Netlink sockets.
	pub struct BaseNetlinkMessageType: u16 {
		const NLMSG_NOOP = 0x1;
		const NLMSG_ERROR = 0x2;
		const NLMSG_DONE = 0x3;
		const NLMSG_OVERRUN = 0x4;
	}
}

/// A Netlink message type, which can be either a base type or a custom type based
/// on the base message type for the socket.
pub enum NetlinkMessageType<T: Flags<Bits = u16>> {
	Base(BaseNetlinkMessageType),
	Other(T),
}

impl<T: Flags<Bits = u16>> WriteToWithEndian for NetlinkMessageType<T>
where
	T::Bits: WriteToWithEndian,
{
	fn write_to_with_endian<W: std::io::Write>(
		&self,
		writer: &mut W,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		match self {
			NetlinkMessageType::Base(base) => base.bits().write_to_with_endian(writer, endian),
			NetlinkMessageType::Other(other) => other.bits().write_to_with_endian(writer, endian),
		}
	}
}

impl<T: Flags<Bits = u16>> ReadFromWithEndian for NetlinkMessageType<T> {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let bits = u16::read_from_with_endian(reader, endian)?;
		if let Some(base) = BaseNetlinkMessageType::from_bits(bits) {
			Ok(NetlinkMessageType::Base(base))
		} else if let Some(other) = T::from_bits(bits) {
			Ok(NetlinkMessageType::Other(other))
		} else {
			Err(std::io::Error::new(
				std::io::ErrorKind::InvalidData,
				"Invalid NetlinkMessageType",
			))
		}
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
