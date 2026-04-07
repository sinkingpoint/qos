use crate::font::Font;

pub struct Canvas<'a> {
	pub width: i32,
	pub height: i32,
	stride: i32,
	pixels: &'a mut [u32],
	x_offset: i32,
	y_offset: i32,
	damage: &'a mut Vec<(i32, i32, i32, i32)>,
}

impl<'a> Canvas<'a> {
	pub fn new(
		pixels: &'a mut [u32],
		width: i32,
		height: i32,
		stride: i32,
		x_offset: i32,
		y_offset: i32,
		damage: &'a mut Vec<(i32, i32, i32, i32)>,
	) -> Self {
		Self {
			width,
			height,
			pixels,
			stride,
			x_offset,
			y_offset,
			damage,
		}
	}

	pub fn fill(&mut self, color: u32) {
		for row in 0..self.height {
			let start = (row * self.stride) as usize;
			for col in 0..self.width as usize {
				self.pixels[start + col] = color;
			}
		}
		self.record_damage(0, 0, self.width, self.height);
	}

	pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
		let x2 = (x + width).min(self.width);
		let y2 = (y + height).min(self.height);
		for j in y.max(0)..y2.max(0) {
			for i in x.max(0)..x2.max(0) {
				(*self.pixels)[(j * self.stride + i) as usize] = color;
			}
		}
		self.record_damage(x.max(0), y.max(0), (x2 - x.max(0)).max(0), (y2 - y.max(0)).max(0));
	}

	fn record_damage(&mut self, x: i32, y: i32, width: i32, height: i32) {
		self.damage.push((x + self.x_offset, y + self.y_offset, width, height));
	}

	pub fn set_pixel(&mut self, x: i32, y: i32, color: u32) {
		if x >= 0 && x < self.width && y >= 0 && y < self.height {
			(*self.pixels)[(y * self.stride + x) as usize] = color;
			self.record_damage(x, y, 1, 1);
		}
	}

	pub fn draw_text(&mut self, font: &dyn Font, x: i32, y: i32, text: &str, color: u32) {
		let mut cursor_x = x;
		for ch in text.chars() {
			font.draw_glyph(self, cursor_x, y, ch, color);
			cursor_x += font.advance(ch);
		}
	}

	pub fn sub(&mut self, x: i32, y: i32, width: i32, height: i32) -> Canvas<'_> {
		let x0 = x.max(0).min(self.width);
		let y0 = y.max(0).min(self.height);
		let x1 = (x + width).min(self.width);
		let y1 = (y + height).min(self.height);
		let clamped_w = (x1 - x0).max(0);
		let clamped_h = (y1 - y0).max(0);

		// Offset into the parent slice at (x0, y0)
		let offset = (y0 * self.width + x0) as usize;

		Canvas::new(
			&mut self.pixels[offset..],
			clamped_w,
			clamped_h,
			self.stride,
			x0 + self.x_offset,
			y0 + self.y_offset,
			self.damage,
		)
	}
}
