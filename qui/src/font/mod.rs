use crate::canvas::Canvas;

mod bdf;
pub use bdf::BdfFont;

pub trait Font {
	/// Returns (width, height) of a rendered glyph in pixels
	fn glyph_size(&self, ch: char) -> (i32, i32);

	/// Renders a glyph at (x, y) in the given color
	fn draw_glyph(&self, canvas: &mut Canvas, x: i32, y: i32, ch: char, color: u32);

	/// Horizontal advance after drawing ch (usually glyph_width + kerning)
	fn advance(&self, ch: char) -> i32;

	/// Line height — vertical advance between baselines
	fn line_height(&self) -> i32;

	fn measure_text(&self, text: &str) -> (i32, i32) {
		let width = text.chars().map(|ch| self.advance(ch)).sum();
		let height = self.line_height();
		(width, height)
	}
}

pub enum FontError {
	UnsupportedCharacter(char),
}
