mod formats;

use std::{
	collections::HashMap,
	fs::{self, File},
	io::{self, stdout},
	path::{Path, PathBuf},
};

use clap::Parser;

use common::obs::assemble_logger;
use slog::info;
use std::process::ExitCode;

use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
	libraries: Vec<PathBuf>,
	binaries: Vec<PathBuf>,
	secure_binaries: Vec<PathBuf>,
	files: HashMap<String, PathBuf>,
	modules: Option<Vec<PathBuf>>,
	output_file: PathBuf,
}

impl Default for Config {
	fn default() -> Self {
		Config {
			libraries: Vec::new(),
			binaries: Vec::new(),
			secure_binaries: Vec::new(),
			files: HashMap::new(),
			modules: None,
			output_file: PathBuf::from("./initramfs.cpio"),
		}
	}
}

impl Config {
	fn load(config_file: &Path) -> io::Result<Self> {
		let config_file = File::open(config_file)?;
		let config: Config = serde_yaml::from_reader(config_file)
			.map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
		Ok(config)
	}
}

#[derive(Parser)]
#[command(about = "Assemble an initramfs structure CPIO archive")]
struct Cli {
	#[arg(
		short,
		long,
		help = "Path to the directory where we will build the initramfs structure"
	)]
	base_dir: Option<String>,

	#[arg(short, long, help = "The kernel release to build on. If set, allows loading modules")]
	kernel_release: Option<String>,

	#[arg(short, long, default_value_t=String::from("./config.yaml"), help="Path to the config file")]
	config: String,
}

fn main() -> ExitCode {
	let cli = Cli::parse();

	let logger = assemble_logger(stdout());

	let mut config = match Config::load(&PathBuf::from(cli.config)) {
		Ok(config) => config,
		Err(err) => {
			slog::error!(logger, "Failed to load config file: {}", err);
			return ExitCode::FAILURE;
		}
	};

	let base_dir = PathBuf::from(match cli.base_dir {
		Some(path) => path,
		None => generate_tmp_path(),
	});

	if let Err(e) = fs::create_dir(&base_dir) {
		slog::error!(logger, "Failed to create base directory"; "path"=>base_dir.display(), "error"=>e);
		return ExitCode::FAILURE;
	};

	info!(logger, "Using base directory {}", base_dir.display());

	if let Err(e) = copy_all_to(&logger, &base_dir.join("lib64"), &config.libraries) {
		slog::error!(logger, "Failed to copy libraries"; "error"=>e);
		return ExitCode::FAILURE;
	}

	if let Err(e) = copy_all_to(&logger, &base_dir.join("bin"), &config.binaries) {
		slog::error!(logger, "Failed to copy binaries"; "error"=>e);
		return ExitCode::FAILURE;
	}

	if let Err(e) = copy_all_to(&logger, &base_dir.join("sbin"), &config.secure_binaries) {
		slog::error!(logger, "Failed to copy sbinaries"; "error"=>e);
		return ExitCode::FAILURE;
	}

	if let Some(mods) = config.modules {
		if cli.kernel_release.is_none() {
			slog::error!(logger, "kernel modules specified, without a release");
			return ExitCode::FAILURE;
		}

		let module_folder = PathBuf::from("/lib/modules").join(cli.kernel_release.unwrap());
		for module in mods {
			let mod_path = module_folder.join(module);
			config.files.insert(mod_path.to_string_lossy().into_owned(), mod_path);
		}
	}

	for (dest, src) in config.files.iter() {
		let dest = base_dir.join(dest.trim_start_matches('/'));
		if let Some(parent) = dest.parent() {
			if let Err(e) = fs::create_dir_all(dest.parent().unwrap()) {
				slog::error!(logger, "Failed to create parent directory"; "path"=>parent.display(), "error"=>e);
				return ExitCode::FAILURE;
			}
		}

		// Handle directories
		if src.is_dir() {
			if let Err(e) = fs::create_dir_all(&dest) {
				slog::error!(logger, "Failed to create directory"; "path"=>dest.display(), "error"=>e);
				return ExitCode::FAILURE;
			}

			let files = match fs::read_dir(src) {
				Ok(files) => files,
				Err(e) => {
					slog::error!(logger, "Failed to read directory"; "path"=>src.display(), "error"=>e);
					return ExitCode::FAILURE;
				}
			}
			.map(|entry| entry.unwrap().path())
			.collect::<Vec<PathBuf>>();

			println!("{:?}", files);

			if let Err(e) = copy_all_to(&logger, &dest, &files) {
				slog::error!(logger, "Failed to copy directory"; "src"=>src.display(), "dest"=>dest.display(), "error"=>e);
				return ExitCode::FAILURE;
			}
		} else if let Err(e) = fs::copy(src, &dest) {
			slog::error!(logger, "Failed to copy file"; "src"=>src.display(), "dest"=>dest.display(), "error"=>e);
			return ExitCode::FAILURE;
		}
	}

	let extension = config
		.output_file
		.extension()
		.expect("Output file must have an extension")
		.to_str()
		.expect("Output file extension must be a valid UTF-8 string");

	let write = match extension {
		"cpio" => formats::write_cpio(&base_dir, &config.output_file),
		"ext4" => formats::write_ext4(&base_dir, &config.output_file),
		_ => {
			slog::error!(logger, "Unsupported output file extension"; "extension"=>extension);
			return ExitCode::FAILURE;
		}
	};

	if let Err(e) = write {
		slog::error!(logger, "Failed to write output file"; "error"=>e);
		return ExitCode::FAILURE;
	}

	slog::info!(
		logger,
		"Successfully wrote Filesystem to {}",
		config.output_file.display()
	);
	ExitCode::SUCCESS
}

fn copy_all_to(logger: &slog::Logger, dest_dir: &Path, files: &[PathBuf]) -> io::Result<()> {
	fs::create_dir_all(dest_dir)?;
	for file in files {
		slog::info!(logger, "Copying file {} to {}", file.display(), dest_dir.display());
		fs::copy(file, dest_dir.join(file.file_name().unwrap()))?;
	}

	Ok(())
}

// Generate a random path in /tmp/assemble-initramfsXXXXX where XXXXX is a random number.
// This is used to create a temporary directory where we will build the initramfs structure,
// with a random number to avoid collisions if we run this multiple times.
fn generate_tmp_path() -> String {
	let mut tmp_path = String::from("/tmp/assemble-initramfs");
	tmp_path.push_str(&rand::random::<u32>().to_string());
	tmp_path
}
