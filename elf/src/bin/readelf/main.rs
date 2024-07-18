use std::process::ExitCode;

use clap::{Arg, Command};
use elf::ElfFile;

fn main() -> ExitCode {
	let matches = Command::new("readelf")
		.about("display information about ELF files")
		.arg(Arg::new("elffile").help("the file to load").num_args(1).required(true))
		.get_matches();

	let filepath: &String = matches.get_one("elffile").expect("missing required arg `elffile`");
	let elffile = match ElfFile::open(filepath) {
		Ok(f) => f,
		Err(e) => {
			eprintln!("failed to open {}: {}", filepath, e);
			return ExitCode::FAILURE;
		}
	};

	println!("{:?}", elffile);

	ExitCode::SUCCESS
}
