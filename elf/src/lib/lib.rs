use std::{
	fs::File,
	io::{self, Seek, SeekFrom},
	path::Path,
};

mod structs;
use bytestruct::ReadFrom;
pub use structs::*;

#[derive(Debug)]
pub struct ElfFile {
	inner: File,

	pub header: ElfHeader,
}

impl ElfFile {
	pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
		let mut file = File::open(path)?;
		let header = ElfHeader::read_from(&mut file)?;

		Ok(Self { inner: file, header })
	}

	pub fn program_headers(&'_ self) -> impl Iterator<Item = io::Result<ProgramHeader>> + '_ {
		ProgramHeaderIterator::new(&self.inner, &self.header)
	}

	pub fn section_headers(&'_ self) -> impl Iterator<Item = io::Result<SectionHeader>> + '_ {
		SectionHeaderIterator::new(&self.inner, &self.header)
	}
}

/// An iterator over a given ElfFile's program headers
struct ProgramHeaderIterator<'a> {
	idx: u64,

	inner: &'a File,
	header: &'a ElfHeader,
}

impl<'a> ProgramHeaderIterator<'a> {
	fn new(inner: &'a File, header: &'a ElfHeader) -> Self {
		Self { idx: 0, inner, header }
	}
}

impl<'a> Iterator for ProgramHeaderIterator<'a> {
	type Item = io::Result<ProgramHeader>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.idx >= self.header.program_header_table_len {
			return None;
		}

		let offset = self.header.program_header_offset + self.idx * self.header.program_table_header_entry_size;

		if let Err(e) = self.inner.seek(SeekFrom::Start(offset)) {
			return Some(Err(e));
		}

		self.idx += 1;

		Some(ProgramHeader::read_from_with_endian(
			&mut self.inner,
			self.header.class,
			self.header.endian,
		))
	}
}

struct SectionHeaderIterator<'a> {
	idx: u64,

	inner: &'a File,
	header: &'a ElfHeader,
}

impl<'a> SectionHeaderIterator<'a> {
	fn new(inner: &'a File, header: &'a ElfHeader) -> Self {
		Self { idx: 0, inner, header }
	}
}

impl<'a> Iterator for SectionHeaderIterator<'a> {
	type Item = io::Result<SectionHeader>;
	fn next(&mut self) -> Option<Self::Item> {
		if self.idx >= self.header.section_header_table_len {
			return None;
		}

		let offset = self.header.section_header_offset + self.idx * self.header.section_table_header_entry_size;
		if let Err(e) = self.inner.seek(SeekFrom::Start(offset)) {
			return Some(Err(e));
		}

		self.idx += 1;

		Some(SectionHeader::read_from_with_endian(
			&mut self.inner,
			self.header.class,
			self.header.endian,
		))
	}
}
