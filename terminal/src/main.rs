use std::{
	collections::VecDeque,
	io::{self, Cursor},
	os::fd::AsRawFd,
	sync::{
		Arc, Mutex,
		mpsc::{self, TryRecvError},
	},
	thread,
};

use escapes::{ANSIEscapeSequence, AnsiParserError};
use nix::{
	pty::forkpty,
	unistd::{execve, write},
};
use qui::font::{BdfFont, Font};

fn main() {
	let pty = unsafe { forkpty(None, None) }.expect("failed to fork pty");
	if pty.fork_result.is_child() {
		execve(c"/bin/qsh", &[c"qsh"], &[c"PATH=/bin"]).expect("failed to exec qsh");
	}

	let font = BdfFont::from_bdf_data(include_bytes!("../assets/ter-u16n.bdf")).expect("failed to load font");
	let (char_width, char_height) = font.measure_text("a");
	let requested_width = char_width * 80;
	let requested_height = char_height * 24;

	let mut app = qui::App::new("qsh".to_string(), requested_width, requested_height).expect("failed to create app");
	let terminal = Arc::new(Mutex::new(Terminal::new()));
	terminal
		.lock()
		.unwrap()
		.render(&mut app.canvas().expect("no canvas ready"), &font);
	app.commit_frame().expect("failed to commit frame");

	let read_fd = pty.master.as_raw_fd();
	let terminal_clone = Arc::clone(&terminal);
	let (shell_exit_tx, shell_exit_rx) = mpsc::channel();
	thread::spawn(move || {
		loop {
			let mut input_buf = [0u8; 1024];
			let n = match nix::unistd::read(read_fd, &mut input_buf) {
				Ok(n) => n,
				Err(_) => {
					let _ = shell_exit_tx.send(());
					break;
				}
			};
			if n == 0 {
				let _ = shell_exit_tx.send(());
				break; // EOF
			}
			let input = &input_buf[..n];
			terminal_clone.lock().unwrap().handle_input(input);
		}
	});

	loop {
		match shell_exit_rx.try_recv() {
			Ok(()) | Err(TryRecvError::Disconnected) => break,
			Err(TryRecvError::Empty) => {}
		}

		let event = app.poll().expect("failed to poll app events");
		match event {
			qui::AppEvent::Keyboard {
				#[allow(unused_variables)]
				keycode,
				pressed,
				keysym,
			} if pressed
				&& let Some(keysym) = keysym
				&& let Some(keycode) = keysym.to_utf32() =>
			{
				let mut c = char::from_u32(keycode).unwrap_or('\0');
				if keycode == 0x08 {
					c = '\u{7f}'; // Backspace should send DEL for terminal compatibility.
				}
				let mut bytes = [0u8; 4];
				let len = c.encode_utf8(&mut bytes).len();
				write(pty.master.as_raw_fd(), &bytes[..len]).expect("failed to write to pty master");
			}
			qui::AppEvent::RenderReady => {
				terminal
					.lock()
					.unwrap()
					.render(&mut app.canvas().expect("no canvas ready"), &font);
				app.commit_frame().expect("failed to commit frame");
			}
			qui::AppEvent::Close => break,
			_ => {}
		}
	}
}

struct Terminal {
	contents: Vec<Vec<char>>,
	dimensions: (usize, usize),
	scroll_position: usize,
	decoder: UTF8Decoder,
	cursor_position: (usize, usize),
	last_key_press_time: Option<std::time::Instant>,
	partial_escape: Option<Vec<u8>>,
}

impl Terminal {
	fn new() -> Self {
		Self {
			contents: vec![vec![' '; 80]; 24],
			dimensions: (80, 24),
			scroll_position: 0,
			cursor_position: (0, 0),
			last_key_press_time: None,
			decoder: UTF8Decoder::new(),
			partial_escape: None,
		}
	}

	fn handle_input(&mut self, input: &[u8]) {
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
				self.contents.extend(vec![vec![' '; self.dimensions.0]; overflow]);
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
			ANSIEscapeSequence::EraseInLine(mode) => {
				let y = self.cursor_position.1;
				match mode.0 {
					0 => {
						for x in self.cursor_position.0..self.dimensions.0 {
							self.contents[y + self.scroll_position][x] = ' ';
						}
					}
					1 => {
						for x in 0..=self.cursor_position.0 {
							self.contents[y + self.scroll_position][x] = ' ';
						}
					}
					2 => {
						for x in 0..self.dimensions.0 {
							self.contents[y + self.scroll_position][x] = ' ';
						}
					}
					_ => {}
				}
			}
			_ => {}
		}
	}

	fn push_char(&mut self, ch: char) {
		self.contents[self.cursor_position.1 + self.scroll_position][self.cursor_position.0] = ch;
		self.cursor_position.0 += 1;
		if self.cursor_position.0 >= self.dimensions.0 {
			self.cursor_position.0 = 0;
			self.move_cursor_y(self.cursor_position.1 + 1);
		}
	}

	fn render(&self, canvas: &mut qui::Canvas, font: &BdfFont) {
		let (char_width, char_height) = font.measure_text("a");
		let font_descent = font.font_descent.unwrap_or(0);
		canvas.fill(0xFF000000);
		let cursor_flash_on = (std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_millis()
			/ 500)
			.is_multiple_of(2)
			|| self.last_key_press_time.is_some_and(|t| t.elapsed().as_millis() < 500);
		for (y, row) in self
			.contents
			.iter()
			.skip(self.scroll_position)
			.take(self.dimensions.1)
			.enumerate()
		{
			for (x, &ch) in row.iter().enumerate() {
				let row_origin_y = (y as i32 * char_height) - font_descent;
				canvas.draw_text(
					font,
					(x * char_width as usize) as i32,
					row_origin_y,
					&ch.to_string(),
					0xFFFFFFFF,
				);
			}
		}

		let cursor_flash_color = if cursor_flash_on { 0xFFFFFFFF } else { 0xFF000000 };
		canvas.fill_rect(
			self.cursor_position.0 as i32 * char_width,
			(self.cursor_position.1) as i32 * char_height,
			char_width,
			char_height,
			cursor_flash_color,
		);
	}
}

struct UTF8Decoder {
	buffer: VecDeque<u8>,
}

impl UTF8Decoder {
	fn new() -> Self {
		Self {
			buffer: VecDeque::new(),
		}
	}

	fn push_bytes(&mut self, bytes: &[u8]) {
		self.buffer.extend(bytes);
	}

	fn next_char(&mut self) -> Option<char> {
		let bytes: Vec<u8> = self.buffer.iter().take(4).cloned().collect();
		let max_len = bytes.len().min(4);
		for len in 1..=max_len {
			if let Ok(s) = std::str::from_utf8(&bytes[..len])
				&& let Some(c) = s.chars().next()
			{
				for _ in 0..len {
					self.buffer.pop_front();
				}
				return Some(c);
			}
		}
		None
	}

	fn next_byte(&mut self) -> Option<u8> {
		self.buffer.pop_front()
	}

	fn peek_next_byte(&self) -> Option<u8> {
		self.buffer.front().cloned()
	}
}
