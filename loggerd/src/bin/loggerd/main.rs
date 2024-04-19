mod api;
mod control;

use ::control::listen::ControlSocket;
use api::Api;
use std::{io::stderr, path::PathBuf, sync::Arc};

use clap::{Arg, Command};
use common::obs::assemble_logger;
use slog::{error, info};

use crate::control::Controller;

#[tokio::main]
async fn main() {
	let matches = Command::new("loggerd")
		.version("0.1.0")
		.author("Colin Douch")
		.about("A simple logging daemon")
		.arg(
			Arg::new("listen-path")
				.default_value("/run/loggerd/loggerd.sock")
				.long("listen-path")
				.short('l')
				.num_args(1)
				.help("The path to the unix socket to listen on"),
		)
		.arg(
			Arg::new("data-dir")
				.default_value("/var/log/loggerd")
				.long("data-dir")
				.short('d')
				.num_args(1)
				.help("The directory to store log files in"),
		)
		.get_matches();

	let logger = assemble_logger(stderr());
	let listen_path: &String = matches.get_one("listen-path").unwrap();
	let data_dir: &PathBuf = matches.get_one("data-dir").unwrap();
	info!(logger, "Listening on {}", listen_path);

	let api = Arc::new(Api::new(data_dir, logger.clone()));

	let control = ControlSocket::open(listen_path, Controller::new(api.clone())).unwrap();

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			info!(logger, "Shutting down");
		}
		_ = control.listen() => {
			error!(logger, "Control socket failed");
		},
		err = api.run() => {
			if let Err(e) = err {
				error!(logger, "Failed to run api: {}", e);
			}
		}
	}
}
