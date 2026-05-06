use std::{
	collections::VecDeque,
	os::fd::AsRawFd,
	sync::{
		Arc, Mutex,
		mpsc::{self, TryRecvError},
	},
	thread,
};

use nix::{
	pty::forkpty,
	unistd::{execve, write},
};
use qui::{
	Scene, TopBar,
	font::{BdfFont, Font},
};

use crate::view::{Terminal, TerminalState};

mod view;

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
	let mut scene = Scene::new(requested_width, requested_height);
	let state = Arc::new(Mutex::new(TerminalState::new(80, 24)));
	let terminal = Terminal::new(Arc::clone(&state));
	let top_bar = TopBar::new("qsh".to_string());
	let top_bar_handle = scene.add_widget(top_bar, 0, 0);
	scene.add_widget(terminal, 0, 30);
	scene.render(&mut app.canvas().unwrap());
	app.commit_frame().expect("failed to commit frame");

	let read_fd = pty.master.as_raw_fd();
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
			state.lock().unwrap().handle_input(input);
		}
	});

	loop {
		match shell_exit_rx.try_recv() {
			Ok(()) | Err(TryRecvError::Disconnected) => break,
			Err(TryRecvError::Empty) => {}
		}

		let event = app.poll().expect("failed to poll app events");
		scene.handle_event(&event);
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
				scene.render(&mut app.canvas().expect("no canvas ready"));
				app.commit_frame().expect("failed to commit frame");
			}
			qui::AppEvent::Close => break,
			_ => {}
		}

		while let Some(scene_event) = scene.poll() {
			if let Some(button_event) = top_bar_handle.extract(&scene_event) {
				match button_event {
					qui::TopBarEvent::DragStarted => app.start_move().unwrap(),
				}
			}
		}
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
