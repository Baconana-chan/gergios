/* elf_loader.h — ELF .ko Loader for GergiOS LKM Compat Layer
 *
 * Provides userspace ELF parsing and relocation for Linux kernel modules
 * (.ko files).  Supports both ELF32 and ELF64 (x86_64) with full .modinfo
 * section parsing, symbol resolution against a host symbol table, and
 * standard x86_64 relocation types (R_X86_64_64, PC32, PLT32, 32, 32S).
 *
 * Architecture:
 *   elf_load_buffer() parses the ELF from memory and returns an opaque
 *   elf_loaded_module handle.  The caller (Driver Manager Kernel API shim)
 *   provides a host symbol table for resolution of external symbols.
 *   elf_loaded_module tracks all allocated memory (code, data, bss) for
 *   later cleanup via elf_free_module().
 *
 * Thread safety: NOT thread-safe.  The caller must serialise access.
 */

#ifndef _GERGIOS_ELF_LOADER_H
#define _GERGIOS_ELF_LOADER_H

#include <stdint.h>
#include <stddef.h>

/*===========================================================================*
 *		x86_64 relocation type constants			     *
 *===========================================================================*/
#define R_X86_64_NONE		0	/* No reloc */
#define R_X86_64_64		1	/* Direct 64-bit */
#define R_X86_64_PC32		2	/* PC relative 32-bit signed */
#define R_X86_64_GOT32		3	/* 32-bit GOT entry */
#define R_X86_64_PLT32		4	/* 32-bit PLT address */
#define R_X86_64_COPY		5	/* Copy symbol at runtime */
#define R_X86_64_GLOB_DAT	6	/* Set GOT entry */
#define R_X86_64_JUMP_SLOT	7	/* Create PLT entry */
#define R_X86_64_RELATIVE	8	/* Adjust by program base */
#define R_X86_64_GOTPCREL	9	/* 32-bit signed PC relative offset to GOT */
#define R_X86_64_32		10	/* Direct 32-bit zero-extended */
#define R_X86_64_32S		11	/* Direct 32-bit sign-extended */
#define R_X86_64_16		12	/* Direct 16-bit */
#define R_X86_64_PC16		13	/* 16-bit PC relative */
#define R_X86_64_8		14	/* Direct 8-bit */
#define R_X86_64_PC8		15	/* 8-bit PC relative */
#define R_X86_64_DTPMOD64	16	/* ID of module containing symbol */
#define R_X86_64_DTPOFF64	17	/* Offset in TLS block */
#define R_X86_64_TPOFF64	18	/* Offset in TLS initial block */
#define R_X86_64_TLSGD		19	/* 32-bit signed PC relative offset to GOT for GD */
#define R_X86_64_TLSLD		20	/* 32-bit signed PC relative offset to GOT for LD */
#define R_X86_64_DTPOFF32	21	/* Offset in TLS block */
#define R_X86_64_GOTTPOFF	22	/* 32-bit signed PC relative to GOT for IE */
#define R_X86_64_TPOFF32	23	/* Offset in TLS initial block */
#define R_X86_64_PC64		24	/* PC relative 64-bit */
#define R_X86_64_GOTOFF64	25	/* 64-bit offset to GOT */
#define R_X86_64_GOTPC32	26	/* 32-bit signed PC relative to GOT */
#define R_X86_64_SIZE32		32	/* Size of symbol */
#define R_X86_64_SIZE64		33	/* Size of symbol */

/*===========================================================================*
 *		Constants for .ko parsing				     *
 *===========================================================================*/

/* Maximum number of sections a loaded module can have */
#define ELF_MODULE_MAX_SECTIONS	256

/* Maximum length of a string in .modinfo */
#define ELF_MODINFO_MAX_VAL	256

/* Number of modinfo entries we can track */
#define ELF_MODINFO_MAX_ENTRIES	32

/* Maximum number of allocated memory regions per module */
#define ELF_MODULE_MAX_REGIONS	16

/*===========================================================================*
 *		Data structures					     *
 *===========================================================================*/

/* A resolved/known symbol for the host symbol table.
 * Used by the caller (Driver Manager) to supply the Kernel API shim symbols
 * that the .ko will reference.  Modelled after Linux kernel EXPORT_SYMBOL. */
struct elf_host_symbol {
	const char *name;		/* Symbol name (NUL-terminated) */
	void       *address;		/* Address in the host's address space */
	size_t      size;		/* Size of the symbol (0 = unknown) */
	int         gpl_only;		/* 1 = EXPORT_SYMBOL_GPL, 0 = unrestricted */
};

/* Flags for sections / module state */
#define ELF_MODULE_GPL_COMPATIBLE	0x0001	/* Module has GPL-compatible license */

/* 32-bit (i386) relocation type constants */
#define R_386_NONE	0	/* No reloc */
#define R_386_32	1	/* Direct 32-bit */
#define R_386_PC32	2	/* PC relative 32-bit */
#define R_386_GOT32	3	/* 32-bit GOT entry */
#define R_386_PLT32	4	/* 32-bit PLT address */
#define R_386_COPY	5	/* Copy symbol at runtime */
#define R_386_GLOB_DAT	6	/* Set GOT entry */
#define R_386_JUMP_SLOT	7	/* Create PLT entry */
#define R_386_RELATIVE	8	/* Adjust by program base */
#define R_386_GOTOFF	9	/* 32-bit offset to GOT */
#define R_386_GOTPC	10	/* 32-bit PC relative offset to GOT */

/* A parsed .modinfo entry (key=value pair) */
struct elf_modinfo_entry {
	char key[64];			/* Key (e.g. "vermagic", "license") */
	char value[ELF_MODINFO_MAX_VAL];/* Value */
};

/* Parsed .modinfo section */
struct elf_modinfo {
	unsigned int count;
	struct elf_modinfo_entry entries[ELF_MODINFO_MAX_ENTRIES];
};

/* A memory region allocated for the loaded module */
struct elf_module_region {
	void   *addr;			/* Virtual address */
	size_t  size;			/* Size in bytes */
	int     is_bss;			/* 1 = .bss (zero-initialised, no file data) */
};

/* The section descriptor used during loading.  Kept to allow looking up
 * local symbol values without re-parsing the ELF. */
struct elf_section_descriptor {
	const char *name;		/* Section name (points into strtab copy) */
	uint32_t    sh_type;		/* SHT_* type */
	uint64_t    sh_flags;		/* SHF_* flags */
	uint64_t    sh_size;		/* Size in bytes */
	uint64_t    sh_addr;		/* Virtual address (0 before allocation) */
	void       *sh_data;		/* Pointer to loaded section data */
	uint64_t    sh_offset;		/* File offset (for non-NOBITS) */
	uint32_t    sh_link;		/* Section header index link */
	uint32_t    sh_info;		/* Extra info */
	uint64_t    sh_addralign;	/* Alignment */
	uint64_t    sh_entsize;		/* Entry size */
};

/* Opaque handle returned by elf_load_buffer().
 * Tracks all allocated memory for cleanup. */
struct elf_loaded_module {
	/* Parsed .modinfo */
	struct elf_modinfo modinfo;

	/* Entry point (init_module / cleanup_module functions) */
	void *init_module_fn;		/* Address of init_module (if found) */
	void *cleanup_module_fn;	/* Address of cleanup_module (if found) */

	/* The .gnu.linkonce.this_module data (struct module), if found.
	 * Contains the module's name and other state that the LKM shim
	 * may need to modify.  Points into an allocated region. */
	void *this_module_data;

	/* Allocated memory regions (for elf_free_module) */
	unsigned int num_regions;
	struct elf_module_region regions[ELF_MODULE_MAX_REGIONS];

	/* Section descriptors (for symbol value lookups after loading) */
	unsigned int num_sections;
	struct elf_section_descriptor sections[ELF_MODULE_MAX_SECTIONS];

	/* Symbol table (local copy, used during relocation)
	 * .symtab data + .strtab strings are stored here. */
	unsigned int num_syms;
	void  *symtab_data;		/* Copy of .symtab section data */
	size_t symtab_size;
	char  *strtab_data;		/* Copy of .strtab section data */
	size_t strtab_size;

	/* Flags from license check */
	unsigned int flags;

	/* Module name (parsed from .modinfo or this_module) */
	char name[64];
};

/*===========================================================================*
 *		Public API						     *
 *===========================================================================*/

/* Load a Linux kernel module (.ko) from a memory buffer.
 *
 * @param data        Pointer to the ELF file data in memory
 * @param size        Size of the ELF data
 * @param host_syms   Array of host symbol table entries (can be NULL)
 * @param host_nsyms  Number of entries in host_syms
 * @param out         On success, filled with an allocated elf_loaded_module.
 *                    The caller owns this and must free it via elf_free_module().
 *
 * @return 0 on success, negative errno on error:
 *   -ENOEXEC  Not a valid ELF
 *   -EINVAL   Unsupported architecture / class / endianness / type (not ET_REL)
 *   -ENOMEM   Memory allocation failure
 *   -ENOTSUP  Unsupported relocation type encountered
 *   -EPERM    GPL-only symbol referenced but module license is not GPL
 */
int elf_load_buffer(const void *data, size_t size,
    const struct elf_host_symbol *host_syms, size_t host_nsyms,
    struct elf_loaded_module **out);

/* Load a Linux kernel module (.ko) from a file path.
 * Reads the entire file, calls elf_load_buffer(), then frees the file data.
 *
 * @param path        Path to the .ko file on disk
 * @param host_syms   Array of host symbol table entries (can be NULL)
 * @param host_nsyms  Number of entries in host_syms
 * @param out         On success, filled with an allocated elf_loaded_module.
 *
 * @return 0 on success, negative errno on error (as above, plus file I/O errors).
 */
int elf_load_file(const char *path,
    const struct elf_host_symbol *host_syms, size_t host_nsyms,
    struct elf_loaded_module **out);

/* Free a loaded module and all its allocated memory.
 * After this call, @mod is invalid and must not be used.
 * Safe to call with NULL (no-op). */
void elf_free_module(struct elf_loaded_module *mod);

/* Look up a local symbol (from the module's own .symtab) by name.
 * Returns the symbol's address (after relocation) or NULL if not found.
 * Does NOT search the host symbol table. */
void *elf_find_local_symbol(struct elf_loaded_module *mod, const char *name);

/* Find a section by name.  Returns a pointer to the loaded section data
 * and writes its size to @size_out (if non-NULL).  Returns NULL if the
 * section does not exist. */
void *elf_get_section(const struct elf_loaded_module *mod,
    const char *name, size_t *size_out);

/* Look up a .modinfo value by key.  Returns the value string, or NULL
 * if the key was not found in the .modinfo section. */
const char *elf_get_modinfo(const struct elf_loaded_module *mod,
    const char *key);

/* Pretty-print a summary of the loaded module to stdout
 * (for debugging / diagnostic purposes). */
void elf_dump_module(const struct elf_loaded_module *mod);

#endif /* _GERGIOS_ELF_LOADER_H */
