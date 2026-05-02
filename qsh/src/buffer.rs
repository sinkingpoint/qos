use std::{
	cmp::Ordering,
	io::{self, Read, Write},
};

use escapes::{
	ANSIEscapeSequence, CursorBack, CursorDown, CursorForward, CursorHide, CursorShow, CursorUp, EraseInLine, ESC,
	GREEN, REVERSE,
};
use tables::RowTable;

// The ASCII character for DEL.
const DELETE_CHAR: char = '\u{7f}';

// The ASCII character for Backspace.
const BACKSPACE_CHAR: char = '\u{0008}';

/// Buffer is a wrapper around a Read that handles terminal IO.
pub struct Buffer<R: Read, W: Write> {
	/// The currently buffered input.
	buffer: String,

	/// The position of the cursor in the buffer.
	position: usize,
	reader: R,
	writer: W,

	history: Vec<String>,
	history_position: usize,

	prompt_length: usize,
	current_tab_completion: Option<TabCompletionBuffer>,
}

impl<R: Read, W: Write> Buffer<R, W> {
	pub fn new(reader: R, writer: W) -> Self {
		Buffer {
			buffer: String::new(),
			position: 0,
			reader,
			writer,
			history: Vec::new(),
			history_position: 0,
			prompt_length: 0,
			current_tab_completion: None,
		}
	}

	/// Read a line from the buffer.
	pub fn read(&mut self, prompt: &str) -> io::Result<String> {
		self.prompt_length = prompt.len();
		write!(self.writer, "\r\n{}", prompt).expect("Failed to write to stdout");
		loop {
			let c = self.read_char()?;
			if c == '\n' {
				if let Some(cmd) = self.handle_newline() {
					return Ok(cmd);
				}
			} else if c == DELETE_CHAR || c == BACKSPACE_CHAR {
				self.backspace();
			} else if c == ESC {
				self.handle_escape_sequence()?;
			} else if c == '\t' {
				self.handle_tab_completion();
			} else {
				self.push_char(c);
			}
		}
	}

	fn handle_newline(&mut self) -> Option<String> {
		if let Some(tab_completion) = &mut self.current_tab_completion {
			let selected_value = tab_completion.rows.value(tab_completion.current_selection)?.clone();
			tab_completion
				.wipe(&mut self.writer)
				.expect("Failed to wipe tab completion buffer");
			write!(self.writer, "{}", CursorShow).expect("Failed to write to stdout");
			self.auto_complete_to(&selected_value);
			self.current_tab_completion = None;
			return None;
		}

		writeln!(self.writer).expect("Failed to write to stdout");
		self.push_history(self.buffer.clone());
		Some(self.flush())
	}

	// Add a line to the history buffer, so that it can be navigated with the up and down arrow keys.
	fn push_history(&mut self, line: String) {
		if line.is_empty() {
			return;
		}

		self.history.push(line);
		self.history_position = self.history.len();
	}

	// Returns a list of candidates for tab completion based on the current buffer.
	fn tab_completion_candidates(&self) -> Vec<String> {
		let last_word = self.buffer[..self.position].split_whitespace().last().unwrap_or("");
		std::fs::read_dir(".")
			.unwrap()
			.filter_map(|entry| entry.ok())
			.filter_map(|entry| entry.file_name().into_string().ok())
			.filter(|name| name.starts_with(last_word))
			.collect()
	}

	fn auto_complete_to(&mut self, completion: &str) {
		let last_word = self.buffer[..self.position].split_whitespace().last().unwrap_or("");
		let to_complete = &completion[last_word.len()..];
		for char in to_complete.chars() {
			self.push_char(char);
		}
	}

	fn handle_tab_completion(&mut self) {
		if let Some(tab_completion) = &mut self.current_tab_completion {
			tab_completion.move_selection_down();
			tab_completion
				.render(&mut self.writer)
				.expect("Failed to render tab completion buffer");
			return;
		}

		let candidates = self.tab_completion_candidates();
		match candidates.len() {
			0 => (), // No candidates to return
			1 => {
				// For the case of just one completion, complete it
				self.auto_complete_to(&candidates[0]);
			}
			_ => {
				if let Some(prefix) = common_prefix(&candidates) {
					// They all have a common prefix, so just complete that
					self.auto_complete_to(&prefix);
					return;
				}

				// Hide the cursor while the tab completion buffer is open, to avoid it being rendered in the middle of the buffer and looking weird.
				write!(self.writer, "{}", CursorHide).expect("Failed to write to stdout");
				self.current_tab_completion = Some(TabCompletionBuffer::new(candidates, self.prompt_length));
				self.current_tab_completion
					.as_ref()
					.unwrap()
					.render(&mut self.writer)
					.expect("Failed to render tab completion buffer");
			}
		}
	}

	fn handle_history_navigation(&mut self, direction: isize) {
		if self.history.is_empty() {
			return;
		}

		let new_position =
			(self.history_position as isize + direction).clamp(0, self.history.len() as isize - 1) as usize;
		if new_position != self.history_position {
			self.history_position = new_position;
			self.move_cursor(-(self.position as isize));
			self.buffer = self.history[self.history_position].clone();
			self.position = self.buffer.len();
			write!(self.writer, "{}{}", EraseInLine(0), &self.buffer).expect("Failed to write to stdout");
		}
	}

	/// Handle an ANSI escape sequence.
	fn handle_escape_sequence(&mut self) -> io::Result<()> {
		let escape = ANSIEscapeSequence::read(&mut self.reader).map_err(|e| {
			io::Error::new(
				io::ErrorKind::InvalidData,
				format!("Failed to parse ANSI escape sequence: {}", e),
			)
		})?;

		// If we are currently showing a tab completion buffer, we want to handle the escape sequence in that buffer instead of the main input buffer.
		if let Some(tab_completion) = &mut self.current_tab_completion {
			match escape {
				ANSIEscapeSequence::CursorUp(_) => tab_completion.move_selection_up(),
				ANSIEscapeSequence::CursorDown(_) => tab_completion.move_selection_down(),
				ANSIEscapeSequence::CursorForward(_) => tab_completion.move_selection_right(),
				ANSIEscapeSequence::CursorBack(_) => tab_completion.move_selection_left(),
				_ => (),
			}

			tab_completion
				.render(&mut self.writer)
				.expect("Failed to render tab completion buffer");
			return Ok(());
		}

		match escape {
			ANSIEscapeSequence::CursorForward(amt) => self.move_cursor(amt.0 as isize),
			ANSIEscapeSequence::CursorBack(amt) => self.move_cursor(-(amt.0 as isize)),
			ANSIEscapeSequence::CursorUp(amt) => self.handle_history_navigation(-(amt.0 as isize)),
			ANSIEscapeSequence::CursorDown(amt) => self.handle_history_navigation(amt.0 as isize),
			_ => (),
		}

		Ok(())
	}

	/// Move the cursor by the given amount across the buffer.
	fn move_cursor(&mut self, amt: isize) {
		// Find the new position and clamp it to the bounds of the buffer.
		let mut new_position = self.position as isize + amt;
		if new_position < 0 {
			new_position = 0;
		} else if new_position > self.buffer.len() as isize {
			new_position = self.buffer.len() as isize;
		}

		let new_position = new_position as usize;

		match new_position.cmp(&self.position) {
			Ordering::Less => write!(self.writer, "{}", CursorBack((self.position - new_position) as u8))
				.expect("Failed to write to stdout"),
			Ordering::Greater => write!(self.writer, "{}", CursorForward((new_position - self.position) as u8))
				.expect("Failed to write to stdout"),
			Ordering::Equal => (),
		}

		self.position = new_position;
	}

	/// Add a character to the buffer at the current position.
	fn push_char(&mut self, c: char) {
		if self.position == self.buffer.len() {
			self.buffer.push(c);
		} else {
			self.buffer.insert(self.position, c);
		}

		self.position += 1;
		self.rerender();
	}

	// Remove a character from the buffer at the current position.
	fn backspace(&mut self) {
		if self.position == 0 {
			return;
		}

		if self.position == self.buffer.len() {
			self.buffer.pop();
		} else {
			self.buffer.remove(self.position - 1);
		}

		self.position -= 1;
		write!(self.writer, "{}{}", CursorBack(1), EraseInLine(0)).expect("Failed to write to stdout");
		write!(self.writer, "{}", &self.buffer[self.position..]).expect("Failed to write to stdout");
		if self.buffer.len() > self.position {
			write!(self.writer, "{}", CursorBack((self.buffer.len() - self.position) as u8))
				.expect("Failed to write to stdout");
		}
	}

	// Rewrite the current line, starting from the current position.
	fn rerender(&mut self) {
		let start = if self.position == 0 { 0 } else { self.position - 1 };
		write!(self.writer, "{}{}", EraseInLine(0), &self.buffer[start..]).expect("Failed to write to stdout");

		// After rewriting a line, we are at the end of it. If we were in the middle of the string, we need to move the cursor back.
		if self.buffer.len() > self.position {
			write!(self.writer, "{}", CursorBack((self.buffer.len() - self.position) as u8))
				.expect("Failed to write to stdout");
		}
	}

	/// Flush the buffer and return the contents.
	fn flush(&mut self) -> String {
		let buffer = self.buffer.clone();
		self.buffer.clear();
		self.position = 0;
		buffer
	}

	/// Read a single character from the input.
	fn read_char(&mut self) -> io::Result<char> {
		let mut char_buffer = [0; 1];
		self.reader.read_exact(&mut char_buffer)?;
		Ok(char_buffer[0] as char)
	}
}

// Returns the common prefix of the list of strings, if any.
fn common_prefix(strs: &[String]) -> Option<String> {
	if strs.is_empty() {
		return None;
	}

	let substr = strs[0]
		.char_indices()
		.take_while(|(idx, c)| strs.iter().all(|s| s.chars().nth(*idx) == Some(*c)))
		.map(|(_, c)| c)
		.collect::<String>();

	if substr.is_empty() {
		return None;
	}

	Some(substr)
}

struct TabCompletionBuffer {
	current_selection: usize,
	prompt_length: usize,

	rows: RowTable,
}

impl TabCompletionBuffer {
	fn new(candidates: Vec<String>, prompt_length: usize) -> Self {
		let mut rows = RowTable::new(211);
		for candidate in &candidates {
			let _ = rows.add_value(candidate.clone());
		}

		rows.style_value(0, vec![GREEN, REVERSE]);
		TabCompletionBuffer {
			current_selection: 0,
			prompt_length,
			rows,
		}
	}

	fn move_selection(&mut self, new_selection: usize) {
		if new_selection >= self.rows.num_values() {
			return;
		}

		self.rows.reset_value_style(self.current_selection);
		self.current_selection = new_selection;
		self.rows.style_value(self.current_selection, vec![GREEN, REVERSE]);
	}

	fn move_selection_left(&mut self) {
		if self.current_selection > 0 {
			self.move_selection(self.current_selection - 1);
		}
	}

	fn move_selection_right(&mut self) {
		if self.current_selection < self.rows.num_values() - 1 {
			self.move_selection(self.current_selection + 1);
		}
	}

	fn move_selection_up(&mut self) {
		if self.current_selection >= self.rows.num_cols() {
			self.move_selection(self.current_selection - self.rows.num_cols());
		} else {
			self.move_selection(self.current_selection % self.rows.num_cols() - 1);
		}
	}

	fn move_selection_down(&mut self) {
		if self.current_selection + self.rows.num_cols() < self.rows.num_values() {
			self.move_selection(self.current_selection + self.rows.num_cols());
		} else {
			self.move_selection(self.current_selection % self.rows.num_cols() + 1);
		}
	}

	fn render(&self, writer: &mut impl Write) -> io::Result<()> {
		write!(writer, "\n{}{}", self.rows, CursorUp(self.rows.num_rows() as u8 + 1))?;
		Ok(())
	}

	fn wipe(&self, writer: &mut impl Write) -> io::Result<()> {
		write!(writer, "{}", CursorDown(1))?;
		for _ in 0..self.rows.num_rows() {
			write!(writer, "{}{}", EraseInLine(0), CursorDown(1))?;
		}
		write!(
			writer,
			"{}\r{}",
			CursorUp(self.rows.num_rows() as u8 + 1),
			CursorForward(self.prompt_length as u8)
		)?;
		Ok(())
	}
}
