use std::io::{self, Cursor, ErrorKind, Read};

use bitflags::bitflags;
use bytestruct::{int_enum, Endian, ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

use crate::{
	read_attribute,
	rtnetlink::{AddressFamily, CacheInfo, IPAddress},
	write_attribute,
};

#[derive(Debug, ByteStruct)]
pub struct RouteMessage {
	pub family: AddressFamily,
	pub dst_length: u8,
	pub src_length: u8,
	pub tos: u8,

	pub table_id: u8,
	pub protocol: RouteProtocol,
	pub scope: RouteScope,
	pub ty: RouteTable,

	pub flags: RouteTableFlags,
}

impl RouteMessage {
	pub fn empty() -> Self {
		Self {
			family: AddressFamily::Unspecified,
			dst_length: 0,
			src_length: 0,
			tos: 0,
			table_id: 0,
			protocol: RouteProtocol::Unspecified,
			scope: RouteScope::Universe,
			ty: RouteTable::Unspecified,
			flags: RouteTableFlags::empty(),
		}
	}
}

int_enum! {
#[derive(Debug, Clone, Copy)]
  pub enum RouteTable: u8 {
	Unspecified = 0,
	Unicast = 1, /* Gateway or direct route	*/
	Local = 2, /* Accept locally		*/
	Broadcast = 3, /* Accept locally as broadcast, but send as broadcast */
	Anycast = 4, /* Accept locally as broadcast, but send as unicast */
	Multicast = 5, /* Multicast route		*/
	Blackhole = 6, /* Drop				*/
	Unreachable = 7, /* Destination is unreachable   */
	Prohibit = 8, /* Administratively prohibited	*/
	Throw = 9, /* Not in this table		*/
	Nat = 10, /* Translate this address	*/
	Xresolve = 11, /* Use external resolver	*/
}
}

// TODO: _Technically_, any u8 value is valid here, but this is all the ones defined in rtnetlink.h
int_enum! {
#[derive(Debug, Clone, Copy)]
pub enum RouteProtocol: u8 {
	Unspecified = 0,
	Redirect = 1, /* Route installed by ICMP redirects; not used by current IPv4 */
	Kernel = 2,   /* Route installed by kernel */
	Boot = 3,     /* Route installed during boot */
	Static = 4,   /* Route installed administratively */
	Gated = 8,    /* Route installed by gated daemon */
	Ra = 9,       /* Route installed by router advertisements */
  Mrt = 10,     /* Route installed by MRT */
  Zebra = 11,   /* Route installed by Zebra */
  Bird = 12,    /* Route installed by BIRD */
  DnRouted = 13,/* Route installed by DECnet routing daemon */
  Xorp = 14,    /* Route installed by XORP */
  Ntk = 15,     /* Route installed by Netsukuku */
  Dhcp = 16,    /* Route installed by DHCP client */
  Mrouted = 17, /* Route installed by multicast daemon */
  Keepalived = 18, /* Route installed by Keepalived daemon */
  Babel = 42,   /* Route installed by Babel daemon */
  Ovn = 84,     /* Route installed by OVN daemon */
  Openr = 99,   /* Route installed by Open Routing (Open/R) */
  Bgp = 186,    /* Route installed by BGP */
  Isis = 187,   /* Route installed by ISIS */
  Ospf = 188,   /* Route installed by OSPF */
  Rip = 189,    /* Route installed by RIP */
  Eigrp = 192,  /* Route installed by EIGRP */
}
}

int_enum! {
#[derive(Debug, Clone, Copy)]
pub enum RouteScope: u8 {
  Universe = 0,
/* User defined values  */
  Site = 200,
  Link = 253,
  Host = 254,
  Nowhere = 255,
}
}

bitflags! {
#[derive(Debug, Clone, Copy)]
  pub struct RouteTableFlags: u32 {
	const NOTIFY = 0x100; /* Notify user of route change */
	const CLONED = 0x200; /* This route is cloned */
  const EQUALIZE = 0x400; /* Multipath equalizer: NI */
  const PREFIX = 0x800; /* Prefix addresses */
  const LOOKUP_TABLE = 0x1000; /* set rtm_table to FIB lookup result */
  const FIB_MATCH = 0x2000; /* return full fib lookup match */
  const OFFLOAD = 0x4000; /* route is offloaded */
  const TRAP = 0x8000; /* route is trapping packets */
  const OFFLOAD_FAILED = 0x20000000; /* route offload failed, this value
			  * is chosen to avoid conflicts with
			  * other flags defined in
			  * include/uapi/linux/ipv6_route.h
			  */
  }
}

impl WriteToWithEndian for RouteTableFlags {
	fn write_to_with_endian<W: std::io::Write>(
		&self,
		writer: &mut W,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		let bits = self.bits();
		bits.write_to_with_endian(writer, endian)
	}
}

impl ReadFromWithEndian for RouteTableFlags {
	fn read_from_with_endian<R: std::io::Read>(reader: &mut R, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let bits = u32::read_from_with_endian(reader, endian)?;
		Ok(RouteTableFlags::from_bits_truncate(bits))
	}
}

int_enum! {
pub enum RouteAttributeType: u16 {
	Unspecified = 0,
	Destination = 1,
	Source = 2,
	InputInterface = 3,
	OutputInterface = 4,
	Gateway = 5,
	Priority = 6,
	PreferredSource = 7,
	Metrics = 8,
	Multihop = 9,
	ProtoInfo = 10, /* no longer used */
	Flow = 11,
	CacheInfo = 12,
	Session = 13, /* no longer used */
	MpAlgo = 14, /* no longer used */
	Table = 15,
	Mark = 16,
	MFCStats = 17,
	Via = 18,
	NewDestination = 19,
	Preference = 20,
	EncapsulationType = 21,
	Encapsulation = 22,
	Expires = 23,
	Pad = 24,
	Uid = 25,
	TtlPropagate = 26,
	IpProto = 27,
	Sport = 28,
	Dport = 29,
	NhId = 30,
	FlowLabel = 31,
}
}

#[derive(Default, Debug)]
pub struct RouteAttributes {
	pub destination: Option<IPAddress>,
	pub source: Option<IPAddress>,
	pub input_interface: Option<u32>,
	pub output_interface: Option<u32>,
	pub gateway: Option<IPAddress>,
	pub priority: Option<u32>,
	pub preferred_source: Option<IPAddress>,
	pub metrics: Option<u32>,
	pub multihop: Option<Vec<NextHop>>,
	pub realm: Option<u32>,
	pub cache: Option<CacheInfo>,
	pub table: Option<u8>,
	pub mark: Option<u32>,
	pub stats: Option<MFCStats>,
	pub via: Option<IPAddress>,
	pub new_destination: Option<IPAddress>,
	pub preference: Option<u8>,
	pub encapsulation_type: Option<u16>,
	pub expires: Option<u32>,
}

impl WriteToWithEndian for RouteAttributes {
	fn write_to_with_endian<W: std::io::Write>(
		&self,
		writer: &mut W,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		write_attribute(writer, endian, RouteAttributeType::Destination, &self.destination)?;
		write_attribute(writer, endian, RouteAttributeType::Source, &self.source)?;
		write_attribute(
			writer,
			endian,
			RouteAttributeType::InputInterface,
			&self.input_interface,
		)?;
		write_attribute(
			writer,
			endian,
			RouteAttributeType::OutputInterface,
			&self.output_interface,
		)?;
		write_attribute(writer, endian, RouteAttributeType::Gateway, &self.gateway)?;
		write_attribute(writer, endian, RouteAttributeType::Priority, &self.priority)?;
		write_attribute(
			writer,
			endian,
			RouteAttributeType::PreferredSource,
			&self.preferred_source,
		)?;
		write_attribute(writer, endian, RouteAttributeType::Metrics, &self.metrics)?;
		write_attribute(writer, endian, RouteAttributeType::Multihop, &self.multihop)?;
		write_attribute(writer, endian, RouteAttributeType::CacheInfo, &self.cache)?;
		write_attribute(writer, endian, RouteAttributeType::Table, &self.table)?;
		write_attribute(writer, endian, RouteAttributeType::Mark, &self.mark)?;
		write_attribute(writer, endian, RouteAttributeType::MFCStats, &self.stats)?;
		write_attribute(writer, endian, RouteAttributeType::Via, &self.via)?;
		write_attribute(
			writer,
			endian,
			RouteAttributeType::NewDestination,
			&self.new_destination,
		)?;
		write_attribute(writer, endian, RouteAttributeType::Preference, &self.preference)?;
		write_attribute(
			writer,
			endian,
			RouteAttributeType::EncapsulationType,
			&self.encapsulation_type,
		)?;
		write_attribute(writer, endian, RouteAttributeType::Expires, &self.expires)?;
		Ok(())
	}
}

impl ReadFromWithEndian for RouteAttributes {
	fn read_from_with_endian<R: Read>(source: &mut R, endian: bytestruct::Endian) -> io::Result<Self> {
		let mut attributes = RouteAttributes::default();

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

impl RouteAttributes {
	pub(crate) fn read_attribute<T: Read>(&mut self, source: &mut T, endian: Endian) -> io::Result<()> {
		let (attr_type, data_buffer) = read_attribute(source, endian)?;
		match RouteAttributeType::try_from(attr_type) {
			Ok(RouteAttributeType::Destination) => self.destination = Some(IPAddress::new(&data_buffer)?),
			Ok(RouteAttributeType::Source) => self.source = Some(IPAddress::new(&data_buffer)?),
			Ok(RouteAttributeType::InputInterface) => {
				self.input_interface = Some(u32::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::OutputInterface) => {
				self.output_interface = Some(u32::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::Gateway) => self.gateway = Some(IPAddress::new(&data_buffer)?),
			Ok(RouteAttributeType::Priority) => {
				self.priority = Some(u32::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::PreferredSource) => self.preferred_source = Some(IPAddress::new(&data_buffer)?),
			Ok(RouteAttributeType::Metrics) => {
				self.metrics = Some(u32::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::Multihop) => {
				let mut hops = Vec::new();
				let mut cursor = Cursor::new(&data_buffer);
				while (cursor.position() as usize) < data_buffer.len() {
					let hop = NextHop::read_from_with_endian(&mut cursor, endian)?;
					hops.push(hop);
				}
				self.multihop = Some(hops);
			}
			Ok(RouteAttributeType::CacheInfo) => {
				self.cache = Some(CacheInfo::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::Table) => {
				self.table = Some(u8::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::Mark) => {
				self.mark = Some(u32::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::MFCStats) => {
				self.stats = Some(MFCStats::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::Via) => self.via = Some(IPAddress::new(&data_buffer)?),
			Ok(RouteAttributeType::NewDestination) => self.new_destination = Some(IPAddress::new(&data_buffer)?),
			Ok(RouteAttributeType::Preference) => {
				self.preference = Some(u8::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::EncapsulationType) => {
				self.encapsulation_type = Some(u16::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			Ok(RouteAttributeType::Expires) => {
				self.expires = Some(u32::read_from_with_endian(&mut &data_buffer[..], endian)?)
			}
			_ => {}
		};

		Ok(())
	}
}

#[derive(Debug, ByteStruct)]
pub struct MFCStats {
	pub packets: u64,
	pub bytes: u64,
	pub wrong_interface: u32,
}

#[derive(Debug, ByteStruct)]
pub struct NextHop {
	pub flags: RouteNextHopFlags,
	pub hops: u8,
	pub interface_index: u32,
	pub attributes: RouteAttributes,
}

bitflags! {
  #[allow(non_camel_case_types, non_upper_case_globals, dead_code)]
	#[derive(Debug, Clone, Copy)]
  pub struct RouteNextHopFlags : u8 {
	const RTNH_F_DEAD = 1;
	const RTNH_F_PERVASIVE = 2;
	const RTNH_F_ONLINK = 4;
	const RTNH_F_OFFLOAD = 8;
	const RTNH_F_LINKDOWN = 16;
	const RTNH_F_UNRESOLVED = 32;
	const RTNH_F_TRAP = 64;
  }
}

impl ReadFromWithEndian for RouteNextHopFlags {
	fn read_from_with_endian<R: Read>(reader: &mut R, _endian: Endian) -> io::Result<Self> {
		let bits = u8::read_from_with_endian(reader, Endian::Little)?;
		Ok(RouteNextHopFlags::from_bits_truncate(bits))
	}
}

impl WriteToWithEndian for RouteNextHopFlags {
	fn write_to_with_endian<W: std::io::Write>(
		&self,
		writer: &mut W,
		_endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		let bits = self.bits();
		bits.write_to_with_endian(writer, Endian::Little)
	}
}
