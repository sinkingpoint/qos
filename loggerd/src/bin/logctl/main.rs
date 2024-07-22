use std::{
	io::stderr,
	path::{Path, PathBuf},
};

use clap::{Arg, Command};
use loggerd::{control::ReadStreamOpts, DEFAULT_CONTROL_SOCKET_PATH, KV};
use slog::{error, Logger};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() {
	let matches = Command::new("logctl")
		.arg(
			Arg::new("control-socket")
				.default_value(DEFAULT_CONTROL_SOCKET_PATH)
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
				),
		)
		.subcommand_required(true)
		.get_matches();

	let logger = common::obs::assemble_logger(stderr());
	let socket_path: &String = matches.get_one("control-socket").expect("expected control-socket");
	let socket_path = PathBuf::from(socket_path);

	match matches.subcommand() {
		Some(("write", write_matches)) => {
			let fields = match write_matches.get_many("kvs") {
				Some(fields) => fields.copied().collect(),
				None => Vec::new(),
			};

			let fields = match validate_kvs(fields) {
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

			start_read_stream(logger, &socket_path, opts).await;
		}
		_ => {
			unreachable!("Subcommand is required")
		}
	}
}

/// Takes a list of key-value pairs in the form `key=value` and returns a list of `KV` structs,
/// or an error if any of the kvs are malformed.
fn validate_kvs(kvs: Vec<&str>) -> Result<Vec<KV>, String> {
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

async fn start_read_stream(logger: Logger, socket_path: &Path, opts: ReadStreamOpts) {
	let socket = match loggerd::control::start_read_stream(socket_path, opts).await {
		Ok(socket) => socket,
		Err(e) => {
			error!(logger, "Failed to start read stream: {}", e);
			return;
		}
	};

	let reader = BufReader::new(socket);
	let mut lines = reader.lines();

	loop {
		let line = match lines.next_line().await {
			Ok(Some(line)) => line,
			Ok(None) => break,
			Err(e) => {
				error!(logger, "Failed to read from socket: {}", e);
				break;
			}
		};

		println!("{}", line);
	}
}
