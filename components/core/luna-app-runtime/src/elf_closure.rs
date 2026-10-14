//! ELF inspection and dependency-closure planning for application launch.
//!
//! This module never invokes a host dynamic loader. It only parses ELF metadata
//! and resolves dependencies through an explicitly supplied resolver. The
//! filesystem resolver is constrained to trusted logical-to-physical sources;
//! it does not consult the host environment, `ld.so.cache`, or host defaults.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ElfClass {
    Class32,
    Class64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ElfEndian {
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ElfMachine {
    X86,
    X86_64,
    Arm,
    Aarch64,
    Riscv,
    Other(u16),
}

impl ElfMachine {
    fn from_raw(value: u16) -> Self {
        match value {
            3 => Self::X86,
            40 => Self::Arm,
            62 => Self::X86_64,
            183 => Self::Aarch64,
            243 => Self::Riscv,
            value => Self::Other(value),
        }
    }
}

impl fmt::Display for ElfMachine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86 => f.write_str("x86"),
            Self::X86_64 => f.write_str("x86_64"),
            Self::Arm => f.write_str("arm"),
            Self::Aarch64 => f.write_str("aarch64"),
            Self::Riscv => f.write_str("riscv"),
            Self::Other(value) => write!(f, "machine({value})"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElfMetadata {
    class: ElfClass,
    endian: ElfEndian,
    machine: ElfMachine,
    interpreter: Option<PathBuf>,
    needed: Vec<String>,
    rpath: Vec<String>,
    runpath: Vec<String>,
    search_path_tags: bool,
}

impl ElfMetadata {
    pub fn class(&self) -> ElfClass {
        self.class
    }

    pub fn endian(&self) -> ElfEndian {
        self.endian
    }

    pub fn machine(&self) -> ElfMachine {
        self.machine
    }

    pub fn interpreter(&self) -> Option<&Path> {
        self.interpreter.as_deref()
    }

    pub fn needed(&self) -> &[String] {
        &self.needed
    }

    pub fn rpath(&self) -> &[String] {
        &self.rpath
    }

    pub fn runpath(&self) -> &[String] {
        &self.runpath
    }

    /// Includes empty DT_RPATH/DT_RUNPATH values, which affect loader semantics.
    pub fn has_search_path_tags(&self) -> bool {
        self.search_path_tags
    }

    pub fn has_dynamic_loader(&self) -> bool {
        self.interpreter.is_some() || !self.needed.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ElfError {
    TooSmall,
    InvalidMagic,
    UnsupportedClass(u8),
    UnsupportedEndian(u8),
    MalformedHeader(&'static str),
    MalformedDynamicTable(&'static str),
    MalformedStringTable,
    InvalidInterpreterPath,
    InvalidDependencyName(String),
    UnsupportedDependencyPath(String),
    DependencyNotFound(String),
    ArchitectureMismatch {
        expected: ElfMachine,
        actual: ElfMachine,
        path: PathBuf,
    },
    Io(String),
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooSmall => f.write_str("ELF file is too small"),
            Self::InvalidMagic => f.write_str("invalid ELF magic"),
            Self::UnsupportedClass(value) => write!(f, "unsupported ELF class: {value}"),
            Self::UnsupportedEndian(value) => write!(f, "unsupported ELF endianness: {value}"),
            Self::MalformedHeader(field) => write!(f, "malformed ELF header: {field}"),
            Self::MalformedDynamicTable(field) => {
                write!(f, "malformed ELF dynamic table: {field}")
            }
            Self::MalformedStringTable => f.write_str("malformed ELF string table"),
            Self::InvalidInterpreterPath => f.write_str("ELF interpreter path is invalid"),
            Self::InvalidDependencyName(name) => {
                write!(f, "invalid ELF dependency name: {name}")
            }
            Self::UnsupportedDependencyPath(name) => {
                write!(f, "unsupported ELF dependency path: {name}")
            }
            Self::DependencyNotFound(name) => write!(f, "ELF dependency not found: {name}"),
            Self::ArchitectureMismatch {
                expected,
                actual,
                path,
            } => write!(
                f,
                "ELF architecture mismatch at {}: expected {expected}, got {actual}",
                path.display()
            ),
            Self::Io(message) => write!(f, "ELF I/O error: {message}"),
        }
    }
}

impl std::error::Error for ElfError {}

pub fn parse_elf(bytes: &[u8]) -> Result<ElfMetadata, ElfError> {
    if bytes.len() < 52 {
        return Err(ElfError::TooSmall);
    }
    if &bytes[0..4] != b"\x7fELF" {
        return Err(ElfError::InvalidMagic);
    }

    let class = match bytes[4] {
        1 => ElfClass::Class32,
        2 => ElfClass::Class64,
        other => return Err(ElfError::UnsupportedClass(other)),
    };
    let endian = match bytes[5] {
        1 => ElfEndian::Little,
        2 => ElfEndian::Big,
        other => return Err(ElfError::UnsupportedEndian(other)),
    };
    let machine = ElfMachine::from_raw(read_u16(bytes, 18, endian)?);
    let (phoff, phentsize, phnum) = match class {
        ElfClass::Class32 => (
            read_u32(bytes, 28, endian)? as u64,
            read_u16(bytes, 42, endian)? as u64,
            read_u16(bytes, 44, endian)? as u64,
        ),
        ElfClass::Class64 => (
            read_u64(bytes, 32, endian)?,
            read_u16(bytes, 54, endian)? as u64,
            read_u16(bytes, 56, endian)? as u64,
        ),
    };

    if phentsize == 0 || phnum == 0 {
        return Ok(ElfMetadata {
            class,
            endian,
            machine,
            interpreter: None,
            needed: Vec::new(),
            rpath: Vec::new(),
            runpath: Vec::new(),
            search_path_tags: false,
        });
    }

    let minimum_phentsize = match class {
        ElfClass::Class32 => 32,
        ElfClass::Class64 => 56,
    };
    if phentsize < minimum_phentsize {
        return Err(ElfError::MalformedHeader("program-header entry size"));
    }
    let ph_table_size = phentsize
        .checked_mul(phnum)
        .ok_or(ElfError::MalformedHeader("program-header size overflow"))?;
    range(bytes, phoff, ph_table_size)?;

    let mut segments = Vec::with_capacity(phnum as usize);
    let mut interpreter = None;
    let mut interpreter_range = None;
    let mut dynamic = None;
    let mut phdr = None;

    for index in 0..phnum {
        let base = phoff + index * phentsize;
        let p_type = read_u32(bytes, base as usize, endian)?;
        let (offset, vaddr, filesz, memsz) = match class {
            ElfClass::Class32 => (
                read_u32(bytes, base as usize + 4, endian)? as u64,
                read_u32(bytes, base as usize + 8, endian)? as u64,
                read_u32(bytes, base as usize + 16, endian)? as u64,
                read_u32(bytes, base as usize + 20, endian)? as u64,
            ),
            ElfClass::Class64 => (
                read_u64(bytes, base as usize + 8, endian)?,
                read_u64(bytes, base as usize + 16, endian)?,
                read_u64(bytes, base as usize + 32, endian)?,
                read_u64(bytes, base as usize + 40, endian)?,
            ),
        };
        range(bytes, offset, filesz)?;

        match p_type {
            1 => {
                if filesz > memsz || vaddr.checked_add(memsz).is_none() {
                    return Err(ElfError::MalformedHeader("PT_LOAD memory range"));
                }
                segments.push(Segment {
                    vaddr,
                    offset,
                    filesz,
                    memsz,
                });
            }
            2 => {
                if dynamic.replace((offset, vaddr, filesz)).is_some() {
                    return Err(ElfError::MalformedHeader("repeated PT_DYNAMIC"));
                }
            }
            3 => {
                if interpreter.is_some() {
                    return Err(ElfError::MalformedHeader("repeated PT_INTERP"));
                }
                let raw = slice(bytes, offset, filesz)?;
                let nul = raw
                    .iter()
                    .position(|byte| *byte == 0)
                    .ok_or(ElfError::InvalidInterpreterPath)?;
                let path = std::str::from_utf8(&raw[..nul])
                    .map_err(|_| ElfError::InvalidInterpreterPath)?;
                let path = PathBuf::from(path);
                if !path.is_absolute() || has_navigation_components(&path) {
                    return Err(ElfError::InvalidInterpreterPath);
                }
                interpreter = Some(path);
                interpreter_range = Some((offset, vaddr, filesz));
            }
            6 if phdr.replace((offset, vaddr, filesz, memsz)).is_some() => {
                return Err(ElfError::MalformedHeader("repeated PT_PHDR"));
            }
            _ => {}
        }
    }

    let Some((dynamic_offset, dynamic_vaddr, dynamic_size)) = dynamic else {
        if interpreter.is_some() {
            return Err(ElfError::MalformedHeader("PT_INTERP without PT_DYNAMIC"));
        }
        return Ok(ElfMetadata {
            class,
            endian,
            machine,
            interpreter,
            needed: Vec::new(),
            rpath: Vec::new(),
            runpath: Vec::new(),
            search_path_tags: false,
        });
    };

    // The accepted native target has 4 KiB ELF mapping pages. Other machines
    // retain generic metadata parsing, not a claim about their kernel loader.
    let load_page_size = (machine == ElfMachine::X86_64).then_some(4096);
    if load_page_size.is_some() {
        validate_mapped_file_range(&segments, 0, 64, load_page_size)?;
        let phdr_address =
            validate_mapped_file_range(&segments, phoff, ph_table_size, load_page_size)?;
        if let Some((offset, address, filesz, memsz)) = phdr
            && (offset != phoff
                || address != phdr_address
                || filesz != ph_table_size
                || memsz != ph_table_size)
        {
            return Err(ElfError::MalformedHeader(
                "PT_PHDR does not match loaded table",
            ));
        }
        if let Some((offset, address, size)) = interpreter_range
            && vaddr_to_offset(&segments, address, size, load_page_size)? != offset
        {
            return Err(ElfError::MalformedHeader(
                "PT_INTERP does not match PT_LOAD",
            ));
        }
    }
    if vaddr_to_offset(&segments, dynamic_vaddr, dynamic_size, load_page_size)? != dynamic_offset {
        return Err(ElfError::MalformedHeader(
            "PT_DYNAMIC does not match PT_LOAD",
        ));
    }
    let entry_size = match class {
        ElfClass::Class32 => 8,
        ElfClass::Class64 => 16,
    } as u64;
    if dynamic_size % entry_size != 0 {
        return Err(ElfError::MalformedDynamicTable("entry alignment"));
    }

    let mut strtab_vaddr = None;
    let mut strtab_size = None;
    let mut needed_offsets = Vec::new();
    let mut rpath_offset = None;
    let mut runpath_offset = None;

    let count = dynamic_size / entry_size;
    let mut terminated = false;
    for index in 0..count {
        let base = dynamic_offset + index * entry_size;
        let (tag, value) = match class {
            ElfClass::Class32 => (
                read_i32(bytes, base as usize, endian)? as i64,
                read_u32(bytes, base as usize + 4, endian)? as u64,
            ),
            ElfClass::Class64 => (
                read_i64(bytes, base as usize, endian)?,
                read_u64(bytes, base as usize + 8, endian)?,
            ),
        };

        match tag {
            0 => {
                terminated = true;
                break;
            }
            1 => needed_offsets.push(value),
            5 => strtab_vaddr = Some(value),
            10 => strtab_size = Some(value),
            15 | 29 => {
                let slot = if tag == 15 {
                    &mut rpath_offset
                } else {
                    &mut runpath_offset
                };
                match slot.replace(value) {
                    None => {}
                    Some(_) => {
                        return Err(ElfError::MalformedDynamicTable("repeated search path tag"));
                    }
                }
            }
            _ => {}
        }
    }
    if !terminated {
        return Err(ElfError::MalformedDynamicTable("missing DT_NULL"));
    }

    let Some(strtab_vaddr) = strtab_vaddr else {
        if needed_offsets.is_empty() && rpath_offset.is_none() && runpath_offset.is_none() {
            return Ok(ElfMetadata {
                class,
                endian,
                machine,
                interpreter,
                needed: Vec::new(),
                rpath: Vec::new(),
                runpath: Vec::new(),
                search_path_tags: false,
            });
        }
        return Err(ElfError::MalformedDynamicTable("missing DT_STRTAB"));
    };
    let strtab_size = strtab_size.ok_or(ElfError::MalformedDynamicTable("missing DT_STRSZ"))?;
    let strtab_offset = vaddr_to_offset(&segments, strtab_vaddr, strtab_size, load_page_size)?;
    let strtab = slice(bytes, strtab_offset, strtab_size)?;

    let mut needed = Vec::with_capacity(needed_offsets.len());
    for offset in needed_offsets {
        needed.push(read_dyn_string(strtab, offset)?);
    }
    let rpath = match rpath_offset {
        Some(offset) => split_search_path(&read_dyn_string(strtab, offset)?),
        None => Vec::new(),
    };
    let runpath = match runpath_offset {
        Some(offset) => split_search_path(&read_dyn_string(strtab, offset)?),
        None => Vec::new(),
    };

    Ok(ElfMetadata {
        class,
        endian,
        machine,
        interpreter,
        needed,
        rpath,
        runpath,
        search_path_tags: rpath_offset.is_some() || runpath_offset.is_some(),
    })
}

fn split_search_path(path: &str) -> Vec<String> {
    path.split(':')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

struct Segment {
    vaddr: u64,
    offset: u64,
    filesz: u64,
    memsz: u64,
}

fn validate_mapped_file_range(
    segments: &[Segment],
    offset: u64,
    size: u64,
    page_size: Option<u64>,
) -> Result<u64, ElfError> {
    let end = offset
        .checked_add(size)
        .ok_or(ElfError::MalformedHeader("metadata file range"))?;
    let mut found = None;
    for segment in segments {
        let file_end = segment
            .offset
            .checked_add(segment.filesz)
            .ok_or(ElfError::MalformedHeader("PT_LOAD file range"))?;
        if offset >= segment.offset && offset < file_end {
            // Linux selects the last LOAD containing e_phoff's first byte.
            // Reject short or distinct aliases rather than selecting another.
            if end > file_end {
                return Err(ElfError::MalformedHeader("partially mapped ELF metadata"));
            }
            let address = segment
                .vaddr
                .checked_add(offset - segment.offset)
                .ok_or(ElfError::MalformedHeader("metadata address"))?;
            if found.is_some_and(|prior| prior != address) {
                return Err(ElfError::MalformedHeader("distinct ELF metadata aliases"));
            }
            if vaddr_to_offset(segments, address, size, page_size)? != offset {
                return Err(ElfError::MalformedHeader(
                    "mapped metadata differs from file",
                ));
            }
            found = Some(address);
        }
    }
    found.ok_or(ElfError::MalformedHeader("unmapped ELF metadata"))
}

fn vaddr_to_offset(
    segments: &[Segment],
    vaddr: u64,
    size: u64,
    page_size: Option<u64>,
) -> Result<u64, ElfError> {
    let requested_end = vaddr
        .checked_add(size)
        .ok_or(ElfError::MalformedStringTable)?;
    let mut translated = None;
    for segment in segments {
        let end = segment
            .vaddr
            .checked_add(segment.filesz)
            .ok_or(ElfError::MalformedDynamicTable("segment overflow"))?;
        if vaddr >= segment.vaddr && requested_end <= end {
            let offset = segment
                .offset
                .checked_add(vaddr - segment.vaddr)
                .ok_or(ElfError::MalformedStringTable)?;
            if translated.is_some_and(|prior| prior != offset) {
                return Err(ElfError::MalformedHeader("ambiguous PT_LOAD translation"));
            }
            translated = Some(offset);
        } else if segment.vaddr < requested_end && vaddr < end {
            // A partly overlapping LOAD could replace part of the metadata.
            return Err(ElfError::MalformedHeader(
                "partly overlapping PT_LOAD metadata",
            ));
        }
    }
    let translated = translated.ok_or(ElfError::MalformedStringTable)?;
    if let Some(page_size) = page_size {
        let mask = page_size - 1;
        let page_end = |address: u64| {
            address
                .checked_add(mask)
                .map(|value| value & !mask)
                .ok_or(ElfError::MalformedHeader("PT_LOAD page range"))
        };
        for segment in segments {
            let page_start = segment.vaddr & !mask;
            let file_end = segment
                .vaddr
                .checked_add(segment.filesz)
                .ok_or(ElfError::MalformedHeader("PT_LOAD file range"))?;
            if segment.offset & mask != segment.vaddr & mask {
                return Err(ElfError::MalformedHeader("PT_LOAD page alignment"));
            }
            if segment.filesz != 0 {
                let mapped_end = page_end(file_end)?;
                if page_start < requested_end && vaddr < mapped_end {
                    let overlap = vaddr.max(page_start);
                    let actual = (segment.offset - (segment.vaddr & mask))
                        .checked_add(overlap - page_start)
                        .ok_or(ElfError::MalformedHeader("PT_LOAD page translation"))?;
                    let expected = translated
                        .checked_add(overlap - vaddr)
                        .ok_or(ElfError::MalformedHeader("metadata page translation"))?;
                    if actual != expected {
                        return Err(ElfError::MalformedHeader(
                            "overlapping PT_LOAD metadata page",
                        ));
                    }
                }
            }
            if segment.memsz > segment.filesz {
                let zero_start = if segment.filesz == 0 {
                    page_start
                } else {
                    file_end
                };
                let zero_end = page_end(segment.vaddr + segment.memsz)?;
                if zero_start < requested_end && vaddr < zero_end {
                    return Err(ElfError::MalformedHeader("PT_LOAD BSS overwrites metadata"));
                }
            }
        }
    }
    Ok(translated)
}

fn read_dyn_string(table: &[u8], offset: u64) -> Result<String, ElfError> {
    let offset = usize::try_from(offset).map_err(|_| ElfError::MalformedStringTable)?;
    if offset >= table.len() {
        return Err(ElfError::MalformedStringTable);
    }
    let rest = &table[offset..];
    let nul = rest
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(ElfError::MalformedStringTable)?;
    let value = std::str::from_utf8(&rest[..nul]).map_err(|_| ElfError::MalformedStringTable)?;
    Ok(value.to_owned())
}

fn range(bytes: &[u8], offset: u64, size: u64) -> Result<(), ElfError> {
    let start =
        usize::try_from(offset).map_err(|_| ElfError::MalformedHeader("offset overflow"))?;
    let size = usize::try_from(size).map_err(|_| ElfError::MalformedHeader("size overflow"))?;
    start
        .checked_add(size)
        .filter(|end| *end <= bytes.len())
        .ok_or(ElfError::MalformedHeader("out of bounds"))?;
    Ok(())
}

fn slice(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], ElfError> {
    range(bytes, offset, size)?;
    let start = offset as usize;
    let end = start + size as usize;
    Ok(&bytes[start..end])
}

fn read_u16(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<u16, ElfError> {
    let raw = bytes.get(offset..offset + 2).ok_or(ElfError::TooSmall)?;
    Ok(match endian {
        ElfEndian::Little => u16::from_le_bytes([raw[0], raw[1]]),
        ElfEndian::Big => u16::from_be_bytes([raw[0], raw[1]]),
    })
}

fn read_u32(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<u32, ElfError> {
    let raw = bytes.get(offset..offset + 4).ok_or(ElfError::TooSmall)?;
    Ok(match endian {
        ElfEndian::Little => u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]),
        ElfEndian::Big => u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
    })
}

fn read_i32(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<i32, ElfError> {
    Ok(read_u32(bytes, offset, endian)? as i32)
}

fn read_u64(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<u64, ElfError> {
    let raw = bytes.get(offset..offset + 8).ok_or(ElfError::TooSmall)?;
    Ok(match endian {
        ElfEndian::Little => u64::from_le_bytes([
            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
        ]),
        ElfEndian::Big => u64::from_be_bytes([
            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
        ]),
    })
}

fn read_i64(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<i64, ElfError> {
    Ok(read_u64(bytes, offset, endian)? as i64)
}

fn has_navigation_components(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedElfSource {
    logical_prefix: PathBuf,
    physical_root: PathBuf,
}

impl TrustedElfSource {
    pub fn new(
        logical_prefix: impl Into<PathBuf>,
        physical_root: impl Into<PathBuf>,
    ) -> Result<Self, ElfError> {
        let logical_prefix = logical_prefix.into();
        let physical_root = physical_root.into();
        validate_absolute_normalized(&logical_prefix)?;
        validate_absolute_normalized(&physical_root)?;
        Ok(Self {
            logical_prefix,
            physical_root,
        })
    }

    pub fn logical_prefix(&self) -> &Path {
        &self.logical_prefix
    }

    pub fn physical_root(&self) -> &Path {
        &self.physical_root
    }
}

#[derive(Clone, Debug, Default)]
pub struct FilesystemElfResolver {
    sources: Vec<TrustedElfSource>,
    default_search_paths: Vec<PathBuf>,
}

impl FilesystemElfResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_source(mut self, source: TrustedElfSource) -> Self {
        self.sources.push(source);
        self.sources
            .sort_by(|left, right| right.logical_prefix.cmp(&left.logical_prefix));
        self
    }

    pub fn with_default_search_path(mut self, path: impl Into<PathBuf>) -> Result<Self, ElfError> {
        let path = path.into();
        validate_absolute_normalized(&path)?;
        self.default_search_paths.push(path);
        Ok(self)
    }

    fn physical_for_logical(&self, logical: &Path) -> Result<PathBuf, ElfError> {
        for source in &self.sources {
            if logical == source.logical_prefix || logical.starts_with(&source.logical_prefix) {
                let relative = logical
                    .strip_prefix(&source.logical_prefix)
                    .map_err(|_| ElfError::DependencyNotFound(logical.display().to_string()))?;
                // Joining an empty suffix adds a trailing slash, which turns
                // an exact-file source into ENOTDIR during canonicalization.
                let candidate = if relative.as_os_str().is_empty() {
                    source.physical_root.clone()
                } else {
                    source.physical_root.join(relative)
                };
                let canonical = fs::canonicalize(&candidate)
                    .map_err(|error| ElfError::Io(error.to_string()))?;
                let trusted_root = fs::canonicalize(&source.physical_root)
                    .map_err(|error| ElfError::Io(error.to_string()))?;
                if !canonical.starts_with(&trusted_root) {
                    return Err(ElfError::UnsupportedDependencyPath(
                        logical.display().to_string(),
                    ));
                }
                return Ok(canonical);
            }
        }
        Err(ElfError::DependencyNotFound(logical.display().to_string()))
    }

    fn logical_for_absolute(&self, logical: &Path) -> Result<PathBuf, ElfError> {
        validate_absolute_normalized(logical)?;
        self.physical_for_logical(logical)
            .map(|_| logical.to_path_buf())
    }

    fn expand_search_entry(entry: &str, origin: &Path) -> Result<PathBuf, ElfError> {
        let replaced = entry.replace("${ORIGIN}", origin.to_string_lossy().as_ref());
        let replaced = replaced.replace("$ORIGIN", origin.to_string_lossy().as_ref());
        normalize_absolute_path(Path::new(&replaced))
    }
}

impl ElfDependencyResolver for FilesystemElfResolver {
    fn inspect(&self, path: &Path) -> Result<ElfMetadata, ElfError> {
        let physical = self.physical_for_logical(path)?;
        let bytes = fs::read(physical).map_err(|error| ElfError::Io(error.to_string()))?;
        parse_elf(&bytes)
    }

    fn resolve_interpreter(&self, interpreter: &Path) -> Result<PathBuf, ElfError> {
        self.logical_for_absolute(interpreter)
    }

    fn resolve_needed(
        &self,
        requester: &Path,
        metadata: &ElfMetadata,
        name: &str,
    ) -> Result<PathBuf, ElfError> {
        validate_dependency_name(name)?;
        if name.starts_with('/') {
            return self.logical_for_absolute(Path::new(name));
        }
        if name.contains('/') {
            return Err(ElfError::UnsupportedDependencyPath(name.to_owned()));
        }

        let origin = requester.parent().unwrap_or_else(|| Path::new("/"));
        if !metadata.runpath.is_empty() {
            for entry in &metadata.runpath {
                let directory = Self::expand_search_entry(entry, origin)?;
                let candidate = directory.join(name);
                if self.physical_for_logical(&candidate).is_ok() {
                    return Ok(candidate);
                }
            }
        } else {
            for entry in &metadata.rpath {
                let directory = Self::expand_search_entry(entry, origin)?;
                let candidate = directory.join(name);
                if self.physical_for_logical(&candidate).is_ok() {
                    return Ok(candidate);
                }
            }
        }

        for directory in &self.default_search_paths {
            let candidate = directory.join(name);
            if self.physical_for_logical(&candidate).is_ok() {
                return Ok(candidate);
            }
        }

        Err(ElfError::DependencyNotFound(name.to_owned()))
    }
}

fn validate_absolute_normalized(path: &Path) -> Result<(), ElfError> {
    if !path.is_absolute() || has_navigation_components(path) {
        return Err(ElfError::UnsupportedDependencyPath(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, ElfError> {
    if !path.is_absolute() {
        return Err(ElfError::UnsupportedDependencyPath(
            path.display().to_string(),
        ));
    }
    let mut normalized = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ElfError::UnsupportedDependencyPath(
                        path.display().to_string(),
                    ));
                }
            }
            Component::Normal(value) => normalized.push(value),
            Component::Prefix(_) => {
                return Err(ElfError::UnsupportedDependencyPath(
                    path.display().to_string(),
                ));
            }
        }
    }
    Ok(normalized)
}

fn validate_dependency_name(name: &str) -> Result<(), ElfError> {
    if name.is_empty() || name.contains('\0') {
        return Err(ElfError::InvalidDependencyName(name.to_owned()));
    }
    if name
        .split('/')
        .any(|component| component == "." || component == "..")
    {
        return Err(ElfError::InvalidDependencyName(name.to_owned()));
    }
    Ok(())
}

pub trait ElfDependencyResolver {
    fn inspect(&self, path: &Path) -> Result<ElfMetadata, ElfError>;
    fn resolve_interpreter(&self, interpreter: &Path) -> Result<PathBuf, ElfError>;
    fn resolve_needed(
        &self,
        requester: &Path,
        metadata: &ElfMetadata,
        name: &str,
    ) -> Result<PathBuf, ElfError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElfDependencyNode {
    path: PathBuf,
    metadata: ElfMetadata,
}

impl ElfDependencyNode {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn metadata(&self) -> &ElfMetadata {
        &self.metadata
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElfDependencyClosure {
    root: PathBuf,
    nodes: BTreeMap<PathBuf, ElfDependencyNode>,
}

impl ElfDependencyClosure {
    pub fn build(
        root: impl Into<PathBuf>,
        resolver: &impl ElfDependencyResolver,
    ) -> Result<Self, ElfError> {
        let root = root.into();
        validate_absolute_normalized(&root)?;
        let root_metadata = resolver.inspect(&root)?;
        let expected_machine = root_metadata.machine();
        let mut closure = Self {
            root: root.clone(),
            nodes: BTreeMap::new(),
        };
        let mut scheduled = BTreeSet::new();
        scheduled.insert(root.clone());
        let mut queue = vec![(root, root_metadata)];

        while let Some((path, metadata)) = queue.pop() {
            if closure.nodes.contains_key(&path) {
                continue;
            }
            if metadata.machine() != expected_machine {
                return Err(ElfError::ArchitectureMismatch {
                    expected: expected_machine,
                    actual: metadata.machine(),
                    path,
                });
            }

            if let Some(interpreter) = metadata.interpreter() {
                let interpreter = resolver.resolve_interpreter(interpreter)?;
                if scheduled.insert(interpreter.clone()) {
                    let interpreter_metadata = resolver.inspect(&interpreter)?;
                    queue.push((interpreter, interpreter_metadata));
                }
            }

            for dependency in metadata.needed() {
                let resolved = resolver.resolve_needed(&path, &metadata, dependency)?;
                if scheduled.insert(resolved.clone()) {
                    let dependency_metadata = resolver.inspect(&resolved)?;
                    queue.push((resolved, dependency_metadata));
                }
            }

            closure
                .nodes
                .insert(path.clone(), ElfDependencyNode { path, metadata });
        }

        Ok(closure)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ElfDependencyNode> {
        self.nodes.values()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct MemoryResolver {
        files: BTreeMap<PathBuf, Vec<u8>>,
        needed: BTreeMap<(PathBuf, String), PathBuf>,
        interpreters: BTreeMap<PathBuf, PathBuf>,
    }

    impl ElfDependencyResolver for MemoryResolver {
        fn inspect(&self, path: &Path) -> Result<ElfMetadata, ElfError> {
            let bytes = self
                .files
                .get(path)
                .ok_or_else(|| ElfError::DependencyNotFound(path.display().to_string()))?;
            parse_elf(bytes)
        }

        fn resolve_interpreter(&self, interpreter: &Path) -> Result<PathBuf, ElfError> {
            self.interpreters
                .get(interpreter)
                .cloned()
                .ok_or_else(|| ElfError::DependencyNotFound(interpreter.display().to_string()))
        }

        fn resolve_needed(
            &self,
            requester: &Path,
            _metadata: &ElfMetadata,
            name: &str,
        ) -> Result<PathBuf, ElfError> {
            self.needed
                .get(&(requester.to_path_buf(), name.to_owned()))
                .cloned()
                .ok_or_else(|| ElfError::DependencyNotFound(name.to_owned()))
        }
    }

    fn fixture(needed: &[&str], interpreter: Option<&str>) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x700];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        write_u16(&mut bytes, 16, 3);
        write_u16(&mut bytes, 18, 62);
        write_u64(&mut bytes, 32, 64);
        write_u16(&mut bytes, 54, 56);
        write_u16(&mut bytes, 56, 3);

        phdr(&mut bytes, 64, 1, 0, 0x400000, 0x700);
        if let Some(interpreter) = interpreter {
            let raw = interpreter.as_bytes();
            bytes[0x200..0x200 + raw.len()].copy_from_slice(raw);
            bytes[0x200 + raw.len()] = 0;
            phdr(&mut bytes, 120, 3, 0x200, 0x400200, (raw.len() + 1) as u64);
        }
        phdr(&mut bytes, 176, 2, 0x300, 0x400300, 0x100);

        let mut strtab = b"\0".to_vec();
        let mut offsets = BTreeMap::new();
        for name in needed {
            offsets.insert(*name, strtab.len() as u64);
            strtab.extend_from_slice(name.as_bytes());
            strtab.push(0);
        }
        bytes[0x500..0x500 + strtab.len()].copy_from_slice(&strtab);

        write_u64(&mut bytes, 0x300, 5);
        write_u64(&mut bytes, 0x308, 0x400500);
        write_u64(&mut bytes, 0x310, 10);
        write_u64(&mut bytes, 0x318, strtab.len() as u64);
        let mut at = 0x320;
        for name in needed {
            write_u64(&mut bytes, at, 1);
            write_u64(&mut bytes, at + 8, offsets[name]);
            at += 16;
        }
        write_u64(&mut bytes, at, 0);
        write_u64(&mut bytes, at + 8, 0);
        bytes
    }

    fn phdr(bytes: &mut [u8], offset: usize, kind: u32, file_offset: u64, vaddr: u64, filesz: u64) {
        write_u32(bytes, offset, kind);
        write_u64(bytes, offset + 8, file_offset);
        write_u64(bytes, offset + 16, vaddr);
        write_u64(bytes, offset + 32, filesz);
        write_u64(bytes, offset + 40, filesz);
    }

    fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn empty_search_path_tags_remain_visible_and_duplicates_fail_closed() {
        for tag in [15, 29] {
            let mut bytes = fixture(&[], None);
            write_u64(&mut bytes, 0x320, tag);
            write_u64(&mut bytes, 0x328, 0);
            let elf = parse_elf(&bytes).unwrap();
            assert!(elf.has_search_path_tags());
            assert!(elf.rpath().is_empty() && elf.runpath().is_empty());
            write_u64(&mut bytes, 0x330, tag);
            assert!(matches!(
                parse_elf(&bytes),
                Err(super::ElfError::MalformedDynamicTable(_))
            ));
        }
    }

    #[test]
    fn duplicate_program_headers_and_inconsistent_dynamic_translation_fail_closed() {
        let mut duplicate_interp = fixture(&[], Some("/lib/ld-musl-x86_64.so.1"));
        phdr(&mut duplicate_interp, 176, 3, 0x200, 0x400200, 26);
        assert!(matches!(
            parse_elf(&duplicate_interp),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut duplicate_dynamic = fixture(&[], None);
        phdr(&mut duplicate_dynamic, 120, 2, 0x300, 0x400300, 0x100);
        assert!(matches!(
            parse_elf(&duplicate_dynamic),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut wrong_address = fixture(&[], None);
        write_u64(&mut wrong_address, 176 + 16, 0x400310);
        assert!(matches!(
            parse_elf(&wrong_address),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut ambiguous = fixture(&[], None);
        phdr(&mut ambiguous, 120, 1, 0x500, 0x400300, 0x100);
        assert!(matches!(
            parse_elf(&ambiguous),
            Err(super::ElfError::MalformedHeader(_))
        ));
    }

    #[test]
    fn page_rounded_load_aliases_and_bss_cannot_replace_metadata() {
        let base = || super::Segment {
            vaddr: 0x400000,
            offset: 0,
            filesz: 0x2000,
            memsz: 0x2000,
        };
        for alias in [
            // No byte-range overlap; MAP_FIXED replaces the whole metadata page.
            super::Segment {
                vaddr: 0x400f00,
                offset: 0x1f00,
                filesz: 0x100,
                memsz: 0x100,
            },
            // Same file translation, but the trailing BSS clears the metadata.
            super::Segment {
                vaddr: 0x400100,
                offset: 0x100,
                filesz: 0x100,
                memsz: 0x300,
            },
            // A pure BSS LOAD starts at the page floor, before its p_vaddr.
            super::Segment {
                vaddr: 0x400f00,
                offset: 0xf00,
                filesz: 0,
                memsz: 0x100,
            },
        ] {
            assert!(super::vaddr_to_offset(&[base(), alias], 0x400300, 0x100, Some(4096)).is_err());
        }
        let same = super::Segment {
            vaddr: 0x400f00,
            offset: 0xf00,
            filesz: 0x100,
            memsz: 0x100,
        };
        assert_eq!(
            super::vaddr_to_offset(&[base(), same], 0x400300, 0x100, Some(4096)).unwrap(),
            0x300
        );
    }

    #[test]
    fn mapped_header_pages_and_invalid_load_memory_fail_closed() {
        let mut bytes = fixture(&[], None);
        let dynamic = bytes[0x300..0x400].to_vec();
        let strings = bytes[0x500..0x700].to_vec();
        bytes.resize(0x3000, 0);
        bytes[0x1300..0x1400].copy_from_slice(&dynamic);
        bytes[0x1500..0x1700].copy_from_slice(&strings);
        write_u64(&mut bytes, 0x1308, 0x401500);
        phdr(&mut bytes, 64, 1, 0, 0x400000, 0x1700);
        phdr(&mut bytes, 176, 2, 0x1300, 0x401300, 0x100);
        assert!(parse_elf(&bytes).is_ok());
        // This replaces only the ELF/program-header page, not dynamic/strtab.
        phdr(&mut bytes, 120, 1, 0x2f00, 0x400f00, 0x100);
        assert!(matches!(
            parse_elf(&bytes),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut bad_memory = fixture(&[], None);
        write_u64(&mut bad_memory, 64 + 40, 0x100);
        assert!(matches!(
            parse_elf(&bad_memory),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut unaligned = fixture(&[], None);
        phdr(&mut unaligned, 120, 1, 0x600, 0x400f00, 0x100);
        assert!(matches!(
            parse_elf(&unaligned),
            Err(super::ElfError::MalformedHeader(_))
        ));
    }

    #[test]
    fn phdr_aliases_and_forged_musl_base_fail_closed() {
        let mut bytes = fixture(&[], None);
        phdr(&mut bytes, 120, 6, 64, 0x400040, 3 * 56);
        assert!(parse_elf(&bytes).is_ok());
        write_u64(&mut bytes, 120 + 16, 0x400048);
        assert!(matches!(
            parse_elf(&bytes),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut duplicate = fixture(&[], None);
        phdr(&mut duplicate, 120, 6, 64, 0x400040, 3 * 56);
        phdr(&mut duplicate, 176, 6, 64, 0x400040, 3 * 56);
        assert!(matches!(
            parse_elf(&duplicate),
            Err(super::ElfError::MalformedHeader(_))
        ));
        for size in [0x20, 3 * 56] {
            let mut alias = fixture(&[], None);
            phdr(&mut alias, 120, 1, 64, 0x402040, size);
            assert!(matches!(
                parse_elf(&alias),
                Err(super::ElfError::MalformedHeader(_))
            ));
        }
    }

    #[test]
    fn dynamic_requires_in_range_sentinel_and_mapped_interpreter() {
        let mut missing_null = fixture(&[], None);
        for at in (0x320..0x400).step_by(16) {
            write_u64(&mut missing_null, at, 21);
        }
        assert!(matches!(
            parse_elf(&missing_null),
            Err(super::ElfError::MalformedDynamicTable(_))
        ));
        let mut interpreter = fixture(&[], Some("/lib/ld-musl-x86_64.so.1"));
        write_u64(&mut interpreter, 120 + 16, 0x400210);
        assert!(matches!(
            parse_elf(&interpreter),
            Err(super::ElfError::MalformedHeader(_))
        ));
        let mut no_dynamic = fixture(&[], Some("/lib/ld-musl-x86_64.so.1"));
        write_u32(&mut no_dynamic, 176, 0);
        assert!(matches!(
            parse_elf(&no_dynamic),
            Err(super::ElfError::MalformedHeader(_))
        ));
    }

    #[test]
    fn parser_extracts_loader_and_needed_entries() {
        let bytes = fixture(
            &["libc.so.6", "libm.so.6"],
            Some("/lib64/ld-luna-x86-64.so.1"),
        );
        let metadata = parse_elf(&bytes).unwrap();
        assert_eq!(metadata.machine(), ElfMachine::X86_64);
        assert_eq!(metadata.class(), ElfClass::Class64);
        assert_eq!(
            metadata.interpreter(),
            Some(Path::new("/lib64/ld-luna-x86-64.so.1"))
        );
        assert_eq!(metadata.needed(), ["libc.so.6", "libm.so.6"]);
    }

    #[test]
    fn static_elf_has_empty_dynamic_closure_inputs() {
        let bytes = fixture(&[], None);
        let metadata = parse_elf(&bytes).unwrap();
        assert!(!metadata.has_dynamic_loader());
        assert!(metadata.needed().is_empty());
        assert!(metadata.interpreter().is_none());
    }

    #[test]
    fn dependency_closure_is_recursive_and_deterministic() {
        let root = PathBuf::from("/bin/app");
        let libc = PathBuf::from("/lib/libc.so.6");
        let ld = PathBuf::from("/lib64/ld.so");
        let mut resolver = MemoryResolver::default();
        resolver
            .files
            .insert(root.clone(), fixture(&["libc.so.6"], Some("/lib64/ld.so")));
        resolver
            .files
            .insert(libc.clone(), fixture(&["libm.so.6"], None));
        resolver
            .files
            .insert(PathBuf::from("/lib/libm.so.6"), fixture(&[], None));
        resolver.files.insert(ld.clone(), fixture(&[], None));
        resolver
            .interpreters
            .insert(PathBuf::from("/lib64/ld.so"), ld);
        resolver
            .needed
            .insert((root.clone(), "libc.so.6".into()), libc);
        resolver.needed.insert(
            (PathBuf::from("/lib/libc.so.6"), "libm.so.6".into()),
            PathBuf::from("/lib/libm.so.6"),
        );

        let closure = ElfDependencyClosure::build(root.clone(), &resolver).unwrap();
        assert_eq!(closure.root(), root);
        assert_eq!(closure.len(), 4);
        let paths = closure
            .nodes()
            .map(|node| node.path().to_owned())
            .collect::<Vec<_>>();
        assert!(paths.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn dependency_cycles_do_not_loop_forever() {
        let root = PathBuf::from("/bin/app");
        let lib = PathBuf::from("/lib/libloop.so");
        let mut resolver = MemoryResolver::default();
        resolver
            .files
            .insert(root.clone(), fixture(&["libloop.so"], None));
        resolver.files.insert(lib.clone(), fixture(&["app"], None));
        resolver
            .files
            .insert(PathBuf::from("/bin/app2"), fixture(&[], None));
        resolver
            .needed
            .insert((root.clone(), "libloop.so".into()), lib.clone());
        resolver.needed.insert((lib, "app".into()), root.clone());

        let closure = ElfDependencyClosure::build(root, &resolver).unwrap();
        assert_eq!(closure.len(), 2);
    }

    #[test]
    fn architecture_mismatch_fails_closed() {
        let root = PathBuf::from("/bin/app");
        let other = PathBuf::from("/lib/libx.so");
        let mut resolver = MemoryResolver::default();
        resolver
            .files
            .insert(root.clone(), fixture(&["libx.so"], None));
        let mut bytes = fixture(&[], None);
        write_u16(&mut bytes, 18, 183);
        resolver.files.insert(other.clone(), bytes);
        resolver.needed.insert((root, "libx.so".into()), other);
        assert!(matches!(
            ElfDependencyClosure::build("/bin/app", &resolver),
            Err(ElfError::ArchitectureMismatch { .. })
        ));
    }

    #[test]
    fn origin_search_path_is_normalized_inside_absolute_namespace() {
        let result = FilesystemElfResolver::expand_search_entry(
            "$ORIGIN/../lib",
            Path::new("/apps/example/bin"),
        )
        .unwrap();
        assert_eq!(result, Path::new("/apps/example/lib"));
    }

    #[test]
    fn traversal_beyond_logical_root_is_rejected() {
        let result =
            FilesystemElfResolver::expand_search_entry("$ORIGIN/../../..", Path::new("/apps"));
        assert!(result.is_err());
    }
}
