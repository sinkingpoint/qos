use std::{
	ffi::{CStr, CString},
	io::{stderr, stdin},
	process::ExitCode,
};

use anyhow::{Context, Result};
use auth::User;
use clap::{Arg, Command};
use common::{io::IOTriple, obs::assemble_logger};
use nix::{
	sys::termios::{tcgetattr, tcsetattr, LocalFlags, SetArg, Termios},
	unistd::{chdir, execvp, setgid, setuid, Gid, Uid},
};
use slog::error;

const PASSWORD_ATTEMPTS: usize = 3;

fn disable_echo() -> Result<Termios> {
	let old_attrs = tcgetattr(stdin()).with_context(|| "failed to get terminal attributes")?;

	let mut new_attrs = old_attrs.clone();
	new_attrs.local_flags.remove(LocalFlags::ECHO);
	tcsetattr(stdin(), SetArg::TCSANOW, &new_attrs).with_context(|| "failed to set terminal attributes")?;

	Ok(old_attrs)
}

fn main() -> ExitCode {
	let matches = Command::new("login")
		.author("Colin Douch")
		.version("0.1.0")
		.about("A simple login")
		.arg(
			Arg::new("username")
				.help("The username to login as")
				.required(true)
				.index(1),
		)
		.get_matches();

	let username: &String = matches.get_one("username").unwrap();
	let logger = assemble_logger(stderr());

	let old_attrs = match disable_echo() {
		Ok(attrs) => attrs,
		Err(e) => {
			error!(logger, "Failed to disable echo"; "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	};

	let user: User = match User::from_username(username) {
		Ok(Some(user)) => user,
		Ok(None) => {
			error!(logger, "User not found"; "username" => username);
			return ExitCode::FAILURE;
		}
		Err(e) => {
			error!(logger, "Failed to read user"; "username" => username, "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	};

	let shadow = match user.shadow() {
		Ok(Some(shadow)) => shadow,
		Ok(None) => {
			error!(logger, "Shadow entry not found"; "username" => username);
			return ExitCode::FAILURE;
		}
		Err(e) => {
			error!(logger, "Failed to read shadow entry"; "username" => username, "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	};

	let mut successful = false;
	for _ in 0..PASSWORD_ATTEMPTS {
		let triple = IOTriple::default();
		let password = match triple.prompt("password:") {
			Ok(pass) => pass,
			Err(e) => {
				error!(logger, "Failed to read password"; "error" => format!("{:?}", e));
				return ExitCode::FAILURE;
			}
		};

		match shadow.verify_password(&password) {
			Ok(true) => {
				successful = true;
				break;
			}
			Ok(false) => {
				error!(logger, "Invalid password"; "username" => username);
			}
			Err(e) => {
				error!(logger, "Failed to verify password"; "username" => username, "error" => format!("{:?}", e));
				return ExitCode::FAILURE;
			}
		};
	}

	match tcsetattr(stdin(), SetArg::TCSANOW, &old_attrs) {
		Ok(_) => (),
		Err(e) => {
			error!(logger, "Failed to restore terminal attributes"; "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	}

	if !successful {
		error!(logger, "Failed to login"; "username" => username);
		return ExitCode::FAILURE;
	}

	let shell = match CString::new(user.shell.to_string_lossy().into_owned()) {
		Ok(shell) => shell,
		Err(e) => {
			error!(logger, "Failed to convert shell to CString"; "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	};

	println!("\nWelcome to qos, {}!", username);

	// Set the user's gid and uid. We have to `setgid` first, because once we drop
	// out of root, we won't be able to setgid anymore.
	match setgid(Gid::from_raw(user.gid)) {
		Ok(_) => (),
		Err(e) => {
			error!(logger, "Failed to setgid"; "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	}

	match setuid(Uid::from_raw(user.uid)) {
		Ok(_) => (),
		Err(e) => {
			error!(logger, "Failed to setuid"; "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	}

	match chdir(&user.home) {
		Ok(_) => (),
		Err(e) => {
			error!(logger, "Failed to chdir to users home"; "error" => format!("{:?}", e));
		}
	}

	match execvp::<&CStr>(&shell, &[]) {
		Ok(_) => (),
		Err(e) => {
			error!(logger, "Failed to execute shell"; "error" => format!("{:?}", e));
			return ExitCode::FAILURE;
		}
	}

	unreachable!("execvp returned successfully")
}
