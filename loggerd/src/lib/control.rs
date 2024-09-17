use std::{collections::HashMap, path::Path};

use bytestruct::{Endian, WriteToWithEndian};
use chrono::{DateTime, Utc};
use thiserror::Error;
use tokio::{
	io::{self, AsyncWriteExt},
	net::{UnixSocket, UnixStream},
};

use crate::{LogMessage, KV};

pub const START_WRITE_STREAM_ACTION: &str = "start-write-stream";

pub const START_READ_STREAM_ACTION: &str = "start-read-stream";

const MIN_TIME_HEADER: &str = "_MIN_TIME";
const MAX_TIME_HEADER: &str = "_MAX_TIME";
const FOLLOW_HEADER: &str = "_FOLLOW";

/// Starts a write stream with the given fields, returning the socket that can then be used
/// to stream logs to a loggerd instance.
pub async fn start_write_stream(socket_path: &Path, fields: Vec<KV>) -> io::Result<UnixStream> {
	let mut conn = UnixSocket::new_stream()?.connect(socket_path).await?;
	let fields_str = fields
		.iter()
		.map(|kv| format!("{}={}", kv.key, kv.value))
		.collect::<Vec<_>>()
		.join(" ");
	let header_string = format!("ACTION={} {}\n", START_WRITE_STREAM_ACTION, fields_str);
	conn.write_all(header_string.as_bytes()).await?;

	Ok(conn)
}

/// Starts a read stream with the given options, returning the socket that can then be used
/// to read logs from a loggerd instance.
pub async fn start_read_stream(socket_path: &Path, opts: ReadStreamOpts) -> io::Result<UnixStream> {
	let mut conn = UnixSocket::new_stream()?.connect(socket_path).await?;
	let header_string = format!("ACTION={} {}\n", START_READ_STREAM_ACTION, opts.to_header_string());
	conn.write_all(header_string.as_bytes()).await?;

	Ok(conn)
}

#[derive(Debug, Clone, Error)]
pub enum ReadStreamOptsParseError {
	#[error("invalid time: {0}")]
	ParseError(#[from] chrono::ParseError),

	#[error("invalid follow: {0}")]
	InvalidFollow(#[from] std::str::ParseBoolError),
}

/// A Builder for the different ways you can filter a log stream.
#[derive(Debug, Clone)]
pub struct ReadStreamOpts {
	min_time: Option<DateTime<Utc>>,
	max_time: Option<DateTime<Utc>>,
	filters: Option<Vec<KV>>,
	follow: bool,
}

impl ReadStreamOpts {
	pub fn new() -> Self {
		Self {
			min_time: None,
			max_time: None,
			filters: None,
			follow: false,
		}
	}

	pub fn from_kvs(kvs: &[(&str, &str)]) -> Result<Self, ReadStreamOptsParseError> {
		let mut opts = Self::new();
		let mut filters = Vec::new();
		for (key, value) in kvs {
			match *key {
				key if key == MIN_TIME_HEADER => {
					let min_time = DateTime::parse_from_rfc3339(value)?.into();
					opts = opts.with_min_time(min_time)
				}
				key if key == MAX_TIME_HEADER => {
					let max_time = DateTime::parse_from_rfc3339(value)?.into();
					opts = opts.with_max_time(max_time)
				}
				key if key == FOLLOW_HEADER => {
					opts = opts.with_follow(value.parse()?);
				}
				_ => {
					filters.push(KV {
						key: key.to_string(),
						value: value.to_string(),
					});
				}
			}
		}

		Ok(opts.with_filters(filters))
	}

	pub fn with_min_time(mut self, min_time: DateTime<Utc>) -> Self {
		self.min_time = Some(min_time);
		self
	}

	pub fn with_max_time(mut self, max_time: DateTime<Utc>) -> Self {
		self.max_time = Some(max_time);
		self
	}

	pub fn with_filters(mut self, filters: Vec<KV>) -> Self {
		self.filters = Some(filters);
		self
	}

	pub fn with_follow(mut self, follow: bool) -> Self {
		self.follow = follow;
		self
	}

	pub fn format_log(&self, log: &LogMessage) -> Vec<u8> {
		let mut msg = HashMap::new();
		msg.insert("__timestamp", log.timestamp.to_rfc3339());
		msg.insert("__msg", log.message.clone());
		for kv in log.fields.iter() {
			msg.insert(&kv.key, kv.value.to_owned());
		}

		let log = serde_json::to_string(&msg).expect("failed to format log");
		let log_bytes = log.as_bytes();
		let frame = log.len() as u32;

		let mut bytes = Vec::with_capacity(log_bytes.len() + 4);
		frame
			.write_to_with_endian(&mut bytes, Endian::Little)
			.expect("write to local buffer");

		bytes.extend_from_slice(log_bytes);

		bytes
	}

	pub fn matches(&self, log: &LogMessage) -> bool {
		if let Some(min_time) = self.min_time {
			if log.timestamp < min_time {
				return false;
			}
		}

		if let Some(max_time) = self.max_time {
			if log.timestamp > max_time {
				return false;
			}
		}

		if let Some(filters) = &self.filters {
			for filter in filters {
				if let Some(kv) = log.fields.iter().find(|f| f.key == filter.key) {
					if kv.value != filter.value {
						return false;
					}
				}
			}
		}
		true
	}

	pub fn to_header_string(&self) -> String {
		let mut parts = Vec::new();
		if let Some(min_time) = self.min_time {
			parts.push(format!("{}={}", MIN_TIME_HEADER, min_time.to_rfc3339()));
		}
		if let Some(max_time) = self.max_time {
			parts.push(format!("{}={}", MAX_TIME_HEADER, max_time.to_rfc3339()));
		}
		if let Some(filters) = &self.filters {
			for filter in filters {
				parts.push(format!("{}={}", filter.key, filter.value));
			}
		}
		parts.push(format!("{}={}", FOLLOW_HEADER, self.follow));
		parts.join(" ")
	}
}

impl Default for ReadStreamOpts {
	fn default() -> Self {
		Self::new()
	}
}
