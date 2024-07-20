use std::{
	fs::File,
	io::{self, ErrorKind, Read, Seek, SeekFrom},
	path::Path,
	sync::Mutex,
};

mod structs;
use bytestruct::ReadFrom;
pub use structs::*;

#[derive(Debug)]
pub struct ElfFile {
	inner: Mutex<File>,

	pub header: ElfHeader,

	section_names: StringTableSection,
}

impl ElfFile {
	pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
		let mut file = File::open(path)?;
		let header = ElfHeader::read_from(&mut file)?;

		let section_names_header = match read_section_header(&mut file, &header, header.section_header_table_name_idx) {
			None => {
				return Err(io::Error::new(
					ErrorKind::InvalidData,
					format!(
						"can't read section names: section {} doesn't exist",
						header.section_header_table_name_idx
					),
				))
			}
			Some(Err(e)) => return Err(e),
			Some(Ok(header)) => header,
		};

		let section_names = match section_names_header.read_string_table_section(&mut file) {
			None => {
				return Err(io::Error::new(
					ErrorKind::InvalidData,
					format!(
						"can't read section names: section isn't a string table: {:?}",
						section_names_header.ty
					),
				))
			}
			Some(Err(e)) => return Err(e),
			Some(Ok(names)) => names,
		};

		Ok(Self {
			inner: Mutex::new(file),
			header,
			section_names,
		})
	}

	pub fn program_headers(&'_ self) -> impl Iterator<Item = io::Result<ProgramHeader>> + '_ {
		ProgramHeaderIterator::new(self, &self.header)
	}

	pub fn section_headers(&'_ self) -> impl Iterator<Item = io::Result<SectionHeader>> + '_ {
		SectionHeaderIterator::new(self, &self.header)
	}

	pub fn section_header_name(&self, header: &SectionHeader) -> Option<&str> {
		self.section_names.get_string_at_offset(header.name_offset as u64)
	}
}

impl Read for &ElfFile {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let mut inner = self.inner.lock().unwrap();
		inner.read(buf)
	}
}

impl Seek for &ElfFile {
	fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
		let mut inner = self.inner.lock().unwrap();
		inner.seek(pos)
	}
}

/// An iterator over a given ElfFile's program headers
struct ProgramHeaderIterator<'a, T: Read + Seek> {
	idx: u64,

	inner: T,
	header: &'a ElfHeader,
}

impl<'a, T: Read + Seek> ProgramHeaderIterator<'a, T> {
	fn new(inner: T, header: &'a ElfHeader) -> Self {
		Self { idx: 0, inner, header }
	}
}

impl<'a, T: Read + Seek> Iterator for ProgramHeaderIterator<'a, T> {
	type Item = io::Result<ProgramHeader>;

	fn next(&mut self) -> Option<Self::Item> {
		let header = read_program_header(&mut self.inner, self.header, self.idx);
		self.idx += 1;
		header
	}
}

struct SectionHeaderIterator<'a, T: Read + Seek> {
	idx: u64,

	inner: T,
	header: &'a ElfHeader,
}

impl<'a, T: Read + Seek> SectionHeaderIterator<'a, T> {
	fn new(inner: T, header: &'a ElfHeader) -> Self {
		Self { idx: 0, inner, header }
	}
}

impl<'a, T: Read + Seek> Iterator for SectionHeaderIterator<'a, T> {
	type Item = io::Result<SectionHeader>;
	fn next(&mut self) -> Option<Self::Item> {
		let header = read_section_header(&mut self.inner, self.header, self.idx);
		self.idx += 1;
		header
	}
}

/// Attempt to read a program header from the given reader, returning None if the program header at the given index doesn't exist.
fn read_program_header<T: Read + Seek>(
	reader: &mut T,
	header: &ElfHeader,
	idx: u64,
) -> Option<io::Result<ProgramHeader>> {
	if idx >= header.program_header_table_len {
		return None;
	}

	let offset = header.program_header_offset + idx * header.program_header_size;

	if let Err(e) = reader.seek(SeekFrom::Start(offset)) {
		return Some(Err(e));
	}

	Some(ProgramHeader::read_from_with_endian(
		reader,
		header.class,
		header.endian,
	))
}

/// Attempt to read a section header from the given reader, returning None if the section header at the given index doesn't exist.
fn read_section_header<T: Read + Seek>(
	reader: &mut T,
	header: &ElfHeader,
	idx: u64,
) -> Option<io::Result<SectionHeader>> {
	if idx >= header.section_header_table_len {
		return None;
	}

	let offset = header.section_header_offset + idx * header.section_header_size;

	if let Err(e) = reader.seek(SeekFrom::Start(offset)) {
		return Some(Err(e));
	}

	Some(SectionHeader::read_from_with_endian(
		reader,
		header.class,
		header.endian,
	))
}
