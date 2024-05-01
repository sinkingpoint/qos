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
