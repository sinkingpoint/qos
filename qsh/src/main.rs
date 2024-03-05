mod buffer;
mod parser;
mod process;
mod shell;

use std::{
	io::{stderr, stdin},
	os::fd::{AsFd, AsRawFd},
};

use common::obs::assemble_logger;
use nix::{
	sys::termios::{tcgetattr, tcsetattr, LocalFlags, SetArg},
	unistd,
};

use shell::Shell;
use slog::error;

fn main() {
	let logger = assemble_logger(stderr());
	let reader = stdin();

	if !isatty(&reader) {
		error!(logger, "stdin is not a tty");
		return;
	}

	let mut attrs = match tcgetattr(&reader) {
		Ok(attrs) => attrs,
		Err(e) => {
			error!(&logger, "Error getting terminal attributes: {}", e);
			return;
		}
	};

	// Disable "Canonical mode" and "Echo".
	// Canonical mode means that the terminal will buffer input until a newline is received, this allows us to read input one char at a time.
	// Echo means that the terminal will print input back to the user, this allows us to read input without the user seeing it.
	attrs.local_flags &= !(LocalFlags::ICANON | LocalFlags::ECHO);

	if let Err(e) = tcsetattr(&reader, SetArg::TCSANOW, &attrs) {
		error!(logger, "Error setting terminal attributes: {}", e);
		return;
	}

	let mut shell = Shell::new();
	shell.run();
}

fn isatty<T: AsFd>(fd: T) -> bool {
	unistd::isatty(fd.as_fd().as_raw_fd()).unwrap_or(false)
}
