use std::{io::{self, Read}, path::Path, fs::{self, File}, os::unix::fs::{MetadataExt, FileTypeExt}};

// The magic number for a CPIO archive.
const CPIO_MAGIC: &[u8; 6] = b"070701";

// The length of the CPIO header, in bytes.
const HEADER_LENGTH: usize = CPIO_MAGIC.len() + 13 * 8;

// The trailer entry name.
const TRAILER_ENTRY_NAME: &str = "TRAILER!!!";

// File type constants from https://man7.org/linux/man-pages/man0/sys_stat.h.0p.html;
const S_IFDIR: u32  = 0o040000; // directory
const S_IFCHR: u32  = 0o020000; // character device
const S_IFBLK: u32  = 0o060000; // block device
const S_IFREG: u32  = 0o100000; // regular file
const S_IFIFO: u32  = 0o010000; // fifo (named pipe)
const S_IFLNK: u32  = 0o120000; // symbolic link
const S_IFSOCK: u32 = 0o140000; // socket file

const ALIGNMENT: usize = 4;

#[derive(Debug)]
pub struct CPIOArchive {
    pub entries: Vec<Entry>,
}

impl CPIOArchive {
    // Read a CPIO archive from the reader.
    pub fn read<T>(reader: &mut T) -> io::Result<CPIOArchive> where T: io::Read {
        let mut entries = Vec::new();

        loop {
            let entry = Entry::read(reader)?;

            if entry.name == TRAILER_ENTRY_NAME {
                break;
            }

            entries.push(entry);
        }

        Ok(CPIOArchive{
            entries,
        })
    }

    // Write a CPIO archive to the writer.
    pub fn write<T>(&self, writer: &mut T) -> io::Result<()> where T: io::Write {
        for entry in &self.entries {
            entry.write(writer)?;
        }

        trailer().write(writer)?;

        Ok(())
    }

    // Create a CPIO archive from a directory, reading all files and subdirectories recursively.
    // The paths in the archive will be relative to the given path.
    pub fn from_path(path: &Path) -> io::Result<CPIOArchive> {
        let mut dirs_to_scan = vec![path.to_path_buf()];
        let mut entries = Vec::new();

        while let Some(dir) = dirs_to_scan.pop() {
            entries.push(Entry::from_file(&dir)?);
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    dirs_to_scan.push(path);
                } else {
                    entries.push(Entry::from_file(&path)?);
                }
            }
        }

        // Trim the file prefix from the paths.
        for entry in &mut entries {
            entry.trim_file_prefix(path);
        }

        Ok(CPIOArchive{
            entries,
        })
    }
}

// The header for a CPIO entry.
#[derive(Debug)]
pub struct EntryHeader {
    // The inode number.
    pub inode: u32,
    // The mode and file type.
    pub mode: u32,
    // The user ID.
    pub uid: u32,
    // The group ID.
    pub gid: u32,
    // The number of hard links.
    pub nlink: u32,
    // The modification time.
    pub mtime: u32,
    // The file size.
    pub size: u32,
    // The upper 4 bytes of the device number.
    pub devmajor: u32,
    // The lower 4 bytes of the device number.
    pub devminor: u32,
    // The upper 4 bytes of the device type number for special files.
    pub rdevmajor: u32,
    // The lower 4 bytes of the device type number for special files.
    pub rdevminor: u32,
    // The length of the file name, including the null terminator.
    pub namesize: u32,
}

impl EntryHeader {
    pub fn read<T>(reader: &mut T) -> io::Result<EntryHeader> where T: io::Read {
        let mut buf = [0; 6];
        reader.read_exact(&mut buf)?;

        if &buf != CPIO_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid CPIO magic"));
        }

        Ok(EntryHeader {
            inode: read_ascii_uint32(reader)?,
            mode: read_ascii_uint32(reader)?,
            uid: read_ascii_uint32(reader)?,
            gid: read_ascii_uint32(reader)?,
            nlink: read_ascii_uint32(reader)?,
            mtime: read_ascii_uint32(reader)?,
            size: read_ascii_uint32(reader)?,
            devmajor: read_ascii_uint32(reader)?,
            devminor: read_ascii_uint32(reader)?,
            rdevmajor: read_ascii_uint32(reader)?,
            rdevminor: read_ascii_uint32(reader)?,
            namesize: read_ascii_uint32(reader)?,
        })
    }

    pub fn write(&self, writer: &mut dyn io::Write) -> io::Result<()> {
        writer.write_all(CPIO_MAGIC)?;
        writer.write_all(format!("{:08x}", self.inode).as_bytes())?;
        writer.write_all(format!("{:08x}", self.mode).as_bytes())?;
        writer.write_all(format!("{:08x}", self.uid).as_bytes())?;
        writer.write_all(format!("{:08x}", self.gid).as_bytes())?;
        writer.write_all(format!("{:08x}", self.nlink).as_bytes())?;
        writer.write_all(format!("{:08x}", self.mtime).as_bytes())?;
        writer.write_all(format!("{:08x}", self.size).as_bytes())?;
        writer.write_all(format!("{:08x}", self.devmajor).as_bytes())?;
        writer.write_all(format!("{:08x}", self.devminor).as_bytes())?;
        writer.write_all(format!("{:08x}", self.rdevmajor).as_bytes())?;
        writer.write_all(format!("{:08x}", self.rdevminor).as_bytes())?;
        writer.write_all(format!("{:08x}", self.namesize).as_bytes())?;
        writer.write_all(format!("{:08x}", 0).as_bytes())?; // Checksum

        Ok(())
    }
}

// A CPIO entry, representing a file or directory.
#[derive(Debug)]
pub struct Entry {
    // The header for the entry.
    pub header: EntryHeader,
    // The file name of the entry.
    pub name: String,
    // The file data.
    pub data: Vec<u8>,
}

impl Entry {
    pub fn read<T>(reader: &mut T) -> io::Result<Entry> where T: io::Read {
        let header = EntryHeader::read(reader)?;

        let _check = read_ascii_uint32(reader)?;

        let mut namebuf = vec![0; header.namesize as usize];
        reader.read_exact(&mut namebuf)?;

        // Pad out to a 4-byte boundary.
        reader.read_exact(&mut vec![0; num_padding_bytes(HEADER_LENGTH + header.namesize as usize, ALIGNMENT)])?;

        let mut data = vec![0; header.size as usize];
        reader.read_exact(&mut data)?;

        reader.read_exact(&mut vec![0; num_padding_bytes(header.size as usize, ALIGNMENT)])?;

        Ok(Entry{
            header,
            name: String::from_utf8(namebuf).unwrap().trim_end_matches('\0').to_string(),
            data,
        })
    }

    pub fn write(&self, writer: &mut dyn io::Write) -> io::Result<()> {
        self.header.write(writer)?;

        writer.write_all(self.name.as_bytes())?;
        writer.write_all(&[0])?; // Null terminator

        writer.write_all(&vec![0; num_padding_bytes(HEADER_LENGTH + self.header.namesize as usize, ALIGNMENT)])?;
        writer.write_all(&self.data)?;

        writer.write_all(&vec![0; num_padding_bytes(self.header.size as usize, ALIGNMENT)])?;

        Ok(())
    }

    // Trim the given prefix from the file name.
    pub fn trim_file_prefix(&mut self, prefix: &Path) {
        let path = Path::new(&self.name);
        if path == prefix {
            self.name = String::from(".");
            self.header.namesize = 2;
        }
        else if let Ok(path) = path.strip_prefix(prefix) {
            self.name = path.to_str().expect("file name is invalid unicode").to_owned();
            self.header.namesize = self.name.len() as u32 + 1;
        }
    }

    // Create a CPIO entry from a file.
    pub fn from_file(path: &Path) -> io::Result<Entry> {
        let metadata = fs::metadata(path)?;

        let mut data = Vec::new();
        if metadata.is_file() {
            File::open(path)?.read_to_end(&mut data)?;
        }

        let dev = metadata.dev();
        let rdev = metadata.rdev();
        let name = path.to_str().expect("file name is invalid unicode").to_string();

        Ok(Entry{
            header: EntryHeader{
                inode: metadata.ino() as u32,
                mode: mode(&metadata),
                uid: metadata.uid(),
                gid: metadata.gid(),
                nlink: metadata.nlink() as u32,
                mtime: metadata.mtime() as u32,
                size: data.len() as u32,
                devmajor: (dev >> 8) as u32,
                devminor: (dev & 0xff) as u32,
                rdevmajor: (rdev >> 8) as u32,
                rdevminor: (rdev & 0xff) as u32,
                namesize: name.len() as u32 + 1,
            },
            name,
            data,
        })
    }
}

// Calculate the number of padding bytes needed to pad num_bytes to pad_to.
fn num_padding_bytes(num_bytes: usize, pad_to: usize) -> usize {
    (pad_to - (num_bytes % pad_to)) % pad_to
}

// Read a 32-bit unsigned integer from the reader, as a hex encoded ASCII number.
fn read_ascii_uint32<T>(reader: &mut T) -> io::Result<u32> where T: io::Read {
    let mut buf = [0; 8];
    reader.read_exact(&mut buf)?;

    let num_str = std::str::from_utf8(&buf).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?;
    u32::from_str_radix(num_str, 16).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid number: {}", num_str)))
}

// Calculate the mode for a file from its metadata.
// Mode is a combination of the file type and the permissions, where the file type comes from stat.h.
fn mode(metadata: &fs::Metadata) -> u32 {
    let mut mode = metadata.mode();

    if metadata.is_dir() {
        mode |= S_IFDIR;
    } else if metadata.is_file() {
        mode |= S_IFREG;
    } else if metadata.file_type().is_symlink() {
        mode |= S_IFLNK;
    } else if metadata.file_type().is_char_device() {
        mode |= S_IFCHR;
    } else if metadata.file_type().is_block_device() {
        mode |= S_IFBLK;
    } else if metadata.file_type().is_fifo() {
        mode |= S_IFIFO;
    } else if metadata.file_type().is_socket() {
        mode |= S_IFSOCK;
    }

    mode
}

fn trailer() -> Entry {
    Entry {
        header: EntryHeader {
            inode: 0,
            mode: 0,
            uid: 0,
            gid: 0,
            nlink: 1,
            mtime: 0,
            size: 0,
            devmajor: 0,
            devminor: 0,
            rdevmajor: 0,
            rdevminor: 0,
            namesize: TRAILER_ENTRY_NAME.len() as u32 + 1,
        },
        name: String::from(TRAILER_ENTRY_NAME),
        data: vec![],
    }
}
