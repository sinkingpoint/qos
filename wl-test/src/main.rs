use qui::{font::BdfFont, Anchor, App, AppEvent, Layer, LayerSurface, Scene};

const SPLEEN_BDF: &[u8] = include_bytes!("../assets/ter-u16n.bdf");

fn main() -> std::io::Result<()> {
	let font = BdfFont::from_bdf_data(SPLEEN_BDF).unwrap();
	let mut app = App::new("wl-test".to_string(), 400, 300)?;
	let mut bar = LayerSurface::new(0, 30, Layer::Top, Anchor::Left | Anchor::Right | Anchor::Top)?;
	let mut cursor: Option<(i32, i32)> = None;
	let mut button_pressed = false;
	let mut last_key: u32 = 0;

	let mut scene = Scene::new(400, 300);
	let _button_handle = scene.add_widget(qui::Button::new("Click me".to_string()), 150, 100);

	draw_frame(&mut app, cursor, &font, button_pressed, last_key);
	scene.render(&mut app.canvas());
	app.commit_frame()?;

	draw_bar(&mut bar, &font);
	bar.commit_frame()?;

	loop {
		let event = app.poll()?;
		scene.handle_event(&event);
		match event {
			AppEvent::Frame => {
				draw_frame(&mut app, cursor, &font, button_pressed, last_key);
				scene.render(&mut app.canvas());
				app.commit_frame()?;
			}
			AppEvent::PointerMotion { x, y } => cursor = Some((x, y)),
			AppEvent::PointerButton { button, pressed, .. } => {
				if button == 0x110 {
					button_pressed = pressed;
				}
			}
			AppEvent::Keyboard { keycode, pressed } => {
				if pressed {
					last_key = keycode;
				}
			}
			AppEvent::Close => break,
		}

		// Drain bar events non-blocking so its socket doesn't fill up.
		while let Some(bar_event) = bar.try_poll()? {
			if matches!(bar_event, AppEvent::Frame) {
				draw_bar(&mut bar, &font);
				bar.commit_frame()?;
			}
		}

		while let Some(scene_event) = scene.poll() {
			if let Some(button_event) = _button_handle.extract(&scene_event) {
				match button_event {
					qui::ButtonEvent::Clicked => println!("Button clicked!"),
				}
			}
		}
	}

	Ok(())
}

fn draw_bar(bar: &mut LayerSurface<'_>, font: &BdfFont) {
	let mut canvas = bar.canvas();
	canvas.fill(0xFF1a1a2e);
	canvas.draw_text(font, 10, 8, "wl-test", 0xFFFFFFFF);
}

fn draw_frame(app: &mut App<'_>, cursor: Option<(i32, i32)>, font: &BdfFont, button_pressed: bool, last_key: u32) {
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
