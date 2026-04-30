use std::{
	os::fd::AsRawFd,
	sync::{Arc, Mutex},
	thread,
};

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
	thread::spawn(move || {
		loop {
			let mut input_buf = [0u8; 1024];
			let n = nix::unistd::read(read_fd, &mut input_buf).expect("failed to read from pty master");
			if n == 0 {
				break; // EOF
			}
			let input = &input_buf[..n];
			terminal_clone.lock().unwrap().handle_input(input);
		}
	});

	loop {
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
				println!("Key pressed: keycode={}, keysym={:?}", keycode, keysym);
				let mut c = char::from_u32(keycode).unwrap_or('\0');
				if keycode == 0x08 {
					c = '\u{7f}'; // Backspace should send DEL for terminal compatibility.
					println!("Interpreting Backspace as DEL");
				}
				let mut bytes = [0u8; 4];
				c.encode_utf8(&mut bytes);
				write(pty.master.as_raw_fd(), &bytes).expect("failed to write to pty master");
			}
			qui::AppEvent::RenderReady => {
				terminal
					.lock()
					.unwrap()
					.render(&mut app.canvas().expect("no canvas ready"), &font);
				app.commit_frame().expect("failed to commit frame");
			}
			_ => {}
		}
	}
}

struct Terminal {
	contents: [[char; 80]; 24],
	cursor_position: (u32, u32),
	last_key_press_time: Option<std::time::Instant>,
}

impl Terminal {
	fn new() -> Self {
		Self {
			contents: [[' '; 80]; 24],
			cursor_position: (0, 0),
			last_key_press_time: None,
		}
	}

	fn handle_input(&mut self, input: &[u8]) {
		self.last_key_press_time = Some(std::time::Instant::now());
		for &byte in input {
			if byte == b'\n' {
				self.contents[self.cursor_position.1 as usize][self.cursor_position.0 as usize] = ' ';
				self.cursor_position.0 = 0;
				self.cursor_position.1 += 1;
			} else {
				self.contents[self.cursor_position.1 as usize][self.cursor_position.0 as usize] = byte as char;
				self.cursor_position.0 += 1;
				if self.cursor_position.0 >= 80 {
					self.cursor_position.0 = 0;
					self.cursor_position.1 += 1;
				}
			}
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
		for (y, row) in self.contents.iter().enumerate() {
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
			self.cursor_position.1 as i32 * char_height,
			char_width,
			char_height,
			cursor_flash_color,
		);
	}
}
