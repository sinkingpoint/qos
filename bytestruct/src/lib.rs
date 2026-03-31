#![feature(array_try_from_fn)]
use std::{
	array,
	io::{self, Read, Write},
	ops::{Deref, DerefMut},
};

mod macros;

pub use macros::TLVValues;

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

/// A trait for writing data to a target with a specified endianness.
pub trait WriteToWithEndian {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()>;
}

/// A trait for writing data to a target with an implied endianness.
pub trait WriteTo {
	fn write_to<T: Write>(&self, target: &mut T) -> io::Result<()>;
}

impl ReadFromWithEndian for u8 {
	fn read_from_with_endian<T: Read>(source: &mut T, _: Endian) -> io::Result<Self> {
		u8::read_from(source)
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

impl WriteTo for u8 {
	fn write_to<T: Write>(&self, target: &mut T) -> io::Result<()> {
		target.write_all(&[*self])
	}
}

impl WriteToWithEndian for u8 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, _endian: Endian) -> io::Result<()> {
		u8::write_to(self, target)
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

impl WriteToWithEndian for u16 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match endian {
			Endian::Big => target.write_all(&self.to_be_bytes()),
			Endian::Little => target.write_all(&self.to_le_bytes()),
		}
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

impl WriteToWithEndian for u32 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match endian {
			Endian::Big => target.write_all(&self.to_be_bytes()),
			Endian::Little => target.write_all(&self.to_le_bytes()),
		}
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

impl WriteToWithEndian for u64 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match endian {
			Endian::Big => target.write_all(&self.to_be_bytes()),
			Endian::Little => target.write_all(&self.to_le_bytes()),
		}
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

impl WriteToWithEndian for i16 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match endian {
			Endian::Big => target.write_all(&self.to_be_bytes()),
			Endian::Little => target.write_all(&self.to_le_bytes()),
		}
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

impl WriteToWithEndian for i32 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match endian {
			Endian::Big => target.write_all(&self.to_be_bytes()),
			Endian::Little => target.write_all(&self.to_le_bytes()),
		}
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

impl WriteToWithEndian for i64 {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		match endian {
			Endian::Big => target.write_all(&self.to_be_bytes()),
			Endian::Little => target.write_all(&self.to_le_bytes()),
		}
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

impl<const MAX_SIZE: usize> WriteToWithEndian for NullTerminatedString<MAX_SIZE> {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, _: Endian) -> io::Result<()> {
		target.write_all(self.0.as_bytes())?;
		target.write_all(&[0])?;
		Ok(())
	}
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LengthPrefixedString<L: UnsignedLength>(pub String, std::marker::PhantomData<L>);

impl<T: UnsignedLength> LengthPrefixedString<T> {
	pub fn new(s: String) -> Self {
		Self(s, std::marker::PhantomData)
	}
}

impl<L: UnsignedLength> Deref for LengthPrefixedString<L> {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<L: UnsignedLength> ReadFromWithEndian for LengthPrefixedString<L> {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let len = L::read_from_with_endian(source, endian)?.to_usize();

		let mut buf = vec![0u8; len];
		source.read_exact(&mut buf)?;
		match std::str::from_utf8(&buf) {
			Ok(s) => Ok(LengthPrefixedString::<L>(s.to_string(), std::marker::PhantomData)),
			Err(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "String is not valid utf8")),
		}
	}
}

impl<L: UnsignedLength> Size for LengthPrefixedString<L> {
	fn size(&self) -> usize {
		self.0.len() + L::size()
	}
}

impl<L: UnsignedLength> WriteToWithEndian for LengthPrefixedString<L> {
	fn write_to_with_endian<T: Write>(&self, target: &mut T, endian: Endian) -> io::Result<()> {
		L::from_usize(self.0.len()).write_to_with_endian(target, endian)?;
		target.write_all(self.0.as_bytes())?;
		Ok(())
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

impl<const SIZE: usize, T: WriteTo> WriteTo for [T; SIZE] {
	fn write_to<W: Write>(&self, target: &mut W) -> io::Result<()> {
		for item in self.iter() {
			item.write_to(target)?;
		}
		Ok(())
	}
}

impl<const SIZE: usize, T: WriteToWithEndian> WriteToWithEndian for [T; SIZE] {
	fn write_to_with_endian<W: Write>(&self, target: &mut W, endian: Endian) -> io::Result<()> {
		for item in self.iter() {
			item.write_to_with_endian(target, endian)?;
		}
		Ok(())
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

impl<T: WriteToWithEndian> WriteToWithEndian for Vec<T> {
	fn write_to_with_endian<W: Write>(&self, target: &mut W, endian: Endian) -> io::Result<()> {
		(self.len() as u64).write_to_with_endian(target, endian)?;
		for item in self.iter() {
			item.write_to_with_endian(target, endian)?;
		}
		Ok(())
	}
}

#[cfg(feature = "time")]
impl ReadFromWithEndian for chrono::DateTime<chrono::Utc> {
	fn read_from_with_endian<T: Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let time = i64::read_from_with_endian(source, endian)?;
		Ok(chrono::DateTime::from_timestamp_nanos(time))
	}
}

#[cfg(feature = "time")]
impl Size for chrono::DateTime<chrono::Utc> {
	fn size(&self) -> usize {
		8
	}
}

#[cfg(feature = "time")]
impl WriteToWithEndian for chrono::DateTime<chrono::Utc> {
	fn write_to_with_endian<W: Write>(&self, target: &mut W, endian: Endian) -> io::Result<()> {
		let nanos = match self.timestamp_nanos_opt() {
			Some(nanos) => nanos,
			None => return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid timestamp")),
		};

		nanos.write_to_with_endian(target, endian)
	}
}

/// Padding is a special type that pads a struct to a given alignment. Notably, you can put
/// it in the middle of a struct, and it will pad only the fields that came before it.
#[derive(Debug, Clone)]
pub struct Padding<const ALIGN: usize> {
	amt: usize,
}

impl<const ALIGN: usize> Padding<ALIGN> {
	pub fn new(prev_size: usize) -> Self {
		let amt = ALIGN - (prev_size % ALIGN);
		Padding { amt }
	}

	pub fn read<R: Read>(prev_size: usize, r: &mut R) -> io::Result<Self> {
		let amt = ALIGN - (prev_size % ALIGN);
		let mut buf = vec![0u8; amt];
		r.read_exact(&mut buf)?;

		Ok(Padding { amt })
	}
}

impl<const ALIGN: usize> Size for Padding<ALIGN> {
	fn size(&self) -> usize {
		self.amt
	}
}

impl<const ALIGN: usize> WriteTo for Padding<ALIGN> {
	fn write_to<W: Write>(&self, target: &mut W) -> io::Result<()> {
		target.write_all(&vec![0u8; self.amt])
	}
}

impl<const ALIGN: usize> WriteToWithEndian for Padding<ALIGN> {
	fn write_to_with_endian<W: Write>(&self, target: &mut W, _: Endian) -> io::Result<()> {
		self.write_to(target)
	}
}

/// The standard implementation of ReadFromWithEndian has the first entry as
/// the number of elements, but for TLV Values, it's the raw length in bytes.
/// This wrapper struct reads a length-prefixed list of T values.
#[derive(Debug, Clone)]
pub struct TLVVec<T>(Vec<T>);
impl<T: ReadFromWithEndian> ReadFromWithEndian for TLVVec<T> {
	fn read_from_with_endian<U: std::io::Read>(source: &mut U, endian: Endian) -> std::io::Result<Self> {
		let mut values = Vec::new();
		loop {
			let value = match T::read_from_with_endian(source, endian) {
				Ok(v) => v,
				Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
				Err(e) => return Err(e),
			};

			values.push(value);
		}

		Ok(Self(values))
	}
}

impl<T: WriteToWithEndian> WriteToWithEndian for TLVVec<T> {
	fn write_to_with_endian<U: std::io::Write>(&self, writer: &mut U, endian: Endian) -> std::io::Result<()> {
		for value in &self.0 {
			value.write_to_with_endian(writer, endian)?;
		}

		Ok(())
	}
}

impl<T> Deref for TLVVec<T> {
	type Target = Vec<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

// This trait is for types that can be used as the length field in a length-prefixed structure. It provides a method to convert the value to usize, which is needed for reading the correct number of bytes.
pub trait UnsignedLength: ReadFromWithEndian + WriteToWithEndian + std::fmt::Debug {
	fn to_usize(self) -> usize;
	fn from_usize(val: usize) -> Self;
	fn size() -> usize;
}

impl UnsignedLength for u8 {
	fn to_usize(self) -> usize {
		self as usize
	}
	fn from_usize(val: usize) -> Self {
		val as u8
	}
	fn size() -> usize {
		1
	}
}
impl UnsignedLength for u16 {
	fn to_usize(self) -> usize {
		self as usize
	}
	fn from_usize(val: usize) -> Self {
		val as u16
	}
	fn size() -> usize {
		2
	}
}
impl UnsignedLength for u32 {
	fn to_usize(self) -> usize {
		self as usize
	}
	fn from_usize(val: usize) -> Self {
		val as u32
	}
	fn size() -> usize {
		4
	}
}
impl UnsignedLength for u64 {
	fn to_usize(self) -> usize {
		self as usize
	}
	fn from_usize(val: usize) -> Self {
		val as u64
	}
	fn size() -> usize {
		8
	}
}

// Implement DerefMut for TLVVec if you want mutable operations
impl<T> DerefMut for TLVVec<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Debug)]
pub struct LengthPrefixedVec<T, L: UnsignedLength>(Vec<T>, std::marker::PhantomData<L>);
impl<T, L: UnsignedLength> LengthPrefixedVec<T, L> {
	pub fn new(vec: Vec<T>) -> Self {
		Self(vec, std::marker::PhantomData)
	}
}

impl<T, L: UnsignedLength> Clone for LengthPrefixedVec<T, L>
where
	T: Clone,
	L: UnsignedLength,
{
	fn clone(&self) -> Self {
		Self(self.0.clone(), std::marker::PhantomData)
	}
}

impl<T: ReadFromWithEndian, L: UnsignedLength> ReadFromWithEndian for LengthPrefixedVec<T, L> {
	fn read_from_with_endian<U: std::io::Read>(source: &mut U, endian: Endian) -> std::io::Result<Self> {
		let length = L::read_from_with_endian(source, endian)?;
		let mut bytes = vec![0u8; length.to_usize()];
		source.read_exact(&mut bytes)?;
		let mut cursor = std::io::Cursor::new(bytes);
		let mut values = Vec::new();
		while (cursor.position() as usize) < cursor.get_ref().len() {
			values.push(T::read_from_with_endian(&mut cursor, endian)?);
		}

		Ok(Self(values, std::marker::PhantomData))
	}
}

impl<T: WriteToWithEndian, L: UnsignedLength> WriteToWithEndian for LengthPrefixedVec<T, L> {
	fn write_to_with_endian<U: std::io::Write>(&self, writer: &mut U, endian: Endian) -> std::io::Result<()> {
		let mut buf = Vec::new();
		for value in &self.0 {
			value.write_to_with_endian(&mut buf, endian)?;
		}

		let length = L::from_usize(buf.len());
		length.write_to_with_endian(writer, endian)?;
		writer.write_all(&buf)?;

		Ok(())
	}
}

impl<T, L: UnsignedLength> Deref for LengthPrefixedVec<T, L> {
	type Target = Vec<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T, L: UnsignedLength> DerefMut for LengthPrefixedVec<T, L> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
