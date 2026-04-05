pub struct Canvas<'a> {
	pub width: i32,
	pub height: i32,
	stride: i32,
	pub pixels: &'a mut [u32],
}

impl<'a> Canvas<'a> {
	pub fn new(pixels: &'a mut [u32], width: i32, height: i32, stride: i32) -> Self {
		Self {
			width,
			height,
			pixels,
			stride,
		}
	}

	pub fn fill(&mut self, color: u32) {
		for row in 0..self.height {
			let start = (row * self.stride) as usize;
			for col in 0..self.width as usize {
				self.pixels[start + col] = color;
			}
		}
	}

	pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
		let x2 = (x + width).min(self.width);
		let y2 = (y + height).min(self.height);
		for j in y.max(0)..y2.max(0) {
			for i in x.max(0)..x2.max(0) {
				(*self.pixels)[(j * self.stride + i) as usize] = color;
			}
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

		Canvas::new(&mut self.pixels[offset..], clamped_w, clamped_h, self.stride)
	}
}
