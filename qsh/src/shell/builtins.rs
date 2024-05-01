use common::io::IOTriple;
use escapes::{ANSIEscapeSequence, CursorPosition, EraseInDisplay};
use std::io::Write;

use crate::process::WaitError;

use super::Shell;

/// A builtin command, i.e. a command that runs inside the shell without executing a new process.
/// This allows closer integration with the shell, such as changing the working directory.
pub trait Builtin {
	fn run(&self, args: &[String], triple: IOTriple, shell: &Shell) -> Result<i32, WaitError>;
}

/// The `clear` builtin, which clears the terminal screen.
pub struct Clear;

impl Builtin for Clear {
	fn run(&self, _args: &[String], triple: IOTriple, _shell: &Shell) -> Result<i32, WaitError> {
		let mut stdout = triple.stdout();
		write!(
			stdout,
			"{}{}",
			ANSIEscapeSequence::EraseInDisplay(EraseInDisplay(2)),
			ANSIEscapeSequence::CursorPosition(CursorPosition(0, 0))
		)?;
		stdout.flush()?;
		Ok(0)
	}
}

/// The `cd` builtin, which changes the current working directory.
pub struct Cd;

impl Builtin for Cd {
	fn run(&self, args: &[String], _triple: IOTriple, _shell: &Shell) -> Result<i32, WaitError> {
		if args.len() != 2 {
			eprintln!("cd: expected 1 argument, got {}", args.len() - 1);
			return Ok(1);
		}

		let path = &args[1];
		if let Err(e) = std::env::set_current_dir(path) {
			eprintln!("cd: {}: {}", path, e);
			return Ok(1);
		}

		Ok(0)
	}
}
