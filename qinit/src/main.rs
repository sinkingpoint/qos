mod config;

use std::{io::stderr, path::PathBuf};

use common::obs::assemble_logger;
use config::load_config;
use slog::error;

fn main() {
	let logger = assemble_logger(stderr());

	let config_directories = ["./configs/services"].map(PathBuf::from);

	let (config, errors) = load_config(config_directories);
	if errors.is_error() {
		error!(logger, "Error loading configuration"; "errors" => format!("{:?}", errors));
	}

	let errors = config.validate();
}
