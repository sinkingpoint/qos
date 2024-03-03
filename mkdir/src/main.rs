use std::{
	fs::{self, create_dir, create_dir_all, Permissions},
	os::unix::fs::PermissionsExt,
	path::PathBuf,
};

use clap::{Arg, ArgAction, Command};

fn main() {
	let matches = Command::new("mkdir")
		.about("make directories")
		.author("Colin Douch")
		.version("0.1")
		.arg(
			Arg::new("mode")
				.short('m')
				.help("set file mode (as in chmod), not a=rwx - umask")
				.num_args(1)
				.default_value("755"),
		)
		.arg(
			Arg::new("parents")
				.short('p')
				.help("no error if existing, make parent directories as needed")
				.long("parents")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("verbose")
				.short('v')
				.help("print a message for each created directory")
				.long("verbose")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("directory")
				.required(true)
				.num_args(1..)
				.help("directories to create"),
		)
		.get_matches();

	let mode = match matches
		.get_one::<String>("mode")
		.map(|m| u32::from_str_radix(m, 8))
		.unwrap()
	{
		Ok(mode) => mode,
		Err(e) => {
			eprintln!(
				"mkdir: invalid mode '{}': {}",
				matches.get_one::<String>("mode").unwrap(),
				e
			);
			return;
		}
	};

	let parents = matches.get_flag("parents");
	let verbose = matches.get_flag("verbose");
	let directories: Vec<String> = matches.get_many("directory").unwrap().cloned().collect();

	for directory in directories {
		let directory = PathBuf::from(&directory);
		let res = if parents {
			create_dir_all(&directory)
		} else {
			create_dir(&directory)
		};

		if let Err(e) = res {
			eprintln!("mkdir: cannot create directory '{}': {}", directory.display(), e);
			continue;
		}

		if let Err(e) = fs::set_permissions(&directory, Permissions::from_mode(mode)) {
			eprintln!(
				"mkdir: cannot set permissions of directory '{}': {}",
				directory.display(),
				e
			);
			continue;
		}

		if verbose {
			println!("mkdir: created directory '{}'", directory.to_string_lossy());
		}
	}
}
