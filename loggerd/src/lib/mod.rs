use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ConnectionHeader {
	LogStream { fields: HashMap<String, String> },
}

#[derive(Debug)]
pub struct LogMessage {
	pub timestamp: DateTime<Utc>,
	pub fields: HashMap<String, String>,
	pub message: String,
}

pub struct LogfileHeader {
	version: u8,
	time_min: DateTime<Utc>,
	time_max: DateTime<Utc>,
	n_chunks: u32,
	n_records: u32,
}

pub struct ChunkHeader {
	n_records: u32,
	n_indexes: u32,
}

pub enum Index {
	IntRange { start: i64, end: i64 },
	StrRange { start: String, end: String },
	Exact(Vec<String>),
}
