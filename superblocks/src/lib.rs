mod types;
mod ext;
mod btrfs;

use std::{fs::File, io::{self, Cursor, Read, Seek, SeekFrom}, path::PathBuf};

use bytestruct::{ReadFrom, UUID};
pub use types::Superblock;
pub use ext::*;
pub use btrfs::*;

/// A device that may contain a filesystem.
pub struct Device {
    /// The absolute path to the device.
    path: PathBuf,
}

impl Device {
    /// Creates a new device from the given path.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
        }
    }

    /// Trys to probe the device to work out what type of filesystem it contains.
    pub fn probe(&self) -> io::Result<Option<ProbeResult>> {
        if let Some(result) = self.probe_fs::<ExtSuperBlock>()? {
            Ok(Some(result))
        } else if let Some(result) = self.probe_fs::<BtrfsSuperBlock>()? {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Trys to probe the device for a filesystem of the given type.
    fn probe_fs<T: Superblock + ReadFrom>(&self) -> io::Result<Option<ProbeResult>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(T::offset()))?;

        let mut buffer = vec![0; T::size()];
        file.read_exact(&mut buffer)?;

        let superblock = T::read_from(&mut Cursor::new(buffer))?;

        if superblock.validate() {
            Ok(Some(ProbeResult {
                path: self.path.clone(),
                filesystem_type: superblock.name(),
                label: superblock.label(),
                uuid: superblock.uuid(),
            }))
        } else {
            Ok(None)
        }
    }
}

/// The result of probing a device.
#[derive(Debug)]
pub struct ProbeResult {
    /// The path to the device.
    pub path: PathBuf,
    /// The name of the type of the filesystem (e.g. "ext4").
    pub filesystem_type: String,
    /// The label of the filesystem.
    pub label: String,
    /// The UUID of the filesystem.
    pub uuid: UUID,
}
