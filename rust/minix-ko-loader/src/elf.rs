/// Memory-safe ELF64 parsing for .ko (kernel module) files.
///
/// Parses ELF headers and section tables using safe slices and
/// checked indexing.  No raw pointer arithmetic in the parsing
/// code — all bounds are validated before access.
///
/// Supports both ELF64 and ELF32 formats (though modern .ko files
/// on x86_64 are always ELF64).

use core::mem;

use crate::error::ElfError;

// ── ELF constants ──────────────────────────────────────────────

pub const EI_NIDENT: usize = 16;
pub const ELFMAG: [u8; 4] = [0x7f, b'E', b'L', b'F'];
pub const ELFCLASS64: u8 = 2;
pub const ELFCLASS32: u8 = 1;
pub const ELFDATA2LSB: u8 = 1;
pub const ET_REL: u16 = 1;
pub const EM_X86_64: u16 = 62;
pub const EM_386: u16 = 3;

// Section header types
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_REL: u32 = 9;

// Section header flags
pub const SHF_WRITE: u64 = 0x1;
pub const SHF_ALLOC: u64 = 0x2;
pub const SHF_EXECINSTR: u64 = 0x4;

// Symbol indices
pub const STN_UNDEF: u32 = 0;
pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xFFF1;

// Symbol type/info
pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;
pub const STT_SECTION: u8 = 3;

// ── ELF structures (repr(C), read via unaligned loads) ────────

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; EI_NIDENT],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Shdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Rela {
    pub r_offset: u64,
    pub r_info: u64,
    pub r_addend: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf32Ehdr {
    pub e_ident: [u8; EI_NIDENT],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u32,
    pub e_phoff: u32,
    pub e_shoff: u32,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf32Shdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u32,
    pub sh_addr: u32,
    pub sh_offset: u32,
    pub sh_size: u32,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u32,
    pub sh_entsize: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf32Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u32,
    pub st_size: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf32Rela {
    pub r_offset: u32,
    pub r_info: u32,
    pub r_addend: i32,
}

// ── Helper functions ───────────────────────────────────────────

/// Read a u16 from a byte slice at an offset (little-endian, unaligned safe).
#[inline]
pub fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        data[offset..offset + 2].try_into().unwrap_or([0; 2]),
    )
}

/// Read a u32 from a byte slice at an offset.
#[inline]
pub fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        data[offset..offset + 4].try_into().unwrap_or([0; 4]),
    )
}

/// Read a u64 from a byte slice at an offset.
#[inline]
pub fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        data[offset..offset + 8].try_into().unwrap_or([0; 8]),
    )
}

/// Read an i64 from a byte slice at an offset.
#[inline]
pub fn read_i64_le(data: &[u8], offset: usize) -> i64 {
    i64::from_le_bytes(
        data[offset..offset + 8].try_into().unwrap_or([0; 8]),
    )
}

// ── ELF helpers ────────────────────────────────────────────────

/// Extract ELF symbol type from st_info byte.
#[inline]
pub fn elf_st_type(info: u8) -> u8 {
    info & 0x0f
}

/// Extract ELF symbol binding from st_info byte.
#[inline]
pub fn elf_st_bind(info: u8) -> u8 {
    info >> 4
}

/// Extract symbol index from r_info (64-bit).
#[inline]
pub fn elf64_r_sym(info: u64) -> u32 {
    (info & 0xffffffff) as u32
}

/// Extract relocation type from r_info (64-bit).
#[inline]
pub fn elf64_r_type(info: u64) -> u32 {
    (info >> 32) as u32
}

/// Extract symbol index from r_info (32-bit).
#[inline]
pub fn elf32_r_sym(info: u32) -> u32 {
    info >> 8
}

/// Extract relocation type from r_info (32-bit).
#[inline]
pub fn elf32_r_type(info: u32) -> u32 {
    info & 0xff
}

// ── Parsed representation (pure data, no unsafe) ───────────────

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedEhdr {
    pub is_64: bool,
    pub e_type: u16,
    pub e_machine: u16,
    pub e_shoff: u64,
    pub e_shnum: u16,
    pub e_shentsize: u16,
    pub e_shstrndx: u16,
}

#[derive(Clone, Debug)]
pub struct ParsedShdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

/// Parse an ELF header from raw bytes (safe, no heap allocation).
pub fn parse_ehdr(data: &[u8]) -> Result<ParsedEhdr, ElfError> {
    if data.len() < EI_NIDENT {
        return Err(ElfError::TruncatedFile);
    }

    // Check magic
    if data[0..4] != ELFMAG {
        return Err(ElfError::InvalidMagic);
    }

    let class = data[4];
    let encoding = data[5];

    // Check for ELF64
    if class == ELFCLASS64 {
        if encoding != ELFDATA2LSB {
            return Err(ElfError::UnsupportedEndian);
        }
        if data.len() < mem::size_of::<Elf64Ehdr>() {
            return Err(ElfError::TruncatedFile);
        }

        let e_type = read_u16_le(data, 16);
        let e_machine = read_u16_le(data, 18);
        let e_shoff = read_u64_le(data, 0x28);
        let e_shentsize = read_u16_le(data, 0x3a);
        let e_shnum = read_u16_le(data, 0x3c);
        let e_shstrndx = read_u16_le(data, 0x3e);

        if e_type != ET_REL {
            return Err(ElfError::NotRelocatable);
        }
        if e_machine != EM_X86_64 {
            return Err(ElfError::UnsupportedMachine);
        }

        // Validate section table bounds
        let shdr_size = e_shnum as u64 * e_shentsize as u64;
        if e_shoff + shdr_size > data.len() as u64 {
            return Err(ElfError::TruncatedFile);
        }

        Ok(ParsedEhdr {
            is_64: true,
            e_type,
            e_machine,
            e_shoff,
            e_shnum,
            e_shentsize,
            e_shstrndx,
        })
    } else if class == ELFCLASS32 {
        // 32-bit ELF — check architecture
        if encoding != ELFDATA2LSB {
            return Err(ElfError::UnsupportedEndian);
        }
        if data.len() < mem::size_of::<Elf32Ehdr>() {
            return Err(ElfError::TruncatedFile);
        }

        let e_type = read_u16_le(data, 16);
        let e_machine = read_u16_le(data, 18);
        let e_shoff = read_u32_le(data, 0x20) as u64;
        let e_shentsize = read_u16_le(data, 0x2e);
        let e_shnum = read_u16_le(data, 0x30);
        let e_shstrndx = read_u16_le(data, 0x32);

        if e_type != ET_REL {
            return Err(ElfError::NotRelocatable);
        }
        if e_machine != EM_386 {
            return Err(ElfError::UnsupportedMachine);
        }

        let shdr_size = e_shnum as u64 * e_shentsize as u64;
        if e_shoff + shdr_size > data.len() as u64 {
            return Err(ElfError::TruncatedFile);
        }

        Ok(ParsedEhdr {
            is_64: false,
            e_type,
            e_machine,
            e_shoff,
            e_shnum,
            e_shentsize,
            e_shstrndx,
        })
    } else {
        Err(ElfError::UnsupportedClass)
    }
}

/// Read a section header at a given index (0-based).
pub fn read_shdr(data: &[u8], ehdr: &ParsedEhdr, idx: u32) -> Result<ParsedShdr, ElfError> {
    if idx >= ehdr.e_shnum as u32 {
        return Err(ElfError::BadSectionIndex(idx));
    }

    let offset = ehdr.e_shoff as usize + idx as usize * ehdr.e_shentsize as usize;
    let end = offset + if ehdr.is_64 { mem::size_of::<Elf64Shdr>() } else { mem::size_of::<Elf32Shdr>() };

    if end > data.len() {
        return Err(ElfError::TruncatedFile);
    }

    if ehdr.is_64 {
        Ok(ParsedShdr {
            sh_name: read_u32_le(data, offset),
            sh_type: read_u32_le(data, offset + 4),
            sh_flags: read_u64_le(data, offset + 8),
            sh_addr: read_u64_le(data, offset + 16),
            sh_offset: read_u64_le(data, offset + 24),
            sh_size: read_u64_le(data, offset + 32),
            sh_link: read_u32_le(data, offset + 40),
            sh_info: read_u32_le(data, offset + 44),
            sh_addralign: read_u64_le(data, offset + 48),
            sh_entsize: read_u64_le(data, offset + 56),
        })
    } else {
        Ok(ParsedShdr {
            sh_name: read_u32_le(data, offset),
            sh_type: read_u32_le(data, offset + 4),
            sh_flags: read_u32_le(data, offset + 8) as u64,
            sh_addr: read_u32_le(data, offset + 16) as u64,
            sh_offset: read_u32_le(data, offset + 0x10) as u64,
            sh_size: read_u32_le(data, offset + 0x14) as u64,
            sh_link: read_u32_le(data, offset + 0x18),
            sh_info: read_u32_le(data, offset + 0x1c),
            sh_addralign: read_u32_le(data, offset + 0x20) as u64,
            sh_entsize: read_u32_le(data, offset + 0x24) as u64,
        })
    }
}

/// Iterate over all section headers.
pub fn all_shdrs<'a>(data: &'a [u8], ehdr: &'a ParsedEhdr) -> SectionIter<'a> {
    SectionIter { data, ehdr, idx: 0 }
}

pub struct SectionIter<'a> {
    data: &'a [u8],
    ehdr: &'a ParsedEhdr,
    idx: u32,
}

impl<'a> Iterator for SectionIter<'a> {
    type Item = (u32, ParsedShdr);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.ehdr.e_shnum as u32 {
            return None;
        }
        let idx = self.idx;
        self.idx += 1;
        match read_shdr(self.data, self.ehdr, idx) {
            Ok(shdr) => Some((idx, shdr)),
            Err(_) => None,
        }
    }
}

/// Read the section header string table (shstrtab) as a byte slice.
pub fn shstrtab<'a>(data: &'a [u8], ehdr: &ParsedEhdr) -> Result<&'a [u8], ElfError> {
    let shdr = read_shdr(data, ehdr, ehdr.e_shstrndx as u32)?;
    let off = shdr.sh_offset as usize;
    let sz = shdr.sh_size as usize;
    if off + sz > data.len() {
        return Err(ElfError::TruncatedSection);
    }
    Ok(&data[off..off + sz])
}

/// Get a section name from the shstrtab.
pub fn section_name<'a>(shstrtab: &'a [u8], shdr: &ParsedShdr) -> &'a [u8] {
    let name_off = shdr.sh_name as usize;
    if name_off >= shstrtab.len() {
        return b"(bad)";
    }
    // Find NUL terminator
    let end = shstrtab[name_off..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(shstrtab.len() - name_off);
    &shstrtab[name_off..name_off + end]
}

/// Find a section by name.  Returns (index, ParsedShdr).
pub fn find_section_by_name<'a>(
    data: &'a [u8],
    ehdr: &ParsedEhdr,
    shstrtab: &[u8],
    name: &str,
) -> Option<(u32, ParsedShdr)> {
    for (idx, shdr) in all_shdrs(data, ehdr) {
        let sname = section_name(shstrtab, &shdr);
        if sname == name.as_bytes() {
            return Some((idx, shdr));
        }
    }
    None
}

/// Read the raw data of a section from the ELF buffer.
pub fn section_data<'a>(data: &'a [u8], shdr: &ParsedShdr) -> Result<&'a [u8], ElfError> {
    let off = shdr.sh_offset as usize;
    let sz = shdr.sh_size as usize;
    if off + sz > data.len() {
        return Err(ElfError::TruncatedSection);
    }
    Ok(&data[off..off + sz])
}

/// Read a symbol table section as a slice of Elf64Sym references.
/// Returns (symbols, strtab_data) from the linked string table.
pub fn read_symtab<'a>(
    data: &'a [u8],
    ehdr: &ParsedEhdr,
    sym_shdr: &ParsedShdr,
    str_shdr: &ParsedShdr,
) -> Result<(Vec<Elf64Sym>, &'a [u8]), ElfError> {
    let raw_syms = section_data(data, sym_shdr)?;
    let strtab = section_data(data, str_shdr)?;

    let entsize = if sym_shdr.sh_entsize == 0 {
        if ehdr.is_64 { mem::size_of::<Elf64Sym>() as u64 } else { mem::size_of::<Elf32Sym>() as u64 }
    } else {
        sym_shdr.sh_entsize
    };

    let count = sym_shdr.sh_size / entsize;
    let mut syms = Vec::with_capacity(count as usize);

    if ehdr.is_64 {
        for i in 0..count as usize {
            let off = i * entsize as usize;
            if off + mem::size_of::<Elf64Sym>() > raw_syms.len() {
                break;
            }
            syms.push(Elf64Sym {
                st_name: read_u32_le(raw_syms, off),
                st_info: raw_syms[off + 4],
                st_other: raw_syms[off + 5],
                st_shndx: read_u16_le(raw_syms, off + 6),
                st_value: read_u64_le(raw_syms, off + 8),
                st_size: read_u64_le(raw_syms, off + 16),
            });
        }
    } else {
        // 32-bit syms → convert to Elf64Sym for uniform handling
        for i in 0..count as usize {
            let off = i * entsize as usize;
            if off + mem::size_of::<Elf32Sym>() > raw_syms.len() {
                break;
            }
            syms.push(Elf64Sym {
                st_name: read_u32_le(raw_syms, off),
                st_info: raw_syms[off + 4],
                st_other: raw_syms[off + 5],
                st_shndx: read_u16_le(raw_syms, off + 6),
                st_value: read_u32_le(raw_syms, off + 8) as u64,
                st_size: read_u32_le(raw_syms, off + 12) as u64,
            });
        }
    }

    Ok((syms, strtab))
}

/// Look up a string by index from a strtab.
pub fn strtab_string<'a>(strtab: &'a [u8], idx: u32) -> &'a [u8] {
    let start = idx as usize;
    if start >= strtab.len() {
        return b"(bad)";
    }
    let end = strtab[start..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(strtab.len() - start);
    &strtab[start..start + end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elf_magic() {
        // Minimal valid ELF64 header (just the ident)
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(&ELFMAG);
        data[4] = ELFCLASS64;
        data[5] = ELFDATA2LSB;
        data[16..18].copy_from_slice(&ET_REL.to_le_bytes());
        data[18..20].copy_from_slice(&EM_X86_64.to_le_bytes());

        let ehdr = parse_ehdr(&data).expect("should parse");
        assert!(ehdr.is_64);
        assert_eq!(ehdr.e_type, ET_REL);
        assert_eq!(ehdr.e_machine, EM_X86_64);
    }

    #[test]
    fn test_invalid_magic() {
        let data = vec![0u8; 64];
        assert_eq!(parse_ehdr(&data), Err(ElfError::InvalidMagic));
    }

    #[test]
    fn test_truncated_file() {
        let data = vec![0x7f, b'E', b'L', b'F']; // magic but too short
        assert_eq!(parse_ehdr(&data), Err(ElfError::TruncatedFile));
    }
}
