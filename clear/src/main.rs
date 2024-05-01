use clap::Command;
use escapes::{ANSIEscapeSequence, CursorPosition, EraseInDisplay};

fn main() {
	Command::new("clear")
		.about("clear the terminal screen")
		.author("Colin Douch <colin@quirl.co.nz>")
		.get_matches();

	print!(
		"{}{}",
		ANSIEscapeSequence::EraseInDisplay(EraseInDisplay(2)),
		ANSIEscapeSequence::CursorPosition(CursorPosition(0, 0))
	);
}
