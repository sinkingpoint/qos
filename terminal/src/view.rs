use std::{
	io::{self, Cursor},
	sync::{Arc, Mutex},
};

use escapes::{ANSIEscapeSequence, AnsiParserError};
use qui::{
	Widget,
	font::{BdfFont, Font},
};

use crate::UTF8Decoder;

#[derive(Debug, Clone)]
struct Cell {
	ch: char,
	fg_color: u32,
	bg_color: u32,
}

impl Cell {
	fn blank() -> Self {
		Self {
			ch: ' ',
			fg_color: 0xFFFFFFFF,
			bg_color: 0xFF000000,
		}
	}
}

pub struct TerminalState {
	decoder: UTF8Decoder,
	cursor_position: (usize, usize),
	last_key_press_time: Option<std::time::Instant>,
	partial_escape: Option<Vec<u8>>,
	contents: Vec<Vec<Cell>>,
	scroll_position: usize,
	dimensions: (usize, usize),
	current_fg_color: u32,
	current_bg_color: u32,
	cursor_visible: bool,
}

impl TerminalState {
	pub fn new(cols: usize, rows: usize) -> Self {
		Self {
			decoder: UTF8Decoder::new(),
			cursor_position: (0, 0),
			last_key_press_time: None,
			partial_escape: None,
			contents: vec![vec![Cell::blank(); cols]; rows],
			scroll_position: 0,
			dimensions: (cols, rows),
			current_fg_color: 0xFFFFFFFF,
			current_bg_color: 0xFF000000,
			cursor_visible: true,
		}
	}

	pub fn handle_input(&mut self, input: &[u8]) {
		self.last_key_press_time = Some(std::time::Instant::now());
		self.decoder.push_bytes(input);
		while let Some(byte) = self.decoder.peek_next_byte() {
			if let Some(escape) = self.partial_escape.as_mut() {
				self.decoder.next_byte();
				escape.push(byte);
				match ANSIEscapeSequence::read(&mut Cursor::new(escape)) {
					Ok(seq) => {
						self.handle_escape(seq);
						self.partial_escape = None;
					}
					Err(AnsiParserError::IO(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
						// Wait for more bytes to complete the escape sequence
						continue;
					}
					Err(_) => {
						// Invalid escape sequence, discard it
						self.partial_escape = None;
					}
				}
				// Handle partial escape sequence
			} else if byte == b'\x1b' {
				self.decoder.next_byte(); // Consume the escape character
				self.partial_escape = Some(vec![]);
			} else if byte == b'\n' {
				self.decoder.next_byte(); // Consume the newline
				self.cursor_position.0 = 0;
				self.move_cursor_y(self.cursor_position.1 + 1);
			} else if let Some(ch) = self.decoder.next_char() {
				self.push_char(ch);
			} else {
				break; // Wait for more bytes to form a complete character
			}
		}
	}

	fn move_cursor_y(&mut self, y: usize) {
		if y < self.dimensions.1 {
			self.cursor_position.1 = y;
		} else {
			let overflow = y - self.dimensions.1 + 1;
			self.cursor_position.1 = self.dimensions.1 - 1;
			// We hit the bottom of the terminal, so scroll up
			self.scroll_position += overflow;
			if self.contents.len() < (self.scroll_position + self.dimensions.1) {
				// Add new blank lines if we haven't already scrolled past the end of the buffer
				self.contents
					.extend(vec![vec![Cell::blank(); self.dimensions.0]; overflow]);
			}
		}
	}

	fn handle_escape(&mut self, escape: ANSIEscapeSequence) {
		match escape {
			ANSIEscapeSequence::CursorPosition(c) => {
				self.cursor_position.0 = (c.0 as usize).saturating_sub(1).min(self.dimensions.0 - 1);
				self.move_cursor_y((c.1 as usize).saturating_sub(1));
			}
			ANSIEscapeSequence::CursorUp(n) => {
				self.cursor_position.1 = self.cursor_position.1.saturating_sub(n.0 as usize);
			}
			ANSIEscapeSequence::CursorDown(n) => {
				self.move_cursor_y(self.cursor_position.1 + n.0 as usize);
			}
			ANSIEscapeSequence::CursorForward(n) => {
				self.cursor_position.0 = (self.cursor_position.0 + n.0 as usize).min(self.dimensions.0 - 1);
			}
			ANSIEscapeSequence::CursorBack(n) => {
				self.cursor_position.0 = self.cursor_position.0.saturating_sub(n.0 as usize);
			}
			ANSIEscapeSequence::Color(c) => match c.0 {
				0 => {
					self.current_fg_color = 0xFFFFFFFF;
					self.current_bg_color = 0xFF000000;
				}
				30..=37 => {
					self.current_fg_color = 0xFF000000 | color_code_to_rgb(c.0 - 30);
				}
				40..=47 => {
					self.current_bg_color = 0xFF000000 | color_code_to_rgb(c.0 - 40);
				}
				90..=97 => {
					self.current_fg_color = 0xFF000000 | color_code_to_rgb(c.0 - 90 + 8);
				}
				100..=107 => {
					self.current_bg_color = 0xFF000000 | color_code_to_rgb(c.0 - 100 + 8);
				}
				_ => {}
			},
			ANSIEscapeSequence::CursorHide(_) => {
				self.cursor_visible = false;
			}
			ANSIEscapeSequence::CursorShow(_) => {
				self.cursor_visible = true;
			}
			ANSIEscapeSequence::EraseInLine(mode) => {
				let y = self.cursor_position.1;
				match mode.0 {
					0 => {
						for x in self.cursor_position.0..self.dimensions.0 {
							self.contents[y + self.scroll_position][x] = Cell::blank();
						}
					}
					1 => {
						for x in 0..=self.cursor_position.0 {
							self.contents[y + self.scroll_position][x] = Cell::blank();
						}
					}
					2 => {
						for x in 0..self.dimensions.0 {
							self.contents[y + self.scroll_position][x] = Cell::blank();
						}
					}
					_ => {}
				}
			}
			_ => {}
		}
	}

	fn push_char(&mut self, ch: char) {
		self.contents[self.cursor_position.1 + self.scroll_position][self.cursor_position.0] = Cell {
			ch,
			fg_color: self.current_fg_color,
			bg_color: self.current_bg_color,
		};
		self.cursor_position.0 += 1;
		if self.cursor_position.0 >= self.dimensions.0 {
			self.cursor_position.0 = 0;
			self.move_cursor_y(self.cursor_position.1 + 1);
		}
	}
}

fn color_code_to_rgb(code: u8) -> u32 {
	match code {
		0 => 0x000000,                                       // Black
		1 => 0x800000,                                       // Red
		2 => 0x008000,                                       // Green
		3 => 0x808000,                                       // Yellow
		4 => 0x000080,                                       // Blue
		5 => 0x800080,                                       // Magenta
		6 => 0x008080,                                       // Cyan
		7 => 0xC0C0C0,                                       // White
		8..=15 => 0x808080 + ((code as u32 - 8) * 0x202020), // Bright variants
		_ => 0,
	}
}

pub struct Terminal {
	font: BdfFont,
	state: Arc<Mutex<TerminalState>>,
}

impl Terminal {
	pub fn new(state: Arc<Mutex<TerminalState>>) -> Self {
		Self {
			font: BdfFont::from_bdf_data(include_bytes!("../assets/ter-u16n.bdf") as &[u8]).unwrap(),
			state,
		}
	}
}

impl Widget for Terminal {
	type Event = ();

	fn handle_event(&mut self, _event: &qui::AppEvent) -> Option<Self::Event> {
		None
	}

	fn render(&mut self, canvas: &mut qui::Canvas) {
		let state = self.state.lock().unwrap();
		let (char_width, char_height) = self.font.measure_text("a");
		let font_descent = self.font.font_descent.unwrap_or(0);
		canvas.fill_rect(0, 0, canvas.width, canvas.height, 0xFF000000);
		let cursor_flash_on = (std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_millis()
			/ 500)
			.is_multiple_of(2)
			|| state.last_key_press_time.is_some_and(|t| t.elapsed().as_millis() < 500);
		for (y, row) in state
			.contents
			.iter()
			.skip(state.scroll_position)
			.take(state.dimensions.1)
			.enumerate()
		{
			for (x, ch) in row.iter().enumerate() {
				let row_origin_y = (y as i32 * char_height) - font_descent;
				canvas.fill_rect(
					(x * char_width as usize) as i32,
					row_origin_y,
					char_width,
					char_height,
					ch.bg_color,
				);
				canvas.draw_text(
					&self.font,
					(x * char_width as usize) as i32,
					row_origin_y,
					&ch.ch.to_string(),
					ch.fg_color,
				);
			}
		}

		if state.cursor_visible {
			let cursor_flash_color = if cursor_flash_on { 0xFFFFFFFF } else { 0xFF000000 };
			canvas.fill_rect(
				state.cursor_position.0 as i32 * char_width,
				(state.cursor_position.1) as i32 * char_height,
				char_width,
				char_height,
				cursor_flash_color,
			);
		}
	}

	fn size_hint(&self) -> (i32, i32) {
		let (char_width, char_height) = self.font.measure_text("a");
		let state = self.state.lock().unwrap();
		(
			state.dimensions.0 as i32 * char_width,
			state.dimensions.1 as i32 * char_height,
		)
	}
}
