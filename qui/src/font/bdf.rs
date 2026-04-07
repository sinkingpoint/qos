use std::{collections::HashMap, path::Path};

use thiserror::Error;

use crate::font::Font;

pub struct BdfFont {
	/// All encoded glyphs, keyed by Unicode character
	pub glyphs: HashMap<char, BdfGlyph>,
	/// XLFD font name (from FONT)
	pub name: String,
	/// Maximum glyph cell: (width, height, x_offset, y_offset) (from FONTBOUNDINGBOX)
	pub bounding_box: (u32, u32, i32, i32),
	/// Human-readable family name, e.g. "Spleen" (from FAMILY_NAME property)
	pub family_name: Option<String>,
	/// Weight descriptor, e.g. "Medium" (from WEIGHT_NAME property)
	pub weight: Option<String>,
	/// Font version string (from FONT_VERSION property)
	pub version: Option<String>,
	/// Foundry/vendor name (from FOUNDRY property)
	pub foundry: Option<String>,
	/// Upright, italic, oblique, etc. (from SLANT property)
	pub slant: Option<Slant>,
	/// Cell, Monospaced, or Proportional (from SPACING property)
	pub spacing: Option<Spacing>,
	/// Glyph height in pixels at intended display size (from PIXEL_SIZE property)
	pub pixel: Option<u32>,
	/// Design size in decipoints, e.g. 160 = 16pt (from POINT_SIZE property)
	pub point: Option<u32>,
	/// Horizontal resolution in DPI (from RESOLUTION_X property)
	pub resolution_x: Option<u32>,
	/// Vertical resolution in DPI (from RESOLUTION_Y property)
	pub resolution_y: Option<u32>,
	/// Average glyph width in 1/10ths of a pixel (from AVERAGE_WIDTH property)
	pub average_width: Option<u32>,
	/// Pixels below the baseline (from FONT_DESCENT property)
	pub font_descent: Option<i32>,
	/// Pixels above the baseline (from FONT_ASCENT property)
	pub font_ascent: Option<i32>,
}

pub struct BdfGlyph {
	/// The Unicode character this glyph represents (from ENCODING)
	pub ch: char,
	/// Bitmap rows as hex values, one entry per row, MSB = leftmost pixel (from BITMAP)
	pub bitmap: Vec<u32>,
	/// Scalable width in 1/1000ths of a point: (x, y) (from SWIDTH)
	pub s_width: (u32, u32),
	/// Device pixel advance after drawing this glyph: (x, y) (from DWIDTH)
	pub d_width: (u32, u32),
	/// Glyph bounding box: (width, height, x_offset, y_offset)
	/// x_offset/y_offset are relative to the pen origin; y_offset is negative when glyph descends below baseline (from BBX)
	pub bbx: (u32, u32, i32, i32),
}

impl BdfGlyph {
	fn from_lines(lines: Vec<&str>) -> Result<Self, BdfParseError> {
		let mut encoding = None;
		let mut s_width = None;
		let mut d_width = None;
		let mut bbx = None;
		let mut scan_lines = None;
		for line in lines {
			if let Some(enc) = line.strip_prefix("ENCODING ") {
				encoding = Some(
					enc.parse::<i32>()
						.map_err(|_| BdfParseError::InvalidField("ENCODING", enc.to_string()))?,
				);
			} else if let Some(sw) = line.strip_prefix("SWIDTH ") {
				let parts: Vec<&str> = sw.split_whitespace().collect();
				if parts.len() != 2 {
					return Err(BdfParseError::InvalidField("SWIDTH", sw.to_string()));
				}
				s_width = Some((
					parts[0]
						.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("SWIDTH", parts[0].to_string()))?,
					parts[1]
						.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("SWIDTH", parts[1].to_string()))?,
				));
			} else if let Some(dw) = line.strip_prefix("DWIDTH ") {
				let parts: Vec<&str> = dw.split_whitespace().collect();
				if parts.len() != 2 {
					return Err(BdfParseError::InvalidField("DWIDTH", dw.to_string()));
				}
				d_width = Some((
					parts[0]
						.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("DWIDTH", parts[0].to_string()))?,
					parts[1]
						.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("DWIDTH", parts[1].to_string()))?,
				));
			} else if let Some(b) = line.strip_prefix("BBX ") {
				let parts: Vec<&str> = b.split_whitespace().collect();
				if parts.len() != 4 {
					return Err(BdfParseError::InvalidField("BBX", b.to_string()));
				}
				bbx = Some((
					parts[0]
						.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("BBX", parts[0].to_string()))?,
					parts[1]
						.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("BBX", parts[1].to_string()))?,
					parts[2]
						.parse::<i32>()
						.map_err(|_| BdfParseError::InvalidField("BBX", parts[2].to_string()))?,
					parts[3]
						.parse::<i32>()
						.map_err(|_| BdfParseError::InvalidField("BBX", parts[3].to_string()))?,
				));
			} else if line == "BITMAP" {
				scan_lines = Some(vec![]);
			} else if let Some(lines) = &mut scan_lines {
				lines.push(
					u32::from_str_radix(line, 16)
						.map_err(|_| BdfParseError::InvalidField("BITMAP", line.to_string()))?,
				);
			}
		}

		Ok(Self {
			ch: std::char::from_u32(encoding.ok_or(BdfParseError::MissingField("ENCODING"))? as u32)
				.ok_or(BdfParseError::InvalidField("ENCODING", encoding.unwrap().to_string()))?,
			s_width: s_width.ok_or(BdfParseError::MissingField("SWIDTH"))?,
			d_width: d_width.ok_or(BdfParseError::MissingField("DWIDTH"))?,
			bbx: bbx.ok_or(BdfParseError::MissingField("BBX"))?,
			bitmap: scan_lines.ok_or(BdfParseError::MissingField("BITMAP"))?,
		})
	}
}

impl BdfFont {
	pub fn load_from_file<T: AsRef<Path>>(path: T) -> Result<Self, BdfParseError> {
		let data = std::fs::read(path).map_err(BdfParseError::IOError)?;
		Self::from_bdf_data(&data)
	}

	pub fn from_bdf_data(data: &[u8]) -> Result<Self, BdfParseError> {
		let mut name = None;
		let mut bounding_box = None;
		let mut family_name = None;
		let mut weight = None;
		let mut version = None;
		let mut foundry = None;
		let mut slant = None;
		let mut spacing = None;
		let mut pixel = None;
		let mut point = None;
		let mut resolution_x = None;
		let mut resolution_y = None;
		let mut average_width = None;
		let mut font_descent = None;
		let mut font_ascent = None;
		let mut current_glyph_lines = None;
		let mut glyphs = HashMap::new();

		for line in std::str::from_utf8(data)
			.map_err(|e| BdfParseError::InvalidField("UTF-8", e.to_string()))?
			.lines()
			.map(|line| line.trim())
			.filter(|line| !line.is_empty() && !line.starts_with("COMMENT"))
		{
			if let Some(lines) = &mut current_glyph_lines {
				if line == "ENDCHAR" {
					let glyph = BdfGlyph::from_lines(current_glyph_lines.take().unwrap())?;
					glyphs.insert(glyph.ch, glyph);
					current_glyph_lines = None;
				} else {
					lines.push(line);
				}
				continue;
			}

			if let Some(font_name) = line.strip_prefix("FONT ") {
				name = Some(font_name.to_string());
			} else if let Some(bb) = line.strip_prefix("FONTBOUNDINGBOX ") {
				let parts: Vec<&str> = bb.split_whitespace().collect();
				if parts.len() != 4 {
					return Err(BdfParseError::InvalidField("FONTBOUNDINGBOX", bb.to_string()));
				}
				let width = parts[0]
					.parse::<u32>()
					.map_err(|_| BdfParseError::InvalidField("FONTBOUNDINGBOX", parts[0].to_string()))?;
				let height = parts[1]
					.parse::<u32>()
					.map_err(|_| BdfParseError::InvalidField("FONTBOUNDINGBOX", parts[1].to_string()))?;
				let x_offset = parts[2]
					.parse::<i32>()
					.map_err(|_| BdfParseError::InvalidField("FONTBOUNDINGBOX", parts[2].to_string()))?;
				let y_offset = parts[3]
					.parse::<i32>()
					.map_err(|_| BdfParseError::InvalidField("FONTBOUNDINGBOX", parts[3].to_string()))?;
				bounding_box = Some((width, height, x_offset, y_offset));
			} else if let Some(family) = line.strip_prefix("FAMILY_NAME ") {
				family_name = Some(strip_surrounding_quotes(family).to_string());
			} else if let Some(w) = line.strip_prefix("WEIGHT ") {
				weight = Some(strip_surrounding_quotes(w).to_string());
			} else if let Some(v) = line.strip_prefix("FONT_VERSION ") {
				version = Some(strip_surrounding_quotes(v).to_string());
			} else if let Some(f) = line.strip_prefix("FOUNDRY ") {
				foundry = Some(strip_surrounding_quotes(f).to_string());
			} else if let Some(s) = line.strip_prefix("SLANT ") {
				slant = Some(match strip_surrounding_quotes(s) {
					"R" => Slant::Roman,
					"I" => Slant::Italic,
					"O" => Slant::Oblique,
					"RI" => Slant::ReversedItalic,
					"RO" => Slant::ReverseOblique,
					_ => Slant::Undecided,
				});
			} else if let Some(s) = line.strip_prefix("SPACING ") {
				spacing = Some(match strip_surrounding_quotes(s) {
					"C" => Spacing::Cell,
					"P" => Spacing::Proportional,
					"M" => Spacing::Monospaced,
					_ => return Err(BdfParseError::InvalidField("SPACING", s.to_string())),
				});
			} else if let Some(p) = line.strip_prefix("PIXEL_SIZE ") {
				pixel = Some(
					p.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("PIXEL_SIZE", p.to_string()))?,
				);
			} else if let Some(p) = line.strip_prefix("POINT_SIZE ") {
				point = Some(
					p.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("POINT_SIZE", p.to_string()))?,
				);
			} else if let Some(r) = line.strip_prefix("RESOLUTION_X ") {
				resolution_x = Some(
					r.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("RESOLUTION_X", r.to_string()))?,
				);
			} else if let Some(r) = line.strip_prefix("RESOLUTION_Y ") {
				resolution_y = Some(
					r.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("RESOLUTION_Y", r.to_string()))?,
				);
			} else if let Some(a) = line.strip_prefix("AVERAGE_WIDTH ") {
				average_width = Some(
					a.parse::<u32>()
						.map_err(|_| BdfParseError::InvalidField("AVERAGE_WIDTH", a.to_string()))?,
				);
			} else if let Some(d) = line.strip_prefix("FONT_DESCENT ") {
				font_descent = Some(
					d.parse::<i32>()
						.map_err(|_| BdfParseError::InvalidField("FONT_DESCENT", d.to_string()))?,
				);
			} else if let Some(a) = line.strip_prefix("FONT_ASCENT ") {
				font_ascent = Some(
					a.parse::<i32>()
						.map_err(|_| BdfParseError::InvalidField("FONT_ASCENT", a.to_string()))?,
				);
			} else if line.starts_with("STARTCHAR") {
				current_glyph_lines = Some(vec![]);
			}
		}

		Ok(Self {
			glyphs,
			bounding_box: bounding_box.ok_or(BdfParseError::MissingField("FONTBOUNDINGBOX"))?,
			name: name.ok_or(BdfParseError::MissingField("FONT"))?,
			family_name,
			weight,
			version,
			foundry,
			slant,
			spacing,
			pixel,
			point,
			resolution_x,
			resolution_y,
			average_width,
			font_descent,
			font_ascent,
		})
	}
}

impl Font for BdfFont {
	fn glyph_size(&self, ch: char) -> (i32, i32) {
		self.glyphs
			.get(&ch)
			.map(|g| (g.bbx.0 as i32, g.bbx.1 as i32))
			.unwrap_or((0, 0))
	}

	fn draw_glyph(&self, canvas: &mut crate::canvas::Canvas, x: i32, y: i32, ch: char, color: u32) {
		if let Some(glyph) = self.glyphs.get(&ch) {
			for (row_idx, row) in glyph.bitmap.iter().enumerate() {
				for col_idx in 0..glyph.bbx.0 {
					if row & (1 << (glyph.bbx.0 - 1 - col_idx)) != 0 {
						canvas.set_pixel(
							x + glyph.bbx.2 + col_idx as i32,
							y - glyph.bbx.3 + row_idx as i32,
							color,
						);
					}
				}
			}
		}
	}

	fn advance(&self, ch: char) -> i32 {
		self.glyphs.get(&ch).map(|g| g.d_width.0 as i32).unwrap_or(0)
	}

	fn line_height(&self) -> i32 {
		self.font_ascent.unwrap_or(0) + self.font_descent.unwrap_or(0)
	}
}

pub enum Spacing {
	Cell,
	Proportional,
	Monospaced,
}

pub enum Slant {
	Roman,
	Italic,
	Oblique,
	ReversedItalic,
	ReverseOblique,
	Undecided,
}

#[derive(Debug, Error)]
pub enum BdfParseError {
	#[error("IOError Reading file: {0}")]
	IOError(std::io::Error),

	#[error("Missing required field: {0}")]
	MissingField(&'static str),

	#[error("Invalid value for field {0}: {1}")]
	InvalidField(&'static str, String),
}

fn strip_surrounding_quotes(s: &str) -> &str {
	s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s)
}
