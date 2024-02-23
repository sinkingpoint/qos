use crate::process::{ExitCode, IOTriple};
use nix::errno::Errno;
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
};

/// Builtin defines a shell builtin command, that takes precedence over external commands.
pub trait Builtin {
    fn run(&self, triple: IOTriple, args: &[String]) -> ExitCode;
}

/// The `cat` builtin - read from a list of files and write to standard output.
pub struct Cat;

impl Builtin for Cat {
    fn run(&self, triple: IOTriple, args: &[String]) -> ExitCode {
        let mut err = triple.stderr();
        let mut output = triple.stdout();
        if args.len() < 2 {
            err.write_all(b"cat: missing file operand\n").unwrap();
            return ExitCode::Err(Errno::EINVAL);
        }

        for file_path in &args[1..] {
            let file = match File::open(file_path) {
                Ok(f) => f,
                Err(e) => {
                    err.write_all(format!("Failed to open {}: {}\n", file_path, e).as_bytes()).unwrap();
                    return e.into();
                }
            };

            let mut reader = BufReader::new(file);
            let mut buffer = String::new();

            loop {
                match reader.read_line(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        output.write_all(buffer.as_bytes()).unwrap();
                        buffer.clear();
                    }
                    Err(e) => return e.into(),
                }

                match output.write_all(buffer.as_bytes()) {
                    Ok(_) => buffer.clear(),
                    Err(e) => return e.into(),
                }
            }
        }

        ExitCode::Success(0)
    }
}

pub struct Echo;

impl Builtin for Echo {
    fn run(&self, triple: IOTriple, args: &[String]) -> ExitCode {
        let mut output = triple.stdout();

        let message = args[1..].join(" ");
        output.write_all(message.as_bytes()).unwrap();
        output.write_all(b"\n").unwrap();

        ExitCode::Success(0)
    }
}
