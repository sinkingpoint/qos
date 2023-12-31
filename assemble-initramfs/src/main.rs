use std::{path::{PathBuf, Path}, io, fs::{File, self}};

use clap::Parser;

use cpio::CPIOArchive;
use slog::{Drain, o, info};
use slog_json::Json;

use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    init_file: PathBuf,
    libraries: Vec<PathBuf>,
    binaries: Vec<PathBuf>,
    output_file: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            init_file: PathBuf::from("/bin/sh"),
            libraries: Vec::new(),
            binaries: Vec::new(),
            output_file: PathBuf::from("./initramfs.cpio"),
        }
    }
}

impl Config {
    fn load(config_file: &Path) -> io::Result<Self> {
        let config_file = File::open(config_file)?;
        let config: Config = serde_yaml::from_reader(config_file).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        Ok(config)
    }
}

#[derive(Parser)]
#[command(about="Assemble an initramfs structure CPIO archive")]
struct Cli {
    #[arg(short, long, help="Path to the directory where we will build the initramfs structure")]
    base_dir: Option<String>,

    #[arg(short, long, default_value_t=String::from("./config.yaml"), help="Path to the config file")]
    config: String,
}

fn main() {
    let cli = Cli::parse();

    let logger = assemble_logger();

    let config = match Config::load(&PathBuf::from(cli.config)) {
        Ok(config) => config,
        Err(err) => {
            slog::error!(logger, "Failed to load config file: {}", err);
            return;
        }
    };

    let base_dir = PathBuf::from(match cli.base_dir {
        Some(path) => path,
        None => generate_tmp_path(),
    });

    if let Err(e) = fs::create_dir(&base_dir) {
        slog::error!(logger, "Failed to create base directory"; "path"=>base_dir.display(), "error"=>e);
        return;
    };

    info!(logger, "Using base directory {}", base_dir.display());

    if let Err(e) = fs::copy(&config.init_file, base_dir.join("init")) {
        slog::error!(logger, "Failed to copy init file"; "file_name"=>config.init_file.display(),"error"=>e);
        return;
    }

    if let Err(e) = copy_all_to(&logger, &base_dir.join("lib64"), &config.libraries) {
        slog::error!(logger, "Failed to copy libraries"; "error"=>e);
        return;
    }

    if let Err(e) = copy_all_to(&logger, &base_dir.join("bin"), &config.binaries) {
        slog::error!(logger, "Failed to copy binaries"; "error"=>e);
        return;
    }

    let cpio = match CPIOArchive::from_path(&base_dir) {
        Ok(cpio) => cpio,
        Err(e) => {
            slog::error!(logger, "Failed to generate CPIO archive"; "error"=>e);
            return;
        },
    };

    let mut output_file = match File::create(&config.output_file) {
        Ok(file) => file,
        Err(e) => {
            slog::error!(logger, "Failed to create output file"; "error"=>e);
            return;
        },
    };
    
    if let Err(e) = cpio.write(&mut output_file) {
        slog::error!(logger, "Failed to write CPIO archive"; "error"=>e);
        return;
    }

    slog::info!(logger, "Successfully wrote CPIO archive to {}", config.output_file.display());
}

fn copy_all_to(logger: &slog::Logger, dest_dir: &Path, files: &[PathBuf]) -> io::Result<()> {
    fs::create_dir(dest_dir)?;
    for file in files {
        slog::info!(logger, "Copying file {} to {}", file.display(), dest_dir.display());
        fs::copy(file, dest_dir.join(file.file_name().unwrap()))?;
    }

    Ok(())
}

// Create a slog logger that writes JSON to stderr.
// TODO: I assume that we'll move this to a separate module when we get more binaries.
fn assemble_logger() -> slog::Logger {
    let drain = slog_async::Async::new(Json::new(std::io::stderr()).add_default_keys().build().fuse()).build().fuse();
    slog::Logger::root(drain, o!())
}

// Generate a random path in /tmp/assemble-initramfsXXXXX where XXXXX is a random number.
// This is used to create a temporary directory where we will build the initramfs structure,
// with a random number to avoid collisions if we run this multiple times.
fn generate_tmp_path() -> String {
    let mut tmp_path = String::from("/tmp/assemble-initramfs");
    tmp_path.push_str(&rand::random::<u32>().to_string());
    tmp_path
}
