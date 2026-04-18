use std::{io, os::fd::AsRawFd};

use bytestruct::{Endian, WriteToWithEndian};
use nix::sys::mman::{MapFlags, ProtFlags, mmap};
use wayland::{shm::CreatePoolRequest, shm_pool::CreateBufferRequest, types::WaylandPayload};

use crate::context::WaylandContext;

pub struct Buffer<'a> {
	pub(crate) id: u32,
	pub(crate) pixels: &'a mut [u32],
	pool_size: usize,
}

impl<'a> Buffer<'a> {
	pub fn new(context: &mut WaylandContext, width: i32, height: i32) -> io::Result<Buffer<'a>> {
		let shm_id: u32 = context
			.globals
			.shm
			.ok_or_else(|| io::Error::other("no wl_shm advertised"))?;
		let stride: i32 = width * 4;
		let pool_size: usize = (stride * height) as usize;

		let memfd = nix::sys::memfd::memfd_create(c"qui-shm", nix::sys::memfd::MemFdCreateFlag::empty())
			.map_err(io::Error::other)?;
		nix::unistd::ftruncate(&memfd, pool_size as i64).map_err(io::Error::other)?;
		let ptr = unsafe {
			mmap(
				None,
				std::num::NonZeroUsize::new(pool_size).unwrap(),
				ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
				MapFlags::MAP_SHARED,
				Some(&memfd),
				0,
			)
			.map_err(io::Error::other)?
		};

		let pool_id = context.next_object_id.next();
		let mut pool_payload = Vec::new();
		CreatePoolRequest {
			pool_id,
			size: pool_size as u32,
		}
		.write_to_with_endian(&mut pool_payload, Endian::Little)?;
		context
			.conn
			.send_with_fd(shm_id, CreatePoolRequest::OPCODE, &pool_payload, memfd.as_raw_fd())?;

		let buffer_id = context.next_object_id.next();
		CreateBufferRequest {
			buffer_id,
			offset: 0,
			width,
			height,
			stride,
			format: 1,
		}
		.write_as_packet(pool_id, &context.conn.stream)?;

		Ok(Self {
			id: buffer_id,
			pixels: unsafe { std::slice::from_raw_parts_mut(ptr as *mut u32, (width * height) as usize) },
			pool_size,
		})
	}
}

impl Drop for Buffer<'_> {
	fn drop(&mut self) {
		unsafe {
			nix::sys::mman::munmap(self.pixels.as_mut_ptr() as *mut _, self.pool_size).ok();
		}
	}
}
