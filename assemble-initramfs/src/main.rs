use std::{fs::{self, File}, path::{PathBuf, Path}};

use clap::Parser;

use slog::{Drain, o, info, debug};
use slog_json::Json;
use cpio::{self, NewcBuilder};

#[derive(Parser)]
#[command(about="Assemble an initramfs structure CPIO archive")]
struct Cli {
    #[arg(short, long, help="Path to the directory where we will build the initramfs structure")]
    base_dir: Option<String>,

    #[arg(short, long, default_value_t=String::from("./initramfs"), help="Path to the output file")]
    output_file: String,
}

fn main() {
    let args = Cli::parse();

    let base_dir = PathBuf::from(match args.base_dir {
        Some(p) => p,
        None => generate_tmp_path(),
    });

    let log = assemble_logger();

    info!(log, "Assembling initramfs structure in {}", base_dir.display());

    // Create the temporary directory where we will build the initramfs structure.
    fs::create_dir_all(&base_dir).expect("Failed to create temporary directory");

    // Copy /bin/sh as a test.
    fs::copy("/bin/sh", base_dir.join("init")).expect("Failed to copy /bin/sh to temporary directory");

    let inputs: Vec<_> = get_inputs(&log, &base_dir);

    let output_file = File::create(&args.output_file).expect("Failed to open output file");
    cpio::write_cpio(inputs.into_iter(), output_file).expect("Failed to write CPIO archive");
}

// Returns a list of files to add to the CPIO archive.
fn get_inputs<T>(log: &slog::Logger, base_dir: &Path) -> T where T: FromIterator<(NewcBuilder, File)> {
    // TODO: This is a hardcoded list of files to add to the CPIO archive. We should walk the directory instead.
    vec!["init"].iter().map(|&s| {
        let full_path = base_dir.join(s);

        debug!(log, "Adding file to CPIO archive"; "path" => full_path.display().to_string());

        let builder = NewcBuilder::new(s)
            .uid(1000)
            .gid(1000)
            .mode(0o100644);

        (builder, File::open(full_path).expect("Failed to open file"))
    }).collect()
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