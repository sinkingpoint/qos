mod parser;
use std::{io::Read, os::fd::{AsFd, AsRawFd}};

use nix::sys::termios::{tcgetattr, LocalFlags, SetArg};
use slog::{
    o,
    Drain,
    error
};
use slog_json::Json;

fn main() {
    let logger = assemble_logger();
    let mut reader = std::io::stdin();

    if !isatty(&reader) {
        error!(logger, "stdin is not a tty");
        return;
    }

    let mut attrs = match tcgetattr(&reader) {
        Ok(attrs) => attrs,
        Err(e) => {
            error!(logger, "Error getting terminal attributes: {}", e);
            return;
        }
    };

    // Disable "Canonical mode" and "Echo".
    // Canonical mode means that the terminal will buffer input until a newline is received, this allows us to read input one char at a time.
    // Echo means that the terminal will print input back to the user, this allows us to read input without the user seeing it.
    attrs.local_flags &= !(LocalFlags::ICANON | LocalFlags::ECHO);

    if let Err(e) = nix::sys::termios::tcsetattr(&reader, SetArg::TCSANOW, &attrs) {
        error!(logger, "Error setting terminal attributes: {}", e);
        return;
    }

    let mut buffer = String::new();
    loop {
        let mut char_buffer = [0; 1];
        if let Err(e) = reader.read_exact(&mut char_buffer) {
            eprintln!("Error reading from stdin: {}", e);
            break;
        }

        let c = char_buffer[0] as char;
        if c == '\n' {
            println!("Buffer: {}", buffer);
            buffer.clear();
        } else {
            buffer.push(c);
        }
    }
}

fn isatty<T: AsFd>(fd: T) -> bool {
    nix::unistd::isatty(fd.as_fd().as_raw_fd()).unwrap_or(false)
}

fn assemble_logger() -> slog::Logger {
    let drain = slog_async::Async::new(
        Json::new(std::io::stderr())
            .add_default_keys()
            .build()
            .fuse(),
    )
    .build()
    .fuse();
    slog::Logger::root(drain, o!())
}
