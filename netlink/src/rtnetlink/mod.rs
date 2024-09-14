mod address;
mod interface;
mod parsing;

use bytestruct_derive::ByteStruct;
pub use interface::*;

use std::io::{self, Cursor, ErrorKind};

use address::{AddressAttributes, AddressFamily, AddressFlags, AddressScope, InterfaceAddressMessage};
use bytestruct::{int_enum, ReadFromWithEndian};
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

#[derive(Debug, ByteStruct)]
pub struct Interface {
	pub family: u16,
	pub ty: InterfaceType,
	pub index: i32,
	pub flags: InterfaceFlags,
	pub change: u32,
	pub attributes: InterfaceAttributes,
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

pub trait RTNetlink {
	fn get_links(&mut self) -> io::Result<Vec<Interface>>;
	fn new_link(&mut self, i: Interface) -> NetlinkResult<NetlinkRoute, Interface>;
	fn get_addrs(&mut self, interface_index: u32) -> io::Result<Vec<Address>>;
}

impl RTNetlink for NetlinkSocket<NetlinkRoute> {
	fn get_links(&mut self) -> io::Result<Vec<Interface>> {
		let header = NetlinkMessageHeader::<NetlinkRoute>::new(
			RTNetlinkMessageType::GetLink,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_MATCH | NetlinkFlags::NLM_F_EXCL,
		);
		let msg = InterfaceInfoMessage::empty();

		self.write_netlink_message(header, msg)?;

		let mut interfaces = Vec::new();
		loop {
			let (header, body) = self.read_netlink_message()?;
			if matches!(header.message_type, RTNetlinkMessageType::Done) {
				break;
			}

			let mut cursor = Cursor::new(&body);
			let interface = Interface::read_from_with_endian(&mut cursor, bytestruct::Endian::Little)?;

			interfaces.push(interface);
		}

		Ok(interfaces)
	}

	fn new_link(&mut self, i: Interface) -> NetlinkResult<NetlinkRoute, Interface> {
		let header = NetlinkMessageHeader::new(
			RTNetlinkMessageType::NewLink,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_ACK,
		);

		self.write_netlink_message(header, i)?;

		let (header, msg) = self.read_netlink_message()?;
		if header.message_type != RTNetlinkMessageType::Error {
			return Err(NetlinkError::IOError(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid message header in response: {:?}", header.message_type),
			)));
		}

		let mut msg = Cursor::new(msg);

		read_netlink_result(&mut msg, bytestruct::Endian::Little)
	}

	fn get_addrs(&mut self, interface_index: u32) -> io::Result<Vec<Address>> {
		let header = NetlinkMessageHeader::<NetlinkRoute>::new(
			RTNetlinkMessageType::GetAddress,
			NetlinkFlags::NLM_F_REQUEST | NetlinkFlags::NLM_F_MATCH | NetlinkFlags::NLM_F_EXCL,
		);

		let mut msg = InterfaceAddressMessage::empty();
		msg.interface_index = interface_index;

		self.write_netlink_message(header, msg)?;

		let mut addresses = Vec::new();

		loop {
			let (header, body) = self.read_netlink_message()?;
			if matches!(header.message_type, RTNetlinkMessageType::Done) {
				break;
			}

			let address = Address::read_from_with_endian(&mut Cursor::new(&body), bytestruct::Endian::Little)?;

			addresses.push(address);
		}

		Ok(addresses)
	}
}
