use std::{
	collections::VecDeque,
	io::{self, stderr},
	path::Path,
};

use bus::BusClient;
use netlink::{AsyncNetlinkSocket, NetlinkKObjectUEvent};
use slog::{error, info};
use tokio::{
	fs::{read_dir, OpenOptions},
	io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
};

const BUSD_TOPIC: &str = "udev_events";

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
	mut output: T,
) -> io::Result<()> {
	let reader = BufReader::new(socket);
	let mut segments = reader.split(b'\0');

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

		output.write_all(line.as_bytes()).await?;
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
