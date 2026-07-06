/// Memory-safe RELA relocation processing for x86_64 .ko modules.
///
/// Supports all relocations needed by modern Linux kernel modules
/// on x86_64:
///   R_X86_64_64       (1)   S + A
///   R_X86_64_PC32     (2)   S + A - P
///   R_X86_64_PLT32    (4)   L + A - P  (alias for PC32 in kernel)
///   R_X86_64_GOTPCREL (9)   G + GOT + A - P
///   R_X86_64_32       (10)  S + A (zero-extended)
///   R_X86_64_32S      (11)  S + A (sign-extended)
///   R_X86_64_PC64     (24)  S + A - P (64-bit)

use crate::elf::{self, Elf64Rela, Elf64Sym};
use crate::error::ElfError;

/// Apply a single RELA relocation to the loaded module image.
///
/// For ET_REL files (which .ko files are), `r_offset` is the byte offset
/// within the target section, NOT an absolute address.
///
/// # Arguments
/// * `rela`         - The RELA relocation entry
/// * `section_slice` - Mutable slice starting at the loaded section data.
///                     `r_offset` is used as the byte offset into this slice.
/// * `section_base`  - The loaded virtual address of the section.
/// * `sym_val`      - Resolved symbol value (loaded address of the symbol,
///                     or 0 for undefined/weak).
///
/// Returns `Ok(())` on success, `Err(ElfError::UnsupportedRelocation)` for
/// unknown relocation types, or `Err(ElfError::TruncatedSection)` if the
/// relocation offset exceeds the target section.
#[inline]
pub fn apply_rela(
    rela: &Elf64Rela,
    section_slice: &mut [u8],
    section_base: u64,
    sym_val: u64,
) -> Result<(), ElfError> {
    let r_type = elf::elf64_r_type(rela.r_info);
    // For ET_REL files, r_offset is the byte offset within the section
    let off = rela.r_offset as usize;

    // Validate offset bounds
    if off + 8 > section_slice.len() {
        return Err(ElfError::TruncatedSection);
    }

    // Place address for PC-relative calculations: P = section_base + r_offset
    let place_addr = section_base + rela.r_offset;

    match r_type {
        1 => apply_64(section_slice, off, sym_val, rela.r_addend),
        2 => apply_pc32(section_slice, off, sym_val, rela.r_addend, place_addr),
        4 => {
            // R_X86_64_PLT32 — in kernel modules, treated identically to PC32
            apply_pc32(section_slice, off, sym_val, rela.r_addend, place_addr)
        }
        9 => {
            // R_X86_64_GOTPCREL — in kernel modules, simply resolve to
            // the symbol value + addend - P (fallback without GOT).
            apply_pc32(section_slice, off, sym_val, rela.r_addend, place_addr)
        }
        10 => apply_32(section_slice, off, sym_val, rela.r_addend),
        11 => apply_32s(section_slice, off, sym_val, rela.r_addend),
        24 => apply_pc64(section_slice, off, sym_val, rela.r_addend, place_addr),
        other => Err(ElfError::UnsupportedRelocation(other)),
    }
}

/// Apply multiple RELA entries to a loaded module.
///
/// `relas` is a slice of Elf64Rela entries (already converted from the ELF
/// buffer).  `section_data` is the writable target section.  `section_addr`
/// is the loaded address of the section.  `sym_resolver` is a closure that
/// maps a symbol index to its loaded address.
pub fn apply_rela_section<F>(
    relas: &[Elf64Rela],
    section_data: &mut [u8],
    section_addr: u64,
    sym_resolver: F,
) -> Result<(), ElfError>
where
    F: Fn(u32) -> Option<u64>,
{
    for rela in relas {
        let sym_idx = elf::elf64_r_sym(rela.r_info);

        // Resolve the symbol — if STN_UNDEF or not found, value is 0
        let sym_val = if sym_idx != elf::STN_UNDEF {
            sym_resolver(sym_idx).unwrap_or(0)
        } else {
            0
        };

        apply_rela(rela, section_data, section_addr, sym_val)?;
    }
    Ok(())
}

// ── Individual relocation appliers ─────────────────────────────

/// R_X86_64_64: S + A  (64-bit absolute)
#[inline]
fn apply_64(slice: &mut [u8], off: usize, sym_val: u64, addend: i64) -> Result<(), ElfError> {
    let val = sym_val.wrapping_add(addend as u64);
    slice[off..off + 8].copy_from_slice(&val.to_le_bytes());
    Ok(())
}

/// R_X86_64_PC32: S + A - P  (32-bit PC-relative)
#[inline]
fn apply_pc32(
    slice: &mut [u8],
    off: usize,
    sym_val: u64,
    addend: i64,
    reloc_offset: u64,
) -> Result<(), ElfError> {
    let value = (sym_val as i64)
        .wrapping_add(addend)
        .wrapping_sub(reloc_offset as i64);
    let val = value as u32;
    slice[off..off + 4].copy_from_slice(&val.to_le_bytes());
    Ok(())
}

/// R_X86_64_32: S + A  (32-bit zero-extended)
#[inline]
fn apply_32(slice: &mut [u8], off: usize, sym_val: u64, addend: i64) -> Result<(), ElfError> {
    let val = sym_val.wrapping_add(addend as u64) as u32;
    slice[off..off + 4].copy_from_slice(&val.to_le_bytes());
    Ok(())
}

/// R_X86_64_32S: S + A  (32-bit sign-extended)
#[inline]
fn apply_32s(slice: &mut [u8], off: usize, sym_val: u64, addend: i64) -> Result<(), ElfError> {
    let val = (sym_val as i64).wrapping_add(addend) as i32;
    slice[off..off + 4].copy_from_slice(&(val as u32).to_le_bytes());
    Ok(())
}

/// R_X86_64_PC64: S + A - P  (64-bit PC-relative)
#[inline]
fn apply_pc64(
    slice: &mut [u8],
    off: usize,
    sym_val: u64,
    addend: i64,
    reloc_offset: u64,
) -> Result<(), ElfError> {
    let value = (sym_val as i64)
        .wrapping_add(addend)
        .wrapping_sub(reloc_offset as i64);
    slice[off..off + 8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rela(r_offset: u64, r_type: u32, r_addend: i64) -> Elf64Rela {
        Elf64Rela {
            r_offset,
            r_info: (r_type as u64) << 32,
            r_addend,
        }
    }

    #[test]
    fn test_r_64() {
        let mut buf = vec![0u8; 16];
        let rela = make_rela(8, 1, 0x100);
        apply_rela(&rela, &mut buf, 0, 0x1000).unwrap();
        let val = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        assert_eq!(val, 0x1100); // S + A = 0x1000 + 0x100
    }

    #[test]
    fn test_r_pc32() {
        let mut buf = vec![0u8; 264]; // 0x100 + 8 = 264
        // S=0x2000, A=0, P = section_base + r_offset = 0x12340000 + 0x100 = 0x12340100
        let rela = make_rela(0x100, 2, 0);
        apply_rela(&rela, &mut buf, 0x12340000, 0x2000).unwrap();
        let val = i32::from_le_bytes(buf[0x100..0x104].try_into().unwrap());
        // S + A - P = 0x2000 - (0x12340000 + 0x100) = 0x2000 - 0x12340100
        // i32: 0x2000 - 0x12340100 = -0x1233e100 = 0xedcc_1f00 as signed
        let expected = (0x2000i64 - (0x12340000i64 + 0x100i64)) as i32;
        assert_eq!(val, expected);
    }

    #[test]
    fn test_r_32() {
        let mut buf = vec![0u8; 16];
        let rela = make_rela(4, 10, 0x50);
        apply_rela(&rela, &mut buf, 0, 0x800).unwrap();
        let val = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        assert_eq!(val, 0x850); // S + A = 0x800 + 0x50
    }

    #[test]
    fn test_r_32s() {
        let mut buf = vec![0u8; 16];
        let rela = make_rela(0, 11, 0);
        apply_rela(&rela, &mut buf, 0, 0xffff_8000).unwrap();
        let val = i32::from_le_bytes(buf[0..4].try_into().unwrap());
        assert_eq!(val, -0x8000); // sign-extended 0xffff8000
    }

    #[test]
    fn test_r_pc64() {
        let mut buf = vec![0u8; 16];
        // P = section_base + r_offset = 0 + 0 = 0
        let rela = make_rela(0, 24, -0x10);
        apply_rela(&rela, &mut buf, 0, 0x1000).unwrap();
        let val = i64::from_le_bytes(buf[0..8].try_into().unwrap());
        assert_eq!(val, 0xff0); // S + A - P = 0x1000 - 0x10 - 0 = 0xff0
    }

    #[test]
    fn test_out_of_bounds() {
        let mut buf = vec![0u8; 4];
        let rela = make_rela(0, 64, 0); // needs 8 bytes but only 4 available after offset
        assert_eq!(
            apply_rela(&rela, &mut buf, 0, 0x1000).unwrap_err(),
            ElfError::TruncatedSection
        );
    }

    #[test]
    fn test_unsupported_type() {
        let mut buf = vec![0u8; 16];
        let rela = make_rela(0, 999, 0);
        assert_eq!(
            apply_rela(&rela, &mut buf, 0, 0).unwrap_err(),
            ElfError::UnsupportedRelocation(999)
        );
    }
}
