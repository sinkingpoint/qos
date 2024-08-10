use std::fs::File;
use std::io::{self, Read};

pub fn rand_bytes(buf: &mut [u8]) -> io::Result<()> {
	let mut file = File::open("/dev/urandom")?;
	file.read_exact(buf)
}

pub fn rand_u8() -> io::Result<u8> {
	let mut buf = [0];
	rand_bytes(&mut buf)?;

	Ok(buf[0])
}

pub fn rand_u16() -> io::Result<u16> {
	let mut buf = [0; 2];
	rand_bytes(&mut buf)?;

	Ok((buf[0] as u16) << 8 | buf[1] as u16)
}

pub fn rand_u32() -> io::Result<u32> {
	let mut buf = [0; 4];
	rand_bytes(&mut buf)?;

	Ok((buf[0] as u32) << 24 | (buf[1] as u32) << 16 | (buf[2] as u32) << 8 | buf[3] as u32)
}
