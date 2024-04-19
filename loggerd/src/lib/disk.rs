use bytestruct::{LengthPrefixedString, Padding, Size, WriteTo, WriteToWithEndian};
use bytestruct_derive::{ByteStruct, Size};
use chrono::{DateTime, Utc};

const MAX_FIELD_SIZE: usize = 48000;
const VERSION: u8 = 1;
const MAGIC: &[u8; 8] = b"QLOGFILE";

/// The compression algorithm used for the log file.
#[derive(Debug, ByteStruct, Size)]
#[repr(u8)]
pub enum Compression {
	None,
	Gzip,
}

#[derive(Debug, ByteStruct, Size)]
#[repr(u8)]
pub enum BlockType {
	Checkpoint,
	Entry,
	Field,
}

impl WriteTo for BlockType {
	fn write_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
		self.write_to_with_endian(writer, bytestruct::Endian::Little)
	}
}

/// The header block of the log file.
#[derive(Debug, ByteStruct, Size)]
#[little_endian]
pub struct HeaderBlock {
	magic: [u8; 8],

	/// The version of the log file format.
	pub version: u8,
	/// The compression algorithm used for the log file.
	pub compression: Compression,

	reserved: u16,

	/// The ID of the machine that generated the log file.
	pub machine_id: u32,

	/// The time range of the log file.
	pub time_min: DateTime<Utc>,
	pub time_max: DateTime<Utc>,

	/// The offset of the first hash block in the file.
	pub first_hash_block_offset: u64,

	/// The offset of the first entry block in the file.
	pub first_entry_block_offset: u64,

	/// The offset of the first checkpoint block in the file.
	pub first_checkpoint_block_offset: u64,
}

impl Default for HeaderBlock {
	fn default() -> Self {
		Self {
			magic: *MAGIC,
			version: VERSION,
			compression: Compression::None,
			reserved: 0,
			machine_id: 0,
			time_min: Utc::now(),
			time_max: Utc::now(),
			first_hash_block_offset: 0,
			first_entry_block_offset: 0,
			first_checkpoint_block_offset: 0,
		}
	}
}

impl HeaderBlock {
	pub fn validate(&self) -> Result<(), String> {
		if *MAGIC != self.magic {
			return Err("Invalid magic number".to_string());
		}

		if VERSION != self.version {
			return Err("Invalid version number".to_string());
		}
		Ok(())
	}
}

#[derive(Debug, ByteStruct, Size)]
struct BlockHeader {
	block_type: BlockType,
	block_size: u64,
}

/// A block containing a hash of the log entries that occur before this block.
#[derive(Debug, ByteStruct, Size)]
#[little_endian]
pub struct CheckpointBlock {
	header: BlockHeader,

	/// The SHA-256 hash of the log entries that occured between the previous checkpoint block and this one.
	pub hash: u64,

	/// The time that the checkpoint was created. Should be >= the last log before the checkpoint and <= the first log after the checkpoint.
	pub time: DateTime<Utc>,

	/// The offset of the next checkpoint block in the file, or 0 if this is the last checkpoint block.
	pub next_checkpoint_block_offset: u64,
	_unused: Padding<64>,
}

#[derive(Debug, ByteStruct, Size)]
pub struct EntryBlockHeader {
	/// The time of the first log entry in the block.
	pub time: DateTime<Utc>,

	/// The offset of the next entry block in the file, or 0 if this is the last entry block.
	pub next_entry_block_offset: u64,
}

/// A block containing a log entry.
#[derive(Debug, ByteStruct, Size)]
#[little_endian]
pub struct EntryBlock {
	header: BlockHeader,

	pub entry_header: EntryBlockHeader,

	/// An array of offsets to the fields in the block.
	pub field_offsets: Vec<u64>,
}

impl EntryBlock {
	pub fn new(time: DateTime<Utc>, field_offsets: Vec<u64>) -> Self {
		let mut new = Self {
			header: BlockHeader {
				block_type: BlockType::Entry,
				block_size: 0,
			},
			entry_header: EntryBlockHeader {
				time,
				next_entry_block_offset: 0,
			},
			field_offsets,
		};

		new.header.block_size = new.size() as u64;

		new
	}
}

/// A block containing a field of a log entry.
#[derive(Debug, ByteStruct, Size)]
#[little_endian]
pub struct FieldBlock {
	header: BlockHeader,

	/// The key of the field.
	key: LengthPrefixedString<MAX_FIELD_SIZE>,

	/// The value of the field.
	value: LengthPrefixedString<MAX_FIELD_SIZE>,

	_unused: Padding<64>,
}

impl FieldBlock {
	pub fn new(key: String, value: String) -> Self {
		let key = LengthPrefixedString(key);
		let value = LengthPrefixedString(value);
		let padding = Padding::new(key.size() + value.size());
		Self {
			header: BlockHeader {
				block_type: BlockType::Field,
				block_size: key.size() as u64 + value.size() as u64 + padding.size() as u64,
			},
			key,
			value,
			_unused: padding,
		}
	}
}
