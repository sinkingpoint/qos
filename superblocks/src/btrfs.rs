use bytestruct::{NullTerminatedString, UUID};
use bytestruct_derive::ByteStruct;

use crate::Superblock;

const BTRFS_MAGIC: [u8; 8] = *b"_BHRfS_M";

#[derive(ByteStruct)]
#[little_endian]
pub struct BtrfsSuperBlock {
	pub checksum: [u8; 32],
	pub uuid: UUID,
	pub physical_address: u64,
	pub flags: u64,
	pub magic: [u8; 8],
	pub generation: u64,
	pub root_tree_logical: u64,
	pub chunk_tree_logical: u64,
	pub log_tree_logical: u64,
	pub log_root_transid: u64,
	pub total_bytes: u64,
	pub bytes_used: u64,
	pub root_dir_objectid: u64,
	pub num_devices: u64,
	pub sectorsize: u32,
	pub nodesize: u32,
	pub leafsize: u32,
	pub stripesize: u32,
	pub sys_chunk_array_size: u32,
	pub compat_flags: u64,
	pub compat_ro_flags: u64,
	pub incompat_flags: u64,
	pub csum_type: u16,
	pub root_level: u8,
	pub chunk_root_level: u8,
	pub log_root_level: u8,
	pub dev_items: [u16; 50],
	pub label: NullTerminatedString<256>,
	pub cache_generation: u64,
	pub uuid_tree_generation: u64,
}

impl Superblock for BtrfsSuperBlock {
	fn offset() -> u64 {
		0x10000
	}

	fn size() -> usize {
		0x1000
	}

	fn validate(&self) -> bool {
		self.magic == BTRFS_MAGIC
	}

	fn name(&self) -> String {
		"btrfs".to_string()
	}

	fn label(&self) -> String {
		self.label.0.clone()
	}

	fn uuid(&self) -> UUID {
		self.uuid
	}
}
