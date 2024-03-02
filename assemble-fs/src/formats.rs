use std::{fs::File, io, path::Path, process::Command};

use cpio::CPIOArchive;

pub fn write_cpio(path: &Path, out_path: &Path) -> io::Result<()> {
    let mut out_file = File::create(out_path)?;
    let archive = CPIOArchive::from_path(path)?;
    archive.write(&mut out_file)
}

pub fn write_ext4(path: &Path, out_path: &Path) -> io::Result<()> {
    // Shell out to mke2fs because writing an ext4 filesystem is hard.
    let status = Command::new("mke2fs")
        .arg("-F")
        .arg("-L")
        .arg("root")
        .arg("-N")
        .arg("0")
        .arg("-d")
        .arg(path)
        .arg("-m")
        .arg("5")
        .arg("-t")
        .arg("ext4")
        .arg(out_path)
        .arg("1G")
        .status()?;

    match status.success() {
        true => Ok(()),
        false => Err(io::Error::new(io::ErrorKind::Other, "mke2fs failed")),
    }
}
