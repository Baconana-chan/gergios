

// POSIX errno values (no libc dependency)
const ENOEXEC: i32 = 8;   // Exec format error
const ENOMEM:  i32 = 12;  // Out of memory
const EINVAL:  i32 = 22;  // Invalid argument
const ENOTSUP: i32 = 95;  // Not supported
const EPERM:   i32 = 1;   // Operation not permitted
const EIO:     i32 = 5;   // I/O error

/// Error types for the memory-safe ELF .ko loader.
///
/// Maps to the same errno-style error codes as the C elf_loader
/// so it can be used as a drop-in replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    /// Not a valid ELF file (magic mismatch)
    InvalidMagic,
    /// Unsupported ELF class (not 64-bit)
    UnsupportedClass,
    /// Unsupported endianness (not little-endian)
    UnsupportedEndian,
    /// Unsupported machine architecture (not x86_64)
    UnsupportedMachine,
    /// File is not a relocatable object (not ET_REL)
    NotRelocatable,
    /// File too small to contain valid ELF headers
    TruncatedFile,
    /// Out of memory
    OutOfMemory,
    /// Unsupported relocation type encountered
    UnsupportedRelocation(u32),
    /// GPL-only symbol referenced by non-GPL module
    GplViolation(String),
    /// Symbol not found in host table
    UnresolvedSymbol(String),
    /// I/O error (file read failure)
    IoError,
    /// Invalid modinfo format
    InvalidModinfo,
    /// Section index out of bounds
    BadSectionIndex(u32),
    /// Section data truncated
    TruncatedSection,
    /// Too many sections (exceeds max)
    TooManySections,
    /// Too many memory regions
    TooManyRegions,
}

impl ElfError {
    /// Convert to a POSIX errno value, matching the C elf_loader conventions.
    pub fn to_errno(&self) -> i32 {
        use ElfError::*;
        match self {
            InvalidMagic | UnsupportedClass | UnsupportedEndian
                | UnsupportedMachine | NotRelocatable
                | TruncatedFile | TruncatedSection => -ENOEXEC,
            OutOfMemory | TooManySections | TooManyRegions => -ENOMEM,
            UnsupportedRelocation(_) => -ENOTSUP,
            GplViolation(_) => -EPERM,
            UnresolvedSymbol(_) | InvalidModinfo | BadSectionIndex(_) => -EINVAL,
            IoError => -EIO,
        }
    }
}

impl core::fmt::Display for ElfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use ElfError::*;
        match self {
            InvalidMagic => write!(f, "invalid ELF magic"),
            UnsupportedClass => write!(f, "unsupported ELF class (not 64-bit)"),
            UnsupportedEndian => write!(f, "unsupported ELF endianness (not LE)"),
            UnsupportedMachine => write!(f, "unsupported machine architecture"),
            NotRelocatable => write!(f, "not a relocatable object (not ET_REL)"),
            TruncatedFile => write!(f, "truncated ELF file"),
            OutOfMemory => write!(f, "out of memory"),
            UnsupportedRelocation(t) => write!(f, "unsupported relocation type {}", t),
            GplViolation(s) => write!(f, "GPL violation: '{}' requires GPL license", s),
            UnresolvedSymbol(s) => write!(f, "unresolved symbol '{}'", s),
            IoError => write!(f, "I/O error"),
            InvalidModinfo => write!(f, "invalid .modinfo section"),
            BadSectionIndex(i) => write!(f, "bad section index {}", i),
            TruncatedSection => write!(f, "truncated section data"),
            TooManySections => write!(f, "too many sections"),
            TooManyRegions => write!(f, "too many memory regions"),
        }
    }
}


