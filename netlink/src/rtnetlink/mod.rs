mod address;
mod interface;
mod parsing;
mod route;

pub use address::*;
use bitflags::bitflags;
use bytestruct_derive::ByteStruct;
pub use interface::*;
pub use route::*;

use std::{
	io::{self, Cursor, ErrorKind},
	sync::Arc,
};

use bytestruct::{int_enum, Endian, ReadFromWithEndian};
use nix::sys::socket::SockProtocol;

use crate::{
	read_netlink_result, NetlinkError, NetlinkFlags, NetlinkMessageHeader, NetlinkResult, NetlinkSockType,
	NetlinkSocket,
};

/// The Netlink socket type for sending and receiving route information.
#[derive(Debug)]
pub struct NetlinkRoute;

impl NetlinkSockType for NetlinkRoute {
	const SOCK_PROTOCOL: SockProtocol = SockProtocol::NetlinkRoute;
	type SockGroups = RTNetlinkGroups;

	type MessageType = RTNetlinkMessageType;
}

int_enum! {
	#[derive(Debug, PartialEq)]
	pub enum RTNetlinkMessageType: u16 {
		NoOp = 0x1,
		Error = 0x2,
		Done = 0x3,
		Overrun = 0x4,

		NewLink = 16,
		DeleteLink = 17,
		GetLink = 18,
		SetLink = 19,

		NewAddress = 20,
		DeleteAddress = 21,
		GetAddress = 22,

		NewRoute = 24,
		DeleteRoute = 25,
		GetRoute = 26,

		NewNeighbor = 28,
		DeleteNeighbor = 29,
		GetNeighbor = 30,

		NewRule = 32,
		DeleteRule = 33,
		GetRule = 34,

		NewQDisc = 36,
		DeleteQDisc = 37,
		GetQDisc = 38,

		NewTrafficClass = 40,
		DeleteTrafficClass = 41,
		GetTrafficClass = 42,

		NewTrafficFilter = 44,
		DeleteTrafficFilter = 45,
		GetTrafficFilter = 46,

		NewAction = 48,
		DeletAction = 49,
		GetAction = 50,

		NewPrefix = 52,
		GetMulticast = 58,
		GetAnycast = 62,
		NewNeighborTable = 64,
		GetNeighborTable = 66,
		SetNeighborTable = 67,
	}
}

bitflags! {
	pub struct RTNetlinkGroups: u32 {
		const RTMGRP_NONE = 0;
		const RTMGRP_LINK = 1;
		const RTMGRP_NOTIFY = 2;
		const RTMGRP_NEIGH = 4;
		const RTMGRP_TC = 8;
		const RTMGRP_IPV4_IFADDR = 0x10;
		const RTMGRP_IPV4_MROUTE = 0x20;
		const RTMGRP_IPV4_ROUTE = 0x40;
		const RTMGRP_IPV4_RULE = 0x80;
		const RTMGRP_IPV6_IFADDR = 0x100;
		const RTMGRP_IPV6_MROUTE = 0x200;
		const RTMGRP_IPV6_ROUTE = 0x400;
		const RTMGRP_IPV6_IFINFO = 0x800;
		const RTMGRP_DECnet_IFADDR = 0x1000;
		const RTMGRP_DECnet_ROUTE = 0x4000;
		const RTMGRP_IPV6_PREFIX = 0x20000;
	}
}

#[derive(Debug, ByteStruct, Clone)]
pub struct Interface {
	pub family: u16,
	pub ty: InterfaceType,
	pub index: u32,
	pub flags: InterfaceFlags,
	pub change: u32,
	pub attributes: InterfaceAttributes,
}

impl Interface {
	pub fn parse(body: Vec<u8>) -> io::Result<Self> {
		Self::read_from_with_endian(&mut Cursor::new(body), Endian::Little)
	}
}

#[derive(Debug, ByteStruct)]
pub struct Address {
	pub family: AddressFamily,
	pub prefix_length: u8,
	pub flags: AddressFlags,
	pub scope: AddressScope,
	pub interface_index: u32,
	pub attributes: AddressAttributes,
}

impl Address {
	pub fn new(address: IPAddress, prefix_length: u8, interface_index: u32) -> Self {
		let family = match address {
			IPAddress::IPv4(_) => AddressFamily::IPv4,
			IPAddress::IPv6(_) => AddressFamily::IPv6,
		};

		let flags = AddressFlags::empty();
		let scope = if address.is_host() {
			AddressScope::Host
		} else if address.is_link_local() {
			AddressScope::Link
		} else {
			AddressScope::Universe
		};

		let mut attributes = AddressAttributes::default();
		if address.is_host() || address.is_local() {
			attributes.local_address = Some(address);
		} else {
			attributes.address = Some(address);
		};

		Self {
			family,
			prefix_length,
			flags,
			scope,
			interface_index,
			attributes,
		}
	}
}

#[derive(Debug, ByteStruct)]
pub struct Route {
	pub family: AddressFamily,
	pub dst_length: u8,
	pub src_length: u8,
	pub tos: u8,

	pub table_id: u8,
	pub protocol: RouteProtocol,
	pub scope: RouteScope,
	pub ty: RouteTable,

	pub flags: RouteTableFlags,
	pub attributes: RouteAttributes,
}

pub trait RTNetlink {
	// Get all the links on the system.
	fn get_links(self: &Arc<Self>) -> io::Result<Vec<Interface>>;

	// Create, or update a link on the system.
	fn new_link(self: &Arc<Self>, i: Interface) -> NetlinkResult<NetlinkRoute, Interface>;

	// Get all the addresses on all the links of the system.
	fn get_addrs(self: &Arc<Self>) -> io::Result<Vec<Address>>;

	// Create a new address on the system.
	fn new_address(self: &Arc<Self>, a: Address) -> NetlinkResult<NetlinkRoute, Address>;

	// Create a new route on the system.
	fn new_route(self: &Arc<Self>, route: Route) -> NetlinkResult<NetlinkRoute, Route>;

	// Get all the routes on the system.
	fn get_route(self: &Arc<Self>) -> io::Result<Vec<Route>>;
}

impl RTNetlink for NetlinkSocket<NetlinkRoute> {
	fn get_links(self: &Arc<Self>) -> io::Result<Vec<Interface>> {
		let header = NetlinkMessageHeader::<NetlinkRoute>::new(
			RTNetlinkMessageType::GetLink,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_MATCH | NetlinkFlags::NLM_F_EXCL,
		);
		let msg = InterfaceInfoMessage::empty();

		let read_handle = self.write_netlink_message(header, msg)?;

		let mut interfaces = Vec::new();
		while let Some((header, body)) = read_handle.read() {
			if matches!(header.message_type, RTNetlinkMessageType::Done) {
				break;
			}

			let mut cursor = Cursor::new(&body);
			let interface = Interface::read_from_with_endian(&mut cursor, bytestruct::Endian::Little)?;

			interfaces.push(interface);
		}

		Ok(interfaces)
	}

	fn new_link(self: &Arc<Self>, i: Interface) -> NetlinkResult<NetlinkRoute, Interface> {
		let header = NetlinkMessageHeader::new(
			RTNetlinkMessageType::NewLink,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_ACK,
		);

		let read_handle = self.write_netlink_message(header, i)?;

		if let Some((header, msg)) = read_handle.read() {
			if header.message_type != RTNetlinkMessageType::Error {
				return Err(NetlinkError::IOError(io::Error::new(
					ErrorKind::InvalidData,
					format!("invalid message header in response: {:?}", header.message_type),
				)));
			}

			let mut msg = Cursor::new(msg);

			read_netlink_result(&mut msg, bytestruct::Endian::Little)
		} else {
			Err(NetlinkError::IOError(io::Error::new(
				ErrorKind::UnexpectedEof,
				"failed to resp response",
			)))
		}
	}

	fn get_addrs(self: &Arc<Self>) -> io::Result<Vec<Address>> {
		let header = NetlinkMessageHeader::<NetlinkRoute>::new(
			RTNetlinkMessageType::GetAddress,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_MATCH | NetlinkFlags::NLM_F_EXCL,
		);

		let msg = InterfaceAddressMessage::empty();

		let read_handle = self.write_netlink_message(header, msg)?;

		let mut addresses = Vec::new();

		while let Some((header, body)) = read_handle.read() {
			if matches!(header.message_type, RTNetlinkMessageType::Done) {
				break;
			}

			let address = Address::read_from_with_endian(&mut Cursor::new(&body), bytestruct::Endian::Little)?;

			addresses.push(address);
		}

		Ok(addresses)
	}

	fn new_address(self: &Arc<Self>, addr: Address) -> NetlinkResult<NetlinkRoute, Address> {
		let header = NetlinkMessageHeader::new(
			RTNetlinkMessageType::NewAddress,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_ACK,
		);

		let read_handle = self.write_netlink_message(header, addr)?;

		if let Some((header, msg)) = read_handle.read() {
			if header.message_type != RTNetlinkMessageType::Error {
				return Err(NetlinkError::IOError(io::Error::new(
					ErrorKind::InvalidData,
					format!("invalid message header in response: {:?}", header.message_type),
				)));
			}

			let mut msg = Cursor::new(msg);

			read_netlink_result(&mut msg, bytestruct::Endian::Little)
		} else {
			Err(NetlinkError::IOError(io::Error::new(
				ErrorKind::UnexpectedEof,
				"failed to resp response",
			)))
		}
	}

	fn new_route(self: &Arc<Self>, route: Route) -> NetlinkResult<NetlinkRoute, Route> {
		let header = NetlinkMessageHeader::new(
			RTNetlinkMessageType::NewRoute,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_ACK,
		);

		let read_handle = self.write_netlink_message(header, route)?;
		if let Some((header, msg)) = read_handle.read() {
			if header.message_type != RTNetlinkMessageType::Error {
				return Err(NetlinkError::IOError(io::Error::new(
					ErrorKind::InvalidData,
					format!("invalid message header in response: {:?}", header.message_type),
				)));
			}

			let mut msg = Cursor::new(msg);

			read_netlink_result(&mut msg, bytestruct::Endian::Little)
		} else {
			Err(NetlinkError::IOError(io::Error::new(
				ErrorKind::UnexpectedEof,
				"failed to resp response",
			)))
		}
	}

	fn get_route(self: &Arc<Self>) -> io::Result<Vec<Route>> {
		let header = NetlinkMessageHeader::<NetlinkRoute>::new(
			RTNetlinkMessageType::GetRoute,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_MATCH | NetlinkFlags::NLM_F_EXCL,
		);
		let msg = RouteMessage::empty();

		let read_handle = self.write_netlink_message(header, msg)?;

		let mut routes = Vec::new();
		while let Some((header, body)) = read_handle.read() {
			if matches!(header.message_type, RTNetlinkMessageType::Done) {
				break;
			}

			let mut cursor = Cursor::new(&body);
			let route = Route::read_from_with_endian(&mut cursor, bytestruct::Endian::Little)?;

			routes.push(route);
		}

		Ok(routes)
	}
}
