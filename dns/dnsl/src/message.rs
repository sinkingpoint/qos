use std::io;

use bytestruct::{int_enum, LengthPrefixedVec, ReadFromWithEndian, WriteToWithEndian};
use bytestruct_derive::ByteStruct;

#[rustfmt::skip]
mod flags {
	// DNS Message Flags

	/// QR (Query/Response) flag: 0 for query, 1 for response
	pub const RESPONSE_FLAG_QR: u16             = 0b1000_0000_0000_0000;

	/// OPCODE (Operation Code) field: 4 bits indicating the type of query
	/// 0 = Standard Query (QUERY)
	/// 1 = Inverse Query (IQUERY)
	/// 2 = Server Status Request (STATUS)
	pub const OPCODE_MASK: u16                  = 0b0111_1000_0000_0000;
	pub const INVERSE_QUERY_OPCODE: u16         = 0b0000_1000_0000_0000;
	pub const SERVER_STATUS_REQUEST_OPCODE: u16 = 0b0010_0000_0000_0000;

	/// AA (Authoritative Answer) flag: 1 if the responding name server is authoritative for the domain name in question
	pub const AUTHORITATIVE_ANSWER_FLAG: u16    = 0b0000_0100_0000_0000;

	/// TC (Truncated) flag: 1 if the message was truncated due to length greater than that permitted on the transmission channel
	pub const TRUNCATED_FLAG: u16               = 0b0000_0010_0000_0000;

	/// RD (Recursion Desired) flag: 1 if the client desires recursive query support
	pub const RECURSION_DESIRED_FLAG: u16       = 0b0000_0001_0000_0000;

	/// RA (Recursion Available) flag: 1 if the name server supports recursive queries
	pub const RECURSION_AVAILABLE_FLAG: u16     = 0b0000_0000_1000_0000;

	/// Z (Zero) flag: Reserved for future use, must be zero in all queries and responses
	pub const Z_FLAG: u16                       = 0b0000_0000_0100_0000;

	/// AD (Authenticated Data) flag: 1 if the data in the response has been authenticated by the server
	pub const AUTHENTICATED_DATA_FLAG: u16      = 0b0000_0000_0010_0000;

	/// CD (Checking Disabled) flag: 1 if the client does not want the server to perform DNSSEC validation
	pub const CHECKING_DISABLED_FLAG: u16       = 0b0000_0000_0001_0000;

	/// RCODE (Response Code) field: 4 bits indicating the response code
	pub const RESPONSE_CODE_MASK: u16           = 0b0000_0000_0000_1111;
}

use flags::*;

#[derive(Debug)]
pub struct DNSMessage {
	pub header: DNSMessageHeader,
	pub questions: Vec<DNSQuestion>,
	pub answers: Vec<DNSAnswer>,
	pub authorities: Vec<DNSAnswer>,
	pub additionals: Vec<DNSAnswer>,
	pub edns_record: Option<EDNSRecord>,
}

impl DNSMessage {
	pub fn new_query(question: DNSQuestion) -> Self {
		Self {
			header: DNSMessageHeader {
				transaction_id: rand::random(),
				flags: DNSMessageFlags::new_query(),
				question_count: 1,
				answer_count: 0,
				authority_count: 0,
				additional_count: 0,
			},
			questions: vec![question],
			answers: Vec::new(),
			authorities: Vec::new(),
			additionals: Vec::new(),
			edns_record: None,
		}
	}

	pub fn new_response(request: &DNSMessage, answers: Vec<DNSAnswer>) -> Self {
		Self {
			header: DNSMessageHeader {
				transaction_id: request.header.transaction_id,
				flags: DNSMessageFlags {
					message_type: DNSMessageType::Response,
					opcode: request.header.flags.opcode.clone(),
					authoritative_answer: true,
					truncated: false,
					recursion_desired: request.header.flags.recursion_desired,
					recursion_available: true,
					z: false,
					authenticated_data: false,
					checking_disabled: false,
					response_code: DNSResponseCode::NoError,
				},
				question_count: request.questions.len() as u16,
				answer_count: answers.len() as u16,
				authority_count: 0,
				additional_count: 0,
			},
			questions: request.questions.clone(),
			answers,
			authorities: Vec::new(),
			additionals: Vec::new(),
			edns_record: None,
		}
	}
}

impl ReadFromWithEndian for DNSMessage {
	fn read_from_with_endian<T: std::io::Read>(source: &mut T, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let header = DNSMessageHeader::read_from_with_endian(source, endian)?;

		let mut questions = Vec::with_capacity(header.question_count as usize);
		for _ in 0..header.question_count {
			questions.push(DNSQuestion::read_from_with_endian(source, endian)?);
		}

		let mut answers = Vec::with_capacity(header.answer_count as usize);
		for _ in 0..header.answer_count {
			answers.push(DNSAnswer::read_from_with_endian(source, endian)?);
		}

		let mut authorities = Vec::with_capacity(header.authority_count as usize);
		for _ in 0..header.authority_count {
			authorities.push(DNSAnswer::read_from_with_endian(source, endian)?);
		}

		let mut edns_record = None;
		let mut additionals = Vec::with_capacity(header.additional_count as usize);
		for _ in 0..header.additional_count {
			let raw_answer = RawDNSAnswer::read_from_with_endian(source, endian)?;
			if raw_answer.atype == QType::OPT {
				edns_record = match raw_answer.try_into() {
					Ok(edns_record) => Some(edns_record),
					Err(e) => {
						return Err(std::io::Error::new(
							std::io::ErrorKind::InvalidData,
							format!("Failed to parse EDNS record: {}", e),
						))
					}
				}
			} else {
				additionals.push(match raw_answer.try_into() {
					Ok(answer) => answer,
					Err(e) => {
						return Err(std::io::Error::new(
							std::io::ErrorKind::InvalidData,
							format!("Failed to parse DNS answer: {}", e),
						))
					}
				});
			}
		}

		Ok(Self {
			header,
			questions,
			answers,
			authorities,
			additionals,
			edns_record,
		})
	}
}

impl WriteToWithEndian for DNSMessage {
	fn write_to_with_endian<T: std::io::Write>(
		&self,
		target: &mut T,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		self.header.write_to_with_endian(target, endian)?;
		for question in &self.questions {
			question.write_to_with_endian(target, endian)?;
		}
		for answer in &self.answers {
			answer.write_to_with_endian(target, endian)?;
		}
		for authority in &self.authorities {
			authority.write_to_with_endian(target, endian)?;
		}
		for additional in &self.additionals {
			additional.write_to_with_endian(target, endian)?;
		}
		if let Some(edns_record) = &self.edns_record {
			edns_record.write_to_with_endian(target, endian)?;
		}

		Ok(())
	}
}

#[derive(Debug, ByteStruct, Clone)]
pub struct DNSMessageHeader {
	pub transaction_id: u16,
	pub flags: DNSMessageFlags,
	pub question_count: u16,
	pub answer_count: u16,
	pub authority_count: u16,
	pub additional_count: u16,
}

#[derive(Debug, Clone)]
pub enum DNSMessageType {
	Query,
	Response,
}

#[derive(Debug, Clone)]
pub enum DNSOpcode {
	StandardQuery,
	InverseQuery,
	ServerStatusRequest,
}

int_enum! {
#[derive(Debug, PartialEq, Clone)]
pub enum DNSResponseCode: u8 {
	NoError = 0,
	FormatError = 1,
	ServerFailure = 2,
	NameError = 3,
	NotImplemented = 4,
	Refused = 5,
	YXDomain = 6,
	YXRRSet = 7,
	NXRRSet = 8,
	NotAuth = 9,
	NotZone = 10,
	DSOTypeNotImplemented = 11,
	BadVersion = 16,
	BadKey = 17,
	BadTime = 18,
	BadMode = 19,
	BadName = 20,
	BadAlgorithm = 21,
	BadTruncation = 22,
	BadCookie = 23,
}
}

#[derive(Debug, Clone)]
pub struct DNSMessageFlags {
	pub message_type: DNSMessageType,
	pub opcode: DNSOpcode,
	pub authoritative_answer: bool,
	pub truncated: bool,
	pub recursion_desired: bool,
	pub recursion_available: bool,
	pub z: bool,
	pub authenticated_data: bool,
	pub checking_disabled: bool,
	pub response_code: DNSResponseCode,
}
impl DNSMessageFlags {
	fn new_query() -> Self {
		Self {
			message_type: DNSMessageType::Query,
			opcode: DNSOpcode::StandardQuery,
			authoritative_answer: false,
			truncated: false,
			recursion_desired: true,
			recursion_available: false,
			z: false,
			authenticated_data: false,
			checking_disabled: false,
			response_code: DNSResponseCode::NoError,
		}
	}
}

impl WriteToWithEndian for DNSMessageFlags {
	fn write_to_with_endian<T: std::io::Write>(
		&self,
		target: &mut T,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		let mut raw_flags: u16 = 0;

		raw_flags |= match self.message_type {
			DNSMessageType::Query => 0,
			DNSMessageType::Response => RESPONSE_FLAG_QR,
		};

		raw_flags |= match self.opcode {
			DNSOpcode::StandardQuery => 0,
			DNSOpcode::InverseQuery => INVERSE_QUERY_OPCODE,
			DNSOpcode::ServerStatusRequest => SERVER_STATUS_REQUEST_OPCODE,
		};

		if self.authoritative_answer {
			raw_flags |= AUTHORITATIVE_ANSWER_FLAG;
		}
		if self.truncated {
			raw_flags |= TRUNCATED_FLAG;
		}
		if self.recursion_desired {
			raw_flags |= RECURSION_DESIRED_FLAG;
		}
		if self.recursion_available {
			raw_flags |= RECURSION_AVAILABLE_FLAG;
		}
		if self.z {
			raw_flags |= Z_FLAG;
		}
		if self.authenticated_data {
			raw_flags |= AUTHENTICATED_DATA_FLAG;
		}
		if self.checking_disabled {
			raw_flags |= CHECKING_DISABLED_FLAG;
		}

		raw_flags |= <u8 as Into<u16>>::into((&self.response_code).into());

		raw_flags.write_to_with_endian(target, endian)
	}
}

impl ReadFromWithEndian for DNSMessageFlags {
	fn read_from_with_endian<T: std::io::Read>(source: &mut T, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let raw_flags = u16::read_from_with_endian(source, endian)?;
		let qr = if raw_flags & RESPONSE_FLAG_QR == 0 {
			DNSMessageType::Query
		} else {
			DNSMessageType::Response
		};

		// Not sure if this should be an int_enum or not, because the values we read are different from the
		// values we write, but it is still a 4-bit field, so maybe it should be?
		let opcode = match (raw_flags & OPCODE_MASK) >> 11 {
			0 => DNSOpcode::StandardQuery,
			1 => DNSOpcode::InverseQuery,
			2 => DNSOpcode::ServerStatusRequest,
			_ => {
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("Invalid DNS Opcode: {:#018b}", (raw_flags & OPCODE_MASK) >> 11),
				))
			}
		};

		let authoritative_answer = (raw_flags & AUTHORITATIVE_ANSWER_FLAG) != 0;
		let truncated = (raw_flags & TRUNCATED_FLAG) != 0;
		let recursion_desired = (raw_flags & RECURSION_DESIRED_FLAG) != 0;
		let recursion_available = (raw_flags & RECURSION_AVAILABLE_FLAG) != 0;
		let z = (raw_flags & Z_FLAG) != 0;
		let authenticated_data = (raw_flags & AUTHENTICATED_DATA_FLAG) != 0;
		let checking_disabled = (raw_flags & CHECKING_DISABLED_FLAG) != 0;
		let rcode = match DNSResponseCode::try_from((raw_flags & RESPONSE_CODE_MASK) as u8) {
			Ok(code) => code,
			Err(_) => {
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					"Invalid DNS Response Code",
				))
			}
		};

		Ok(DNSMessageFlags {
			message_type: qr,
			opcode,
			authoritative_answer,
			truncated,
			recursion_desired,
			recursion_available,
			z,
			authenticated_data,
			checking_disabled,
			response_code: rcode,
		})
	}
}

#[derive(Debug, Clone)]
pub struct DNSLabels {
	labels: Vec<String>,
}

impl DNSLabels {
	pub fn from_domain_name(domain: &str) -> io::Result<Self> {
		let labels: Vec<String> = domain.split('.').map(|s| s.to_string()).collect();
		if labels.iter().any(|label| label.len() > 63) {
			return Err(std::io::Error::new(
				std::io::ErrorKind::InvalidInput,
				format!("DNS label too long in domain: {}", domain),
			));
		}
		Ok(Self { labels })
	}

	pub fn to_domain_name(&self) -> String {
		self.labels.join(".")
	}
}

impl ReadFromWithEndian for DNSLabels {
	fn read_from_with_endian<T: std::io::Read>(source: &mut T, endian: bytestruct::Endian) -> std::io::Result<Self> {
		let mut labels = Vec::new();
		loop {
			let length = u8::read_from_with_endian(source, endian)?;
			if length == 0 {
				break;
			}

			if length & 0b1100_0000 == 0b1100_0000 {
				println!("Skipping DNS label compression pointer");
				// This is a pointer, we need to skip the next byte as well
				u8::read_from_with_endian(source, endian)?;
				break;
			}

			if length > 63 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					format!("DNS label length too long: {}", length),
				));
			}

			let mut label_bytes = vec![0u8; length as usize];
			source.read_exact(&mut label_bytes)?;
			let label = String::from_utf8(label_bytes).map_err(|e| {
				std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("Invalid UTF-8 in DNS label: {}", e),
				)
			})?;
			labels.push(label);
		}

		Ok(Self { labels })
	}
}

impl WriteToWithEndian for DNSLabels {
	fn write_to_with_endian<T: std::io::Write>(
		&self,
		target: &mut T,
		endian: bytestruct::Endian,
	) -> std::io::Result<()> {
		for label in &self.labels {
			let length = label.len();
			if length > 63 {
				return Err(std::io::Error::new(
					std::io::ErrorKind::InvalidInput,
					format!("DNS label too long: {}", label),
				));
			}

			(length as u8).write_to_with_endian(target, endian)?;
			target.write_all(label.as_bytes())?;
		}

		// Write the null byte at the end of the labels
		0u8.write_to_with_endian(target, endian)?;

		Ok(())
	}
}

int_enum! {
#[derive(Debug, PartialEq, Clone)]
pub enum QType: u16 {
	/// Host address (IPv4)
	A = 0x0001,
	/// Authoritative name server
	NS = 0x0002,
	/// Mail destination (obsolete)
	MD = 0x0003,
	/// Mail forwarder (obsolete)
	MF = 0x0004,
	/// Canonical name for an alias
	CNAME = 0x0005,
	/// Start of authority record
	SOA = 0x0006,
	/// Mailbox domain name (experimental)
	MB = 0x0007,
	/// Mail group member (experimental)
	MG = 0x0008,
	/// Mail rename domain name (experimental)
	MR = 0x0009,
	/// Null resource record (experimental)
	NULL = 0x000A,
	/// Well known services
	WKS = 0x000B,
	/// Domain name pointer (reverse DNS)
	PTR = 0x000C,
	/// Host information
	HINFO = 0x000D,
	/// Mailbox or mail list information
	MINFO = 0x000E,
	/// Mail exchange
	MX = 0x000F,
	/// Text strings
	TXT = 0x0010,
	/// Responsible person
	RP = 0x0011,
	/// AFS database location
	AFSDB = 0x0012,
	/// X.25 PSDN address
	X25 = 0x0013,
	/// ISDN address
	ISDN = 0x0014,
	/// Route through
	RT = 0x0015,
	/// NSAP address
	NSAP = 0x0016,
	/// NSAP pointer
	NSAPPTR = 0x0017,
	/// Security signature
	SIG = 0x0018,
	/// Security key
	KEY = 0x0019,
	/// X.400 mail mapping information
	PX = 0x001A,
	/// Geographical position
	GPOS = 0x001D,
	/// IPv6 address
	AAAA = 0x001C,
	/// Location information
	LOC = 0x001D,
	/// Next domain (obsolete)
	NXT = 0x001E,
	/// Service location record
	SRV = 0x0021,
	/// Naming authority pointer
	NAPTR = 0x0023,
	/// Key exchanger
	KX = 0x0024,
	/// Certificate record
	CERT = 0x0025,
	/// IPv6 address (experimental)
	A6 = 0x0026,
	/// DNAME (delegation name)
	DNAME = 0x0027,
	/// Sink record (experimental)
	SINK = 0x0028,
	/// OPT pseudo-record (EDNS)
	OPT = 0x0029,
	/// Address prefix list
	APL = 0x002A,
	/// Delegation signer
	DS = 0x002B,
	/// DNSSEC signature
	RRSIG = 0x0046,
	/// DNSSEC next secure
	NSEC = 0x0047,
	/// DNSSEC key
	DNSKEY = 0x0030,
	/// DHCP identifier
	DHCID = 0x0031,
	/// DNSSEC next secure (version 3)
	NSEC3 = 0x0032,
	/// DNSSEC next secure parameters (version 3)
	NSEC3PARAM = 0x0033,
	/// TLSA certificate association
	TLSA = 0x0034,
	/// Host identity protocol
	HIP = 0x0037,
	/// Certification authority authorization
	CAA = 0x0101,
	/// Child DS (DNSSEC delegation signer)
	CDS = 0x003B,
	/// Child DNSKEY
	CDNSKEY = 0x003C,
	/// Child synchronization
	CSYNC = 0x0032,
	/// Sender policy framework (obsolete)
	SPF = 0x0063,
	/// Unspecified format
	UNSPEC = 0x0067,
	/// Node identifier
	NID = 0x0068,
	/// 32-bit locator
	L32 = 0x0069,
	/// 64-bit locator
	L64 = 0x006A,
	/// Locator pointer
	LP = 0x006B,
	/// EUI-48 address
	EUI48 = 0x0064,
	/// EUI-64 address
	EUI64 = 0x0065,
	/// SSH fingerprint
	SSHFP = 0x0044,
	/// TKEY (transaction key)
	TKEY = 0x0097,
	/// TSIG (transaction signature)
	TSIG = 0x00FA,
	/// Incremental zone transfer
	IXFR = 0x00FB,
	/// Axiomatic zone transfer
	AXFR = 0x00FC,
	/// Mailbox-related records
	MAILB = 0x00FD,
	/// Mail agent records
	MAILA = 0x00FE,
	/// All records (wildcard)
	ANY = 0x00FF,
	/// URI record
	URI = 0x0100,
	/// DNSSEC trust anchor
	TA = 0x0102,
	/// DNSSEC lookaside validation
	DLV = 0x0103,
}
}

int_enum! {
#[derive(Debug, Clone)]
pub enum QClass: u16 {
	/// Internet
	IN = 0x0001,
	/// Chaos network (historical)
	CH = 0x0003,
	/// Hesiod network (historical)
	HS = 0x0004,
	/// None (used in some DNSSEC operations)
	NONE = 0x00FE,
	/// Any class (wildcard)
	ANY = 0x00FF,
}
}

#[derive(Debug, ByteStruct, Clone)]
pub struct DNSQuestion {
	pub name: DNSLabels,
	pub qtype: QType,
	pub qclass: QClass,
}

#[derive(Debug, ByteStruct, Clone)]
pub struct DNSAnswer {
	pub name: DNSLabels,
	pub atype: QType,
	pub aclass: QClass,
	pub ttl: u32,
	pub rdata: LengthPrefixedVec<u8, u16>,
}

int_enum! {
#[derive(Debug, Clone)]
pub enum EDNSOptionCode: u16 {
	LLQ = 1,
	UL = 2,
	NSID = 3,
	DAU = 5,
	DHU = 6,
	N3U = 7,
	CLIENTSUBNET = 8,
	EXPIRE = 9,
	COOKIE = 10,
	TCPKEEPALIVE = 11,
	PADDING = 12,
	CHAIN = 13,
	KEYTAG = 14,
	ERROR = 15,
	CLIENTTAG = 16,
	SERVERTAG = 17,
	REPORTCHANNEL = 18,
	ZONEVERSION = 19,
	MQTYPEQUERY = 20,
	MQTYPERESPONSE = 21,
}
}

#[derive(Debug, ByteStruct, Clone)]
pub struct EDNSOption {
	pub code: EDNSOptionCode,
	pub data: LengthPrefixedVec<u8, u16>,
}

#[derive(Debug, Clone, ByteStruct)]
pub struct EDNSRecord {
	pub udp_payload_size: u16,
	pub extended_rcode: u8,
	pub edns_version: u8,
	pub flags: u16,
	pub options: LengthPrefixedVec<EDNSOption, u16>,
}

// This is a "raw" DNS answer that we read from the network, which may be an EDNS record
// or a normal answer. We need to parse it first to determine which one it is, and then
// we can convert it into either a DNSAnswer or an EDNSRecord.
#[derive(Debug, ByteStruct)]
struct RawDNSAnswer {
	name: DNSLabels,
	atype: QType,
	class_or_udp_size: u16,
	ttl_or_edns_data: u32,
	rdata: LengthPrefixedVec<u8, u16>,
}

impl TryInto<DNSAnswer> for RawDNSAnswer {
	type Error = String;

	fn try_into(self) -> Result<DNSAnswer, Self::Error> {
		if self.atype == QType::OPT {
			return Err("This is an EDNS record, not a normal DNS answer".to_string());
		}

		Ok(DNSAnswer {
			name: self.name,
			atype: self.atype,
			aclass: QClass::try_from(self.class_or_udp_size).map_err(|_| "Invalid QClass in DNS answer".to_string())?,
			ttl: self.ttl_or_edns_data,
			rdata: self.rdata,
		})
	}
}

impl TryInto<EDNSRecord> for RawDNSAnswer {
	type Error = String;

	fn try_into(self) -> Result<EDNSRecord, Self::Error> {
		if self.atype != QType::OPT {
			return Err("This is a normal DNS answer, not an EDNS record".to_string());
		}

		let udp_payload_size = self.class_or_udp_size;
		let extended_rcode = (self.ttl_or_edns_data >> 24) as u8;
		let edns_version = ((self.ttl_or_edns_data >> 16) & 0xFF) as u8;
		let flags = (self.ttl_or_edns_data & 0xFFFF) as u16;

		// We need to parse the options from the rdata, which is a length-prefixed vector of options
		let mut options = Vec::new();
		let mut option_data = &self.rdata[..];
		while !option_data.is_empty() {
			let option = EDNSOption::read_from_with_endian(&mut option_data, bytestruct::Endian::Big)
				.map_err(|e| format!("Failed to read EDNS option: {}", e))?;
			options.push(option);
		}

		Ok(EDNSRecord {
			udp_payload_size,
			extended_rcode,
			edns_version,
			flags,
			options: LengthPrefixedVec::new(options),
		})
	}
}
