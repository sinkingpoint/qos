pub mod control;
mod disk;

use std::{
	fs::File,
	io::{self, Seek, SeekFrom},
};

use bytestruct::{ReadFrom, WriteTo};
use chrono::{DateTime, Utc};
use disk::{BlockType, EntryBlock};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ConnectionHeader {
	LogStream { fields: Vec<KV> },
}

#[derive(Debug, Deserialize, Serialize)]
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

/// A log file that is open for writing.
pub struct OpenLogFile {
	pub path: String,
	pub file: File,

	/// The header block of the log file.
	header: disk::HeaderBlock,

	/// The offset and contents of the last entry block in the file.
	last_entry_block: Option<(u64, EntryBlock)>,
}

impl OpenLogFile {
	/// Creates a new log file at the given path.
	pub async fn new(path: &str) -> io::Result<Self> {
		let file = File::create_new(path)?;
		let mut file = OpenLogFile {
			path: path.to_string(),
			file,
			header: disk::HeaderBlock::default(),
			last_entry_block: None,
		};

		file.write_header().await?;

		Ok(file)
	}

	/// Open an existing log file at the given path.
	pub async fn open(path: &str) -> io::Result<Self> {
		let mut file = File::open(path)?;
		let header = disk::HeaderBlock::read_from(&mut file)?;

		if let Err(e) = header.validate() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, e));
		}

		// Find the last entry block by following the linked list.
		let mut offset = header.first_entry_block_offset;
		while offset != 0 {
			file.seek(SeekFrom::Start(offset))?;
			let block = EntryBlock::read_from(&mut file)?;

			if block.header.next_entry_block_offset == 0 {
				break;
			}

			offset = block.header.next_entry_block_offset;
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
			path: path.to_string(),
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

		BlockType::Field.write_to(&mut self.file)?;
		field_offsets.push(self.file.seek(SeekFrom::End(0))?);
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
			block.header.next_entry_block_offset = next_offset;
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

	pub async fn read_log_stream(
		&mut self,
		time_min: Option<chrono::DateTime<Utc>>,
		time_max: Option<chrono::DateTime<Utc>>,
	) {
		println!("{:?}", self.header);
		if let Some(time_min) = time_min {
			if time_min > self.header.time_max {
				return;
			}
		}

		if let Some(time_max) = time_max {
			if time_max < self.header.time_min {
				return;
			}
		}

		let mut offset = self.header.first_entry_block_offset;
		while offset != 0 {
			self.file.seek(SeekFrom::Start(offset)).unwrap();
			let block = EntryBlock::read_from(&mut self.file).unwrap();
			println!("{} {:?}", offset, block);

			offset = block.header.next_entry_block_offset;
		}
	}

	/// Writes the header block to the start of the file.
	async fn write_header(&mut self) -> io::Result<()> {
		self.file.seek(SeekFrom::Start(0))?;
		self.header.write_to(&mut self.file)?;
		self.file.seek(SeekFrom::End(0))?;
		Ok(())
	}
}
