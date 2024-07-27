#[cfg(feature = "async")]
mod async_socket;
#[cfg(feature = "async")]
pub use async_socket::*;

use std::{
	io::{self, Read, Write},
	marker::PhantomData,
	os::fd::{AsRawFd, OwnedFd},
};

use bitflags::bitflags;
use bytestruct::{int_enum, ReadFromWithEndian, WriteToWithEndian};
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

	fn recv(&self, buf: &mut [u8], flags: MsgFlags) -> io::Result<usize> {
		recv(self.socket_fd.as_raw_fd(), buf, flags).map_err(io::Error::from)
	}

	fn send(&self, buf: &[u8], flags: MsgFlags) -> io::Result<usize> {
		send(self.socket_fd.as_raw_fd(), buf, flags).map_err(io::Error::from)
	}
}

impl<T: NetlinkSockType> Read for NetlinkSocket<T> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.recv(buf, MsgFlags::empty())
	}
}

impl<T: NetlinkSockType> Write for NetlinkSocket<T> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		self.send(buf, MsgFlags::empty())
	}

	fn flush(&mut self) -> std::io::Result<()> {
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

impl NetlinkSockType for NetlinkKObjectUEvent {
	const SOCK_PROTOCOL: SockProtocol = SockProtocol::NetlinkKObjectUEvent;
	type MessageType = BaseNetlinkMessageType;
}

/// The Netlink socket type for sending and receiving route information.
pub struct NetlinkRoute;

impl NetlinkSockType for NetlinkRoute {
	const SOCK_PROTOCOL: SockProtocol = SockProtocol::NetlinkRoute;
	type MessageType = BaseNetlinkMessageType;
}

/// A trait for types that can be used as the message type for a Netlink socket.
pub trait NetlinkSockType {
	const SOCK_PROTOCOL: SockProtocol;
	type MessageType: ReadFromWithEndian + WriteToWithEndian;
}

#[derive(ByteStruct)]
pub struct NetlinkMessageHeader<T: NetlinkSockType> {
	pub length: u32,
	pub message_type: T::MessageType,
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

int_enum! {
	/// The available base message types which are common to all Netlink sockets.
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
