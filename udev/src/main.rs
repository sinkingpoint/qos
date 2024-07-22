use std::{
	collections::HashMap,
	io::stderr,
	path::{Path, PathBuf},
	process::ExitCode,
};

use anyhow::anyhow;
use bus::BusClient;
use clap::{Arg, ArgAction, Command};
use common::{obs::assemble_logger, qinit::mark_running};
use modprobe::load_module;
use nix::sys::utsname::uname;
use regex::Regex;
use slog::error;
use tokio::{
	fs::File,
	io::{AsyncBufReadExt, BufReader},
};

const BUS_TOPIC: &str = "udev_events";

#[tokio::main]
async fn main() -> ExitCode {
	let matches = Command::new("udev")
		.author("Colin Douch <colin@quirl.co.nz>")
		.about("Listens for new devices and loads modules as necessary")
		.arg(
			Arg::new("topic")
				.short('t')
				.long("topic")
				.num_args(0..1)
				.action(ArgAction::Set)
				.default_value(BUS_TOPIC),
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

	let topic = matches
		.get_one::<String>("topic")
		.expect("missing topic, even though it has a default");
	let mut bus_socket = match BusClient::new().await.unwrap().subscribe(topic).await {
		Ok(s) => s,
		Err(e) => {
			error!(logger, "failed to open bus connection"; "error" => e.to_string());
			return ExitCode::FAILURE;
		}
	};

	let module_loader = match ModuleLoader::new(&logger, &modules_path).await {
		Ok(m) => m,
		Err(e) => {
			error!(logger, "failed to load modules"; "error" => e.to_string());
			return ExitCode::FAILURE;
		}
	};

	mark_running().expect("failed to mark udev as running");

	while let Ok(line) = bus_socket.read_message().await {
		if let Ok(line) = String::from_utf8(line) {
			let event = match serde_json::from_str::<HashMap<String, String>>(&line) {
				Ok(map) => map,
				Err(e) => {
					error!(logger, "failed to parse hashmap from message"; "msg" => line, "error" => e.to_string());
					continue;
				}
			};

			if let Some(alias) = event.get("MODALIAS") {
				for module in module_loader.get_modules_for_device(alias) {
					if let Err(e) = load_module(&logger, &modules_path, module, &[]) {
						error!(logger, "failed to load module for device"; "modalias" => alias, "module" => module, "error" => e.to_string());
					}
				}
			}
		}
	}

	ExitCode::SUCCESS
}

struct ModuleLoader {
	aliases: Vec<(Regex, String)>,
}

impl ModuleLoader {
	async fn new(logger: &slog::Logger, modules_path: &Path) -> anyhow::Result<Self> {
		let mod_alias_path = modules_path.join("modules.alias");
		if !mod_alias_path.exists() {
			return Err(anyhow!(
				"module alias file at {} doesn't exist. Did depmod fail?",
				mod_alias_path.display()
			));
		}

		let mut aliases = Vec::new();

		let mod_alias_file = BufReader::new(File::open(mod_alias_path).await?);
		let mut lines = mod_alias_file.lines();
		while let Some(line) = lines.next_line().await? {
			// Format is `alias <glob> <module_name>, with optional comment lines that start with #
			if line.starts_with('#') {
				continue;
			}

			let parts: Vec<&str> = line.splitn(3, ' ').collect();
			if parts.len() != 3 || parts[0] != "alias" {
				error!(logger, "malformed modalias line: {}", line);
				continue;
			}

			let regex = match glob_to_regex(parts[1]) {
				Ok(r) => r,
				Err(e) => {
					error!(logger, "failed to tranlate glob into regex"; "glob" => parts[1], "error" => e.to_string());
					continue;
				}
			};

			aliases.push((regex, parts[2].to_owned()));
		}

		Ok(Self { aliases })
	}

	fn get_modules_for_device(&self, device: &str) -> Vec<&str> {
		self.aliases
			.iter()
			.filter(|(r, _)| r.is_match(device))
			.map(|(_, s)| s.as_ref())
			.collect()
	}
}

/// mod alias's come in the form of globs, which Rust doesn't have a decent
/// library to evaluate. This translates the glob into a regex that is a bit easier to work with,
/// if not a bit slower.
fn glob_to_regex(s: &str) -> Result<Regex, regex::Error> {
	let regex = s.replace('*', ".*");
	let regex = regex.replace('?', ".");
	Regex::new(&format!("^{}$", regex))
}
