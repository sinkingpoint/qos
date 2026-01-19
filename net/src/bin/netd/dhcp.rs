use std::{
	ffi::OsString,
	io::{self, ErrorKind},
	net::SocketAddr,
	os::fd::{AsRawFd, OwnedFd},
};

use slog::info;

use anyhow::Context;
use bitflags::bitflags;
use bytestruct::{int_enum, tlv_values, Endian, ReadFromWithEndian, WriteToWithEndian};
use common::{io::RawFdReader, rand::rand_u32};
use netlink::rtnetlink::{IPAddress, Interface, MacAddress};
use nix::{
	errno::Errno,
	sys::socket::{
		bind, sendto, setsockopt, socket,
		sockopt::{BindToDevice, Broadcast, ReceiveTimeout},
		AddressFamily, MsgFlags, SockFlag, SockProtocol, SockType, SockaddrIn, SockaddrStorage,
	},
	sys::time::TimeVal,
};

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_MAGIC_COOKIE: u32 = 0x63825363;
const MAC_ADDRESS_SIZE: u8 = 6;
const DHCP_BASE_TIMEOUT_SECS: i64 = 1;
const DHCP_MAX_TIMEOUT_SECS: i64 = 64;
const MAX_DHCP_DISCOVER_RETRIES: u32 = 30;

int_enum! {
	#[derive(Debug)]
  pub enum DHCPOptionsVariant: u8 {
	Pad = 0,
	SubnetMask = 1,
	TimeOffset = 2,
	Router = 3,
	TimeServer = 4,
	NameServer = 5,
	DNSServer = 6,
	LogServer = 7,
	CookieServer = 8,
	LPRServer = 9,
	ImpressServer = 10,
	ResourceLocationServer = 11,
	HostName = 12,
	BootFileSize = 13,
	MeritDumpFile = 14,
	DomainName = 15,
	SwapServer = 16,
	RootPath = 17,
	ExtensionsPath = 18,
	IPForward = 19,
	NonLocalSourceRouting = 20,
	PolicyFilter = 21,
	MaxReassemblySize = 22,
	IPTTL = 23,
	PMTUTimeout = 24,
	PTMUPlateuTable = 25,
	InterfaceMTU = 26,
	AllSubnetsAreLocal = 27,
	BroadcastAddress = 28,
	PerformMaskDiscovery = 29,
	MaskSupplier = 30,
	PerformRouterDiscovery = 31,
	RouterSolicitationAddress = 32,
	StaticRoute = 33,
	TrailerEncapsulation = 34,
	ArpCacheTimeout = 35,
	EthernetEncapsulation = 36,
	TCPDefaultTTL = 37,
	TCPKeepaliveInterval = 38,
	TCPKeepaliveGarbage = 39,
	RequestedIPAddress = 50,
	IPAddressLeaseTime = 51,
	OptionOverload = 52,
	DHCPMessageType = 53,
	ServerIdentifier = 54,
	ParameterRequestList = 55,
	Message = 56,
	MaximumMessageSize = 57,
	RenewalTimeValue = 58,
	RebindingTimeValue = 59,
	VendorClassIdentifier = 60,
	ClientIdentifier = 61,
	TFTPServerName = 66,
	BootFileName = 67,
	End = 255,
  }
}

int_enum! {
	#[derive(Debug)]
  pub enum MessageType: u8 {
	Discover = 1,
  Offer = 2,
  Request = 3,
  Decline = 4,
  Ack = 5,
  Nak = 6,
  Release = 7,
  Inform = 8,
  ForceRenew = 9,
  LeaseQuery = 10,
  LeaseUnassigned = 11,
  LeaseUnknown = 12,
  LeaseActive = 13,
  BulkLeaseQuery = 14,
  LeaseQueryDone = 15,
  ActiveLeaseQuery = 16,
  LeaseQueryStates = 17,
  TLS = 18,
  }
}

int_enum! {
	#[derive(Debug)]
	pub enum HardwareType: u8 {
		Ethernet = 1,
	}
}

bitflags! {
	#[derive(Debug)]
	pub struct DHCPFlags: u16 {
		const Broadcast = 0x8000;
	}
}

int_enum! {
	#[derive(Debug)]
	pub enum OpCode: u8 {
		Request = 1,
		Response = 2,
	}
}

tlv_values! {
	#[no_length_type(DHCPOptionsVariant::Pad)]
	#[derive(Debug)]
	pub struct DHCPOptions: (DHCPOptionsVariant, u8, DHCPOptionsVariant::End) {
		subnet_mask: [u8; 4] = DHCPOptionsVariant::SubnetMask,
		time_offset: u32 = DHCPOptionsVariant::TimeOffset,
		router: Vec<u32> = DHCPOptionsVariant::Router,
		time_servers: Vec<u32> = DHCPOptionsVariant::TimeServer,
		name_servers: Vec<u32> = DHCPOptionsVariant::NameServer,
		dns_servers: Vec<u32> = DHCPOptionsVariant::DNSServer,
		requested_ip_address: u32 = DHCPOptionsVariant::RequestedIPAddress,
		lease_time: u32 = DHCPOptionsVariant::IPAddressLeaseTime,
		message_type: MessageType = DHCPOptionsVariant::DHCPMessageType,
	}
}

#[derive(Debug)]
struct DHCPMessage {
	op_code: OpCode,
	transaction_id: u32,
	flags: DHCPFlags,
	client_ip: IPAddress,
	your_ip: IPAddress,
	server_ip: IPAddress,
	gateway_ip: IPAddress,
	client_hw_address: MacAddress,
	options: DHCPOptions,
}

impl ReadFromWithEndian for DHCPMessage {
	fn read_from_with_endian<T: std::io::Read>(source: &mut T, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let op_code = OpCode::read_from_with_endian(source, endian)?;
		let htype = HardwareType::read_from_with_endian(source, endian)?;
		if !matches!(htype, HardwareType::Ethernet) {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				"unexpected hardware type. Expected Ethernet (1)",
			));
		}

		let hlen = u8::read_from_with_endian(source, endian)?;
		if hlen != 6 {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				format!(
					"unexpected hardware address length. MAC addreses are 6 bytes, got {}",
					hlen
				),
			));
		}

		let _hops = u8::read_from_with_endian(source, endian)?;
		let transaction_id = u32::read_from_with_endian(source, endian)?;
		let _secs = u16::read_from_with_endian(source, endian)?;
		let flags = DHCPFlags::from_bits_retain(u16::read_from_with_endian(source, endian)?);
		let client_ip = IPAddress::IPv4(<[u8; 4]>::read_from_with_endian(source, endian)?);
		let your_ip = IPAddress::IPv4(<[u8; 4]>::read_from_with_endian(source, endian)?);
		let server_ip = IPAddress::IPv4(<[u8; 4]>::read_from_with_endian(source, endian)?);
		let gateway_ip = IPAddress::IPv4(<[u8; 4]>::read_from_with_endian(source, endian)?);
		let client_hw_address = MacAddress::new(<[u8; 6]>::read_from_with_endian(source, endian)?);
		let _pad = <[u8; 192]>::read_from_with_endian(source, endian)?;
		let cookie = u32::read_from_with_endian(source, endian)?;

		if cookie != DHCP_MAGIC_COOKIE {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				format!(
					"invalid magic cookie. expected {:#x}, got {:#x}",
					DHCP_MAGIC_COOKIE, cookie
				),
			));
		}

		let options = DHCPOptions::read_from_with_endian(source, endian)?;

		Ok(Self {
			op_code,
			transaction_id,
			flags,
			client_ip,
			your_ip,
			server_ip,
			gateway_ip,
			client_hw_address,
			options,
		})
	}
}

impl WriteToWithEndian for DHCPMessage {
	fn write_to_with_endian<T: io::Write>(&self, target: &mut T, endian: bytestruct::Endian) -> io::Result<()> {
		let mut msg = Vec::new();
		self.op_code.write_to_with_endian(&mut msg, endian)?;
		HardwareType::Ethernet.write_to_with_endian(&mut msg, endian)?;
		MAC_ADDRESS_SIZE.write_to_with_endian(&mut msg, endian)?;
		0_u8.write_to_with_endian(&mut msg, endian)?; // HOPS
		self.transaction_id.write_to_with_endian(&mut msg, endian)?;
		0_u16.write_to_with_endian(&mut msg, endian)?; // SECS
		self.flags.bits().write_to_with_endian(&mut msg, endian)?;
		self.client_ip.write_to_with_endian(&mut msg, endian)?;
		self.your_ip.write_to_with_endian(&mut msg, endian)?;
		self.server_ip.write_to_with_endian(&mut msg, endian)?;
		self.gateway_ip.write_to_with_endian(&mut msg, endian)?;
		self.client_hw_address.write_to_with_endian(&mut msg, endian)?;
		[0_u8; 192].write_to_with_endian(&mut msg, endian)?; // Padding
		DHCP_MAGIC_COOKIE.write_to_with_endian(&mut msg, endian)?;
		self.options.write_to_with_endian(&mut msg, endian)?;

		target.write_all(&msg)?;

		Ok(())
	}
}

enum State {
	Discover,
	Offer,
	Request,
	Acknowledge,
}

pub struct DHCPClient {
	logger: slog::Logger,
	source_interface: Interface,
	socket: OwnedFd,
	state: State,
	retry_count: u32,
}

impl DHCPClient {
	pub fn new(logger: slog::Logger, source_interface: Interface) -> anyhow::Result<Self> {
		let socket = socket(
			AddressFamily::Inet,
			SockType::Datagram,
			SockFlag::empty(),
			SockProtocol::Udp,
		)?;

		let device_name = match source_interface.attributes.name.as_ref() {
			Some(name) => name,
			None => {
				return Err(anyhow::anyhow!("missing device name"));
			}
		};

		info!(logger, "starting DHCP client"; "interface_name" => device_name);

		setsockopt(&socket, BindToDevice, &OsString::from(&device_name))
			.with_context(|| "failed tssssso bind to device")?;
		setsockopt(&socket, Broadcast, &true).with_context(|| "failed to set broadcast")?;

		bind(socket.as_raw_fd(), &SockaddrIn::new(0, 0, 0, 0, DHCP_CLIENT_PORT))
			.with_context(|| "failed to bind to device")?;

		Ok(Self {
			logger,
			source_interface,
			socket,
			state: State::Discover,
			retry_count: 0,
		})
	}

	fn send(&self, address: IPAddress, message: DHCPMessage) -> nix::Result<usize> {
		let mut buffer = Vec::new();
		message
			.write_to_with_endian(&mut buffer, Endian::Big)
			.map_err(|e| Errno::from_i32(e.raw_os_error().unwrap_or(0)))?;

		sendto(
			self.socket.as_raw_fd(),
			&buffer,
			&SockaddrStorage::from(SocketAddr::new(address.to_std(), DHCP_SERVER_PORT)),
			MsgFlags::empty(),
		)
	}

	pub fn run(mut self) {
		let transaction_id = rand_u32().expect("random transaction id");
		let mut reader = RawFdReader::new(self.socket.as_raw_fd());
		loop {
			match &self.state {
				State::Discover => {
					if self.retry_count >= MAX_DHCP_DISCOVER_RETRIES {
						info!(self.logger, "Max DHCP retries reached, giving up");
						return;
					}

					// Exponential backoff: 1s, 2s, 4s, 8s, 16s
					let timeout_secs = (DHCP_BASE_TIMEOUT_SECS << self.retry_count).min(DHCP_MAX_TIMEOUT_SECS);
					let timeout = TimeVal::new(timeout_secs, 0);
					setsockopt(&self.socket, ReceiveTimeout, &timeout).expect("failed to set socket timeout");

					info!(self.logger, "Sending DHCP Discover"; "attempt" => self.retry_count + 1, "timeout_secs" => timeout_secs);

					let options = DHCPOptions {
						message_type: Some(MessageType::Discover),
						..DHCPOptions::default()
					};

					let message = DHCPMessage {
						op_code: OpCode::Request,
						transaction_id,
						flags: DHCPFlags::Broadcast,
						client_ip: IPAddress::IPv4([0, 0, 0, 0]),
						your_ip: IPAddress::IPv4([0, 0, 0, 0]),
						server_ip: IPAddress::IPv4([0, 0, 0, 0]),
						gateway_ip: IPAddress::IPv4([0, 0, 0, 0]),
						client_hw_address: self.source_interface.attributes.mac_address.clone().unwrap(),
						options,
					};

					self.send(IPAddress::IPv4([0xFF, 0xFF, 0xFF, 0xFF]), message).unwrap();
					self.retry_count += 1;
					self.state = State::Offer;
				}
				State::Offer => match DHCPMessage::read_from_with_endian(&mut reader, Endian::Big) {
					Ok(msg) => {
						info!(self.logger, "Received DHCP Offer");
						println!("Got offer: {:?}", msg);
						self.state = State::Request;
					}
					Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
						info!(self.logger, "DHCP Offer timeout, retrying Discover");
						self.state = State::Discover;
					}
					Err(e) => {
						info!(self.logger, "Error reading DHCP message"; "error" => %e);
						return;
					}
				},
				State::Request => {
					self.state = State::Acknowledge;
				}
				_ => {}
			}
		}
	}
}
