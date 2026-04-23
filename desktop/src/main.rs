use qui::{
	Anchor, AppEvent, Layer, LayerSurface, WaylandContext,
	font::{BdfFont, Font},
};

const SPLEEN_BDF: &[u8] = include_bytes!("../assets/ter-u16n.bdf");

fn main() -> std::io::Result<()> {
	let font = BdfFont::from_bdf_data(SPLEEN_BDF).unwrap();
	let context = WaylandContext::connect()?;
	let mut bar = LayerSurface::new_with_context(
		context.clone(),
		0,
		30,
		Layer::Top,
		Anchor::Left | Anchor::Right | Anchor::Top,
	)?;
	let mut background = LayerSurface::new_with_context(
		context.clone(),
		0,
		0,
		Layer::Bottom,
		Anchor::Left | Anchor::Right | Anchor::Top | Anchor::Bottom,
	)?;

	draw_bar(&mut bar, &font);
	draw_background(&mut background);

	loop {
		let mut handled_any = false;
		let mut bar_render_ready = false;
		let mut background_render_ready = false;

		while let Some(bar_event) = bar.try_poll()? {
			handled_any = true;
			if matches!(bar_event, AppEvent::RenderReady) {
				bar_render_ready = true;
			}
		}

		while let Some(background_event) = background.try_poll()? {
			handled_any = true;
			if matches!(background_event, AppEvent::RenderReady) {
				background_render_ready = true;
			}
		}

		// Reconciliation pass: polling one surface can route packets that enqueue
		// events for the other surface without yielding an event in that call.
		while let Some(bar_event) = bar.try_poll()? {
			handled_any = true;
			if matches!(bar_event, AppEvent::RenderReady) {
				bar_render_ready = true;
			}
		}

		while let Some(background_event) = background.try_poll()? {
			handled_any = true;
			if matches!(background_event, AppEvent::RenderReady) {
				background_render_ready = true;
			}
		}

		if bar_render_ready {
			draw_bar(&mut bar, &font);
		}

		if background_render_ready {
			draw_background(&mut background);
		}

		if !handled_any {
			// Nothing queued for our surfaces right now, so block for one incoming packet.
			context.borrow_mut().dispatch_one()?;
		}
	}
}

fn draw_bar(bar: &mut LayerSurface, font: &BdfFont) {
	let Some(mut canvas) = bar.canvas() else {
		return;
	};
	canvas.fill(0xFF1A1A2E);

	let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
	let (tw, th) = font.measure_text(&current_time);
	canvas.draw_text(font, canvas.width - tw - 10, (30 - th) / 2, &current_time, 0xFFFFFFFF);
	if let Err(e) = bar.commit_frame() {
		eprintln!("[desktop] draw bar commit error: {}", e);
	}
}

fn draw_background(background: &mut LayerSurface) {
	let Some(mut canvas) = background.canvas() else {
		return;
	};
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
	if let Err(e) = background.commit_frame() {
		eprintln!("[desktop] draw background commit error: {}", e);
	}
}
