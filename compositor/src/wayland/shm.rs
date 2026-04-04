use std::{num::NonZeroUsize, os::unix::net::UnixStream, sync::Arc};

use wayland::shm::CreatePoolRequest;
use wayland::shm_pool::CreateBufferRequest;

use crate::{
	VideoBuffer,
	wayland::{
		buffer::Buffer,
		types::{ClientEffect, Command, SubSystem, SubsystemType, WaylandResult, WithFd},
	},
};

pub struct SharedMemory;

impl SubSystem for SharedMemory {
	type Request = SharedMemoryRequest;
	const NAME: &'static str = "wl_shm";
	const VERSION: u32 = 1;
}

wayland_interface!(SharedMemory, SharedMemoryRequest {
  CreatePoolRequest::OPCODE => CreatePool(WithFd<CreatePoolRequest>),
});

impl Command<SharedMemory> for WithFd<CreatePoolRequest> {
	fn handle(self, _connection: &Arc<UnixStream>, _shm: &mut SharedMemory) -> WaylandResult<Option<ClientEffect>> {
		let ptr = unsafe {
			nix::sys::mman::mmap(
				None,
				NonZeroUsize::new(self.cmd.size as usize).expect("client requested shared memory pool of size 0"),
				nix::sys::mman::ProtFlags::PROT_READ | nix::sys::mman::ProtFlags::PROT_WRITE,
				nix::sys::mman::MapFlags::MAP_SHARED,
				Some(&self.fd),
				0,
			)
		}?;

		let pool = SharedMemoryPool::new(self.cmd.pool_id, self.cmd.size, ptr as *mut u8);
		Ok(Some(ClientEffect::Register(
			self.cmd.pool_id,
			SubsystemType::SharedMemoryPool(pool),
		)))
	}
}

pub struct SharedMemoryPool {
	pub pool_id: u32,
	pub size: u32,
	pub ptr: *mut u8,
}

impl SubSystem for SharedMemoryPool {
	type Request = SharedMemoryPoolRequest;
	const NAME: &'static str = "wl_shm_pool";
}

impl SharedMemoryPool {
	pub fn new(pool_id: u32, size: u32, ptr: *mut u8) -> Self {
		Self { pool_id, size, ptr }
	}

	pub fn blit_onto(&self, buffer: &Buffer, framebuffer: &mut VideoBuffer, x: i32, y: i32) {
		if buffer.offset < 0 {
			eprintln!("blit_onto: negative buffer offset {}", buffer.offset);
			return;
		}
		let end = (buffer.offset as u64).saturating_add((buffer.height as u64).saturating_mul(buffer.stride as u64));
		if end > self.size as u64 {
			eprintln!(
				"blit_onto: buffer region ({} bytes) exceeds pool size ({})",
				end, self.size
			);
			return;
		}

		// Clip to framebuffer bounds
		let src_stride_pixels = buffer.stride / 4;
		let clip_x = x.max(0);
		let clip_y = y.max(0);
		let clip_x2 = (x + buffer.width).min(framebuffer.width as i32);
		let clip_y2 = (y + buffer.height).min(framebuffer.height as i32);
		if clip_x2 <= clip_x || clip_y2 <= clip_y {
			return;
		}

		// Offset into source to skip the clipped rows/columns
		let src_skip_x = (clip_x - x) as usize;
		let src_skip_y = (clip_y - y) as usize;
		let src = unsafe {
			(self.ptr.add(buffer.offset as usize) as *const u32)
				// skip rows
				.add(src_skip_y * src_stride_pixels as usize)
				// skip columns
				.add(src_skip_x)
		};

		framebuffer.blit_and_mark_dirty(
			src,
			src_stride_pixels as u32,
			clip_x as u32,
			clip_y as u32,
			(clip_x2 - clip_x) as u32,
			(clip_y2 - clip_y) as u32,
		);
	}
}

impl Drop for SharedMemoryPool {
	fn drop(&mut self) {
		unsafe {
			nix::sys::mman::munmap(self.ptr as *mut _, self.size as usize).expect("Failed to unmap shared memory pool");
		}
	}
}

wayland_interface!(SharedMemoryPool, SharedMemoryPoolRequest {
  CreateBufferRequest::OPCODE => CreateBuffer(CreateBufferRequest),
});

impl Command<SharedMemoryPool> for CreateBufferRequest {
	fn handle(self, _connection: &Arc<UnixStream>, pool: &mut SharedMemoryPool) -> WaylandResult<Option<ClientEffect>> {
		if self.offset < 0 || self.width <= 0 || self.height <= 0 || self.stride < self.width.saturating_mul(4) {
			eprintln!(
				"create_buffer: invalid dimensions (offset={}, width={}, height={}, stride={})",
				self.offset, self.width, self.height, self.stride
			);
			return Ok(None);
		}
		let end = (self.offset as u64).saturating_add((self.height as u64).saturating_mul(self.stride as u64));
		if end > pool.size as u64 {
			eprintln!(
				"create_buffer: region ({} bytes) exceeds pool size ({})",
				end, pool.size
			);
			return Ok(None);
		}
		let buffer = Buffer::new(
			pool.pool_id,
			self.offset,
			self.width,
			self.height,
			self.stride,
			self.format,
		);
		Ok(Some(ClientEffect::Register(
			self.buffer_id,
			SubsystemType::Buffer(buffer),
		)))
	}
}
