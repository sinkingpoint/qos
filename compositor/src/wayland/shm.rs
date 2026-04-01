use std::{os::unix::net::UnixStream, process::Command, sync::Arc};

use bytestruct_derive::ByteStruct;

use crate::wayland::types::{ClientEffect, SubSystem, WaylandResult};

pub struct SharedMemory;

impl SubSystem for SharedMemory {
	type Request = SharedMemoryRequest;
}

wayland_interface!(SharedMemory, SharedMemoryRequest {
  0 => CreatePool(CreatePoolCommand),
});

#[derive(Debug, ByteStruct)]
pub struct CreatePoolCommand {
	pub pool_id: u32,
	pub size: u32,
}

impl Command<SharedMemory> for CreatePoolCommand {
	fn handle(&self, _connection: &Arc<UnixStream>, _shm: &mut SharedMemory) -> WaylandResult<Option<ClientEffect>> {
		return Ok(None);
	}
}
