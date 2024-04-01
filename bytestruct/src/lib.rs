#![feature(array_try_from_fn)]
use std::{
	array,
	io::{self, Read},
};

/// A string that is null-terminated (C-style), with some maximum size.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NullTerminatedString<const SIZE: usize>(pub String);

/// A UUID (Universally Unique Identifier).
pub type UUID = [u8; 16];

/// The endianness of the data.
#[derive(Clone, Copy, Debug)]
pub enum Endian {
	Little,
	Big,
}

/// A trait for reading data from a source with a specified endianness.
pub trait ReadFromWithEndian {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self>
	where
		Self: Sized;
}

/// A trait for reading data from a source with an implied endianess.
pub trait ReadFrom {
	fn read_from<T: Read>(source: &mut T) -> io::Result<Self>
	where
		Self: Sized;
}

/// A trait for determining the size of the data as would be read from a source.
pub trait Size {
	fn size(&self) -> usize;
}

impl ReadFromWithEndian for u8 {
	fn read_from_with_endian<T: Read>(source: &mut T, _: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 1];
		source.read_exact(&mut buf)?;
		Ok(buf[0])
	}
}

impl ReadFrom for u8 {
	fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
		let mut buf = [0u8; 1];
		source.read_exact(&mut buf)?;
		Ok(buf[0])
	}
}

impl Size for u8 {
	fn size(&self) -> usize {
		1
	}
}

impl ReadFromWithEndian for u16 {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 2];
		source.read_exact(&mut buf)?;
		Ok(match endian {
			Endian::Big => u16::from_be_bytes(buf),
			Endian::Little => u16::from_le_bytes(buf),
		})
	}
}

impl Size for u16 {
	fn size(&self) -> usize {
		2
	}
}

impl ReadFromWithEndian for u32 {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 4];
		source.read_exact(&mut buf)?;
		Ok(match endian {
			Endian::Big => u32::from_be_bytes(buf),
			Endian::Little => u32::from_le_bytes(buf),
		})
	}
}

impl Size for u32 {
	fn size(&self) -> usize {
		4
	}
}

impl ReadFromWithEndian for u64 {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 8];
		source.read_exact(&mut buf)?;
		Ok(match endian {
			Endian::Big => u64::from_be_bytes(buf),
			Endian::Little => u64::from_le_bytes(buf),
		})
	}
}

impl Size for u64 {
	fn size(&self) -> usize {
		8
	}
}

impl ReadFromWithEndian for i16 {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 2];
		source.read_exact(&mut buf)?;
		Ok(match endian {
			Endian::Big => i16::from_be_bytes(buf),
			Endian::Little => i16::from_le_bytes(buf),
		})
	}
}

impl Size for i16 {
	fn size(&self) -> usize {
		2
	}
}

impl ReadFromWithEndian for i32 {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 4];
		source.read_exact(&mut buf)?;
		Ok(match endian {
			Endian::Big => i32::from_be_bytes(buf),
			Endian::Little => i32::from_le_bytes(buf),
		})
	}
}

impl Size for i32 {
	fn size(&self) -> usize {
		4
	}
}

impl ReadFromWithEndian for i64 {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let mut buf = [0u8; 8];
		source.read_exact(&mut buf)?;
		Ok(match endian {
			Endian::Big => i64::from_be_bytes(buf),
			Endian::Little => i64::from_le_bytes(buf),
		})
	}
}

impl Size for i64 {
	fn size(&self) -> usize {
		8
	}
}

impl<const MAX_SIZE: usize> ReadFromWithEndian for NullTerminatedString<MAX_SIZE> {
	fn read_from_with_endian<T: Read>(source: &mut T, _: Endian) -> io::Result<Self> {
		let mut buf = [0u8; MAX_SIZE];
		source.read_exact(&mut buf)?;
		let mut len = 0;
		for c in buf.iter().take(MAX_SIZE) {
			if *c == 0 {
				break;
			}
			len += 1;
		}

		if len == MAX_SIZE {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"String is not null terminated",
			));
		}

		match std::str::from_utf8(&buf[..len]) {
			Ok(s) => Ok(NullTerminatedString(s.to_string())),
			Err(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "String is not valid utf8")),
		}
	}
}

impl<const MAX_SIZE: usize> Size for NullTerminatedString<MAX_SIZE> {
	fn size(&self) -> usize {
		self.0.len() + 1
	}
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LengthPrefixedString<const MAX_SIZE: usize>(pub String);

impl<const MAX_SIZE: usize> ReadFromWithEndian for LengthPrefixedString<MAX_SIZE> {
	fn read_from_with_endian<T: Read>(source: &mut T, _: Endian) -> io::Result<Self> {
		let len = match MAX_SIZE {
			0..=0xFF => u8::read_from_with_endian(source, Endian::Big)? as usize,
			256..=0xFFFF => u16::read_from_with_endian(source, Endian::Big)? as usize,
			65536..=0xFFFFFFFF => u32::read_from_with_endian(source, Endian::Big)? as usize,
			_ => u64::read_from_with_endian(source, Endian::Big)? as usize,
		};

		let mut buf = vec![0u8; len];
		source.read_exact(&mut buf)?;
		match std::str::from_utf8(&buf) {
			Ok(s) => Ok(LengthPrefixedString(s.to_string())),
			Err(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "String is not valid utf8")),
		}
	}
}

impl<const MAX_SIZE: usize> Size for LengthPrefixedString<MAX_SIZE> {
	fn size(&self) -> usize {
		self.0.len()
			+ match MAX_SIZE {
				0..=0xFF => 1,
				256..=0xFFFF => 2,
				65536..=0xFFFFFFFF => 4,
				_ => 8,
			}
	}
}

impl<const SIZE: usize, T: ReadFromWithEndian> ReadFromWithEndian for [T; SIZE] {
	fn read_from_with_endian<R: Read>(source: &mut R, endian: Endian) -> io::Result<Self> {
		array::try_from_fn(|_| T::read_from_with_endian(source, endian))
	}
}

impl<const SIZE: usize, T: ReadFrom> ReadFrom for [T; SIZE] {
	fn read_from<R: Read>(source: &mut R) -> io::Result<Self> {
		array::try_from_fn(|_| T::read_from(source))
	}
}

impl<const SIZE: usize, T: Size> Size for [T; SIZE] {
	fn size(&self) -> usize {
		self.iter().map(Size::size).sum()
	}
}

impl<I: ReadFromWithEndian> ReadFromWithEndian for Vec<I> {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self>
	where
		Self: Sized,
	{
		let count = u64::read_from_with_endian(source, endian)?;
		let mut vec = Vec::with_capacity(count as usize);
		for _ in 0..count {
			vec.push(I::read_from_with_endian(source, endian)?);
		}

		Ok(vec)
	}
}

impl<T: Size> Size for Vec<T> {
	fn size(&self) -> usize {
		self.iter().map(Size::size).sum()
	}
}

#[cfg(feature = "time")]
impl ReadFromWithEndian for chrono::DateTime<chrono::Utc> {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let time = i64::read_from_with_endian(source, endian)?;
		Ok(chrono::DateTime::from_timestamp_nanos(time))
	}
}

impl Size for chrono::DateTime<chrono::Utc> {
	fn size(&self) -> usize {
		8
	}
}
