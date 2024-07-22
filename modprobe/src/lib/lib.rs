#![feature(hash_extract_if)]
use lzma_rs::xz_decompress;
use std::{
	collections::HashMap,
	ffi::CString,
	fs::File,
	io::{self, BufRead, BufReader, ErrorKind, Read},
	path::{Path, PathBuf},
};

use nix::kmod::init_module;
use slog::{debug, warn};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModuleLoadError {
	#[error("Failed to load file from disk: {0}")]
	IOError(#[from] io::Error),

	#[error("Found unmet dependencies: {0:?}")]
	DependencyError(HashMap<String, Vec<String>>),

	#[error("Unknown Module: {0}")]
	UnknownModule(String),

	#[error("Failed to load module: {0}")]
	ModuleLoadError(#[from] nix::Error),
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

/// Intelligently loads the module with the given name, resolving dependencies and paths.
pub fn load_module(
	logger: &slog::Logger,
	module_base_path: &Path,
	mod_name: &str,
	parameters: &[String],
) -> Result<(), ModuleLoadError> {
	let modules_to_load = find_modules_to_load(logger, mod_name, &module_base_path.join("modules.dep"))?;
	let module_paths = load_module_names(logger, &module_base_path.join("modules.name"))?;

	for module in modules_to_load {
		let path = match module_paths.get(&module) {
			Some(path) => path,
			None => {
				return Err(ModuleLoadError::UnknownModule(module));
			}
		};

		debug!(logger, "loading module"; "name" => module, "path" => path.display());
		let module_contents = load_file(path)?;

		let module_file = File::open(path)?;
		init_module(&module_contents, &CString::new(parameters.join(" ")).unwrap())?;
	}

	Ok(())
}

/// Starting with the given modules, calculates the order of modules to load that satisfies all the dependencies that each modules has.
pub fn find_modules_to_load(
	logger: &slog::Logger,
	mod_name: &str,
	mod_deps_path: &Path,
) -> Result<Vec<String>, ModuleLoadError> {
	let mut all_dependencies = load_mod_dependencies(logger, mod_deps_path)?;
	let mut deps = HashMap::new();
	let mut mods_to_load = Vec::new();
	let mut mods_to_scan = vec![mod_name];

	while let Some(mod_name) = mods_to_scan.pop() {
		if deps.contains_key(mod_name) {
			continue;
		}

		deps.insert(
			mod_name.to_owned(),
			all_dependencies.remove(mod_name).unwrap_or(Vec::new()),
		);
	}

	// This is basically Kuhn's algorithm.
	while !deps.is_empty() {
		let ok_to_start = deps.extract_if(|_, v| v.is_empty()).map(|(n, _)| n).collect::<Vec<_>>();

		if ok_to_start.is_empty() {
			// We have unmet dependencies, but nothing to start to resolve them.
			return Err(ModuleLoadError::DependencyError(deps));
		}

		for (_, v) in deps.iter_mut() {
			for mod_name in ok_to_start.iter() {
				v.retain(|s| s != mod_name);
			}
		}
		mods_to_load.extend(ok_to_start.into_iter());
	}

	Ok(mods_to_load)
}

/// Load the modules.dep file, returning a map of module names to a list of the module names that that module depends on.
fn load_mod_dependencies(logger: &slog::Logger, mod_deps_path: &Path) -> io::Result<HashMap<String, Vec<String>>> {
	let mod_deps_file = BufReader::new(File::open(mod_deps_path)?);
	let mut found_dependencies = HashMap::new();

	for line in mod_deps_file.lines() {
		let line = line?;
		// Lines in modules.dep are in the form `<path>:<dep1> <dep2>...`
		let (path, deps) = match line.split_once(':') {
			Some(s) => s,
			None => {
				warn!(logger, "invalid line in modules.dep: {}", line);
				continue;
			}
		};

		found_dependencies.insert(
			path.to_owned(),
			deps.split_ascii_whitespace().map(ToOwned::to_owned).collect(),
		);
	}

	Ok(found_dependencies)
}

fn load_module_names(logger: &slog::Logger, mod_names_path: &Path) -> io::Result<HashMap<String, PathBuf>> {
	let mod_names_file = BufReader::new(File::open(mod_names_path)?);
	let mut out = HashMap::new();
	for line in mod_names_file.lines() {
		let line = line?;
		let (name, path) = match line.split_once(':') {
			Some((k, v)) => (k.to_owned(), PathBuf::from(v)),
			None => {
				warn!(logger, "invalid line in module.name: {}", line);
				continue;
			}
		};

		out.insert(name, path);
	}

	Ok(out)
}
