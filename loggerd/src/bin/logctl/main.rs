use std::{
	io::stderr,
	path::{Path, PathBuf},
};

use clap::{Arg, Command};
use loggerd::{DEFAULT_CONTROL_SOCKET_PATH, KV};
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
		.subcommand_required(true)
		.get_matches();

	let logger = common::obs::assemble_logger(stderr());

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

			let socket_path: &String = matches.get_one("control-socket").expect("expected control-socket");
			let socket_path = PathBuf::from(socket_path);
			start_write_stream(logger, &socket_path, fields).await;
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
		let parts: Vec<&str> = kv.splitn(2, '=').collect();
		if parts.len() != 2 {
			return Err(format!("Invalid key-value pair: {}", kv));
		}

		result.push(KV {
			key: parts[0].to_string(),
			value: parts[1].to_string(),
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
