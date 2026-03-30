use std::io::{self, Cursor, Read};

use bytestruct::ReadFromWithEndian;
use bytestruct_derive::ByteStruct;

// A simple BMP image loader that supports 24-bit and 32-bit BMP files. It reads the pixel data into a Vec<u32> in ARGB format.
pub struct BMPImage {
	pub width: u32,
	pub height: u32,
	pub pixels: Vec<u32>,
}

// Header of a bitmap file, including the DIB header (BITMAPINFOHEADER).
#[derive(Debug, Clone, ByteStruct)]
struct BitmapHeader {
	pub file_type: [u8; 2],
	pub file_size: u32,
	pub reserved1: u16,
	pub reserved2: u16,
	pub offset: u32,
	pub dib_header_size: u32,
	pub width: u32,
	pub height: u32,
	pub planes: u16,
	pub bits_per_pixel: u16,
	pub compression: u32,
	pub image_size: u32,
	pub x_pixels_per_meter: u32,
	pub y_pixels_per_meter: u32,
	pub total_colors: u32,
	pub important_colors: u32,
}

impl BMPImage {
	pub fn from_file(path: &str) -> io::Result<Self> {
		let mut file = std::fs::File::open(path)?;
		let mut contents = Vec::new();
		file.read_to_end(&mut contents)?;

		let mut cursor = Cursor::new(contents);
		let header = BitmapHeader::read_from_with_endian(&mut cursor, bytestruct::Endian::Little)?;
		if header.file_type != [b'B', b'M'] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a BMP file"));
		}

		if header.bits_per_pixel != 24 && header.bits_per_pixel != 32 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"Only 24-bit and 32-bit BMP files are supported",
			));
		}

		let pixel_data_start = header.offset;
		cursor.set_position(pixel_data_start as u64);
		let mut pixels = Vec::with_capacity((header.width * header.height) as usize);
		for _ in 0..(header.width * header.height) {
			let mut pixel_bytes = [0u8; 4];
			cursor.read_exact(&mut pixel_bytes[0..(header.bits_per_pixel as usize / 8)])?;
			let pixel_value = if header.bits_per_pixel == 24 {
				// BMP stores pixels in BGR format, convert to ARGB
				(0xFF << 24) | // Alpha
            ((pixel_bytes[2] as u32) << 16) | // Red
            ((pixel_bytes[1] as u32) << 8) |  // Green
            (pixel_bytes[0] as u32) // Blue
			} else {
				// BMP 32-bit is BGRA in file byte order; from_le_bytes gives (A<<24)|(R<<16)|(G<<8)|B = ARGB8888.
				u32::from_le_bytes(pixel_bytes)
			};
			pixels.push(pixel_value);
		}

		// BMP rows are stored bottom-to-top; reverse to get top-to-bottom.
		let pixels: Vec<u32> = pixels
			.chunks(header.width as usize)
			.rev()
			.flat_map(|row| row.iter().copied())
			.collect();

		// If no pixel has a non-zero alpha, the file is BGRX (alpha unused); force opaque.
		let has_real_alpha = pixels.iter().any(|&p| p >> 24 != 0);
		let pixels = if has_real_alpha {
			pixels
		} else {
			pixels.into_iter().map(|p| p | 0xFF000000).collect()
		};

		Ok(Self {
			width: header.width,
			height: header.height,
			pixels,
		})
	}
}
