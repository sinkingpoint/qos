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

	let mut symbol_names_section_header = None;
	let mut symbols_section_header = None;

	for section in elffile.section_headers() {
		let section = section.unwrap();
		match elffile.section_header_name(&section) {
			Some(".symtab") => symbols_section_header = Some(section),
			Some(".strtab") => symbol_names_section_header = Some(section),
			_ => continue,
		}
	}

	let symbol_names_section = match symbol_names_section_header.unwrap().read_string_table_section(&elffile) {
		Some(Ok(table)) => table,
		Some(Err(e)) => {
			eprintln!("failed to read symbol names section: {}", e);
			return ExitCode::FAILURE;
		}
		None => {
			eprintln!("symbol names section is not a string table");
			return ExitCode::FAILURE;
		}
	};

	let symbols_section = match symbols_section_header.unwrap().read_symbol_table_section(&elffile) {
		Some(Ok(table)) => table,
		Some(Err(e)) => {
			eprintln!("failed to read symbol section: {}", e);
			return ExitCode::FAILURE;
		}
		None => {
			eprintln!("symbol section is not a symbol table");
			return ExitCode::FAILURE;
		}
	};

	for symbol in symbols_section.iter() {
		let name = symbol_names_section.get_string_at_offset(symbol.name_offset).unwrap();
		println!("{}", name);
	}

	ExitCode::SUCCESS
}
