use std::{num::NonZeroUsize, os::unix::net::UnixStream, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::{
	VideoBuffer,
	wayland::{
		buffer::Buffer,
		types::{ClientEffect, Command, SubSystem, WaylandResult, WithFd},
	},
};

pub struct SharedMemory;

impl SubSystem for SharedMemory {
	type Request = SharedMemoryRequest;
	const NAME: &'static str = "wl_shm";
	const VERSION: u32 = 1;
}

wayland_interface!(SharedMemory, SharedMemoryRequest {
  0 => CreatePool(WithFd<CreatePoolCommand>),
});

#[derive(Debug, ByteStruct)]
pub struct CreatePoolCommand {
	pub pool_id: u32,
	pub size: u32,
}

impl Command<SharedMemory> for WithFd<CreatePoolCommand> {
	fn handle(&self, _connection: &Arc<UnixStream>, _shm: &mut SharedMemory) -> WaylandResult<Option<ClientEffect>> {
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
			crate::wayland::types::SubsystemType::SharedMemoryPool(pool),
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

	pub fn blit_onto(&self, buffer: &Buffer, framebuffer: &mut VideoBuffer) {
		let blit_width = buffer.width.min(framebuffer.width as i32);
		let blit_height = buffer.height.min(framebuffer.height as i32);
		for y in 0..blit_height {
			for x in 0..blit_width {
				let src_offset = (y * buffer.stride + x * 4) as usize;
				let dst_offset = (y * framebuffer.pitch as i32 + x) as usize;
				unsafe {
					let pixel = std::ptr::read_unaligned(self.ptr.add(src_offset) as *const u32);
					std::ptr::write_unaligned(framebuffer.pixels.add(dst_offset), pixel);
				}
			}
		}
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
  0 => CreatePool(CreateBufferCommand),
});

#[derive(Debug, ByteStruct)]
pub struct CreateBufferCommand {
	pub buffer_id: u32,
	pub offset: i32,
	pub width: i32,
	pub height: i32,
	pub stride: i32,
	pub format: u32,
}

impl Command<SharedMemoryPool> for CreateBufferCommand {
	fn handle(
		&self,
		_connection: &Arc<UnixStream>,
		pool: &mut SharedMemoryPool,
	) -> WaylandResult<Option<ClientEffect>> {
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
			crate::wayland::types::SubsystemType::Buffer(buffer),
		)))
	}
}
