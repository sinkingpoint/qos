use std::{io::stderr, path::PathBuf, process::ExitCode};

use clap::{Arg, ArgAction, Command};
use common::obs::assemble_logger;
use modprobe::load_module;
use nix::sys::utsname::uname;
use slog::error;

fn main() -> ExitCode {
	let matches = Command::new("modprobe")
		.author("Colin Douch <iam@colindou.ch")
		.about("Load kernel modules")
		.arg(
			Arg::new("module")
				.help("the name of the module to load")
				.num_args(1)
				.required(true),
		)
		.arg(
			Arg::new("parameters")
				.help("the parameters to pass to the module")
				.num_args(0..),
		)
		.arg(
			Arg::new("modules_path")
				.long("modules-path")
				.action(ArgAction::Set)
				.help("the path to scan for modules"),
		)
		.get_matches();

	let logger = assemble_logger(stderr());
	let name = match uname() {
		Ok(n) => n,
		Err(e) => {
			error!(logger, "failed to read uname"; "error"=>e.to_string());
			return ExitCode::FAILURE;
		}
	};
	let default_module_path = PathBuf::from("/lib/modules").join(name.release());

	let modules_path = matches
		.get_one::<String>("modules_path")
		.map(PathBuf::from)
		.unwrap_or(default_module_path);

	let module_name = matches.get_one::<String>("module").unwrap();

	let parameters = match matches.get_many("parameters") {
		Some(p) => p.cloned().collect(),
		None => Vec::new(),
	};

	match load_module(&logger, &modules_path, module_name, &parameters) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("failed to load module: {}", e);
			ExitCode::FAILURE
		}
	}
}
