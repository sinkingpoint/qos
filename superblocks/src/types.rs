use bytestruct::UUID;

/// A trait for filesystem superblocks.
pub trait Superblock {
    /// The offset of the superblock in the device.
    fn offset() -> u64;
    /// The size of the superblock in bytes.
    fn size() -> usize;
    /// Returns true if the superblock is valid (i.e the filesystem is the format of this superblock).
    fn validate(&self) -> bool;
    /// Returns the name of the filesystem.
    fn name(&self) -> String;
    fn label(&self) -> String;
    fn uuid(&self) -> UUID;
}
