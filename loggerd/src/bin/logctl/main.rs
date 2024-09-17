use std::{
	collections::HashMap,
	io::{stderr, Cursor, ErrorKind},
	path::{Path, PathBuf},
};

use bytestruct::{Endian, ReadFromWithEndian};
use clap::{Arg, Command};
use loggerd::{control::ReadStreamOpts, DEFAULT_CONTROL_SOCKET_PATH, KV};
use slog::{error, Logger};
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() {
	let matches = Command::new("logctl")
		.arg(
			Arg::new("control-socket")
				.default_value(DEFAULT_CONTROL_SOCKET_PATH)
				.short('c')
				.long("control-socket")
				.num_args(1)
				.help("The path to the control socket for loggerd"),
		)
		.subcommand(
			Command::new("write").arg(
				Arg::new("kvs")
					.num_args(0..)
					.help("Key-value pairs to include in the log"),
			),
		)
		.subcommand(
			Command::new("read")
				.arg(
					Arg::new("min_time")
						.long("since")
						.num_args(1)
						.help("The time to start reading from"),
				)
				.arg(
					Arg::new("max_time")
						.long("until")
						.num_args(1)
						.help("The time to finish reading at"),
				)
				.arg(
					Arg::new("format")
						.short('o')
						.long("output")
						.default_value("text")
						.help("The format to write logs in"),
				),
		)
		.subcommand_required(true)
		.get_matches();

	let logger = common::obs::assemble_logger(stderr());
	let socket_path: &String = matches.get_one("control-socket").expect("expected control-socket");
	let socket_path = PathBuf::from(socket_path);

	match matches.subcommand() {
		Some(("write", write_matches)) => {
			let fields: Vec<String> = match write_matches.get_many("kvs") {
				Some(fields) => fields.cloned().collect(),
				None => Vec::new(),
			};

			let fields = match validate_kvs(&fields) {
				Ok(kvs) => kvs,
				Err(e) => {
					error!(logger, "Failed to validate key-value pairs: {}", e);
					return;
				}
			};

			start_write_stream(logger, &socket_path, fields).await;
		}
		Some(("read", read_matches)) => {
			let mut opts = ReadStreamOpts::new();

			if let Some(min_time) = read_matches.get_one::<String>("min_time") {
				let min_time = chrono::DateTime::parse_from_rfc3339(min_time)
					.map_err(|e| format!("Failed to parse min_time: {}", e))
					.unwrap()
					.into();
				opts = opts.with_min_time(min_time);
			}

			if let Some(max_time) = read_matches.get_one::<String>("max_time") {
				let max_time = chrono::DateTime::parse_from_rfc3339(max_time)
					.map_err(|e| format!("Failed to parse max_time: {}", e))
					.unwrap()
					.into();
				opts = opts.with_max_time(max_time);
			}

			let log_format = read_matches.get_one::<String>("format").map_or("text", |s| s.as_str());

			let log_format = match OutputLogFormat::try_from(log_format) {
				Ok(f) => f,
				Err(e) => {
					eprintln!("{}", e);
					return;
				}
			};

			start_read_stream(logger, &socket_path, opts, log_format).await;
		}
		_ => {
			unreachable!("Subcommand is required")
		}
	}
}

/// Takes a list of key-value pairs in the form `key=value` and returns a list of `KV` structs,
/// or an error if any of the kvs are malformed.
fn validate_kvs(kvs: &Vec<String>) -> Result<Vec<KV>, String> {
	let mut result = vec![];
	for kv in kvs {
		let (key, value) = match kv.split_once('=') {
			Some(kv) => kv,
			None => return Err(format!("invalid kv: {}", kv)),
		};

		result.push(KV {
			key: key.to_string(),
			value: value.to_string(),
		});
	}

	Ok(result)
}

/// Starts a write stream to the loggerd instance at the given socket path, reading
/// logs from stdin and sending them to loggerd.
async fn start_write_stream(logger: Logger, socket_path: &Path, kvs: Vec<KV>) {
	let mut socket = match loggerd::control::start_write_stream(socket_path, kvs).await {
		Ok(socket) => socket,
		Err(e) => {
			error!(logger, "Failed to start write stream: {}", e);
			return;
		}
	};

	let mut stdin = BufReader::new(io::stdin()).lines();
	while let Ok(Some(line)) = stdin.next_line().await {
		match socket.write_all(line.as_bytes()).await {
			Ok(_) => {}
			Err(_) => {
				error!(logger, "Failed to write to socket");
				break;
			}
		}

		match socket.write_all(b"\n").await {
			Ok(_) => {}
			Err(_) => {
				error!(logger, "Failed to write to socket");
				break;
			}
		}
	}
}

#[derive(Debug)]
pub enum OutputLogFormat {
	Text,
	JSON,
}

impl TryFrom<&str> for OutputLogFormat {
	type Error = String;
	fn try_from(value: &str) -> Result<Self, Self::Error> {
		match value {
			"text" => Ok(Self::Text),
			"json" => Ok(Self::JSON),
			_ => Err(format!("unknown log format: {}", value)),
		}
	}
}

impl OutputLogFormat {
	fn format_log(&self, log: &HashMap<String, String>) -> String {
		match self {
			Self::JSON => serde_json::to_string(log).expect("format log"),
			Self::Text => {
				let mut kv_string = String::new();
				for (k, v) in log {
					if k.starts_with("__") {
						continue;
					}

					kv_string.push_str(&format!(" {}={}", k, v));
				}

				let timestamp: &str = log.get("__timestamp").map_or("<no timestamp>", |s| s.as_str());
				let msg = log.get("__msg").map_or("<no message>", |s| s.as_str());

				format!("{}{} {}", timestamp, kv_string, msg)
			}
		}
	}
}

async fn start_read_stream(logger: Logger, socket_path: &Path, opts: ReadStreamOpts, format: OutputLogFormat) {
	let socket = match loggerd::control::start_read_stream(socket_path, opts).await {
		Ok(socket) => socket,
		Err(e) => {
			error!(logger, "Failed to start read stream: {}", e);
			return;
		}
	};

	let mut reader = BufReader::new(socket);

	loop {
		let mut len_bytes = vec![0; 4];
		match reader.read_exact(&mut len_bytes).await {
			Ok(_) => {}
			Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
			Err(e) => {
				eprintln!("failed to read log socket: {}", e);
				break;
			}
		}
		let len = <u32>::read_from_with_endian(&mut Cursor::new(&len_bytes), Endian::Little)
			.expect("sucessful read len from vec");
		let mut data = vec![0_u8; len as usize];
		match reader.read_exact(&mut data).await {
			Ok(_) => {}
			Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
			Err(e) => {
				eprintln!("failed to read log socket: {}", e);
				break;
			}
		}

		let msg = match serde_json::from_slice::<HashMap<String, String>>(&data) {
			Ok(m) => m,
			Err(e) => {
				eprintln!("failed to read log socket: {}", e);
				break;
			}
		};

		println!("{}", format.format_log(&msg));
	}
}
