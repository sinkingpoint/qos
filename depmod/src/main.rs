use std::{
	borrow::Cow,
	collections::HashMap,
	fs::{read_dir, File},
	io::{stderr, BufReader, Cursor, ErrorKind, Read, Seek, Write},
	path::{Path, PathBuf},
	process::ExitCode,
};

use anyhow::anyhow;
use clap::{Arg, ArgAction, Command};
use common::iter::SplitOn;
use common::obs::assemble_logger;
use elf::{ElfFile, ElfSymbolBinding, ElfSymbolType};
use lzma_rs::xz_decompress;
use nix::sys::utsname::uname;
use slog::{debug, error, info};
use std::io;

fn main() -> ExitCode {
	let logger = assemble_logger(stderr());
	let name = match uname() {
		Ok(n) => n,
		Err(e) => {
			error!(logger, "failed to read uname"; "error"=>e.to_string());
			return ExitCode::FAILURE;
		}
	};
	let default_module_path = PathBuf::from("/lib/modules").join(name.release());

	let matches = Command::new("depmod")
		.arg(
			Arg::new("modules_path")
				.action(ArgAction::Set)
				.help("the path to scan for modules"),
		)
		.get_matches();

	let modules_path = matches
		.get_one::<String>("modules_path")
		.map(PathBuf::from)
		.unwrap_or(default_module_path);

	let mut deps_out = match File::create(modules_path.join("modules.dep")) {
		Ok(f) => f,
		Err(e) => {
			error!(logger, "failed to open modules.dep"; "error" => e.to_string());
			return ExitCode::FAILURE;
		}
	};

	let mut aliases_out = match File::create(modules_path.join("modules.alias")) {
		Ok(f) => f,
		Err(e) => {
			error!(logger, "failed to open modules.alias"; "error" => e.to_string());
			return ExitCode::FAILURE;
		}
	};

	let mut symbols_out = match File::create(modules_path.join("modules.symbols")) {
		Ok(f) => f,
		Err(e) => {
			error!(logger, "failed to open modules.symbols"; "error" => e.to_string());
			return ExitCode::FAILURE;
		}
	};

	let found_modules = match find_modules(&logger, modules_path) {
		Ok(modules) => modules,
		Err(e) => {
			error!(logger, "failed to find kernel modules"; "error" => e.to_string());
			return ExitCode::FAILURE;
		}
	};

	for module_path in found_modules {
		let data = load_file(&module_path).unwrap();
		let elffile = match ElfFile::new(Cursor::new(data)) {
			Ok(e) => e,
			Err(e) => {
				error!(logger, "failed to read {} as an ELF file", module_path.display(); "error" => e.to_string());
				continue;
			}
		};

		let modinfo = match ModInfo::read(&elffile) {
			Ok(modinfo) => modinfo,
			Err(e) => {
				error!(logger, "failed to read modinfo from {}", module_path.display(); "error" => e.to_string());
				continue;
			}
		};

		write_aliases(&modinfo, &mut aliases_out).expect("failed to write aliases");
		write_deps(&modinfo, &mut deps_out).expect("failed to write dependencies");
		write_symbols(&logger, &modinfo, &elffile, &mut symbols_out).expect("failed to write symbols");
	}

	ExitCode::SUCCESS
}

fn load_file(path: &Path) -> io::Result<Vec<u8>> {
	let mut file = BufReader::new(File::open(path)?);
	let mut buffer = Vec::new();
	match path.extension().and_then(|s| s.to_str()) {
		Some("o") | Some("ko") => {
			file.read_to_end(&mut buffer)?;
		}
		Some("xz") => {
			xz_decompress(&mut file, &mut buffer).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
		}
		Some(_) | None => return Err(io::Error::new(ErrorKind::InvalidData, "invalid extension")),
	};

	Ok(buffer)
}

/// Searches the module folder for the running kernel, returning a list of all
/// the modules that it finds in the directory structure.
fn find_modules(logger: &slog::Logger, module_path: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
	info!(logger, "Reading modules from {}", module_path.display());

	let mut to_search = vec![module_path];
	let mut found_modules = Vec::new();
	while let Some(search_path) = to_search.pop() {
		let dir = match read_dir(&search_path) {
			Ok(dir) => dir,
			Err(e) => {
				return Err(anyhow!("failed to read directory: {}: {}", search_path.display(), e));
			}
		};

		for file in dir {
			let file = match file {
				Ok(entry) => entry,
				Err(e) => {
					return Err(anyhow!("failed to read file: {}: {}", search_path.display(), e));
				}
			};

			let ty = match file.file_type() {
				Ok(f) => f,
				Err(e) => {
					return Err(anyhow!("failed to get file type: {}: {}", file.path().display(), e));
				}
			};

			let path = file.path();

			if ty.is_symlink() {
				continue; // Ignore symlinks to avoid loops
			} else if ty.is_dir() {
				to_search.push(file.path());
			} else if ty.is_file() {
				let extension = path
					.extension()
					.map(|o| o.to_string_lossy())
					.unwrap_or(Cow::Borrowed(""));

				if extension == "ko" || extension == "xz" {
					found_modules.push(path);
				}
			} else {
				debug!(logger, "skipping file {} {}", path.display(), path.ends_with(".ko.xz"));
			}
		}
	}

	Ok(found_modules)
}

fn write_aliases<W: Write>(modinfo: &ModInfo, mut writer: W) -> Result<(), io::Error> {
	for alias in modinfo.aliases.iter() {
		writer.write_all(
			&format!("alias {} {}\n", alias, modinfo.name)
				.bytes()
				.collect::<Vec<u8>>(),
		)?;
	}

	Ok(())
}

fn write_deps<W: Write>(modinfo: &ModInfo, mut writer: W) -> Result<(), io::Error> {
	return writer.write_all(
		&format!("{}: {}\n", modinfo.name, modinfo.dependencies.join(", "))
			.bytes()
			.collect::<Vec<u8>>(),
	);
}

fn write_symbols<T: Read + Seek, W: Write>(
	logger: &slog::Logger,
	module: &ModInfo,
	file: &ElfFile<T>,
	mut writer: W,
) -> Result<(), io::Error> {
	let symbol_table_header = match file
		.section_headers()
		.map(|s| s.unwrap())
		.find(|s| file.section_header_name(s) == Some(".symtab"))
	{
		Some(s) => s,
		None => {
			return Err(io::Error::new(ErrorKind::InvalidData, "missing .symtab section"));
		}
	};

	let string_table_header = match file
		.section_headers()
		.map(|s| s.unwrap())
		.find(|s| file.section_header_name(s) == Some(".strtab"))
	{
		Some(s) => s,
		None => {
			return Err(io::Error::new(ErrorKind::InvalidData, "missing .strtab section"));
		}
	};

	let symbol_table = symbol_table_header.read_symbol_table_section(file).unwrap().unwrap();
	let string_table = string_table_header.read_string_table_section(file).unwrap().unwrap();

	for symbol in symbol_table.iter() {
		let symbol_name = match string_table.get_string_at_offset(symbol.name_offset) {
			Some(s) => s,
			None => {
				debug!(logger, "skipping symbol without a name: {:?}", symbol);
				continue;
			}
		};

		if symbol.ty == ElfSymbolType::Func && symbol.binding == ElfSymbolBinding::Global {
			writer.write_all(
				&format!("alias symbol:{} {}\n", symbol_name, module.name)
					.bytes()
					.collect::<Vec<u8>>(),
			)?;
		}
	}

	Ok(())
}

#[derive(Default, Debug)]
struct ModInfo {
	name: String,
	version: String,
	author: String,
	description: String,
	license: String,
	src_version: String,
	parameter_descriptions: HashMap<String, String>,
	parameter_types: HashMap<String, String>,
	aliases: Vec<String>,
	dependencies: Vec<String>,
	return_trampoline: bool,
	in_tree: bool,
	version_magic: String,
}

impl ModInfo {
	fn read<T: Read + Seek>(elffile: &ElfFile<T>) -> io::Result<Self> {
		let modinfo_section = match elffile
			.section_headers()
			.map(|s| s.expect("failed to read section headers"))
			.find(|s| elffile.section_header_name(s) == Some(".modinfo"))
		{
			Some(s) => s,
			None => {
				return Err(io::Error::new(ErrorKind::InvalidData, "missing .modinfo section"));
			}
		};

		let mut modinfo = ModInfo::default();
		for line in modinfo_section
			.read_section(elffile)?
			.into_iter()
			.split_on_exclusive(b'\0')
		{
			let line = String::from_utf8(line).map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
			let (key, value) = {
				let mut split = line.splitn(2, '=');
				(split.next().unwrap().to_owned(), split.next().unwrap().to_owned())
			};

			match key.as_ref() {
				"name" => modinfo.name = value,
				"vermagic" => modinfo.version_magic = value,
				"intree" => modinfo.in_tree = value == "Y",
				"retpoline" => modinfo.return_trampoline = value == "Y",
				"srcversion" => modinfo.src_version = value,
				"author" => modinfo.author = value,
				"description" => modinfo.description = value,
				"version" => modinfo.version = value,
				"license" => modinfo.license = value,
				"depends" | "alias" => {
					if !value.trim().is_empty() {
						if key == "depends" {
							modinfo.dependencies.push(value)
						} else if key == "alias" {
							modinfo.aliases.push(value)
						}
					}
				}
				"parm" | "parmtype" => {
					let (parmname, parmvalue) = {
						let mut split = line.splitn(2, '=');
						(split.next().unwrap().to_owned(), split.next().unwrap().to_owned())
					};

					if key == "parm" {
						modinfo.parameter_descriptions.insert(parmname, parmvalue);
					} else if key == "parmtype" {
						modinfo.parameter_types.insert(parmname, parmvalue);
					}
				}
				_ => {
					println!("Unhandled key: {}", key);
				}
			}
		}

		Ok(modinfo)
	}
}
