/// Memory-safe Rust `.ko` ELF loader for GergiOS LKM compat layer.
///
/// This crate provides a safe, pure-Rust alternative to the C `elf_loader`
/// for loading Linux kernel modules (.ko files).  It supports:
///
///   - ELF64 header and section header parsing
///   - SHF_ALLOC section memory allocation with alignment
///   - .modinfo key=value section parsing
///   - .symtab / .strtab symbol table loading
///   - x86_64 RELA relocation processing (R_X86_64_64, PC32, PLT32, 32, 32S, PC64, GOTPCREL)
///   - Host symbol resolution with GPL license checking
///   - init_module / cleanup_module entry point discovery
///
/// The entire parser uses safe slices and checked indexing — no raw pointer
/// dereferences or `unsafe` code in the parsing logic.

pub mod error;
pub mod elf;
pub mod modinfo;
pub mod reloc;
pub mod loader;

pub use error::ElfError;
pub use elf::*;
pub use modinfo::*;
pub use reloc::*;
pub use loader::*;
