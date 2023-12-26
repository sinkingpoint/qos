use std::{fs::{self, File}, path::{PathBuf, Path}, io};

use clap::Parser;

use slog::{Drain, o, info, debug};
use slog_json::Json;
use cpio::{self, NewcBuilder, newc};

// The default mode for files in the CPIO archive - 0o100000 (a file) | 0o777. Eventually we'll read these from the file system.
const FILE_MODE: u32 = 0o100777;

// The default mode for directories in the CPIO archive - 0o40000 (a directory) | 0o755
const DIR_MODE: u32 = 0o40755;

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
    fs::create_dir_all(&base_dir.join("lib64")).expect("Failed to create lib64 directory");

    // Copy /bin/sh to the temporary directory as the init process for now. We'll replace this with a real init process later.
    fs::copy("/bin/sh", base_dir.join("init")).expect(format!("Failed to copy /bin/sh to temporary directory").as_str());

    // Copy the required shared libs.
    for file in &["lib64/libtinfo.so.6", "lib64/libc.so.6", "lib64/ld-linux-x86-64.so.2"] {
        let from = PathBuf::from("/").join(file);
        let to = base_dir.join(file);
        fs::copy(&from, &to).expect(format!("Failed to copy {} to temporary directory", from.display()).as_str());
    }

    let inputs: Vec<_> = get_inputs(&log, &base_dir);

    let output_file = File::create(&args.output_file).expect("Failed to open output file");
    write_cpio(inputs.into_iter(), output_file).expect("Failed to write CPIO archive");

    info!(log, "Wrote CPIO archive to {}", args.output_file);
}

// Returns a list of files to add to the CPIO archive.
fn get_inputs<T>(log: &slog::Logger, base_dir: &Path) -> T where T: FromIterator<(NewcBuilder, Option<File>)> {
    // TODO: This is a hardcoded list of files to add to the CPIO archive. We should walk the directory instead.
    vec!["init", "lib64", "lib64/libtinfo.so.6", "lib64/libc.so.6", "lib64/ld-linux-x86-64.so.2"].iter().map(|&s| {
        let full_path = base_dir.join(s);

        debug!(log, "Adding file to CPIO archive"; "path" => full_path.display().to_string());

        let metadata = fs::metadata(&full_path).expect("Failed to get metadata");

        let (mode, reader) = if metadata.is_file() {
            (FILE_MODE, Some(File::open(full_path).expect("Failed to open file")))
        } else if metadata.is_dir() {
            (DIR_MODE, None)
        } else {
            panic!("Unsupported file type");
        };

        let builder = NewcBuilder::new(s)
            .uid(0)
            .gid(0)
            .mode(mode);

        (builder, reader)
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

// Copies https://github.com/jcreekmore/cpio-rs/blob/master/src/lib.rs#L16 , but takes an `Option<RS>` for each
// file to add to the CPIO archive. If the `Option<RS>` is `None`, then we create a directory instead of a file.
fn write_cpio<I, RS, W>(inputs: I, output: W) -> io::Result<W>
where
    I: Iterator<Item = (NewcBuilder, Option<RS>)> + Sized,
    RS: io::Read + io::Seek,
    W: io::Write,
{
    let output = inputs
        .enumerate()
        .fold(Ok(output), |output, (idx, (builder, input))| {
            // If the output is valid, try to write the next input file
            output.and_then(move |output| {
                // Grab the length of the input file
                let fp = match input {
                    Some(mut rs) => {
                        let len = rs.seek(io::SeekFrom::End(0))?;
                        rs.seek(io::SeekFrom::Start(0))?;

                        // Create our writer fp with a unique inode number
                        let mut fp = builder.ino(idx as u32).write(output, len as u32);

                        // If we have an input file, copy it to the writer
                        io::copy(&mut rs, &mut fp)?;

                        fp
                    },
                    None => {
                        builder.ino(idx as u32).write(output, 0)
                    }
                };

                // And finish off the input file
                fp.finish()
            })
        })?;

    newc::trailer(output)
}