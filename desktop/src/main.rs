use qui::{
	Anchor, AppEvent, Layer, LayerSurface,
	font::{BdfFont, Font},
};

const SPLEEN_BDF: &[u8] = include_bytes!("../assets/ter-u16n.bdf");

fn main() -> std::io::Result<()> {
	let font = BdfFont::from_bdf_data(SPLEEN_BDF).unwrap();
	let mut bar = LayerSurface::new(0, 30, Layer::Top, Anchor::Left | Anchor::Right | Anchor::Top)?;
	let mut background = LayerSurface::new(
		0,
		0,
		Layer::Bottom,
		Anchor::Left | Anchor::Right | Anchor::Top | Anchor::Bottom,
	)?;

	draw_bar(&mut bar, &font);
	draw_background(&mut background);

	loop {
		while let Some(bar_event) = bar.try_poll()? {
			if matches!(bar_event, AppEvent::Frame) {
				draw_bar(&mut bar, &font);
			}
		}

		while let Some(background_event) = background.try_poll()? {
			if matches!(background_event, AppEvent::Frame) {
				draw_background(&mut background);
			}
		}
	}
}

fn draw_bar(bar: &mut LayerSurface<'_>, font: &BdfFont) {
	let mut canvas = bar.canvas();
	canvas.fill(0xFF1A1A2E);

	let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
	let (tw, th) = font.measure_text(&current_time);
	canvas.draw_text(font, canvas.width - tw - 10, (30 - th) / 2, &current_time, 0xFFFFFFFF);
	bar.commit_frame().unwrap();
}

fn draw_background(background: &mut LayerSurface<'_>) {
	let mut canvas = background.canvas();
	canvas.fill(0xFF16213E);
	let text = "Hello, QOS!";
	let font = BdfFont::from_bdf_data(SPLEEN_BDF).unwrap();
	let (tw, th) = font.measure_text(text);
	canvas.draw_text(
		&font,
		(canvas.width - tw) / 2,
		(canvas.height - th) / 2,
		text,
		0xFFFFFFFF,
	);
	background.commit_frame().unwrap();
}
