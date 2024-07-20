use std::fmt::Debug;
use std::io::{self, Cursor, ErrorKind, Read, Seek, SeekFrom};

use bitflags::bitflags;
use bytestruct::{Endian, ReadFrom, ReadFromWithEndian};
use bytestruct_derive::ByteStruct;

const ELF_VERSION: u8 = 1;

/// The Class of the ELF file, which determines the size of various parts of the header.
#[derive(Debug, PartialEq, Copy, Clone, ByteStruct)]
#[repr(u8)]
pub enum Class {
	ThirtyTwoBit = 1,
	SixtyFourBit = 2,
}

impl ReadFrom for Class {
	fn read_from<T: io::Read>(source: &mut T) -> io::Result<Self> {
		Self::read_from_with_endian(source, Endian::Little)
	}
}

impl Class {
	fn read_value<T: io::Read>(&self, source: &mut T, endian: Endian) -> io::Result<u64> {
		match self {
			Self::ThirtyTwoBit => u32::read_from_with_endian(source, endian).map(|v| v as u64),
			Self::SixtyFourBit => u64::read_from_with_endian(source, endian),
		}
	}
}

#[derive(Debug, Copy, Clone, ByteStruct)]
#[repr(u8)]
pub enum Abi {
	SystemV,
	HpUx,
	NetBSD,
	Linux,
	Hurd,
	_Blank,
	Solaris,
	Aix,
	Irix,
	FreeBSD,
	Tru64,
	Modesto,
	OpenBSD,
	OpenVMS,
	NonStopKernel,
	Aros,
	FenixOS,
	CloudABI,
	OpenVOS,
}

#[derive(Debug, Copy, Clone)]
#[repr(u16)]
pub enum ElfType {
	None,
	RelocatableFile,
	ExecutableFile,
	SharedObject,
	CoreFile,
	OSSpecific(u16),
	ProcessorSpecific(u16),
}

impl ReadFromWithEndian for ElfType {
	fn read_from_with_endian<T: io::Read>(source: &mut T, endian: bytestruct::Endian) -> io::Result<Self>
	where
		Self: Sized,
	{
		let val = u16::read_from_with_endian(source, endian)?;
		match val {
			0 => Ok(Self::None),
			1 => Ok(Self::RelocatableFile),
			2 => Ok(Self::ExecutableFile),
			3 => Ok(Self::SharedObject),
			4 => Ok(Self::CoreFile),
			0xFE00..=0xFEFF => Ok(Self::OSSpecific(val)),
			0xFF00..=0xFFFF => Ok(Self::ProcessorSpecific(val)),
			_ => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid ELF class: {}", val),
			)),
		}
	}
}

#[derive(Debug, Copy, Clone, ByteStruct)]
#[repr(u16)]
pub enum TargetArch {
	None = 0x0,
	WE32100 = 0x1,
	Sparc = 0x2,
	Intelx86 = 0x3,
	Motorolla68000 = 0x4,
	Motorolla88000 = 0x5,
	IntelMCU = 0x6,
	Intel80860 = 0x7,
	Mips = 0x08,
	IBM370 = 0x09,
	MipsLittleEndian = 0x0A,
	HpPaRISC = 0x0F,
	Intel80960 = 0x13,
	PowerPC = 0x14,
	PowerPC64 = 0x15,
	S390 = 0x16,
	Arm = 0x28,
	SuperH = 0x2A,
	IA64 = 0x32,
	AMD64 = 0x3E,
	ARM64 = 0xB7,
	RiscV = 0xF3,
}

#[derive(Debug)]
pub struct ElfHeader {
	pub class: Class,
	pub endian: Endian,
	pub abi: Abi,
	pub abi_version: u8,
	pub ty: ElfType,
	pub architecture: TargetArch,

	pub entrypoint_offset: u64,
	pub program_header_offset: u64,
	pub section_header_offset: u64,
	pub flags: u32,
	pub header_size: u64,
	pub program_header_size: u64,
	pub program_header_table_len: u64,
	pub section_header_size: u64,
	pub section_header_table_len: u64,
	pub section_header_table_name_idx: u64,
}

impl ReadFrom for ElfHeader {
	fn read_from<T: io::Read>(source: &mut T) -> io::Result<Self> {
		let magic = <[u8; 4]>::read_from(source)?;
		if magic != [0x7F, b'E', b'L', b'F'] {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid magic number: {:?}", magic),
			));
		}

		let class = Class::read_from(source)?;
		let endian = match u8::read_from(source)? {
			1 => Endian::Little,
			2 => Endian::Big,
			other => {
				return Err(io::Error::new(
					ErrorKind::InvalidData,
					format!("invalid endian: {}", other),
				))
			}
		};

		let version = u8::read_from(source)?;
		if version != ELF_VERSION {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid version: {}", version),
			));
		}

		let abi = Abi::read_from_with_endian(source, endian)?;
		let abi_version = u8::read_from(source)?;

		// Padding.
		<[u8; 7]>::read_from(source)?;

		let ty = ElfType::read_from_with_endian(source, endian)?;
		let architecture = TargetArch::read_from_with_endian(source, endian)?;

		// Second version for some reason.
		u32::read_from_with_endian(source, endian)?;

		let entrypoint_offset = class.read_value(source, endian)?;
		let program_header_offset = class.read_value(source, endian)? as u64;
		let section_header_offset = class.read_value(source, endian)? as u64;

		let flags = u32::read_from_with_endian(source, endian)?;
		let header_size = u16::read_from_with_endian(source, endian)? as u64;
		let program_table_header_entry_size = u16::read_from_with_endian(source, endian)? as u64;
		let program_header_table_len = u16::read_from_with_endian(source, endian)? as u64;
		let section_table_header_entry_size = u16::read_from_with_endian(source, endian)? as u64;
		let section_header_table_len = u16::read_from_with_endian(source, endian)? as u64;
		let section_header_table_name_idx = u16::read_from_with_endian(source, endian)? as u64;

		<[u8; 0x6]>::read_from_with_endian(source, endian)?;

		Ok(Self {
			class,
			endian,
			abi,
			abi_version,
			ty,
			architecture,

			entrypoint_offset,
			program_header_offset,
			section_header_offset,
			flags,
			header_size,
			program_header_size: program_table_header_entry_size,
			program_header_table_len,
			section_header_size: section_table_header_entry_size,
			section_header_table_len,
			section_header_table_name_idx,
		})
	}
}

#[derive(Debug)]
pub enum ProgramHeaderType {
	Null,
	LoadableSegment,
	DynamicLink,
	Interpreter,
	Auxiliary,
	Reserved,
	ProgramHeaderTable,
	ThreadLocalStorage,
	OSSpecific(u32),
	ProcessorSpecific(u32),
}

impl ReadFromWithEndian for ProgramHeaderType {
	fn read_from_with_endian<T: io::Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		match u32::read_from_with_endian(source, endian)? {
			0x00000000 => Ok(Self::Null),
			0x00000001 => Ok(Self::LoadableSegment),
			0x00000002 => Ok(Self::DynamicLink),
			0x00000003 => Ok(Self::Interpreter),
			0x00000004 => Ok(Self::Auxiliary),
			0x00000005 => Ok(Self::Reserved),
			0x00000006 => Ok(Self::ProgramHeaderTable),
			0x00000007 => Ok(Self::ThreadLocalStorage),
			n @ 0x60000000..=0x6FFFFFFF => Ok(Self::OSSpecific(n)),
			n @ 0x70000000..=0x7FFFFFFF => Ok(Self::ProcessorSpecific(n)),
			n => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid program header type: {}", n),
			)),
		}
	}
}

#[derive(PartialEq)]
pub struct ProgramHeaderFlags(u32);

impl ReadFromWithEndian for ProgramHeaderFlags {
	fn read_from_with_endian<T: io::Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		let val = u32::read_from_with_endian(source, endian)?;
		if val > 7 {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid program header flags: {:#b}", val),
			));
		}

		Ok(Self(val))
	}
}

impl Debug for ProgramHeaderFlags {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let r = if self.readable() { "r" } else { "-" };
		let w = if self.writable() { "w" } else { "-" };
		let x = if self.executable() { "x" } else { "-" };

		f.write_fmt(format_args!("ProgramHeaderFlags({}{}{}, {})", r, w, x, self.0))
	}
}

impl ProgramHeaderFlags {
	pub fn executable(&self) -> bool {
		self.0 & 0x1 == 0x1
	}

	pub fn writable(&self) -> bool {
		self.0 & 0x2 == 0x2
	}

	pub fn readable(&self) -> bool {
		self.0 & 0x4 == 0x4
	}
}

#[derive(Debug)]
pub struct ProgramHeader {
	pub ty: ProgramHeaderType,
	pub flags: ProgramHeaderFlags,
	pub offset: u64,
	pub virtual_address: u64,
	pub physical_address: u64,
	pub file_size: u64,
	pub memory_size: u64,
	pub alignment: u64,
}

impl ProgramHeader {
	pub fn read_from_with_endian<T: io::Read>(source: &mut T, class: Class, endian: Endian) -> io::Result<Self> {
		let ty = ProgramHeaderType::read_from_with_endian(source, endian)?;

		// Flags definitely gets assigned, but the compiler is unable to prove it. This initialises
		// it to a default value, and then we assign it properly below.
		let mut flags = ProgramHeaderFlags(0);
		if class == Class::SixtyFourBit {
			flags = ProgramHeaderFlags::read_from_with_endian(source, endian)?;
		}

		let offset = class.read_value(source, endian)?;
		let virtual_address = class.read_value(source, endian)?;
		let physical_address = class.read_value(source, endian)?;
		let file_size = class.read_value(source, endian)?;
		let memory_size = class.read_value(source, endian)?;

		if class == Class::ThirtyTwoBit {
			flags = ProgramHeaderFlags::read_from_with_endian(source, endian)?;
		}

		let alignment = class.read_value(source, endian)?;

		// padding.
		<[u8; 0x18]>::read_from_with_endian(source, endian)?;

		Ok(Self {
			ty,
			flags,
			offset,
			virtual_address,
			physical_address,
			file_size,
			memory_size,
			alignment,
		})
	}
}

#[derive(Debug, PartialEq)]
pub enum SectionHeaderType {
	Null,
	ProgramData,
	SymbolTable,
	StringTable,
	RelocationTableAppends,
	SymbolHashTable,
	DynamicLinkingInfo,
	Notes,
	Blank,
	RelocationTable,
	DynamicLinkerSymbols,
	Constructors,
	Destructors,
	PreConstructors,
	SectionGroup,
	SectionIndices,
	Number,
	OSSpecific(u32),
}

impl ReadFromWithEndian for SectionHeaderType {
	fn read_from_with_endian<T: io::Read>(source: &mut T, endian: Endian) -> io::Result<Self> {
		match u32::read_from_with_endian(source, endian)? {
			0x0 => Ok(Self::Null),
			0x1 => Ok(Self::ProgramData),
			0x2 => Ok(Self::SymbolTable),
			0x3 => Ok(Self::StringTable),
			0x4 => Ok(Self::RelocationTableAppends),
			0x5 => Ok(Self::SymbolHashTable),
			0x6 => Ok(Self::DynamicLinkingInfo),
			0x7 => Ok(Self::Notes),
			0x8 => Ok(Self::Blank),
			0x9 => Ok(Self::RelocationTable),
			0xB => Ok(Self::DynamicLinkerSymbols),
			0xE => Ok(Self::Constructors),
			0xF => Ok(Self::Destructors),
			0x10 => Ok(Self::PreConstructors),
			0x11 => Ok(Self::SectionGroup),
			0x12 => Ok(Self::SectionIndices),
			0x13 => Ok(Self::Number),
			n @ 0x60000000.. => Ok(Self::OSSpecific(n)),
			n => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid section header type: {}", n),
			)),
		}
	}
}

bitflags! {
	#[derive(Debug)]
	pub struct SectionHeaderFlags: u64 {
		const Writable = 0x1;
		const Allocated = 0x2;
		const Executable = 0x4;
		const Mergable = 0x10;
		const NullTerminatedStrings = 0x20;
		const SectionHeaderIndex = 0x40;
		const PreserveOrder = 0x80;
		const OSNonconforming = 0x100;
		const Group = 0x200;
		const ThreadLocalStorage = 0x400;
	}
}

impl SectionHeaderFlags {
	pub fn read_from_with_endian<T: io::Read>(source: &mut T, class: Class, endian: Endian) -> io::Result<Self> {
		let flags = class.read_value(source, endian)?;
		if let Some(s) = Self::from_bits(flags) {
			return Ok(s);
		}

		Err(io::Error::new(
			ErrorKind::InvalidData,
			format!("invalid section header flags: {}", flags),
		))
	}
}

/// The header of a section in an ELF file.
#[derive(Debug)]
pub struct SectionHeader {
	class: Class,
	endian: Endian,

	/// The offset in the special section names section that contains the name of this section.
	pub name_offset: u32,

	/// The underlying type of this section.
	pub ty: SectionHeaderType,

	pub flags: SectionHeaderFlags,

	/// The address to place this section in memory.
	pub address: u64,

	/// The offset of this section in the ELF file.
	pub offset: u64,

	/// The size of this section in bytes.
	pub size: u64,
	pub link: u32,
	pub info: u32,
	pub alignment: u64,
	pub entry_size: u64,
}

impl SectionHeader {
	pub fn read_from_with_endian<T: io::Read>(source: &mut T, class: Class, endian: Endian) -> io::Result<Self> {
		let name_offset = u32::read_from_with_endian(source, endian)?;
		let ty = SectionHeaderType::read_from_with_endian(source, endian)?;
		let flags = SectionHeaderFlags::read_from_with_endian(source, class, endian)?;
		let address = class.read_value(source, endian)?;
		let offset = class.read_value(source, endian)?;
		let size = class.read_value(source, endian)?;
		let link = u32::read_from_with_endian(source, endian)?;
		let info = u32::read_from_with_endian(source, endian)?;
		let alignment = class.read_value(source, endian)?;
		let entry_size = class.read_value(source, endian)?;

		Ok(Self {
			class,
			endian,
			name_offset,
			ty,
			flags,
			address,
			offset,
			size,
			link,
			info,
			alignment,
			entry_size,
		})
	}

	pub fn read_section<T: Read + Seek>(&self, mut reader: T) -> io::Result<Vec<u8>> {
		reader.seek(SeekFrom::Start(self.offset))?;
		let mut bytes = vec![0; self.size as usize];
		reader.read_exact(&mut bytes)?;

		Ok(bytes)
	}

	/// Attempt to read this section as a String Table, returning None if `ty` is not SectionHeaderType::StringTable.
	pub fn read_string_table_section<T: Read + Seek>(&self, reader: T) -> Option<io::Result<StringTableSection>> {
		if !matches!(self.ty, SectionHeaderType::StringTable) {
			return None;
		}
		let bytes = self.read_section(reader).ok()?;
		Some(StringTableSection::read(&bytes))
	}

	pub fn read_symbol_table_section<T: Read + Seek>(&self, reader: T) -> Option<io::Result<SymbolTableSection>> {
		if !matches!(
			self.ty,
			SectionHeaderType::SymbolTable | SectionHeaderType::DynamicLinkerSymbols
		) {
			return None;
		}

		let bytes = self.read_section(reader).ok()?;
		Some(SymbolTableSection::read(&bytes, self.class, self.endian))
	}
}

/// A string table section, with strings and their offsets in the section.
#[derive(Debug)]
pub struct StringTableSection(Vec<(u64, String)>);

impl StringTableSection {
	fn read(bytes: &[u8]) -> io::Result<Self> {
		let mut start = 0;
		let mut build = String::new();
		let mut strings = Vec::new();

		for (i, byte) in bytes.iter().enumerate() {
			if *byte == 0 {
				strings.push((start as u64, build.clone()));
				build.clear();

				start = i + 1;
			} else {
				build.push(*byte as char);
			}
		}

		if !build.is_empty() {
			return Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("found non null terminated string in string table: `{}`", build),
			));
		}

		Ok(Self(strings))
	}

	/// Try get the string at the given offset, returning None if it doesn't exist.
	pub fn get_string_at_offset(&self, offset: u64) -> Option<&str> {
		let (str_offset, potential) = self
			.0
			.iter()
			.find(|(o, s)| offset >= *o && offset <= *o + s.len() as u64)?;

		let potential: &str = potential.as_ref();
		let split_at = (offset - str_offset) as usize;
		if split_at == 0 {
			Some(potential)
		} else {
			Some(potential.split_at(split_at).1)
		}
	}
}

#[derive(Debug)]
pub enum ElfSymbolType {
	None,
	Object,
	Func,
	Section,
	File,
	Common,
	ThreadLocal,
	OSSpecific(u8),
	ProcessorSpecific(u8),
}

impl TryFrom<u8> for ElfSymbolType {
	type Error = io::Error;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::None),
			1 => Ok(Self::Object),
			2 => Ok(Self::Func),
			3 => Ok(Self::Section),
			4 => Ok(Self::File),
			5 => Ok(Self::Common),
			6 => Ok(Self::ThreadLocal),
			10..=12 => Ok(Self::OSSpecific(value)),
			13..=15 => Ok(Self::ProcessorSpecific(value)),
			_ => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid elf symbol type: {}", value),
			)),
		}
	}
}

#[derive(Debug)]
pub enum ElfSymbolBinding {
	Local,
	Global,
	Weak,
	OSSpecific(u8),
	ProcessorSpecific(u8),
}

impl TryFrom<u8> for ElfSymbolBinding {
	type Error = io::Error;

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Self::Local),
			1 => Ok(Self::Global),
			2 => Ok(Self::Weak),
			10..=12 => Ok(Self::OSSpecific(value)),
			13..=15 => Ok(Self::ProcessorSpecific(value)),
			_ => Err(io::Error::new(
				ErrorKind::InvalidData,
				format!("invalid elf symbol binding: {}", value),
			)),
		}
	}
}

#[derive(Debug, ByteStruct)]
#[repr(u8)]
pub enum ElfSymbolVisibility {
	Default = 0,
	Internal = 1,
	Hidden = 2,
	Protected = 3,
}

#[derive(Debug)]
pub struct ElfSymbol {
	pub name_offset: u64,
	pub value: u64,
	pub size: u64,
	pub ty: ElfSymbolType,
	pub binding: ElfSymbolBinding,
	pub visibility: ElfSymbolVisibility,
	pub symbol_table_index: u64,
}

impl ElfSymbol {
	pub fn read_from_with_endian<T: io::Read>(source: &mut T, class: Class, endian: Endian) -> io::Result<Self> {
		let name_offset: u32;
		let value: u64;
		let size: u64;
		let info: u8;
		let visibility: ElfSymbolVisibility;
		let symbol_table_index: u64;

		match class {
			Class::ThirtyTwoBit => {
				name_offset = u32::read_from_with_endian(source, endian)?;
				value = class.read_value(source, endian)?;
				size = class.read_value(source, endian)?;
				info = u8::read_from(source)?;
				visibility = ElfSymbolVisibility::read_from_with_endian(source, endian)?;
				symbol_table_index = u16::read_from_with_endian(source, endian)? as u64;
			}
			Class::SixtyFourBit => {
				name_offset = u32::read_from_with_endian(source, endian)?;
				info = u8::read_from(source)?;
				visibility = ElfSymbolVisibility::read_from_with_endian(source, endian)?;
				symbol_table_index = u16::read_from_with_endian(source, endian)? as u64;
				value = class.read_value(source, endian)?;
				size = class.read_value(source, endian)?;
			}
		};

		let binding = ElfSymbolBinding::try_from(info >> 4)?;
		let ty = ElfSymbolType::try_from(info & 0xF)?;

		Ok(Self {
			name_offset: name_offset as u64,
			binding,
			size,
			symbol_table_index,
			ty,
			value,
			visibility,
		})
	}
}

#[derive(Debug)]
pub struct SymbolTableSection(Vec<ElfSymbol>);

impl SymbolTableSection {
	fn read(bytes: &[u8], class: Class, endian: Endian) -> io::Result<Self> {
		let mut source = Cursor::new(bytes);
		let mut symbols = Vec::new();
		loop {
			match ElfSymbol::read_from_with_endian(&mut source, class, endian) {
				Ok(symbol) => symbols.push(symbol),
				Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
				Err(e) => return Err(e),
			};
		}

		Ok(Self(symbols))
	}

	pub fn iter(&self) -> impl Iterator<Item = &ElfSymbol> {
		self.0.iter()
	}
}
