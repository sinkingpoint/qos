use std::{
	fmt::Display,
	io::{self, Cursor, Read, Write},
};

use bitflags::bitflags;
use bytestruct::{int_enum, Endian, ReadFromWithEndian, Size, WriteToWithEndian};
use bytestruct_derive::{ByteStruct, Size};

use crate::rtnetlink::parsing::{new_mac_address, new_string, new_u32};

use super::{address::MacAddress, parsing::read_attribute};

int_enum! {
	enum InterfaceAttributeType: u16 {
		MacAddress = 1,
		BroadcastAddress = 2,
		Name = 3,
		MTU = 4,
		QDisc = 6,
		Stats = 7,
		TransmitQueueLength = 13,
		OperationalState = 16,
		LinkMode = 17,
		Stats64 = 23,
		Group = 27,
		Promiscuity = 30,
		NumTransmitQueues = 31,
		GenericSegmentOffloadMaxSegments = 40,
		GenericSegmentOffloadMaxSize = 41,
		NewInterfaceOrder = 50,
		MinimumMTU = 51,
		TCPSegmentOffloadMaxSegments = 61,
		Unknown = 9999,
	}
}

#[derive(Debug, Default)]
pub struct InterfaceAttributes {
	pub mac_address: Option<MacAddress>,
	pub broadcast_address: Option<MacAddress>,
	pub name: Option<String>,
	pub mtu: Option<u32>,
	pub qdisc: Option<String>,
	pub stats: Option<LinkStats>,
	pub transmit_queue_length: Option<u32>,
	pub operational_state: Option<InterfaceOperationalState>,
	pub link_mode: Option<InterfaceLinkMode>,
	pub stats64: Option<LinkStats64>,
	pub group: Option<u32>,
	pub promiscuity: Option<u32>,
	pub num_transmit_queues: Option<u32>,
	pub generic_segment_offload_max_segments: Option<u32>,
	pub generic_segment_offload_max_size: Option<u32>,
	pub new_interface_index: Option<u32>,
	pub minimum_mtu: Option<u32>,
	pub tcp_segment_offload_max_segments: Option<u32>,

	unknown: Vec<(u16, Vec<u8>)>,
}

impl InterfaceAttributes {
	pub(crate) fn read_attribute<T: Read>(&mut self, source: &mut T, endian: Endian) -> io::Result<()> {
		let (attr_type, data_buffer) = read_attribute(source, endian)?;

		match InterfaceAttributeType::try_from(attr_type).unwrap_or(InterfaceAttributeType::Unknown) {
			InterfaceAttributeType::MacAddress => self.mac_address = Some(new_mac_address(&data_buffer)?),
			InterfaceAttributeType::BroadcastAddress => self.broadcast_address = Some(new_mac_address(&data_buffer)?),
			InterfaceAttributeType::Name => self.name = Some(new_string(&data_buffer)?),
			InterfaceAttributeType::MTU => self.mtu = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::QDisc => self.qdisc = Some(new_string(&data_buffer)?),
			InterfaceAttributeType::Stats => {
				self.stats = Some(LinkStats::read_from_with_endian(&mut Cursor::new(data_buffer), endian)?)
			}
			InterfaceAttributeType::TransmitQueueLength => self.transmit_queue_length = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::OperationalState => {
				self.operational_state = Some(
					InterfaceOperationalState::try_from(data_buffer[0])
						.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
				)
			}
			InterfaceAttributeType::LinkMode => {
				self.link_mode = Some(
					InterfaceLinkMode::try_from(data_buffer[0])
						.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
				)
			}
			InterfaceAttributeType::Stats64 => {
				self.stats64 = Some(LinkStats64::read_from_with_endian(
					&mut Cursor::new(data_buffer),
					endian,
				)?)
			}
			InterfaceAttributeType::Group => self.group = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::Promiscuity => self.promiscuity = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::NumTransmitQueues => self.num_transmit_queues = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::GenericSegmentOffloadMaxSegments => {
				self.generic_segment_offload_max_segments = Some(new_u32(&data_buffer)?)
			}
			InterfaceAttributeType::GenericSegmentOffloadMaxSize => {
				self.generic_segment_offload_max_size = Some(new_u32(&data_buffer)?)
			}
			InterfaceAttributeType::NewInterfaceOrder => self.new_interface_index = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::MinimumMTU => self.minimum_mtu = Some(new_u32(&data_buffer)?),
			InterfaceAttributeType::TCPSegmentOffloadMaxSegments => {
				self.tcp_segment_offload_max_segments = Some(new_u32(&data_buffer)?)
			}
			InterfaceAttributeType::Unknown => self.unknown.push((attr_type, data_buffer)),
		}

		Ok(())
	}
}

int_enum! {
  #[derive(Debug)]
  pub enum InterfaceOperationalState: u8 {
	  Unknown = 0,
	  NotPresent = 1,
	  Down = 2,
	  LinkLayerDown = 3,
	  Testing = 4,
	  Dormant = 5,
	  Up = 6,
  }
}

int_enum! {
  #[derive(Debug)]
  pub enum InterfaceLinkMode: u8 {
	Default = 0,
	Dormant = 1,
	Testing = 2,
  }
}

#[derive(Debug, ByteStruct, Size)]
pub struct InterfaceInfoMessage {
	pub family: u16,
	pub ty: InterfaceType,
	pub index: i32,
	pub flags: InterfaceFlags,
	pub change: u32,
}

impl InterfaceInfoMessage {
	pub fn empty() -> InterfaceInfoMessage {
		InterfaceInfoMessage {
			family: 0,
			ty: InterfaceType::NetRom,
			index: 0,
			flags: InterfaceFlags::empty(),
			change: 0xFFFFFFFF,
		}
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

impl Display for InterfaceFlags {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		bitflags::parser::to_writer_strict(self, f)
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

#[derive(Debug, ByteStruct)]
pub struct LinkStats {
	received_packets: u32,
	transmitted_packets: u32,
	received_bytes: u32,
	transmitted_bytes: u32,
	receive_errors: u32,
	transmit_errors: u32,
	receive_dropped: u32,
	transmit_dropped: u32,
	multicast: u32,
	collisions: u32,

	receive_length_errors: u32,
	receive_over_errors: u32,
	receive_crc_errors: u32,
	receive_fifo_errors: u32,
	receive_missed_errors: u32,

	transmit_aborted_errors: u32,
	transmit_carrier_errors: u32,
	transmit_fifo_errors: u32,
	transmit_heartbeat_errors: u32,
	transmit_window_errors: u32,

	receive_compressed: u32,
	transmit_compressed: u32,
	receive_nohandler: u32,
}

#[derive(Debug, ByteStruct)]
pub struct LinkStats64 {
	received_packets: u64,
	transmitted_packets: u64,
	received_bytes: u64,
	transmitted_bytes: u64,
	receive_errors: u64,
	transmit_errors: u64,
	receive_dropped: u64,
	transmit_dropped: u64,
	multicast: u64,
	collisions: u64,

	receive_length_errors: u64,
	receive_over_errors: u64,
	receive_crc_errors: u64,
	receive_fifo_errors: u64,
	receive_missed_errors: u64,

	transmit_aborted_errors: u64,
	transmit_carrier_errors: u64,
	transmit_fifo_errors: u64,
	transmit_heartbeat_errors: u64,
	transmit_window_errors: u64,

	receive_compressed: u64,
	transmit_compressed: u64,
	receive_nohandler: u64,
	receive_otherhost_dropped: u64,
}