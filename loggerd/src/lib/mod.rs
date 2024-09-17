pub mod control;
mod disk;

use std::{
	fs::File,
	io::{self, ErrorKind, Seek, SeekFrom},
	path::{Path, PathBuf},
};

use bytestruct::{ReadFrom, ReadFromWithEndian, WriteTo};
use chrono::{DateTime, Utc};
use control::ReadStreamOpts;
use disk::{BlockType, EntryBlock, FieldBlock};
use serde::{Deserialize, Serialize};

/// The default path to the control socket.
pub const DEFAULT_CONTROL_SOCKET_PATH: &str = "/run/loggerd/loggerd.sock";

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ConnectionHeader {
	LogStream { fields: Vec<KV> },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KV {
	pub key: String,
	pub value: String,
}

#[derive(Debug)]
pub struct LogMessage {
	pub timestamp: DateTime<Utc>,
	pub fields: Vec<KV>,
	pub message: String,
}

impl LogMessage {
	pub fn new(timestamp: DateTime<Utc>, fields: Vec<KV>, message: String) -> Self {
		LogMessage {
			timestamp,
			fields,
			message,
		}
	}
}

/// A log file that is open for writing.
#[derive(Debug)]
pub struct OpenLogFile {
	pub path: PathBuf,
	pub file: File,

	/// The header block of the log file.
	pub header: disk::HeaderBlock,

	/// The offset and contents of the last entry block in the file.
	last_entry_block: Option<(u64, EntryBlock)>,
}

impl OpenLogFile {
	/// Creates a new log file at the given path.
	pub async fn new(path: &Path) -> io::Result<Self> {
		let file = File::create_new(path)?;
		let mut file = OpenLogFile {
			path: path.to_owned(),
			file,
			header: disk::HeaderBlock::default(),
			last_entry_block: None,
		};

		file.write_header().await?;

		Ok(file)
	}

	pub fn read_entry_at(&mut self, offset: u64) -> io::Result<(LogMessage, u64)> {
		let current_offset = self.file.stream_position()?;
		self.file.seek(SeekFrom::Start(offset))?;
		let res = EntryBlock::read_from(&mut self.file)?;
		let mut message = None;
		let mut fields = Vec::new();
		for offset in res.field_offsets {
			self.file.seek(SeekFrom::Start(offset))?;
			let block_type = BlockType::read_from_with_endian(&mut self.file, bytestruct::Endian::Little)?;
			if !matches!(block_type, BlockType::Field) {
				return Err(io::Error::new(
					ErrorKind::InvalidData,
					format!("invalid block type. Expected Field, got: {:?}", block_type),
				));
			}

			let field = FieldBlock::read_from(&mut self.file)?;

			if field.key.0 == "message" && !field.value.0.is_empty() {
				message = Some(field.value.0);
				continue;
			}

			fields.push(KV {
				key: field.key.0,
				value: field.value.0,
			});
		}

		self.file.seek(SeekFrom::Start(current_offset))?;
		let message = message.unwrap_or(String::from("<no message>"));

		Ok((
			LogMessage::new(res.entry_header.time, fields, message),
			res.entry_header.next_entry_block_offset,
		))
	}

	/// Open an existing log file at the given path.
	pub async fn open(path: &Path) -> io::Result<Self> {
		let mut file = File::options().read(true).write(true).open(path)?;
		let header = disk::HeaderBlock::read_from(&mut file)?;

		if let Err(e) = header.validate() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, e));
		}

		// Find the last entry block by following the linked list.
		let mut offset = header.first_entry_block_offset;
		while offset != 0 {
			file.seek(SeekFrom::Start(offset))?;
			let block = EntryBlock::read_from(&mut file)?;

			if block.entry_header.next_entry_block_offset == 0 {
				break;
			}

			offset = block.entry_header.next_entry_block_offset;
		}

		// Read the last entry block.
		let block = if offset != 0 {
			file.seek(SeekFrom::Start(offset))?;
			Some((offset, EntryBlock::read_from(&mut file)?))
		} else {
			None
		};

		file.seek(SeekFrom::End(0))?;

		Ok(OpenLogFile {
			path: path.to_owned(),
			file,
			header,
			last_entry_block: block,
		})
	}

	/// Writes a log message to the log file.
	pub async fn write_log(&mut self, message: LogMessage) -> io::Result<()> {
		// Write all the fields and collect the offsets.
		let mut field_offsets = vec![];
		for field in message.fields {
			field_offsets.push(self.file.seek(SeekFrom::End(0))?);
			BlockType::Field.write_to(&mut self.file)?;
			disk::FieldBlock::new(field.key, field.value).write_to(&mut self.file)?;
		}

		field_offsets.push(self.file.seek(SeekFrom::End(0))?);
		BlockType::Field.write_to(&mut self.file)?;
		disk::FieldBlock::new("message".to_string(), message.message).write_to(&mut self.file)?;

		// Write the entry block.
		let next_offset = self.file.seek(SeekFrom::End(0))?;
		let block = disk::EntryBlock::new(message.timestamp, field_offsets);
		block.write_to(&mut self.file)?;

		// Update the pointers in the file to the new entry block.
		if self.header.first_entry_block_offset == 0 {
			self.header.first_entry_block_offset = next_offset;
			self.header.time_min = message.timestamp;
			self.header.time_max = message.timestamp;

			self.write_header().await?;
		} else if let Some((offset, mut block)) = self.last_entry_block.take() {
			block.entry_header.next_entry_block_offset = next_offset;
			self.file.seek(SeekFrom::Start(offset))?;
			block.write_to(&mut self.file)?;
		} else {
			return Err(io::Error::new(
				io::ErrorKind::Other,
				"no last entry block, even though the header block thinks there is",
			));
		}

		self.last_entry_block = Some((next_offset, block));

		Ok(())
	}

	/// Reads the log stream from the log file.
	pub async fn read_log_stream(self, opts: ReadStreamOpts) -> impl Iterator<Item = io::Result<LogMessage>> {
		ReadIter::new(self, opts)
	}

	/// Writes the header block to the start of the file.
	async fn write_header(&mut self) -> io::Result<()> {
		self.file.seek(SeekFrom::Start(0))?;
		self.header.write_to(&mut self.file)?;
		self.file.seek(SeekFrom::End(0))?;
		Ok(())
	}
}

struct ReadIter {
	file: OpenLogFile,
	opts: ReadStreamOpts,
	offset: u64,
}

impl ReadIter {
	fn new(file: OpenLogFile, opts: ReadStreamOpts) -> Self {
		let offset = file.header.first_entry_block_offset;
		ReadIter { file, opts, offset }
	}
}

impl Iterator for ReadIter {
	type Item = io::Result<LogMessage>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.offset == 0 {
			return None;
		}

		while self.offset != 0 {
			let (message, next_offset) = match self.file.read_entry_at(self.offset) {
				Ok(message) => message,
				Err(e) => return Some(Err(e)),
			};

			if self.opts.matches(&message) {
				self.offset = next_offset;
				return Some(Ok(message));
			}

			self.offset = next_offset;
		}

		None
	}
}
