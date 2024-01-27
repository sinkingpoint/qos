use bytestruct::{ByteArray, LittleEndianU16, LittleEndianU32, NullTerminatedString, U8};
use bytestruct_derive::ByteStruct;

use crate::types::Superblock;

pub const EXT_MAGIC: LittleEndianU16 = LittleEndianU16(0xEF53);

/// Sparse superblocks.
pub const RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
/// Allow storing files larger than 2GiB.
pub const RO_COMPAT_LARGE_FILE: u32 = 0x0002;
/// This filesystem has files whose space usage is stored in i_blocks in units of filesystem blocks, 
/// not 512-byte sectors. Inodes using this feature will be marked with EXT4_INODE_HUGE_FILE.
pub const RO_COMPAT_HUGE_FILE: u32 = 0x0008;
/// Group descriptors have checksums. In addition to detecting corruption, this is useful for lazy formatting with uninitialized groups.
pub const RO_COMPAT_GDT_CSUM: u32 = 0x0010;
/// Indicates that the old ext3 32,000 subdirectory limit no longer applies. 
/// A directory's i_links_count will be set to 1 if it is incremented past 64,999.
pub const RO_COMPAT_DIR_NLINK: u32 = 0x0020;
/// Indicates that large inodes exist on this filesystem, storing extra fields after EXT2_GOOD_OLD_INODE_SIZE.
pub const RO_COMPAT_EXTRA_ISIZE: u32 = 0x0040;
/// This filesystem has a snapshot. Not implemented in ext4.
pub const RO_COMPAT_HAS_SNAPSHOT: u32 = 0x0080;
/// Quota is handled transactionally with the journal.
pub const RO_COMPAT_QUOTA: u32 = 0x0100;
/// This filesystem supports "bigalloc", which means that filesystem block 
/// allocation bitmaps are tracked in units of clusters (of blocks) instead of blocks.
pub const RO_COMPAT_BIGALLOC: u32 = 0x0200;
/// This filesystem supports metadata checksumming.
pub const RO_COMPAT_METADATA_CSUM: u32 = 0x0400;
/// Filesystem supports replicas. This feature is neither in the kernel nor e2fsprogs.
pub const RO_COMPAT_REPLICA: u32 = 0x0800;
/// Read-only filesystem image; the kernel will not mount this image read-write and most tools will refuse to write to the image.
pub const RO_COMPAT_READONLY: u32 = 0x1000;
/// Filesystem tracks project quotas.
pub const RO_COMPAT_PROJECT: u32 = 0x2000;

/// Compression. Not implemented.
pub const INCOMPAT_COMPRESSION: u32 = 0x0001;
/// Directory entries record the file type.
pub const INCOMPAT_FILETYPE: u32 = 0x0002;
/// Filesystem needs journal recovery.
pub const INCOMPAT_RECOVER: u32 = 0x0004;
/// Filesystem has a separate journal device.
pub const INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
/// Meta block groups. See the earlier discussion of this feature.
pub const INCOMPAT_META_BG: u32 = 0x0010;
/// Files in this filesystem use extents.
pub const INCOMPAT_EXTENTS: u32 = 0x0040;
/// Enable a filesystem size over 2^32 blocks.
pub const INCOMPAT_64BIT: u32 = 0x0080;
/// Multiple mount protection. Prevent multiple hosts from mounting the filesystem
/// concurrently by updating a reserved block periodically while mounted and checking 
/// this at mount time to determine if the filesystem is in use on another host.
pub const INCOMPAT_MMP: u32 = 0x0100;
/// Flexible block groups. See the earlier discussion of this feature.
pub const INCOMPAT_FLEX_BG: u32 = 0x0200;
/// Inodes can be used to store large extended attribute values.
pub const INCOMPAT_EA_INODE: u32 = 0x0400;
/// Data in directory entry. Allow additional data fields to be stored in each dirent, after struct ext4_dirent.
pub const INCOMPAT_DIRDATA: u32 = 0x1000;
/// Metadata checksum seed is stored in the superblock.
pub const INCOMPAT_CSUM_SEED: u32 = 0x2000;
/// Large directory >2GB or 3-level htree.
pub const INCOMPAT_LARGEDIR: u32 = 0x4000;
/// Data in inode. Small files or directories are stored directly in the inode i_blocks and/or xattr space.
pub const INCOMPAT_INLINE_DATA: u32 = 0x8000;
/// Encrypted inodes are present on the filesystem.
pub const INCOMPAT_ENCRYPT: u32 = 0x10000;

/// Directory preallocation.
pub const COMPAT_DIR_PREALLOC: u32 = 0x0001;
/// "imagic inodes". Used by AFS to indicate inodes that are not linked into the directory namespace.
pub const COMPAT_IMAGIC_INODES: u32 = 0x0002;
/// Has a journal.
pub const COMPAT_HAS_JOURNAL: u32 = 0x0004;
/// Supports extended attributes.
pub const COMPAT_EXT_ATTR: u32 = 0x0008;
/// Has reserved GDT blocks for filesystem expansion. Requires RO_COMPAT_SPARSE_SUPER.
pub const COMPAT_RESIZE_INODE: u32 = 0x0010;
/// Has indexed directories.
pub const COMPAT_DIR_INDEX: u32 = 0x0020;
/// "Lazy BG". Not in Linux kernel, seems to have been for uninitialized block groups?
pub const COMPAT_LAZY_BG: u32 = 0x0040;
/// "Exclude inode". Intended for filesystem snapshot feature, but not used.
pub const COMPAT_EXCLUDE_INODE: u32 = 0x0080;
/// "Exclude bitmap". Seems to be used to indicate the presence of snapshot-related exclude bitmaps?
pub const COMPAT_EXCLUDE_BITMAP: u32 = 0x0100;
/// Sparse Super Block, v2. If this flag is set, the SB field s_backup_bgs points to the two block groups that contain backup superblocks. 
pub const COMPAT_SPARSE_SUPER2: u32 = 0x0200;

#[derive(ByteStruct)]
pub struct ExtSuperBlock {
    pub inode_count: LittleEndianU32,
    pub blocks_count: LittleEndianU32,
    pub reserved_blocks_count: LittleEndianU32,
    pub free_blocks_count: LittleEndianU32,
    pub free_inodes_count: LittleEndianU32,
    pub first_data_block: LittleEndianU32,
    pub log_block_size: LittleEndianU32,
    pub log_cluster_size: LittleEndianU32,
    pub blocks_per_group: LittleEndianU32,
    pub clusters_per_group: LittleEndianU32,
    pub inodes_per_group: LittleEndianU32,
    pub mount_time: LittleEndianU32,
    pub write_time: LittleEndianU32,
    pub mount_count: LittleEndianU16,
    pub max_mount_count: LittleEndianU16,
    pub magic: LittleEndianU16,
    pub state: LittleEndianU16,
    pub errors: LittleEndianU16,
    pub minor_rev_level: LittleEndianU16,
    pub last_check: LittleEndianU32,
    pub check_interval: LittleEndianU32,
    pub creator_os: LittleEndianU32,
    pub rev_level: LittleEndianU32,
    pub default_resuid: LittleEndianU16,
    pub default_resgid: LittleEndianU16,
    pub first_inode: LittleEndianU32,
    pub inode_size: LittleEndianU16,
    pub block_group_number: LittleEndianU16,
    pub feature_compat: LittleEndianU32,
    pub feature_incompat: LittleEndianU32,
    pub feature_ro_compat: LittleEndianU32,
    pub uuid: ByteArray<16>,
    pub label: NullTerminatedString<16>,
    pub last_mount_path: NullTerminatedString<64>,
    pub algorithm_usage_bitmap: LittleEndianU32,
    pub prealloc_blocks: U8,
    pub prealloc_dir_blocks: U8,
    _unused: LittleEndianU16,
    pub journal_uuid: ByteArray<16>,
    pub journal_inode: LittleEndianU32,
    pub journal_dev: LittleEndianU32,
    pub orphan_inode_head: LittleEndianU32,
}

impl Superblock for ExtSuperBlock {
    fn offset() -> u64 {
        0x400
    }

    fn size() -> usize {
        0x400
    }

    fn validate(&self) -> bool {
        self.magic == EXT_MAGIC
    }

    fn name(&self) -> String {
        match self.ext_type() {
            ExtType::Ext2 => "ext2",
            ExtType::Ext3 => "ext3",
            ExtType::Ext4 => "ext4",
        }.to_string()
    }

    fn label(&self) -> String {
        self.label.0.clone()
    }

    fn uuid(&self) -> bytestruct::UUID {
        self.uuid
    }
}

/// The type of the ext filesystem.
pub enum ExtType {
    Ext2,
    Ext3,
    Ext4,
}

impl ExtSuperBlock {
    /// Returns the type of the ext filesystem.
    /// EXT2/3/4 are basically the same file system, with different features. Here we check the features
    /// of each against the super block, and return the first one that matches.
    pub fn ext_type(&self) -> ExtType {
        // Features that were introduced in ext4.
        let ext4_ro_features = [RO_COMPAT_BIGALLOC, RO_COMPAT_DIR_NLINK, RO_COMPAT_EXTRA_ISIZE, RO_COMPAT_HUGE_FILE, RO_COMPAT_GDT_CSUM];
        let ext4_incompat_features = [INCOMPAT_64BIT, INCOMPAT_EXTENTS, INCOMPAT_FLEX_BG, INCOMPAT_META_BG, INCOMPAT_MMP];

        // Features that were introduced in ext3.
        let ext3_compat_features = [COMPAT_DIR_INDEX, COMPAT_HAS_JOURNAL];

        if has_any(self.feature_ro_compat.0, &ext4_ro_features) || has_any(self.feature_incompat.0, &ext4_incompat_features) {
            ExtType::Ext4
        } else if has_any(self.feature_compat.0, &ext3_compat_features) {
            ExtType::Ext3
        } else {
            ExtType::Ext2
        }
    }
}

/// Returns true if val has any of the features in features.
fn has_any(val: u32, features: &[u32]) -> bool {
    for feature in features {
        if val & feature != 0 {
            return true;
        }
    }
    false
}
