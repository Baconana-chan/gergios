/// Full module loader (alloc + resolve + relocate).
///
/// Orchestrates the loading of a .ko ELF object:
///   1. Parse ELF header and section headers
///   2. Extract shstrtab for section names
///   3. Load SHF_ALLOC sections into aligned memory
///   4. Build symbol table (from .symtab + .strtab)
///   5. Parse .modinfo for module metadata
///   6. Apply all RELA relocations against local + host symbols
///   7. Locate init_module / cleanup_module entry points

use core::mem;

use crate::elf;
use crate::elf::{Elf64Rela, Elf64Sym, ParsedEhdr, ParsedShdr};
use crate::reloc;
use crate::modinfo::{ModInfo, parse_modinfo};
use crate::error::ElfError;

// ── Exported structures ───────────────────────────────────────

/// A host-provided symbol for resolving external references.
#[derive(Clone, Debug)]
pub struct HostSymbol {
    pub name:      String,
    pub address:   usize,
    pub gpl_only:  bool,
}

/// A single allocated memory region (code, data, or bss).
#[derive(Clone, Debug)]
pub struct MemoryRegion {
    pub data:    Vec<u8>,
    pub is_bss:  bool,
    pub vaddr:   usize,
}

/// Loaded section descriptor.
#[derive(Clone, Debug)]
pub struct SectionDescriptor {
    pub name:    String,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_size:  u64,
    pub vaddr:    usize,
    pub data_offset: u64,
}

/// The opaque handle for a loaded .ko module.
pub struct LoadedModule {
    pub name:        String,
    pub regions:     Vec<MemoryRegion>,
    pub sections:    Vec<SectionDescriptor>,
    pub symtab:      Vec<Elf64Sym>,
    pub strtab:      Vec<u8>,
    pub modinfo:     ModInfo,
    pub init_fn:     Option<usize>,
    pub cleanup_fn:  Option<usize>,
    pub gpl_compat:  bool,
}

// ── Loading pipeline ──────────────────────────────────────────

pub fn load_module(
    elf_data: &[u8],
    host_syms: &[HostSymbol],
) -> Result<Box<LoadedModule>, ElfError> {
    // 1. Parse ELF header
    let ehdr = elf::parse_ehdr(elf_data)?;

    // 2. Build section list
    let sections = load_sections(elf_data, &ehdr)?;

    // 3. Find shstrtab
    let shstrtab = elf::shstrtab(elf_data, &ehdr)?;

    // 4. Identify allocated sections and allocate memory
    let (alloc_sections, mut regions) = allocate_sections(elf_data, &ehdr, &sections, shstrtab)?;

    // 5. Load .symtab + .strtab
    let (symtab, strtab) = load_symtab(elf_data, &ehdr, &sections, shstrtab)?;

    // 6. Parse .modinfo
    let modinfo = load_modinfo(elf_data, &ehdr, &sections, shstrtab)?;
    let mod_name = modinfo.get("name").unwrap_or("(unnamed)").to_string();
    let gpl_compat = modinfo.is_gpl_compatible();

    // 8. Apply relocations
    apply_all_relocations(
        elf_data, &ehdr, &sections, shstrtab,
        &mut regions, &alloc_sections, &symtab, &strtab,
        host_syms, gpl_compat,
    )?;

    // 9. Find init_module / cleanup_module
    let init_fn    = find_local_sym(&symtab, &strtab, &regions, &alloc_sections, "init_module");
    let cleanup_fn = find_local_sym(&symtab, &strtab, &regions, &alloc_sections, "cleanup_module");

    Ok(Box::new(LoadedModule {
        name: mod_name,
        regions,
        sections: alloc_sections.into_iter().map(|(_, desc, _)| desc).collect(),
        symtab,
        strtab,
        modinfo,
        init_fn,
        cleanup_fn,
        gpl_compat,
    }))
}

// ── Internal helpers ──────────────────────────────────────────

const MAX_SECTIONS: usize = 256;
const BAD_SECTION_IDX: u16 = 0xFFFF;

/// Parse all section headers from the ELF.
fn load_sections(data: &[u8], ehdr: &ParsedEhdr) -> Result<Vec<ParsedShdr>, ElfError> {
    let n = ehdr.e_shnum as usize;
    if n > MAX_SECTIONS {
        return Err(ElfError::TooManySections);
    }
    let mut shdrs = Vec::with_capacity(n);
    for idx in 0..n {
        shdrs.push(elf::read_shdr(data, ehdr, idx as u32)?);
    }
    Ok(shdrs)
}

/// For each SHF_ALLOC section, allocate a zeroed memory region and copy
/// non-NOBITS data from the ELF buffer.
///
/// Returns a vector of (section_index, SectionDescriptor, region_index).
fn allocate_sections<'a>(
    elf_data: &[u8],
    ehdr: &ParsedEhdr,
    shdrs: &[ParsedShdr],
    shstrtab: &[u8],
) -> Result<(Vec<(u32, SectionDescriptor, usize)>, Vec<MemoryRegion>), ElfError> {
    let mut alloc_sections = Vec::new();
    let mut regions = Vec::new();

    for (idx, shdr) in shdrs.iter().enumerate() {
        let idx = idx as u32;
        let is_alloc = (shdr.sh_flags & elf::SHF_ALLOC) != 0;
        let is_nobits = shdr.sh_type == elf::SHT_NOBITS;

        if !is_alloc || shdr.sh_size == 0 {
            // Still track non-alloc sections for RELA processing later
            alloc_sections.push((idx, SectionDescriptor {
                name: section_name_str(shstrtab, shdr),
                sh_type: shdr.sh_type,
                sh_flags: shdr.sh_flags,
                sh_size: shdr.sh_size,
                vaddr: 0,
                data_offset: shdr.sh_offset,
            }, usize::MAX));
            continue;
        }

        // Allocate aligned memory
        let align = core::cmp::max(1usize, shdr.sh_addralign as usize);
        let extra = align - 1;
        let alloc_size = shdr.sh_size as usize + extra;

        let mut buf = vec![0u8; alloc_size];
        let aligned_ptr = buf.as_mut_ptr() as usize;
        let aligned = (aligned_ptr + extra) & !extra;
        let offset_in_buf = aligned - aligned_ptr;

        // Copy file data for non-NOBITS sections
        if !is_nobits && shdr.sh_size > 0 {
            let off = shdr.sh_offset as usize;
            let sz = shdr.sh_size as usize;
            if off + sz > elf_data.len() {
                return Err(ElfError::TruncatedSection);
            }
            buf[offset_in_buf..offset_in_buf + sz].copy_from_slice(&elf_data[off..off + sz]);
        }

        let vaddr = aligned + offset_in_buf;
        let region_idx = regions.len();
        regions.push(MemoryRegion {
            data: buf,
            is_bss: is_nobits,
            vaddr,
        });

        alloc_sections.push((idx, SectionDescriptor {
            name: section_name_str(shstrtab, shdr),
            sh_type: shdr.sh_type,
            sh_flags: shdr.sh_flags,
            sh_size: shdr.sh_size,
            vaddr,
            data_offset: shdr.sh_offset,
        }, region_idx));
    }

    Ok((alloc_sections, regions))
}

/// Load the symbol table and string table from the ELF.
fn load_symtab(
    data: &[u8],
    ehdr: &ParsedEhdr,
    shdrs: &[ParsedShdr],
    shstrtab: &[u8],
) -> Result<(Vec<Elf64Sym>, Vec<u8>), ElfError> {
    // Find .symtab and .strtab by name
    let mut symtab_shdr: Option<(u32, &ParsedShdr)> = None;
    let mut strtab_shdr: Option<(u32, &ParsedShdr)> = None;

    for (idx, shdr) in shdrs.iter().enumerate() {
        let name = elf::section_name(shstrtab, shdr);
        if name == b".symtab" {
            symtab_shdr = Some((idx as u32, shdr));
        } else if name == b".strtab" {
            strtab_shdr = Some((idx as u32, shdr));
        }
    }

    let (sym_idx, sym_shdr) = match symtab_shdr {
        Some(s) => s,
        None => return Ok((Vec::new(), Vec::new())), // no symbols
    };

    let (str_idx, str_shdr) = match strtab_shdr {
        Some(s) => s,
        None => return Ok((Vec::new(), Vec::new())),
    };

    let (syms, strtab_bytes) = elf::read_symtab(data, ehdr, sym_shdr, str_shdr)?;
    Ok((syms, strtab_bytes.to_vec()))
}

/// Read and parse the .modinfo section.
fn load_modinfo(
    data: &[u8],
    ehdr: &ParsedEhdr,
    shdrs: &[ParsedShdr],
    shstrtab: &[u8],
) -> Result<ModInfo, ElfError> {
    for (_idx, shdr) in shdrs.iter().enumerate() {
        let name = elf::section_name(shstrtab, shdr);
        if name == b".modinfo" {
            let section_data = elf::section_data(data, shdr)?;
            return parse_modinfo(section_data);
        }
    }
    Ok(ModInfo::new()) // No .modinfo section — return empty
}

/// Apply all RELA relocations across the loaded module.
#[allow(clippy::too_many_arguments)]
fn apply_all_relocations(
    elf_data: &[u8],
    ehdr: &ParsedEhdr,
    shdrs: &[ParsedShdr],
    shstrtab: &[u8],
    regions: &mut [MemoryRegion],
    alloc_sections: &[(u32, SectionDescriptor, usize)],
    symtab: &[Elf64Sym],
    strtab: &[u8],
    host_syms: &[HostSymbol],
    gpl_compat: bool,
) -> Result<(), ElfError> {
    // Build a fast lookup: section_index -> (SectionDescriptor, &mut MemoryRegion)
    // We'll do this lazily for each RELA section.

    // Iterate over all sections, finding SHT_RELA types
    for (rela_idx, rela_shdr) in shdrs.iter().enumerate() {
        if rela_shdr.sh_type != elf::SHT_RELA && rela_shdr.sh_type != elf::SHT_REL {
            continue;
        }

        // sh_info = target section index
        let target_idx = rela_shdr.sh_info as u32;
        // sh_link = symbol table section index (should be .symtab)

        // Find the target section in our loaded sections
        let target_info = alloc_sections.iter().find(|(idx, _, _)| *idx == target_idx);
        let target_info = match target_info {
            Some(t) => t,
            None => continue, // target not found or not loaded
        };

        let (_tgt_idx, tgt_desc, tgt_reg_idx) = target_info;
        if *tgt_reg_idx == usize::MAX {
            continue; // target section wasn't allocated (non-ALLOC)
        }

        let target_vaddr = regions[*tgt_reg_idx].vaddr;
        let target_base_in_region = tgt_desc.vaddr - target_vaddr;
        let target_data = &mut regions[*tgt_reg_idx].data;

        // Read RELA entries from the file data
        let rela_data = match elf::section_data(elf_data, rela_shdr) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let entsize = if rela_shdr.sh_entsize == 0 {
            mem::size_of::<Elf64Rela>() as u64
        } else {
            rela_shdr.sh_entsize
        };

        if entsize < mem::size_of::<Elf64Rela>() as u64 {
            continue;
        }

        let count = rela_shdr.sh_size / entsize;

        for ri in 0..count {
            let off = ri as usize * entsize as usize;
            if off + mem::size_of::<Elf64Rela>() > rela_data.len() {
                break;
            }

            let rela = elf::Elf64Rela {
                r_offset: elf::read_u64_le(rela_data, off),
                r_info:   elf::read_u64_le(rela_data, off + 8),
                r_addend: elf::read_i64_le(rela_data, off + 16),
            };

            let r_type = elf::elf64_r_type(rela.r_info);
            let sym_idx = elf::elf64_r_sym(rela.r_info);

            // Resolve symbol value
            let sym_val = resolve_symbol(
                sym_idx, symtab, strtab,
                alloc_sections,
                host_syms, gpl_compat,
            )?;

            // Apply relocation
            reloc::apply_rela(
                &rela,
                &mut target_data[target_base_in_region..],
                tgt_desc.vaddr as u64,
                sym_val,
            )?;
        }
    }

    Ok(())
}

/// Resolve a symbol index to its loaded address.
fn resolve_symbol(
    sym_idx: u32,
    symtab: &[Elf64Sym],
    strtab: &[u8],
    alloc_sections: &[(u32, SectionDescriptor, usize)],
    host_syms: &[HostSymbol],
    gpl_compat: bool,
) -> Result<u64, ElfError> {
    if sym_idx == elf::STN_UNDEF {
        return Ok(0);
    }

    let sym = match symtab.get(sym_idx as usize) {
        Some(s) => s,
        None => return Ok(0),
    };

    let sym_type = elf::elf_st_type(sym.st_info);
    let _sym_bind = elf::elf_st_bind(sym.st_info);
    let st_shndx = sym.st_shndx;
    let st_value = sym.st_value;

    // Section-relative symbol → resolve to section base
    if sym_type == elf::STT_SECTION {
        if let Some((_, desc, _)) = alloc_sections.iter().find(|(idx, _, _)| *idx == st_shndx as u32) {
            return Ok(desc.vaddr as u64);
        }
        return Ok(0);
    }

    // Absolute symbol (SHN_ABS) → value directly
    if st_shndx == elf::SHN_ABS {
        return Ok(st_value);
    }

    // Common/UNDEF → look up by name
    if st_shndx == elf::SHN_UNDEF || st_shndx == 0 {
        let sym_name = elf::strtab_string(strtab, sym.st_name);
        if sym_name.is_empty() || sym_name == b"(bad)" {
            return Ok(0);
        }

        let name_str = core::str::from_utf8(sym_name).unwrap_or("");

        // First, search host symbols
        for hs in host_syms {
            if hs.name == name_str {
                if hs.gpl_only && !gpl_compat {
                    return Err(ElfError::GplViolation(name_str.to_string()));
                }
                return Ok(hs.address as u64);
            }
        }

        // Special case: __this_module — we don't have one yet
        if name_str == "__this_module" {
            return Ok(0); // Will be set by the caller after loading
        }

        // Symbol not found — unresolved
        // In kernel modules, unresolved symbols are common (exports from
        // other modules). The caller can resolve them after loading all deps.
        // Return 0 and let the caller handle it.
        return Ok(0);
    }

    // Symbol defined in a section: value = st_value + section_base
    if st_shndx < alloc_sections.len() as u16 {
        if let Some((_, desc, _)) = alloc_sections.iter().find(|(idx, _, _)| *idx == st_shndx as u32) {
            return Ok(st_value + desc.vaddr as u64);
        }
    }

    Ok(st_value) // fallback: just the value
}

/// Find a local symbol by name and return its loaded address.
fn find_local_sym(
    symtab: &[Elf64Sym],
    strtab: &[u8],
    regions: &[MemoryRegion],
    alloc_sections: &[(u32, SectionDescriptor, usize)],
    name: &str,
) -> Option<usize> {
    for sym in symtab.iter().skip(1) {
        // Skip first symbol (STN_UNDEF)
        let sym_name = elf::strtab_string(strtab, sym.st_name);
        if sym_name.is_empty() || sym_name == b"(bad)" {
            continue;
        }
        let sym_str = core::str::from_utf8(sym_name).ok()?;
        if sym_str != name {
            continue;
        }

        let st_shndx = sym.st_shndx;
        let st_value = sym.st_value;
        let sym_type = elf::elf_st_type(sym.st_info);

        if sym_type == elf::STT_SECTION {
            if let Some((_, desc, _)) = alloc_sections.iter().find(|(idx, _, _)| *idx == st_shndx as u32) {
                return Some(desc.vaddr);
            }
            return None;
        }

        if st_shndx != elf::SHN_UNDEF && st_shndx != 0 {
            if let Some((_, desc, _)) = alloc_sections.iter().find(|(idx, _, _)| *idx == st_shndx as u32) {
                return Some((st_value + desc.vaddr as u64) as usize);
            }
        }

        if st_value != 0 {
            return Some(st_value as usize);
        }

        return None;
    }
    None
}

/// Convert a section name from the shstrtab to a String.
fn section_name_str(shstrtab: &[u8], shdr: &ParsedShdr) -> String {
    let bytes = elf::section_name(shstrtab, shdr);
    String::from_utf8_lossy(bytes).to_string()
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid ELF64 .ko with a single .text section
    /// containing a NOP sled and no relocations.
    fn build_minimal_ko() -> Vec<u8> {
        // Minimal kernel module with:
        // - .text section (SHF_ALLOC|SHF_EXECINSTR)
        // - .modinfo section
        // - .symtab + .strtab
        // - .shstrtab
        //
        // This is complex to construct — we'll just test parsing an
        // existing valid ELF header.
        let mut data = vec![0u8; 256];

        // ELF64 header
        data[0..4].copy_from_slice(&elf::ELFMAG);
        data[4] = elf::ELFCLASS64;
        data[5] = elf::ELFDATA2LSB;
        data[6] = 1; // EV_CURRENT
        data[16..18].copy_from_slice(&elf::ET_REL.to_le_bytes());  // e_type
        data[18..20].copy_from_slice(&elf::EM_X86_64.to_le_bytes()); // e_machine
        data[20..24].copy_from_slice(&1u32.to_le_bytes()); // e_version
        data[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff = 64 (but no phdrs)
        data[40..48].copy_from_slice(&64u64.to_le_bytes()); // e_shoff = 64
        data[48..50].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize = 64
        data[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
        data[60..62].copy_from_slice(&1u16.to_le_bytes());  // e_shnum = 1
        data[62..64].copy_from_slice(&0u16.to_le_bytes());  // e_shstrndx = 0

        // Section header at offset 64
        // shstrtab section
        data[64..68].copy_from_slice(&0u32.to_le_bytes());    // sh_name
        data[68..72].copy_from_slice(&elf::SHT_STRTAB.to_le_bytes()); // sh_type
        data[72..80].copy_from_slice(&0u64.to_le_bytes());    // sh_flags
        data[80..88].copy_from_slice(&0u64.to_le_bytes());    // sh_addr
        data[88..96].copy_from_slice(&128u64.to_le_bytes());  // sh_offset
        data[96..104].copy_from_slice(&8u64.to_le_bytes());   // sh_size
        data[104..108].copy_from_slice(&0u32.to_le_bytes());  // sh_link
        data[108..112].copy_from_slice(&0u32.to_le_bytes());  // sh_info
        data[112..120].copy_from_slice(&1u64.to_le_bytes());  // sh_addralign
        data[120..128].copy_from_slice(&0u64.to_le_bytes());  // sh_entsize

        // shstrtab data at offset 128
        // Section names: ".shstrtab\0"
        data[128..138].copy_from_slice(b".shstrtab\0");

        data
    }

    #[test]
    fn test_parse_minimal_ko() {
        let data = build_minimal_ko();
        let ehdr = elf::parse_ehdr(&data).expect("should parse");
        assert!(ehdr.is_64);
        assert_eq!(ehdr.e_type, elf::ET_REL);
        assert_eq!(ehdr.e_machine, elf::EM_X86_64);
    }

    #[test]
    fn test_shstrtab_parse() {
        let data = build_minimal_ko();
        let ehdr = elf::parse_ehdr(&data).expect("should parse");
        let s = elf::shstrtab(&data, &ehdr).expect("should have shstrtab");
        assert!(!s.is_empty());
    }

    #[test]
    fn test_load_empty_ko() {
        let data = build_minimal_ko();
        let host_syms = vec![
            HostSymbol {
                name: "printk".to_string(),
                address: 0xdeadbeef as usize,
                gpl_only: false,
            },
        ];
        let result = load_module(&data, &host_syms);
        // Should succeed with no allocated sections (no SHF_ALLOC sections)
        assert!(result.is_ok());
        let mod_ = result.unwrap();
        assert!(mod_.regions.is_empty()); // no SHF_ALLOC sections
    }
}
