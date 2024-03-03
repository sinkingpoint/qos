use bytestruct::{Array, ByteArray, LittleEndianU16, LittleEndianU32, LittleEndianU64, NullTerminatedString, U8, UUID};
use bytestruct_derive::ByteStruct;

use crate::Superblock;

const BTRFS_MAGIC: [u8; 8] = *b"_BHRfS_M";

#[derive(ByteStruct)]
pub struct BtrfsSuperBlock {
	pub checksum: ByteArray<32>,
	pub uuid: UUID,
	pub physical_address: LittleEndianU64,
	pub flags: LittleEndianU64,
	pub magic: ByteArray<8>,
	pub generation: LittleEndianU64,
	pub root_tree_logical: LittleEndianU64,
	pub chunk_tree_logical: LittleEndianU64,
	pub log_tree_logical: LittleEndianU64,
	pub log_root_transid: LittleEndianU64,
	pub total_bytes: LittleEndianU64,
	pub bytes_used: LittleEndianU64,
	pub root_dir_objectid: LittleEndianU64,
	pub num_devices: LittleEndianU64,
	pub sectorsize: LittleEndianU32,
	pub nodesize: LittleEndianU32,
	pub leafsize: LittleEndianU32,
	pub stripesize: LittleEndianU32,
	pub sys_chunk_array_size: LittleEndianU32,
	pub compat_flags: LittleEndianU64,
	pub compat_ro_flags: LittleEndianU64,
	pub incompat_flags: LittleEndianU64,
	pub csum_type: LittleEndianU16,
	pub root_level: U8,
	pub chunk_root_level: U8,
	pub log_root_level: U8,
	pub dev_items: Array<LittleEndianU16, 50>,
	pub label: NullTerminatedString<256>,
	pub cache_generation: LittleEndianU64,
	pub uuid_tree_generation: LittleEndianU64,
}

impl Superblock for BtrfsSuperBlock {
	fn offset() -> u64 {
		0x10000
	}

	fn size() -> usize {
		0x1000
	}

	fn validate(&self) -> bool {
		self.magic.0.map(|c| c.0) == BTRFS_MAGIC
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
