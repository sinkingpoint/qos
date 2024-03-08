use std::{
	ffi::{CStr, CString},
	io::stderr,
	path::PathBuf,
};

use common::{io::IOTriple, obs::assemble_logger};
use slog::error;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use nix::{
	fcntl::{fcntl, open, FcntlArg, OFlag},
	libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO},
	sys::{
		signal::{signal, SigHandler, Signal},
		stat::Mode,
		utsname,
	},
	unistd::{close, dup2, execve},
};

fn ignore_signals() -> Result<()> {
	// Ignore all signals.
	unsafe {
		signal(Signal::SIGHUP, SigHandler::SigIgn).with_context(|| "failed to ignore SIGHUP")?;
		signal(Signal::SIGINT, SigHandler::SigIgn).with_context(|| "failed to ignore SIGINT")?;
		signal(Signal::SIGQUIT, SigHandler::SigIgn).with_context(|| "failed to ignore SIGQUIT")?;
	}

	Ok(())
}

fn print_issue(tty: &str) -> Result<()> {
	// Print the issue file.
	let issue_file = PathBuf::from("/etc/issue");
	if !issue_file.exists() {
		return Ok(());
	}

	let tty = tty.strip_prefix("/dev/").unwrap_or(tty);

	let mut issue = std::fs::read_to_string(&issue_file)
		.with_context(|| format!("failed to read the issue file at {}", issue_file.display()))?;
	let utsinfo = utsname::uname().with_context(|| "failed to fetch system information")?;

	let templates = [
		('l', tty),
		('m', utsinfo.machine().to_str().unwrap()),
		('n', utsinfo.nodename().to_str().unwrap()),
		('r', utsinfo.release().to_str().unwrap()),
		('s', utsinfo.sysname().to_str().unwrap()),
		('v', utsinfo.version().to_str().unwrap()),
	];

	for (escape, value) in templates.iter() {
		issue = issue.replace(&format!("\\{}", escape), value);
	}

	Ok(())
}

fn open_tty(tty: &str) -> Result<()> {
	// Open the given TTY and set it up to read/write.
	if tty != "-" {
		close(STDIN_FILENO).with_context(|| "failed to close stdin")?;
		open(tty, OFlag::O_RDWR, Mode::empty()).with_context(|| format!("failed to open {}", tty))?;
	}

	// Make sure that stdin is opened in R/W mode.
	let options = fcntl(STDIN_FILENO, FcntlArg::F_GETFL).with_context(|| "failed to get file descriptor flags")?;
	if !OFlag::from_bits_retain(options).contains(OFlag::O_RDWR) {
		return Err(anyhow::anyhow!("stdin is not opened in R/W mode"));
	}

	// Close the existing stdout and stderr, and copy the new TTY to them.
	close(STDOUT_FILENO).with_context(|| "failed to close stdout")?;
	close(STDERR_FILENO).with_context(|| "failed to close stderr")?;
	dup2(STDIN_FILENO, STDOUT_FILENO).with_context(|| "failed to copy stdin to stdout")?;
	dup2(STDIN_FILENO, STDERR_FILENO).with_context(|| "failed to copy stdin to stderr")?;

	Ok(())
}

fn main() {
	let matches = Command::new("getty")
		.author("Colin Douch")
		.version("0.1.0")
		.about("A simple getty")
		.arg(
			Arg::new("login-program")
				.short('l')
				.num_args(1)
				.default_value("/bin/login")
				.help("The login program to run"),
		)
		.arg(Arg::new("tty").help("The tty to open").required(true).index(1))
		.get_matches();

	let logger = assemble_logger(stderr());
	let login_program: &String = matches.get_one("login-program").unwrap();
	let tty: &String = matches.get_one("tty").unwrap();

	if let Err(e) = ignore_signals() {
		error!(logger, "Failed to ignore signals"; "error" => format!("{:?}", e));
		return;
	}

	if let Err(e) = print_issue(tty) {
		error!(logger, "Failed to print issue"; "error" => format!("{:?}", e));
		return;
	}

	if let Err(e) = open_tty(tty) {
		error!(logger, "Failed to open tty"; "error" => format!("{:?}", e));
		return;
	}

	// After here, `logger` is no longer valid because we've swapped out the underlying file descriptors.
	// Manually drop it here so that the compiler can tell us off if we try to use it again.
	drop(logger);

	let triple = IOTriple::default();
	let username = match triple.prompt("login:") {
		Ok(username) => username,
		Err(e) => {
			eprintln!("Failed to read username: {}", e);
			return;
		}
	};

	// Run the login program.
	let command = CString::new(login_program.as_str()).expect("login program contains null bytes");
	let args = [
		command.as_c_str(),
		&CString::new(username.trim()).expect("username contains null bytes"),
	];

	if let Err(e) = execve::<_, &CStr>(&command, &args, &[]) {
		eprintln!("Failed to execute {}: {}", login_program, e);
		return;
	}

	unreachable!("execve failed")
}
