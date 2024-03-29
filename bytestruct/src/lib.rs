#![feature(array_try_from_fn)]
use std::{
	array,
	io::{self, Read},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NullTerminatedString<const SIZE: usize>(pub String);

pub type UUID = [u8; 16];

#[derive(Clone, Copy, Debug)]
pub enum Endian {
	Little,
	Big,
}

pub trait ReadFromWithEndian {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self>
	where
		Self: Sized;
}

pub trait ReadFrom {
	fn read_from<T: Read>(source: &mut T) -> io::Result<Self>
	where
		Self: Sized;
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

impl<const SIZE: usize> ReadFromWithEndian for NullTerminatedString<SIZE> {
	fn read_from_with_endian<T: Read>(source: &mut T, _: Endian) -> io::Result<Self> {
		let mut buf = [0u8; SIZE];
		source.read_exact(&mut buf)?;
		let mut len = 0;
		for c in buf.iter().take(SIZE) {
			if *c == 0 {
				break;
			}
			len += 1;
		}

		if len == SIZE {
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

#[cfg(feature = "time")]
impl ReadFromWithEndian for chrono::DateTime<chrono::Utc> {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let time = i64::read_from_with_endian(source, endian)?;
		Ok(chrono::DateTime::from_timestamp_nanos(time))
	}
}
