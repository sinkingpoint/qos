use qui::{App, AppEvent};

fn main() -> std::io::Result<()> {
	let mut app = App::new("wl-test".to_string(), 400, 300)?;
	let mut cursor: Option<(i32, i32)> = None;
	let mut button_pressed = false;
	let mut last_key: u32 = 0;

	draw_frame(&mut app, cursor, button_pressed, last_key);
	app.commit_frame()?;

	loop {
		match app.poll()? {
			AppEvent::Frame => {
				draw_frame(&mut app, cursor, button_pressed, last_key);
				app.commit_frame()?;
			}
			AppEvent::PointerMotion { x, y } => cursor = Some((x, y)),
			AppEvent::PointerButton { button, pressed } => {
				if button == 0x110 {
					button_pressed = pressed;
					if pressed {
						app.start_move()?;
					}
				}
			}
			AppEvent::Keyboard { keycode, pressed } => {
				if pressed {
					last_key = keycode;
				}
			}
			AppEvent::Close => break,
		}
	}

	Ok(())
}

fn draw_frame(app: &mut App<'_>, cursor: Option<(i32, i32)>, button_pressed: bool, last_key: u32) {
	let mut canvas = app.canvas();

	canvas.fill(0xFF222244);

	let bar_colour = if last_key > 0 {
		let palette = [
			0xFFFF4444, 0xFF44FF44, 0xFF4444FF, 0xFF44FFFF, 0xFFFF44FF, 0xFFFFFF44, 0xFFFF8800, 0xFF00FF88,
		];
		palette[(last_key as usize) % palette.len()]
	} else {
		0xFF333333
	};
	let w = canvas.width;
	canvas.fill_rect(0, 0, w, 30, bar_colour);

	if let Some((cx, cy)) = cursor {
		let colour = if button_pressed { 0xFFFF4444 } else { 0xFFFFFF00 };
		canvas.fill_rect(cx - 15, cy - 15, 30, 30, colour);
	}
}
