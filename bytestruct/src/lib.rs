#![feature(array_try_from_fn)]
use std::{array, io::{self, Read}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U8(pub u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LittleEndianU16(pub u16);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigEndianU16(pub u16);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LittleEndianU32(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigEndianU32(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LittleEndianU64(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigEndianU64(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LittleEndianI16(pub i16);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigEndianI16(pub i16);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LittleEndianI32(pub i32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigEndianI32(pub i32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LittleEndianI64(pub i64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigEndianI64(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Array<T, const SIZE: usize>(pub [T; SIZE]);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NullTerminatedString<const SIZE: usize>(pub String);

pub type ByteArray<const T: usize> = Array<U8, T>;
pub type UUID = ByteArray<16>;

pub trait ReadFrom {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> where Self: Sized;
}

impl ReadFrom for U8 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 1];
        source.read_exact(&mut buf)?;
        Ok(U8(buf[0]))
    }
}

impl ReadFrom for LittleEndianU16 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 2];
        source.read_exact(&mut buf)?;
        Ok(LittleEndianU16(u16::from_le_bytes(buf)))
    }
}

impl ReadFrom for BigEndianU16 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 2];
        source.read_exact(&mut buf)?;
        Ok(BigEndianU16(u16::from_be_bytes(buf)))
    }
}

impl ReadFrom for LittleEndianU32 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
        Ok(LittleEndianU32(u32::from_le_bytes(buf)))
    }
}

impl ReadFrom for BigEndianU32 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
        Ok(BigEndianU32(u32::from_be_bytes(buf)))
    }
}

impl ReadFrom for LittleEndianU64 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf)?;
        Ok(LittleEndianU64(u64::from_le_bytes(buf)))
    }
}

impl ReadFrom for BigEndianU64 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf)?;
        Ok(BigEndianU64(u64::from_be_bytes(buf)))
    }
}

impl ReadFrom for LittleEndianI16 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 2];
        source.read_exact(&mut buf)?;
        Ok(LittleEndianI16(i16::from_le_bytes(buf)))
    }
}

impl ReadFrom for BigEndianI16 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 2];
        source.read_exact(&mut buf)?;
        Ok(BigEndianI16(i16::from_be_bytes(buf)))
    }
}

impl ReadFrom for LittleEndianI32 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
        Ok(LittleEndianI32(i32::from_le_bytes(buf)))
    }
}

impl ReadFrom for BigEndianI32 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf)?;
        Ok(BigEndianI32(i32::from_be_bytes(buf)))
    }
}

impl ReadFrom for LittleEndianI64 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf)?;
        Ok(LittleEndianI64(i64::from_le_bytes(buf)))
    }
}

impl ReadFrom for BigEndianI64 {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf)?;
        Ok(BigEndianI64(i64::from_be_bytes(buf)))
    }
}

impl <const SIZE: usize> ReadFrom for NullTerminatedString<SIZE> {
    fn read_from<T: Read>(source: &mut T) -> io::Result<Self> {
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
            return Err(io::Error::new(io::ErrorKind::InvalidData, "String is not null terminated"));
        }

        Ok(NullTerminatedString(String::from_utf8_lossy(&buf[..len]).to_string()))
    }
}

impl <const SIZE: usize, T: ReadFrom> ReadFrom for Array<T, SIZE> {
    fn read_from<R: Read>(source: &mut R) -> io::Result<Self> {
        Ok(Array(array::try_from_fn(|_| T::read_from(source))?))
    }
}
