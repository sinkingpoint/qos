use std::{
	collections::{HashMap, VecDeque},
	io::{self, stderr},
	path::Path,
};

use bus::{BusClient, PublishHook};
use netlink::{AsyncNetlinkSocket, NetlinkKObjectUEvent};
use slog::{error, info};
use tokio::{
	fs::{read_dir, OpenOptions},
	io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
};

const BUSD_TOPIC: &str = "udev_events";

// The presence of the SEQ_NUM_KEY KV indicates the end of a single event.
const SEQ_NUM_KEY: &str = "SEQNUM";

#[tokio::main]
async fn main() {
	let logger = common::obs::assemble_logger(stderr());
	let socket = AsyncNetlinkSocket::<NetlinkKObjectUEvent>::new(1).unwrap();

	let bus_socket = BusClient::new().await.unwrap().publish(BUSD_TOPIC).await.unwrap();

	let el_logger = logger.clone();
	let hook = tokio::spawn(async move {
		if let Err(e) = event_loop(&el_logger, socket, bus_socket).await {
			error!(el_logger, "Error in event loop"; "error" => e.to_string());
		}
	});

	info!(logger, "Starting udevd");
	let device_count = match do_initial_device_add(&logger).await {
		Ok(device_count) => device_count,
		Err(e) => {
			error!(logger, "Failed to add initial devices"; "error" => e.to_string());
			return;
		}
	};

	info!(logger, "Finished initial device add"; "device_count" => device_count);

	tokio::join!(hook).0.unwrap();

	info!(logger, "Exiting udevd");
}

async fn event_loop<T: AsyncWrite + Unpin>(
	logger: &slog::Logger,
	socket: AsyncNetlinkSocket<NetlinkKObjectUEvent>,
	mut output: PublishHook<T>,
) -> io::Result<()> {
	let reader = BufReader::new(socket);
	let mut segments = reader.split(b'\0');
	let mut current_event = HashMap::new();

	// Udev events come in the form:
	// <summary>
	// K1=V1
	// K2=V2
	// ...
	// SEQNUM=<number>
	// So this reads those groups of lines, and merges them into single
	// events that can be easily consumed by downstream services.

	while let Some(line) = segments.next_segment().await? {
		if line.is_empty() {
			error!(logger, "Received empty netlink message");
			continue;
		}

		let line = match String::from_utf8(line) {
			Ok(line) => line,
			Err(e) => {
				error!(logger, "Failed to parse netlink message"; "error" => e.to_string());
				continue;
			}
		};

		if !line.contains('=') {
			// This is the summary line.
			current_event.insert(String::from("summary"), line.to_owned());
			continue;
		}

		// Otherwise this is a K=V line.
		let (key, value) = {
			let mut parts = line.splitn(2, '=');
			(parts.next().unwrap(), parts.next().unwrap())
		};

		current_event.insert(key.to_owned(), value.to_owned());
		if key == SEQ_NUM_KEY {
			// SEQNUM is always the last key of an event, so flush it.
			let output_event = match serde_json::to_string(&current_event) {
				Ok(o) => o,
				Err(e) => {
					error!(logger, "failed to construct event"; "error" => e.to_string());
					current_event.clear();
					continue;
				}
			};
			current_event.clear();

			output.publish_message(output_event.as_bytes()).await?;
		}
	}

	Ok(())
}

// This function is called when the udevd daemon starts up. It is responsible for
// scanning the /sys directory and adding all devices that are already present.
// This is done by calling the `add_device` function for each device.
async fn do_initial_device_add(logger: &slog::Logger) -> io::Result<usize> {
	let mut queue = VecDeque::new();
	queue.push_back("/sys".to_string());
	let mut device_count = 0;

	while let Some(path) = queue.pop_front() {
		let mut dir = match read_dir(&path).await {
			Ok(dir) => dir,
			Err(_) => continue,
		};

		while let Some(entry) = dir.next_entry().await? {
			let path = entry.path();
			if path.is_symlink() {
				continue;
			}

			if path.is_dir() {
				queue.push_back(path.to_str().unwrap().to_string());
			} else if path.is_file() && path.file_name().unwrap() == "uevent" {
				add_device(logger, &path).await;
				device_count += 1;
			}
		}
	}

	Ok(device_count)
}

async fn add_device(logger: &slog::Logger, path: &Path) {
	let mut file = match OpenOptions::new().write(true).open(path).await {
		Ok(file) => file,
		Err(e) => {
			error!(logger, "Failed to open uevent file"; "path" => path.to_str().unwrap(), "error" => e.to_string());
			return;
		}
	};

	if let Err(e) = file.write_all(b"add\n").await {
		error!(logger, "Failed to write to uevent file"; "path" => path.to_str().unwrap(), "error" => e.to_string());
	}
}
