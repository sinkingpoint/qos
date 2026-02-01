use std::{
	ffi::OsString,
	io::{self, Cursor, ErrorKind, Read},
	net::SocketAddr,
	os::fd::{AsRawFd, OwnedFd},
	sync::Arc,
	thread,
	time::Duration,
};

use slog::{error, info};

use anyhow::Context;
use bitflags::bitflags;
use bytestruct::{int_enum, tlv_values, Endian, ReadFromWithEndian, TLVVec, WriteToWithEndian};
use common::{io::RawFdReader, rand::rand_u32};
use netlink::{
	rtnetlink::{
		Address, IPAddress, Interface, MacAddress, NetlinkRoute, RTNetlink, Route, RouteAttributes, RouteProtocol,
		RouteTable,
	},
	NetlinkSocket,
};
use nix::{
	errno::Errno,
	sys::{
		socket::{
			self, bind, sendto, setsockopt, socket,
			sockopt::{BindToDevice, Broadcast, ReceiveTimeout},
			MsgFlags, SockFlag, SockProtocol, SockType, SockaddrIn, SockaddrStorage,
		},
		time::TimeVal,
	},
};

const DHCP_SERVER_PORT: u16 = 67;
const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_MAGIC_COOKIE: u32 = 0x63825363;
const MAC_ADDRESS_SIZE: u8 = 6;
const DHCP_BASE_TIMEOUT_SECS: u64 = 1;
const DHCP_MAX_TIMEOUT_SECS: u64 = 64;
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
		router: TLVVec<[u8; 4]> = DHCPOptionsVariant::Router,
		time_servers: TLVVec<[u8; 4]> = DHCPOptionsVariant::TimeServer,
		name_servers: TLVVec<[u8; 4]> = DHCPOptionsVariant::NameServer,
		dns_servers: TLVVec<[u8; 4]> = DHCPOptionsVariant::DNSServer,
		requested_ip_address: u32 = DHCPOptionsVariant::RequestedIPAddress,
		server_identifier: [u8; 4] = DHCPOptionsVariant::ServerIdentifier,
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
		let _pad = <[u8; 202]>::read_from_with_endian(source, endian)?; // Client hardware address padding + padding
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
		[0_u8; 10].write_to_with_endian(&mut msg, endian)?; // Client hardware address padding
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
	netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>,
}

impl DHCPClient {
	pub fn new(
		logger: slog::Logger,
		netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>,
		source_interface: Interface,
	) -> anyhow::Result<Self> {
		let socket = socket(
			socket::AddressFamily::Inet,
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

		setsockopt(&socket, BindToDevice, &OsString::from(&device_name)).with_context(|| "failed to bind to device")?;
		setsockopt(&socket, Broadcast, &true).with_context(|| "failed to set broadcast")?;
		setsockopt(&socket, ReceiveTimeout, &TimeVal::new(5_i64, 0))?;

		bind(socket.as_raw_fd(), &SockaddrIn::new(0, 0, 0, 0, DHCP_CLIENT_PORT))
			.with_context(|| "failed to bind to device")?;

		Ok(Self {
			logger,
			source_interface,
			socket,
			state: State::Discover,
			netlink_socket,
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
		let mut offer_dhcp_message: Option<DHCPMessage> = None;
		loop {
			match &self.state {
				State::Discover => {
					if self.retry_count >= MAX_DHCP_DISCOVER_RETRIES {
						info!(self.logger, "Max DHCP retries reached, giving up");
						return;
					}

					let timeout_secs = (DHCP_BASE_TIMEOUT_SECS << self.retry_count).min(DHCP_MAX_TIMEOUT_SECS);
					thread::sleep(Duration::from_secs(timeout_secs));

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
				State::Offer => {
					let mut buf = [0_u8; 1500];
					match reader.read(&mut buf) {
						Ok(_) => {}
						Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
							info!(self.logger, "DHCP Offer timeout, retrying Discover {} {}", e, e.kind());
							self.state = State::Discover;
							continue;
						}
						Err(e) => {
							info!(self.logger, "Error reading DHCP message"; "error" => %e);
							return;
						}
					};

					let offer = match DHCPMessage::read_from_with_endian(&mut Cursor::new(&buf), Endian::Big) {
						Ok(msg) => msg,
						Err(e) => {
							info!(self.logger, "Error reading DHCP message"; "error" => %e);
							return;
						}
					};

					if offer.transaction_id != transaction_id {
						info!(self.logger, "Ignoring DHCP message with unexpected transaction ID"; "expected" => transaction_id, "got" => offer.transaction_id);
						continue;
					}

					offer_dhcp_message = Some(offer);
					self.state = State::Request;
					self.retry_count = 0;
				}
				State::Request => {
					if self.retry_count >= MAX_DHCP_DISCOVER_RETRIES {
						info!(self.logger, "Max DHCP retries reached, giving up");
						return;
					}

					let timeout_secs = (DHCP_BASE_TIMEOUT_SECS << self.retry_count).min(DHCP_MAX_TIMEOUT_SECS);
					thread::sleep(Duration::from_secs(timeout_secs));

					info!(self.logger, "Sending DHCP Request"; "attempt" => self.retry_count + 1, "timeout_secs" => timeout_secs);

					let offer = match &offer_dhcp_message {
						Some(offer) => offer,
						None => {
							info!(self.logger, "No DHCP offer to request from");
							return;
						}
					};

					let options = DHCPOptions {
						message_type: Some(MessageType::Request),
						requested_ip_address: match offer.your_ip {
							IPAddress::IPv4(addr) => Some(u32::from_be_bytes(addr)),
							_ => None,
						},
						server_identifier: match offer.options.server_identifier {
							Some(addr) => Some(addr),
							None => match offer.server_ip {
								IPAddress::IPv4(addr) => Some(addr),
								_ => None,
							},
						},
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

					self.send(IPAddress::IPv4([0xFF, 0xFF, 0xFF, 0xFF]), message)
						.expect("failed to send DHCP request");

					self.state = State::Acknowledge;
				}
				State::Acknowledge => {
					let mut buf = [0_u8; 1500];
					match reader.read(&mut buf) {
						Ok(_) => {}
						Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
							info!(
								self.logger,
								"DHCP Acknowledge timeout, retrying Request {} {}",
								e,
								e.kind()
							);
							self.state = State::Request;
							continue;
						}
						Err(e) => {
							info!(self.logger, "Error reading DHCP message"; "error" => %e);
							return;
						}
					};

					let ack = match DHCPMessage::read_from_with_endian(&mut Cursor::new(&buf), Endian::Big) {
						Ok(msg) => msg,
						Err(e) => {
							info!(self.logger, "Error reading DHCP message"; "error" => %e);
							return;
						}
					};

					if ack.transaction_id != transaction_id {
						info!(self.logger, "Ignoring DHCP message with unexpected transaction ID"; "expected" => transaction_id, "got" => ack.transaction_id);
						continue;
					}

					self.handle_dhcp_ack(ack);
					return;
				}
			}
		}
	}

	fn handle_dhcp_ack(&self, msg: DHCPMessage) {
		let assigned_ip = match msg.your_ip {
			IPAddress::IPv4(addr) => addr,
			_ => {
				error!(self.logger, "Unexpected non-IPv4 address assigned by DHCP server");
				return;
			}
		};

		let lease_time = match msg.options.lease_time {
			Some(time) => time,
			None => {
				error!(self.logger, "No lease time provided by DHCP server");
				return;
			}
		};

		let dns_servers = match msg.options.dns_servers {
			Some(servers) => servers,
			None => {
				error!(self.logger, "No DNS servers provided by DHCP server");
				return;
			}
		};

		let route = match msg.options.router {
			Some(routers) => routers,
			None => {
				error!(self.logger, "No router provided by DHCP server");
				return;
			}
		};

		info!(self.logger, "DHCP Lease Acquired";
			"assigned_ip" => format!("{}.{}.{}.{}", assigned_ip[0], assigned_ip[1], assigned_ip[2], assigned_ip[3]),
			"lease_time_secs" => lease_time,
			"dns_servers" => format!("{:?}", dns_servers.iter().map(|s| format!("{}.{}.{}.{}", s[0], s[1], s[2], s[3])).collect::<Vec<_>>()),
			"routers" => format!("{:?}", route.iter().map(|s| format!("{}.{}.{}.{}", s[0], s[1], s[2], s[3])).collect::<Vec<_>>()),
		);

		// Assign the IP address to the interface
		let subnet_mask = msg.options.subnet_mask.unwrap_or([255, 255, 255, 255]);
		let combined_mask = u32::from_be_bytes(subnet_mask);
		let prefix_length = combined_mask.count_ones() as u8;
		let address = Address::new(IPAddress::IPv4(assigned_ip), prefix_length, self.source_interface.index);

		if let Err(e) = self.netlink_socket.new_address(address) {
			error!(self.logger, "Failed to add IP address to interface"; "error" => format!("{:?}", e));
			return;
		}

		info!(self.logger, "Adding default route";
			"gateway" => format!("{}.{}.{}.{}", route[0][0], route[0][1], route[0][2], route[0][3]),
			"interface_index" => self.source_interface.index
		);

		if let Err(e) = self.netlink_socket.new_route(Route {
			family: netlink::rtnetlink::AddressFamily::IPv4,
			dst_length: 0,
			src_length: 0,
			tos: 0,
			table_id: 0,
			protocol: RouteProtocol::Boot,
			scope: netlink::rtnetlink::RouteScope::Universe,
			ty: RouteTable::Unicast,
			flags: netlink::rtnetlink::RouteTableFlags::empty(),
			attributes: RouteAttributes {
				gateway: Some(IPAddress::IPv4(route[0])),
				output_interface: Some(self.source_interface.index),
				..RouteAttributes::default()
			},
		}) {
			error!(self.logger, "Failed to add default route"; "error" => format!("{:?}", e));
			return;
		}

		info!(self.logger, "Default route added successfully");
	}
}
