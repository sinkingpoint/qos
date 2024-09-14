use std::{
	fmt::Display,
	io::{self, Cursor, ErrorKind, Read, Write},
};

use bitflags::bitflags;
use bytestruct::{int_enum, Endian, NullTerminatedString, ReadFromWithEndian, Size, WriteToWithEndian};
use bytestruct_derive::{ByteStruct, Size};

use crate::{new_string, new_u32, read_attribute, rtnetlink::parsing::new_mac_address, write_attribute};

use super::address::MacAddress;

int_enum! {
	enum AttributeType: u16 {
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
		NewInterfaceIndex = 50,
		MinimumMTU = 51,
		TCPSegmentOffloadMaxSegments = 61,
		Unknown = 9999,
	}
}

/// The rtattr's that can apply to an interface as received from a Netlink GET_LINK call.
#[derive(Debug, Default)]
pub struct InterfaceAttributes {
	// The layer-2 address of the interface.
	pub mac_address: Option<MacAddress>,
	// The layer-2 broadcast address of the interface.
	pub broadcast_address: Option<MacAddress>,
	// The name of the interface.
	pub name: Option<String>,
	// The maximum size of a packet before the interface fragments it.
	pub mtu: Option<u32>,
	// The queueing discipline of the link.
	pub qdisc: Option<String>,
	// The stats on the link.
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

impl WriteToWithEndian for InterfaceAttributes {
	fn write_to_with_endian<T: Write>(&self, t: &mut T, e: Endian) -> io::Result<()> {
		write_attribute(t, e, AttributeType::MacAddress, &self.mac_address)?;
		write_attribute(t, e, AttributeType::BroadcastAddress, &self.broadcast_address)?;
		write_attribute(
			t,
			e,
			AttributeType::Name,
			&self.name.clone().map(NullTerminatedString::<0>),
		)?;
		write_attribute(t, e, AttributeType::MTU, &self.mtu)?;
		write_attribute(
			t,
			e,
			AttributeType::Name,
			&self.qdisc.clone().map(NullTerminatedString::<0>),
		)?;
		write_attribute(
			t,
			e,
			AttributeType::Name,
			&self.name.clone().map(NullTerminatedString::<0>),
		)?;
		write_attribute(t, e, AttributeType::TransmitQueueLength, &self.transmit_queue_length)?;
		write_attribute(t, e, AttributeType::OperationalState, &self.operational_state)?;
		write_attribute(t, e, AttributeType::LinkMode, &self.link_mode)?;
		write_attribute(t, e, AttributeType::Group, &self.group)?;
		write_attribute(t, e, AttributeType::Promiscuity, &self.promiscuity)?;
		write_attribute(t, e, AttributeType::NumTransmitQueues, &self.num_transmit_queues)?;
		write_attribute(
			t,
			e,
			AttributeType::GenericSegmentOffloadMaxSegments,
			&self.generic_segment_offload_max_segments,
		)?;
		write_attribute(
			t,
			e,
			AttributeType::GenericSegmentOffloadMaxSize,
			&self.generic_segment_offload_max_size,
		)?;
		write_attribute(t, e, AttributeType::NewInterfaceIndex, &self.new_interface_index)?;
		write_attribute(t, e, AttributeType::MinimumMTU, &self.minimum_mtu)?;
		Ok(())
	}
}

impl ReadFromWithEndian for InterfaceAttributes {
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

impl InterfaceAttributes {
	pub(crate) fn read_attribute<T: Read>(&mut self, source: &mut T, endian: Endian) -> io::Result<()> {
		let (attr_type, data_buffer) = read_attribute(source, endian)?;

		match AttributeType::try_from(attr_type).unwrap_or(AttributeType::Unknown) {
			AttributeType::MacAddress => self.mac_address = Some(new_mac_address(&data_buffer)?),
			AttributeType::BroadcastAddress => self.broadcast_address = Some(new_mac_address(&data_buffer)?),
			AttributeType::Name => self.name = Some(new_string(&data_buffer)?),
			AttributeType::MTU => self.mtu = Some(new_u32(&data_buffer)?),
			AttributeType::QDisc => self.qdisc = Some(new_string(&data_buffer)?),
			AttributeType::Stats => {
				self.stats = Some(LinkStats::read_from_with_endian(&mut Cursor::new(data_buffer), endian)?)
			}
			AttributeType::TransmitQueueLength => self.transmit_queue_length = Some(new_u32(&data_buffer)?),
			AttributeType::OperationalState => {
				self.operational_state = Some(
					InterfaceOperationalState::try_from(data_buffer[0])
						.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
				)
			}
			AttributeType::LinkMode => {
				self.link_mode = Some(
					InterfaceLinkMode::try_from(data_buffer[0])
						.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
				)
			}
			AttributeType::Stats64 => {
				self.stats64 = Some(LinkStats64::read_from_with_endian(
					&mut Cursor::new(data_buffer),
					endian,
				)?)
			}
			AttributeType::Group => self.group = Some(new_u32(&data_buffer)?),
			AttributeType::Promiscuity => self.promiscuity = Some(new_u32(&data_buffer)?),
			AttributeType::NumTransmitQueues => self.num_transmit_queues = Some(new_u32(&data_buffer)?),
			AttributeType::GenericSegmentOffloadMaxSegments => {
				self.generic_segment_offload_max_segments = Some(new_u32(&data_buffer)?)
			}
			AttributeType::GenericSegmentOffloadMaxSize => {
				self.generic_segment_offload_max_size = Some(new_u32(&data_buffer)?)
			}
			AttributeType::NewInterfaceIndex => self.new_interface_index = Some(new_u32(&data_buffer)?),
			AttributeType::MinimumMTU => self.minimum_mtu = Some(new_u32(&data_buffer)?),
			AttributeType::TCPSegmentOffloadMaxSegments => {
				self.tcp_segment_offload_max_segments = Some(new_u32(&data_buffer)?)
			}
			AttributeType::Unknown => self.unknown.push((attr_type, data_buffer)),
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

impl Display for InterfaceOperationalState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let out = match self {
			Self::Unknown => "unknown",
			Self::NotPresent => "not present",
			Self::Down => "down",
			Self::LinkLayerDown => "link layer down",
			Self::Testing => "testing",
			Self::Dormant => "dormant",
			Self::Up => "up",
		};

		f.write_str(out)
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
