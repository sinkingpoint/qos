mod config;
mod service;

use std::{collections::HashMap, io::stderr, path::PathBuf, process::ExitCode};

use common::obs::assemble_logger;
use config::load_config;
use service::Service;
use slog::error;

fn main() -> ExitCode {
	let logger = assemble_logger(stderr());

	let config_directories = ["./configs/services", "/etc/qinit/services"].map(PathBuf::from);

	let (config, errors) = load_config(config_directories);
	if errors.is_error() {
		error!(logger, "Error loading configuration"; "errors" => format!("{:?}", errors));
	}

	let errors = config.validate();
	if errors.is_error() {
		error!(logger, "Error validating configuration"; "errors" => format!("{:?}", errors));
		if errors.is_fatal() {
			return ExitCode::FAILURE;
		}
	}

	let services =
		match config.resolve_to_service_set("getty", HashMap::from([("TTY".to_owned(), "/dev/tty0".to_owned())])) {
			Ok(services) => services,
			Err(errors) => {
				error!(logger, "Error resolving services"; "errors" => format!("{:?}", errors));
				return ExitCode::FAILURE;
			}
		};

	for (definition, args) in services {
		let mut service = Service::new(definition, args);
		service.start().unwrap();
	}

	loop {}
}
