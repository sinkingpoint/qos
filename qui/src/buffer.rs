use std::{io, os::fd::AsRawFd};

use bytestruct::{Endian, WriteToWithEndian};
use nix::sys::mman::{MapFlags, ProtFlags, mmap};
use wayland::{shm::CreatePoolRequest, shm_pool::CreateBufferRequest, types::WaylandPayload};

use crate::{canvas::Canvas, context::WaylandContext};

pub struct Buffer {
	pub(crate) id: u32,
	pixels: *mut u32,
	pixel_count: usize,
	pool_size: usize,
	width: i32,
	height: i32,
	damage: Vec<(i32, i32, i32, i32)>,
	released: bool,
}

impl Buffer {
	pub fn new(context: &mut WaylandContext, width: i32, height: i32) -> io::Result<Buffer> {
		let shm_id: u32 = context
			.globals
			.shm
			.ok_or_else(|| io::Error::other("no wl_shm advertised"))?;
		let stride: i32 = width * 4;
		let pool_size: usize = (stride * height) as usize;
		let pixel_count: usize = (width * height) as usize;

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
			pixels: ptr as *mut u32,
			pixel_count,
			pool_size,
			width,
			height,
			damage: Vec::new(),
			released: true,
		})
	}

	fn clear(&mut self) {
		unsafe {
			std::ptr::write_bytes(self.pixels, 0, self.pixel_count);
		}
		self.damage.push((0, 0, self.width, self.height));
	}

	pub fn canvas(&mut self) -> Canvas<'_> {
		let pixels = unsafe { std::slice::from_raw_parts_mut(self.pixels, self.pixel_count) };
		Canvas::new(pixels, self.width, self.height, self.width, 0, 0, &mut self.damage)
	}

	pub fn drain_damage(&mut self) -> impl Iterator<Item = (i32, i32, i32, i32)> + '_ {
		self.damage.drain(..)
	}

	pub fn commit(&mut self) {
		self.released = false;
	}

	pub fn release(&mut self) {
		self.released = true;
	}
}

impl Drop for Buffer {
	fn drop(&mut self) {
		unsafe {
			nix::sys::mman::munmap(self.pixels as *mut _, self.pool_size).ok();
		}
	}
}

pub struct DoubleBuffer {
	buffers: [Buffer; 2],
	current_index: usize,
}

impl DoubleBuffer {
	pub fn new(context: &mut WaylandContext, width: i32, height: i32) -> io::Result<DoubleBuffer> {
		Ok(Self {
			buffers: [
				Buffer::new(context, width, height)?,
				Buffer::new(context, width, height)?,
			],
			current_index: 0,
		})
	}

	pub fn release(&mut self, buffer_id: u32) {
		for buffer in &mut self.buffers {
			if buffer.id == buffer_id {
				buffer.release();
			}
		}
	}

	pub fn id(&self) -> u32 {
		self.buffers[self.current_index].id
	}

	pub fn all_ids(&self) -> [u32; 2] {
		[self.buffers[0].id, self.buffers[1].id]
	}

	pub fn swap(&mut self) {
		self.buffers[self.current_index].commit();
		self.current_index = (self.current_index + 1) % 2;
		if self.buffers[self.current_index].released {
			self.buffers[self.current_index].clear();
		}
	}

	pub fn current_buffer(&mut self) -> &mut Buffer {
		&mut self.buffers[self.current_index]
	}

	pub fn canvas(&mut self) -> Option<Canvas<'_>> {
		if self.buffers[self.current_index].released {
			return Some(self.buffers[self.current_index].canvas());
		}

		// If the current buffer is blocked, fall back to the other released buffer.
		let other_index = (self.current_index + 1) % 2;
		if self.buffers[other_index].released {
			self.current_index = other_index;
			return Some(self.buffers[self.current_index].canvas());
		}

		None
	}
}
