use std::path::PathBuf;

use clap::{Arg, Command};
use nix::mount::{mount, MsFlags};

mod switchroot;

fn create_device_folders() {
	let device_folders = [
		("/dev", "devtmpfs"),
		("/proc", "proc"),
		("/sys", "sysfs"),
		("/run", "tmpfs"),
		("/tmp", "tmpfs"),
	];
	for (folder, devtype) in device_folders {
		std::fs::create_dir_all(folder).unwrap();
		mount::<_, _, _, str>(Some(folder), folder, Some(devtype), MsFlags::empty(), None).unwrap();
	}
}

fn main() {
	let matches = Command::new("qinit")
		.version("0.1.0")
		.about("The qinit init system")
		.subcommand(
			Command::new("switchroot").about("Switch the root filesystem").arg(
				Arg::new("new_root")
					.help("The new root filesystem")
					.required(false)
					.index(1),
			),
		)
		.get_matches();

	create_device_folders();

	match matches.subcommand() {
		Some(("switchroot", sub_m)) => {
			let new_root = sub_m.get_one::<PathBuf>("new_root").cloned();
			let cmd = switchroot::SwitchrootCommand::new(new_root).unwrap();
			cmd.run().unwrap();
		}
		None => {
			switchroot::SwitchrootCommand::new(None).unwrap().run().unwrap();
		}
		_ => {}
	}
}
