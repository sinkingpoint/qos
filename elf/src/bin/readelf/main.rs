use std::{
	io::{self, Read, Seek},
	process::ExitCode,
};

use clap::{Arg, ArgAction, Command};
use elf::{ElfFile, SectionHeaderType, StringTableSection};
use tables::{Table, TableSetting};

fn main() -> ExitCode {
	let matches = Command::new("readelf")
		.about("display information about ELF files")
		.disable_help_flag(true)
		.arg(
			Arg::new("file-header")
				.short('h')
				.help("Display the ELF file header")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("program-headers")
				.short('l')
				.help("Display the program headers")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("section-headers")
				.short('S')
				.help("Display the section headers")
				.action(ArgAction::SetTrue),
		)
		.arg(
			Arg::new("symbols")
				.short('s')
				.help("Display the symbols")
				.action(ArgAction::SetTrue),
		)
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

	let file_header = matches.get_flag("file-header");
	if file_header {
		print_elf_file_header(&elffile);
	}

	let program_headers = matches.get_flag("program-headers");
	if program_headers {
		print_program_headers(&elffile);
	}

	let section_headers = matches.get_flag("section-headers");
	if section_headers {
		print_section_headers(&elffile);
	}

	let symbols = matches.get_flag("symbols");
	if symbols {
		print_symbols(&elffile);
	}

	ExitCode::SUCCESS
}

fn print_elf_file_header<T: Read + Seek>(file: &ElfFile<T>) {
	println!("ELF Header:");
	println!("	Class: {:?}", file.header.class);
	println!("	Endian: {:?}", file.header.endian);
	println!("	Version: 1");
	println!("	OS/ABI: {:?}", file.header.abi);
	println!("	ABI Version: {}", file.header.abi_version);
	println!("	Type: {:?}", file.header.ty);
	println!("	Machine {:?}", file.header.architecture);
	println!("	Entry Point Address: {:#x}", file.header.entrypoint_offset);
	println!("	Program Header Offset: {:#x}", file.header.program_header_offset);
	println!("	Section Header Offset: {:#x}", file.header.section_header_offset);
	println!("	Flags: {:?}", file.header.flags);
	println!("	Header Size: {}", file.header.header_size);
	println!("	Program Header Size: {}", file.header.program_header_size);
	println!("	Number of Program Headers: {}", file.header.program_header_table_len);
	println!("	Section Header Size: {}", file.header.section_header_size);
	println!("	Number of Section Headers: {}", file.header.section_header_table_len);
	println!(
		"	Section Header Name Index: {}",
		file.header.section_header_table_name_idx
	);
}

fn print_program_headers<T: Read + Seek>(file: &ElfFile<T>) {
	if file.header.program_header_table_len == 0 {
		println!("There are no program headers in this file");
		return;
	}

	let mut table = Table::new_with_headers([
		"Type",
		"Offset",
		"Virtual Address",
		"Physical Address",
		"File Size",
		"Memory Size",
		"Flags",
		"Alignment",
	])
	.with_setting(TableSetting::HeaderSeperator)
	.with_setting(TableSetting::ColumnSeperators);

	for header in file.program_headers() {
		if header.is_err() {
			continue;
		}

		let header = header.unwrap();
		table.add_row([
			&format!("{:?}", header.ty),
			&header.offset.to_string(),
			&format!("0x{:x}", header.virtual_address),
			&format!("0x{:x}", header.physical_address),
			&header.file_size.to_string(),
			&header.memory_size.to_string(),
			&format!("{:?}", header.flags),
			&header.alignment.to_string(),
		]);
	}

	println!("{}", table);
}

fn print_section_headers<T: Read + Seek>(file: &ElfFile<T>) {
	if file.header.section_header_table_len == 0 {
		println!("There are no section headers in this file");
	}

	let mut table = Table::new_with_headers([
		"Name",
		"Type",
		"Address",
		"Offset",
		"Size",
		"Ent Size",
		"Flags",
		"Link",
		"Info",
		"Alignment",
	])
	.with_setting(TableSetting::HeaderSeperator)
	.with_setting(TableSetting::ColumnSeperators);

	for header in file.section_headers() {
		if header.is_err() {
			continue;
		}

		let header = header.unwrap();

		table.add_row([
			file.section_header_name(&header).unwrap_or("<None>"),
			&format!("{:?}", header.ty),
			&format!("0x{:x}", header.address),
			&format!("0x{:x}", header.offset),
			&header.size.to_string(),
			&header.entry_size.to_string(),
			&format!("{:?}", header.flags),
			&header.link.to_string(),
			&header.info.to_string(),
			&header.alignment.to_string(),
		])
	}

	println!("{}", table);
}

fn get_string_section<T: Read + Seek>(file: &ElfFile<T>, name: &str) -> Option<io::Result<StringTableSection>> {
	file.section_headers()
		.find(|s| {
			if let Ok(s) = s {
				file.section_header_name(s) == Some(name) && s.ty == SectionHeaderType::StringTable
			} else {
				false
			}
		})
		.map(|s| s.unwrap().read_string_table_section(file).unwrap())
}

fn print_symbols<T: Read + Seek>(file: &ElfFile<T>) {
	let sym_string_table = match get_string_section(file, ".strtab") {
		Some(Ok(s)) => Some(s),
		None => None,
		Some(Err(e)) => {
			eprintln!("failed to read .strtab section: {}", e);
			return;
		}
	};

	let dyn_string_table = match get_string_section(file, ".dynstr") {
		Some(Ok(s)) => Some(s),
		None => None,
		Some(Err(e)) => {
			eprintln!("failed to read .strtab section: {}", e);
			return;
		}
	};

	for header in file.section_headers() {
		if header.is_err() {
			continue;
		}

		let header = header.unwrap();
		if header.ty != SectionHeaderType::SymbolTable && header.ty != SectionHeaderType::DynamicLinkerSymbols {
			continue;
		}

		let name = file.section_header_name(&header).unwrap_or("<None>");
		let num_symbols = header.size / header.entry_size;
		println!("Symbol table `{}` contains {} entries", name, num_symbols);

		let mut table = Table::new_with_headers(["Value", "Size", "Type", "Binding", "Visibility", "Name"]);
		let symbols = match header.read_symbol_table_section(file).unwrap() {
			Ok(s) => s,
			Err(e) => {
				println!("Failed to read symbols: {}", e);
				continue;
			}
		};

		for symbol in symbols.iter() {
			let name = if header.ty == SectionHeaderType::SymbolTable {
				match &sym_string_table {
					Some(s) => s.get_string_at_offset(symbol.name_offset).unwrap_or("<Unknown>"),
					None => "<Unknown>",
				}
			} else {
				match &dyn_string_table {
					Some(s) => s.get_string_at_offset(symbol.name_offset).unwrap_or("<Unknown>"),
					None => "<Unknown>",
				}
			};

			let name = if name.len() <= 20 {
				name.to_string()
			} else {
				format!("{}[...]", name.split_at(20).0)
			};

			table.add_row([
				&format!("{:10}", symbol.value),
				&symbol.size.to_string(),
				&format!("{:?}", symbol.ty),
				&format!("{:?}", symbol.binding),
				&format!("{:?}", symbol.visibility),
				&name,
			])
		}

		println!("{}", table);
	}
}
