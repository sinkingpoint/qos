use std::io::{self, BufReader, Cursor, Read, Write};

use bitflags::bitflags;
use bytestruct::{int_enum, ReadFromWithEndian, Size, WriteToWithEndian};
use bytestruct_derive::{ByteStruct, Size};
use nix::sys::socket::SockProtocol;

use crate::{NetlinkFlags, NetlinkMessageHeader, NetlinkSockType, NetlinkSocket};

/// The Netlink socket type for sending and receiving route information.
#[derive(Debug)]
pub struct NetlinkRoute;

impl NetlinkSockType for NetlinkRoute {
	const SOCK_PROTOCOL: SockProtocol = SockProtocol::NetlinkRoute;
	type MessageType = RTNetlinkMessageType;
}

int_enum! {
	#[derive(Debug)]
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

int_enum! {
	#[derive(Debug)]
	pub enum InterfaceType: u16 {
		NetRom = 0,
		Ether  = 1,
		EEther = 2,
		AX25 = 3,
		ProNet = 4,
		Chaos = 5,
		IEEE802 = 6,
		ArcNet = 7,
		AppletLK = 8,
		Dlci = 15,
		Atm = 191,
		MetriCom = 23,
		IEEE1394 = 24,
		Eui64 = 27,
		InfiniBand = 32,
		Slip = 256,
		CSlip = 257,
		Slip6 = 258,
		CSplip6 = 259,
		Adapt = 264,
		Rose = 270,
		X25 = 271,
		HWX25 = 272,
		Can = 280,
		MCTP = 290,
		Ppp = 512,
		Cisco = 513,
		Hdlc = 513,
		LapB = 516,
		Ddcmp = 517,
		RawHDLC = 518,
		RawIP = 519,
		Tunnel = 768,
		Tunnel6 = 769,
		Frad = 770,
		Skip = 771,
		Loopback = 772,
		LocalTLK = 773,
		Fddi = 774,
		Bif = 775,
		Sit = 776,
		Ipddp = 777,
		Ipgre = 778,
		Primreg = 779,
		Hippi = 780,
		Ash = 781,
		EcoNet = 782,
		Irdb = 783,
		Fccp = 784,
		FCal = 785,
		FClp = 786,
		FCFabric = 787,
		Void =   0xFFFF,
		None =   0xFFFE,
	}
}

bitflags! {
	#[derive(Debug)]
	pub struct InterfaceFlags: u32 {
		const IFF_UP = 0x1;		/* Interface is up.  */
		const IFF_BROADCAST = 0x2;	/* Broadcast address valid.  */
		const IFF_DEBUG = 0x4;		/* Turn on debugging.  */
		const IFF_LOOPBACK = 0x8;		/* Is a loopback net.  */
		const IFF_POINTOPOINT = 0x10;	/* Interface is point-to-point link.  */
		const IFF_NOTRAILERS = 0x20;	/* Avoid use of trailers.  */
		const IFF_RUNNING = 0x40;		/* Resources allocated.  */
		const IFF_NOARP = 0x80;		/* No address resolution protocol.  */
		const IFF_PROMISC = 0x100;	/* Receive all packets.  */
		const IFF_ALLMULTI = 0x200;	/* Receive all multicast packets.  */
		const IFF_MASTER = 0x400;		/* Master of a load balancer.  */
		const IFF_SLAVE = 0x800;		/* Slave of a load balancer.  */
		const IFF_MULTICAST = 0x1000;	/* Supports multicast.  */
		const IFF_PORTSEL = 0x2000;	/* Can set media type.  */
		const IFF_AUTOMEDIA = 0x4000;	/* Auto media select active.  */
		const IFF_DYNAMIC = 0x8000;	/* Dialup device with changing addresses.  */
	}
}

impl WriteToWithEndian for InterfaceFlags {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: bytestruct::Endian) -> io::Result<()> {
		self.bits().write_to_with_endian(target, endian)
	}
}

impl ReadFromWithEndian for InterfaceFlags {
	fn read_from_with_endian<T: io::Read>(source: &mut T, endian: bytestruct::Endian) -> io::Result<Self> {
		let val = u32::read_from_with_endian(source, endian)?;
		Ok(Self::from_bits_retain(val))
	}
}

impl Size for InterfaceFlags {
	fn size(&self) -> usize {
		4
	}
}

#[derive(Debug, ByteStruct, Size)]
pub struct InterfaceInfoMessage {
	family: u16,
	ty: InterfaceType,
	index: i32,
	flags: InterfaceFlags,
	change: u32,
}

impl InterfaceInfoMessage {
	fn empty() -> InterfaceInfoMessage {
		InterfaceInfoMessage {
			family: 0,
			ty: InterfaceType::NetRom,
			index: 0,
			flags: InterfaceFlags::empty(),
			change: 0xFFFFFFFF,
		}
	}
}

pub trait RTNetlink {
	fn get_links(&mut self) -> io::Result<Vec<InterfaceInfoMessage>>;
}

impl RTNetlink for NetlinkSocket<NetlinkRoute> {
	fn get_links(&mut self) -> io::Result<Vec<InterfaceInfoMessage>> {
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

			let interface =
				InterfaceInfoMessage::read_from_with_endian(&mut Cursor::new(&body), bytestruct::Endian::Little)?;

			println!("{:?}", interface);
			interfaces.push(interface);
		}

		Ok(interfaces)
	}
}
