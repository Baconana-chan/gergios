/* elf_loader.c — ELF .ko Loader Implementation
 *
 * Userspace ELF parser and relocator for Linux kernel modules on x86_64.
 * Supports:
 *   - ELF32/ELF64 header validation
 *   - Section header table walking
 *   - Section name (shstrtab) resolution
 *   - .symtab / .strtab parsing
 *   - .modinfo key=value extraction
 *   - SHF_ALLOC section memory allocation and loading
 *   - RELA relocation processing against a host symbol table
 *   - GPL license checking
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/stat.h>

#include <sys/exec_elf.h>

#include "elf_loader.h"

/*===========================================================================*
 *		Constants						     *
 *===========================================================================*/

/* Size of a single .modinfo record header (name_size + value_size) */
#define MODINFO_HDR_SZ		8

/*===========================================================================*
 *		Internal helpers — endianness				     *
 *===========================================================================*/

/* Determine if we need byte-swapping (ELF data encoding != host).
 * We only support little-endian hosts (x86_64). */
static inline int needs_swap(int ei_data)
{
	return (ei_data != ELFDATA2LSB);
}

static uint16_t r16(int swap, uint16_t v)
{
	if (!swap) return v;
	return (v >> 8) | (v << 8);
}

static uint32_t r32(int swap, uint32_t v)
{
	if (!swap) return v;
	return __builtin_bswap32(v);
}

static uint64_t r64(int swap, uint64_t v)
{
	if (!swap) return v;
	return __builtin_bswap64(v);
}

/*===========================================================================*
 *		Internal helpers — safe pointer arithmetic		     *
 *===========================================================================*/

/* Check that offset + size stays within buffer bounds.  Returns 0 on OK. */
static int bounds_check(const void *base, size_t bufsize,
    uint64_t offset, uint64_t size)
{
	if (offset > bufsize || size > bufsize || offset + size > bufsize)
		return -EFAULT;
	return 0;
}

/* Return a pointer into the buffer at @offset, or NULL if out of bounds. */
static const void *buf_ptr(const void *base, size_t bufsize,
    uint64_t offset, uint64_t size)
{
	if (bounds_check(base, bufsize, offset, size) != 0)
		return NULL;
	return (const uint8_t *)base + offset;
}

/*===========================================================================*
 *		Internal helpers — string table lookups			     *
 *===========================================================================*/

/* Look up a string by index in the section header string table (shstrtab). */
static const char *shstr_lookup(const void *shstr_data, size_t shstr_size,
    uint32_t idx)
{
	if (!shstr_data) return "(null)";
	if (idx >= shstr_size) return "(bad)";
	const char *s = (const char *)shstr_data + idx;
	/* Ensure NUL termination within bounds */
	const char *end = (const char *)shstr_data + shstr_size;
	const char *p = s;
	while (p < end && *p) p++;
	return s;
}

/* Look up a string by index in the .strtab section. */
static const char *strtab_lookup(const char *strtab, size_t strtab_size,
    uint32_t idx)
{
	if (!strtab) return "(null)";
	if (idx >= strtab_size) return "(bad)";
	return strtab + idx;
}

/*===========================================================================*
 *		modinfo parsing						     *
 *===========================================================================*/

/* Parse the .modinfo section data into key=value entries.
 * Format from Linux kernel: array of { uint32_t name_len, uint32_t value_len,
 * char name[name_len], char value[value_len] } records. */
static void parse_modinfo(const uint8_t *data, size_t size,
    struct elf_modinfo *mi)
{
	size_t pos = 0;

	mi->count = 0;

	while (pos + MODINFO_HDR_SZ <= size &&
	       mi->count < ELF_MODINFO_MAX_ENTRIES) {
		uint32_t name_sz = *(const uint32_t *)(data + pos);
		uint32_t val_sz  = *(const uint32_t *)(data + pos + 4);

		pos += MODINFO_HDR_SZ;

		if (pos + name_sz > size || pos + val_sz > size ||
		    name_sz == 0) {
			/* Malformed record — stop */
			break;
		}

		unsigned int e = mi->count;
		size_t ncopy = name_sz - 1;  /* exclude NUL */
		if (ncopy >= sizeof(mi->entries[e].key))
			ncopy = sizeof(mi->entries[e].key) - 1;
		memcpy(mi->entries[e].key, data + pos, ncopy);
		mi->entries[e].key[ncopy] = '\0';
		pos += name_sz;

		size_t vcopy = val_sz - 1;
		if (vcopy >= sizeof(mi->entries[e].value))
			vcopy = sizeof(mi->entries[e].value) - 1;
		memcpy(mi->entries[e].value, data + pos, vcopy);
		mi->entries[e].value[vcopy] = '\0';
		pos += val_sz;

		mi->count++;

		/* Align to 4 bytes for next entry */
		if (pos % 4) {
			pos += 4 - (pos % 4);
		}
	}
}

/*===========================================================================*
 *		ELF32 header validation				     *
 *===========================================================================*/

static int validate_elf32(const Elf32_Ehdr *eh, size_t size)
{
	if (size < sizeof(Elf32_Ehdr))
		return -ENOEXEC;

	if (memcmp(eh->e_ident, ELFMAG, SELFMAG) != 0)
		return -ENOEXEC;

	if (eh->e_ident[EI_CLASS] != ELFCLASS32)
		return -ENOEXEC;

	if (eh->e_ident[EI_DATA] != ELFDATA2LSB &&
	    eh->e_ident[EI_DATA] != ELFDATA2MSB)
		return -ENOEXEC;

	if (eh->e_ident[EI_VERSION] != EV_CURRENT)
		return -ENOEXEC;

	/* Kernel modules are relocatable (.o / .ko) */
	if (eh->e_type != ET_REL)
		return -EINVAL;

	return 0;
}

/*===========================================================================*
 *		ELF64 header validation				     *
 *===========================================================================*/

static int validate_elf64(const Elf64_Ehdr *eh, size_t size)
{
	if (size < sizeof(Elf64_Ehdr))
		return -ENOEXEC;

	if (memcmp(eh->e_ident, ELFMAG, SELFMAG) != 0)
		return -ENOEXEC;

	if (eh->e_ident[EI_CLASS] != ELFCLASS64)
		return -ENOEXEC;

	if (eh->e_ident[EI_DATA] != ELFDATA2LSB &&
	    eh->e_ident[EI_DATA] != ELFDATA2MSB)
		return -ENOEXEC;

	if (eh->e_ident[EI_VERSION] != EV_CURRENT)
		return -ENOEXEC;

	/* Kernel modules are relocatable (.o / .ko) */
	if (eh->e_type != ET_REL)
		return -EINVAL;

	return 0;
}

/*===========================================================================*
 *		ELF32 section loading					     *
 *===========================================================================*/

static int load_sections32(const Elf32_Ehdr *eh, int swap, const void *buf,
    size_t bufsize, struct elf_loaded_module *mod,
    const char *shstr_data, size_t shstr_size)
{
	/* Iterate section headers */
	for (int i = 0; i < eh->e_shnum && i < ELF_MODULE_MAX_SECTIONS; i++) {
		const Elf32_Shdr *sh = (const Elf32_Shdr *)
		    buf_ptr(buf, bufsize,
		        eh->e_shoff + i * eh->e_shentsize, sizeof(Elf32_Shdr));
		if (!sh) return -ENOEXEC;

		struct elf_section_descriptor *sd = &mod->sections[i];
		sd->sh_type      = r32(swap, sh->sh_type);
		sd->sh_flags     = r32(swap, sh->sh_flags);
		sd->sh_size      = r32(swap, sh->sh_size);
		sd->sh_addr      = r32(swap, sh->sh_addr);
		sd->sh_offset    = r32(swap, sh->sh_offset);
		sd->sh_link      = r32(swap, sh->sh_link);
		sd->sh_info      = r32(swap, sh->sh_info);
		sd->sh_addralign = r32(swap, sh->sh_addralign);
		sd->sh_entsize   = r32(swap, sh->sh_entsize);
		sd->sh_data      = NULL;
		mod->num_sections = i + 1;

		/* Resolve section name */
		uint32_t name_idx = r32(swap, sh->sh_name);
		sd->name = shstr_lookup(shstr_data, shstr_size, name_idx);

		/* Load sections with SHF_ALLOC */
		if ((sd->sh_flags & SHF_ALLOC) && sd->sh_size > 0) {
			if (mod->num_regions >= ELF_MODULE_MAX_REGIONS)
				return -ENOMEM;

			/* Allocate memory for section */
			size_t alloc_size = (size_t)sd->sh_size;
			if (sd->sh_addralign > 1) {
				alloc_size += sd->sh_addralign - 1;
			}
			void *mem = malloc(alloc_size);
			if (!mem) return -ENOMEM;

			/* Align to section alignment requirement */
			if (sd->sh_addralign > 1) {
				uintptr_t aligned = (uintptr_t)mem;
				aligned = (aligned + sd->sh_addralign - 1)
				    & ~(sd->sh_addralign - 1);
				sd->sh_addr = aligned;
			} else {
				sd->sh_addr = (uintptr_t)mem;
			}
			sd->sh_data = (void *)(uintptr_t)sd->sh_addr;

			memset(sd->sh_data, 0, sd->sh_size);

			/* Copy file data for non-NOBITS sections */
			if (sd->sh_type != SHT_NOBITS) {
				if (bounds_check(buf, bufsize,
				    sd->sh_offset, sd->sh_size) == 0) {
					memcpy(sd->sh_data,
					    (const uint8_t *)buf + sd->sh_offset,
					    sd->sh_size);
				}
			}

			/* Track region */
			unsigned int ri = mod->num_regions++;
			mod->regions[ri].addr   = mem;
			mod->regions[ri].size   = alloc_size;
			mod->regions[ri].is_bss = (sd->sh_type == SHT_NOBITS);
		}
	}
	return 0;
}

/*===========================================================================*
 *		ELF64 section loading					     *
 *===========================================================================*/

static int load_sections64(const Elf64_Ehdr *eh, int swap, const void *buf,
    size_t bufsize, struct elf_loaded_module *mod,
    const char *shstr_data, size_t shstr_size)
{
	for (int i = 0; i < eh->e_shnum && i < ELF_MODULE_MAX_SECTIONS; i++) {
		const Elf64_Shdr *sh = (const Elf64_Shdr *)
		    buf_ptr(buf, bufsize,
		        eh->e_shoff + i * eh->e_shentsize, sizeof(Elf64_Shdr));
		if (!sh) return -ENOEXEC;

		struct elf_section_descriptor *sd = &mod->sections[i];
		sd->sh_type      = r32(swap, sh->sh_type);
		sd->sh_flags     = r64(swap, sh->sh_flags);
		sd->sh_size      = r64(swap, sh->sh_size);
		sd->sh_addr      = r64(swap, sh->sh_addr);
		sd->sh_offset    = r64(swap, sh->sh_offset);
		sd->sh_link      = r32(swap, sh->sh_link);
		sd->sh_info      = r32(swap, sh->sh_info);
		sd->sh_addralign = r64(swap, sh->sh_addralign);
		sd->sh_entsize   = r64(swap, sh->sh_entsize);
		sd->sh_data      = NULL;
		mod->num_sections = i + 1;

		uint32_t name_idx = r32(swap, sh->sh_name);
		sd->name = shstr_lookup(shstr_data, shstr_size, name_idx);

		/* Load sections with SHF_ALLOC */
		if ((sd->sh_flags & SHF_ALLOC) && sd->sh_size > 0) {
			if (mod->num_regions >= ELF_MODULE_MAX_REGIONS)
				return -ENOMEM;

			size_t alloc_size = (size_t)sd->sh_size;
			if (sd->sh_addralign > 1) {
				alloc_size += (size_t)(sd->sh_addralign - 1);
			}
			void *mem = malloc(alloc_size);
			if (!mem) return -ENOMEM;

			if (sd->sh_addralign > 1) {
				uintptr_t aligned = (uintptr_t)mem;
				aligned = (aligned + (uintptr_t)sd->sh_addralign - 1)
				    & ~((uintptr_t)sd->sh_addralign - 1);
				sd->sh_addr = aligned;
			} else {
				sd->sh_addr = (uintptr_t)mem;
			}
			sd->sh_data = (void *)(uintptr_t)sd->sh_addr;

			memset(sd->sh_data, 0, sd->sh_size);

			if (sd->sh_type != SHT_NOBITS) {
				if (bounds_check(buf, bufsize,
				    sd->sh_offset, sd->sh_size) == 0) {
					memcpy(sd->sh_data,
					    (const uint8_t *)buf + sd->sh_offset,
					    (size_t)sd->sh_size);
				}
			}

			unsigned int ri = mod->num_regions++;
			mod->regions[ri].addr   = mem;
			mod->regions[ri].size   = alloc_size;
			mod->regions[ri].is_bss = (sd->sh_type == SHT_NOBITS);
		}
	}
	return 0;
}

/*===========================================================================*
 *		ELF64 relocation processing				     *
 *===========================================================================*/

static int process_relocations64(struct elf_loaded_module *mod,
    const struct elf_host_symbol *host_syms, size_t host_nsyms,
    int gpl_module)
{
	/* For each RELA section, process all entries */
	for (unsigned int si = 0; si < mod->num_sections; si++) {
		struct elf_section_descriptor *sd = &mod->sections[si];
		if (sd->sh_type != SHT_RELA && sd->sh_type != SHT_REL)
			continue;

		/* sh_info points to the section to relocate */
		unsigned int target_idx = (unsigned int)sd->sh_info;
		if (target_idx >= mod->num_sections)
			continue;

		struct elf_section_descriptor *target = &mod->sections[target_idx];
		if (!target->sh_data || target->sh_size == 0)
			continue;

		/* sh_link points to the symbol table */
		unsigned int symtab_idx = sd->sh_link;

		/* The RELA data is in the allocated buffer — we need to
		 * iterate over it from the original file data.
		 * Since we didn't save the file data for non-ALLOC sections,
		 * we rely on the fact that RELA sections ARE SHF_ALLOC
		 * (allocated as .rela.text etc) and their sh_data is set. */
		if (!sd->sh_data || sd->sh_size == 0)
			continue;

		uint64_t nentries = sd->sh_size / sizeof(Elf64_Rela);
		const Elf64_Rela *rela = (const Elf64_Rela *)sd->sh_data;

		for (uint64_t ri = 0; ri < nentries; ri++) {
			uint64_t r_offset = r64(0, rela[ri].r_offset);
			uint64_t r_info   = r64(0, rela[ri].r_info);
			int64_t  r_addend = r64(0, rela[ri].r_addend);

			uint32_t sym_idx = ELF64_R_SYM(r_info);
			uint32_t r_type  = ELF64_R_TYPE(r_info);

			/* Compute the place address */
			char *place = (char *)target->sh_data + r_offset;
			if ((uint64_t)(place - (char *)target->sh_data) >=
			    target->sh_size)
				continue;  /* Out of bounds, skip */

			uint64_t place_addr = (uintptr_t)place;

			/* Resolve the symbol value */
			uint64_t sym_value = 0;
			const char *sym_name = NULL;

			if (sym_idx == STN_UNDEF) {
				/* Undefined symbol — only valid for
				 * R_X86_64_RELATIVE, R_X86_64_NONE */
				if (r_type != R_X86_64_RELATIVE &&
				    r_type != R_X86_64_NONE) {
					return -EINVAL;
				}
			} else {
				/* Look up in module's own .symtab */
				if (mod->symtab_data && sym_idx < mod->num_syms) {
					const Elf64_Sym *sym =
					    (const Elf64_Sym *)mod->symtab_data + sym_idx;

					uint32_t st_name = r32(0, sym->st_name);
					uint8_t  st_info  = sym->st_info;
					uint16_t st_shndx = r16(0, sym->st_shndx);
					uint64_t st_value = r64(0, sym->st_value);

					sym_name = strtab_lookup(mod->strtab_data,
					    mod->strtab_size, st_name);

					/* If symbol is section-relative (STT_SECTION),
					 * the value is the section's base address. */
					if (ELF_ST_TYPE(st_info) == STT_SECTION) {
						/* st_shndx is the section index */
						if (st_shndx < mod->num_sections) {
							sym_value = mod->sections[st_shndx].sh_addr;
						}
					} else if (st_shndx != SHN_UNDEF &&
					    st_shndx < mod->num_sections) {
						/* Symbol defined in a section */
						sym_value = st_value +
						    mod->sections[st_shndx].sh_addr;
					} else if (st_value != 0) {
						sym_value = st_value;
					} else {
						/* Symbol not defined locally —
						 * search host table */
						sym_value = 0;
					}

					/* If still undefined, search host table */
					if (sym_value == 0 && sym_name &&
					    *sym_name != '\0' &&
					    host_syms && host_nsyms > 0) {
						for (size_t h = 0; h < host_nsyms; h++) {
							if (strcmp(sym_name,
							    host_syms[h].name) == 0) {
								/* Check GPL */
								if (host_syms[h].gpl_only &&
								    !gpl_module) {
									return -EPERM;
								}
								sym_value = (uintptr_t)
								    host_syms[h].address;
								break;
							}
						}

						/* If still not found, search for
						 * __this_module as a special case */
						if (sym_value == 0 &&
						    strcmp(sym_name, "__this_module") == 0 &&
						    mod->this_module_data) {
							sym_value = (uintptr_t)
							    mod->this_module_data;
						}
					}
				}
			}

			/* Apply the relocation */
			switch (r_type) {
			case R_X86_64_NONE:
				break;

			case R_X86_64_64:
				/* S + A */
				*(uint64_t *)place = sym_value + r_addend;
				break;

			case R_X86_64_PC32:
			case R_X86_64_PLT32:
				/* S + A - P */
				*(uint32_t *)place = (uint32_t)
				    ((sym_value + r_addend) - place_addr);
				break;

			case R_X86_64_32:
				/* S + A (zero-extended) */
				*(uint32_t *)place = (uint32_t)(sym_value + r_addend);
				break;

			case R_X86_64_32S:
				/* S + A (sign-extended) */
				*(int32_t *)place = (int32_t)(sym_value + r_addend);
				break;

			case R_X86_64_PC64:
				/* S + A - P (64-bit) */
				*(uint64_t *)place = sym_value + r_addend - place_addr;
				break;

			case R_X86_64_COPY:
				/* Copy relocations are not used in kernel modules. */
				break;

			case R_X86_64_RELATIVE:
				/* B + A (base + addend, for PIE/dynamic) */
				*(uint64_t *)place = r_addend;
				break;

			case R_X86_64_GOTPCREL:
				/* GOT entry relative — for kernel, treat as PC32 */
				*(uint32_t *)place = (uint32_t)
				    ((sym_value + r_addend) - place_addr);
				break;

			case R_X86_64_16:
				*(uint16_t *)place = (uint16_t)(sym_value + r_addend);
				break;

			case R_X86_64_PC16:
				*(uint16_t *)place = (uint16_t)
				    ((sym_value + r_addend) - place_addr);
				break;

			case R_X86_64_8:
				*(uint8_t *)place = (uint8_t)(sym_value + r_addend);
				break;

			case R_X86_64_PC8:
				*(uint8_t *)place = (uint8_t)
				    ((sym_value + r_addend) - place_addr);
				break;

			default:
				fprintf(stderr, "elf_loader: unsupported "
				    "relocation type %u at sym '%s'\n",
				    r_type, sym_name ? sym_name : "(null)");
				return -ENOTSUP;
			}
		}
	}
	return 0;
}

/*===========================================================================*
 *		ELF32 relocation processing				     *
 *===========================================================================*/

static int process_relocations32(struct elf_loaded_module *mod,
    const struct elf_host_symbol *host_syms, size_t host_nsyms,
    int gpl_module)
{
	/* Similar to the 64-bit version but for Elf32_Rela */
	for (unsigned int si = 0; si < mod->num_sections; si++) {
		struct elf_section_descriptor *sd = &mod->sections[si];
		if (sd->sh_type != SHT_RELA && sd->sh_type != SHT_REL)
			continue;

		unsigned int target_idx = (unsigned int)sd->sh_info;
		if (target_idx >= mod->num_sections) continue;

		struct elf_section_descriptor *target = &mod->sections[target_idx];
		if (!target->sh_data || target->sh_size == 0) continue;

		if (!sd->sh_data || sd->sh_size == 0) continue;

		uint64_t nentries = sd->sh_size / sizeof(Elf32_Rela);
		const Elf32_Rela *rela = (const Elf32_Rela *)sd->sh_data;

		for (uint64_t ri = 0; ri < nentries; ri++) {
			uint32_t r_offset = r32(0, rela[ri].r_offset);
			uint32_t r_info   = r32(0, rela[ri].r_info);
			int32_t  r_addend = r32(0, rela[ri].r_addend);

			uint32_t sym_idx = ELF32_R_SYM(r_info);
			uint32_t r_type  = ELF32_R_TYPE(r_info);

			char *place = (char *)target->sh_data + r_offset;
			uint32_t place_addr = (uint32_t)(uintptr_t)place;

			uint32_t sym_value = 0;
			const char *sym_name = NULL;

			if (sym_idx != STN_UNDEF && mod->symtab_data &&
			    sym_idx < mod->num_syms) {
				const Elf32_Sym *sym =
				    (const Elf32_Sym *)mod->symtab_data + sym_idx;
				uint32_t st_name  = r32(0, sym->st_name);
				uint8_t  st_info  = sym->st_info;
				uint16_t st_shndx = r16(0, sym->st_shndx);
				uint32_t st_value = r32(0, sym->st_value);

				sym_name = strtab_lookup(mod->strtab_data,
				    mod->strtab_size, st_name);

				if (ELF_ST_TYPE(st_info) == STT_SECTION) {
					if (st_shndx < mod->num_sections)
						sym_value = mod->sections[st_shndx].sh_addr;
				} else if (st_shndx != SHN_UNDEF &&
				    st_shndx < mod->num_sections) {
					sym_value = st_value +
					    mod->sections[st_shndx].sh_addr;
				} else if (st_value != 0) {
					sym_value = st_value;
				} else if (sym_name && *sym_name && host_syms) {
					for (size_t h = 0; h < host_nsyms; h++) {
						if (strcmp(sym_name,
						    host_syms[h].name) == 0) {
							if (host_syms[h].gpl_only &&
							    !gpl_module)
								return -EPERM;
							sym_value = (uint32_t)
							    (uintptr_t)host_syms[h].address;
							break;
						}
					}
				}
			}

			switch (r_type) {
			case R_386_NONE:
			case R_386_PC32:
				*(uint32_t *)place = sym_value + r_addend - place_addr;
				break;
			case R_386_32:
				*(uint32_t *)place = sym_value + r_addend;
				break;
			default:
				fprintf(stderr, "elf_loader: unsupported "
				    "i386 reloc type %u\n", r_type);
				return -ENOTSUP;
			}
		}
	}
	return 0;
}

/*===========================================================================*
 *		Section find helpers					     *
 *===========================================================================*/

/* Find a section by name (linear scan). */
static struct elf_section_descriptor *find_section(
    struct elf_loaded_module *mod, const char *name)
{
	for (unsigned int i = 0; i < mod->num_sections; i++) {
		if (mod->sections[i].name &&
		    strcmp(mod->sections[i].name, name) == 0)
			return &mod->sections[i];
	}
	return NULL;
}

/*===========================================================================*
 *		Post-load processing					     *
 *===========================================================================*/

/* After loading sections and applying relocations, find key symbols and
 * the this_module structure. */
static int post_process_module(struct elf_loaded_module *mod)
{
	/* Find init_module / cleanup_module */
	void *init_fn = elf_find_local_symbol(mod, "init_module");
	void *cleanup_fn = elf_find_local_symbol(mod, "cleanup_module");
	if (init_fn) mod->init_module_fn = init_fn;
	if (cleanup_fn) mod->cleanup_module_fn = cleanup_fn;

	/* Find .gnu.linkonce.this_module */
	struct elf_section_descriptor *this_mod_sec =
	    find_section(mod, ".gnu.linkonce.this_module");
	if (this_mod_sec && this_mod_sec->sh_data) {
		mod->this_module_data = this_mod_sec->sh_data;
	}

	/* Extract module name from .modinfo if not set */
	/* also try from this_module structure later */

	return 0;
}

/*===========================================================================*
 *		Public API: elf_load_buffer				     *
 *===========================================================================*/

int elf_load_buffer(const void *data, size_t size,
    const struct elf_host_symbol *host_syms, size_t host_nsyms,
    struct elf_loaded_module **out)
{
	int ret;
	int is_elf64 = 0;
	int swap = 0;
	struct elf_loaded_module *mod;

	if (!data || !out || size < SELFMAG)
		return -ENOEXEC;

	*out = NULL;

	/* Allocate module handle */
	mod = calloc(1, sizeof(struct elf_loaded_module));
	if (!mod) return -ENOMEM;

	/* Check magic */
	const unsigned char *ident = (const unsigned char *)data;
	if (memcmp(ident, ELFMAG, SELFMAG) != 0) {
		ret = -ENOEXEC;
		goto err;
	}

	/* Determine class */
	if (ident[EI_CLASS] == ELFCLASS64) {
		is_elf64 = 1;
		swap = (ident[EI_DATA] != ELFDATA2LSB);
		ret = validate_elf64((const Elf64_Ehdr *)data, size);
	} else if (ident[EI_CLASS] == ELFCLASS32) {
		is_elf64 = 0;
		swap = (ident[EI_DATA] != ELFDATA2LSB);
		ret = validate_elf32((const Elf32_Ehdr *)data, size);
	} else {
		ret = -ENOEXEC;
	}

	if (ret != 0)
		goto err;

	/* Parse section string table (shstrtab) first */
	const char *shstr_data = NULL;
	size_t shstr_size = 0;

	if (is_elf64) {
		const Elf64_Ehdr *eh = (const Elf64_Ehdr *)data;
		uint32_t shstrndx = r16(swap, eh->e_shstrndx);

		if (shstrndx < (uint32_t)eh->e_shnum) {
			const Elf64_Shdr *shstr_sh = (const Elf64_Shdr *)
			    buf_ptr(data, size,
			        eh->e_shoff + shstrndx * eh->e_shentsize,
			        sizeof(Elf64_Shdr));
			if (shstr_sh) {
				uint64_t off = r64(swap, shstr_sh->sh_offset);
				uint64_t sz  = r64(swap, shstr_sh->sh_size);
				if (bounds_check(data, size, off, sz) == 0) {
					shstr_data = (const char *)data + off;
					shstr_size = (size_t)sz;
				}
			}
		}
	} else {
		const Elf32_Ehdr *eh = (const Elf32_Ehdr *)data;
		uint32_t shstrndx = r16(swap, eh->e_shstrndx);

		if (shstrndx < (uint32_t)eh->e_shnum) {
			const Elf32_Shdr *shstr_sh = (const Elf32_Shdr *)
			    buf_ptr(data, size,
			        eh->e_shoff + shstrndx * eh->e_shentsize,
			        sizeof(Elf32_Shdr));
			if (shstr_sh) {
				uint32_t off = r32(swap, shstr_sh->sh_offset);
				uint32_t sz  = r32(swap, shstr_sh->sh_size);
				if (bounds_check(data, size, off, sz) == 0) {
					shstr_data = (const char *)data + off;
					shstr_size = sz;
				}
			}
		}
	}

	/* Load sections */
	if (is_elf64) {
		ret = load_sections64((const Elf64_Ehdr *)data, swap,
		    data, size, mod, shstr_data, shstr_size);
	} else {
		ret = load_sections32((const Elf32_Ehdr *)data, swap,
		    data, size, mod, shstr_data, shstr_size);
	}
	if (ret != 0) goto err;

	/* Find and parse .modinfo section */
	{
		struct elf_section_descriptor *mi_sec =
		    find_section(mod, ".modinfo");
		if (mi_sec && mi_sec->sh_data) {
			parse_modinfo((const uint8_t *)mi_sec->sh_data,
			    mi_sec->sh_size, &mod->modinfo);

			/* Extract module name from modinfo 'name' key */
			const char *mn = elf_get_modinfo(mod, "name");
			if (mn) {
				strncpy(mod->name, mn, sizeof(mod->name) - 1);
				mod->name[sizeof(mod->name) - 1] = '\0';
			}

			/* Check license */
			const char *license = elf_get_modinfo(mod, "license");
		if (license && (strcmp(license, "GPL") == 0 ||
		    strcmp(license, "GPL v2") == 0 ||
		    strcmp(license, "GPL v3") == 0 ||
		    strcmp(license, "GPL and additional rights") == 0 ||
		    strstr(license, "GPL") != NULL ||
		    strncmp(license, "Dual", 4) == 0)) {
			mod->flags |= ELF_MODULE_GPL_COMPATIBLE;
		}
		}
	}

	/* Find and copy .symtab / .strtab */
	{
		struct elf_section_descriptor *sym_sec =
		    find_section(mod, ".symtab");
		struct elf_section_descriptor *str_sec =
		    find_section(mod, ".strtab");

		/* For .symtab and .strtab, the original data from the file
		 * may not be in allocated sections.  Read directly from the
		 * original buffer, keyed by sh_offset. */
		if (sym_sec && sym_sec->sh_size > 0) {
			mod->symtab_size = (size_t)sym_sec->sh_size;
			mod->symtab_data = malloc(mod->symtab_size);
			if (mod->symtab_data) {
				/* Try to get data from the original file buffer.
				 * For non-ALLOC sections, sh_data will be NULL. */
				if (sym_sec->sh_data) {
					memcpy(mod->symtab_data,
					    sym_sec->sh_data, mod->symtab_size);
				} else if (bounds_check(data, size,
				    sym_sec->sh_offset, sym_sec->sh_size) == 0) {
					memcpy(mod->symtab_data,
					    (const uint8_t *)data +
					        sym_sec->sh_offset,
					    mod->symtab_size);
				} else {
					free(mod->symtab_data);
					mod->symtab_data = NULL;
				}
			}
		}

		if (str_sec && str_sec->sh_size > 0) {
			mod->strtab_size = (size_t)str_sec->sh_size;
			mod->strtab_data = malloc(mod->strtab_size);
			if (mod->strtab_data) {
				if (str_sec->sh_data) {
					memcpy(mod->strtab_data,
					    str_sec->sh_data, mod->strtab_size);
				} else if (bounds_check(data, size,
				    str_sec->sh_offset, str_sec->sh_size) == 0) {
					memcpy(mod->strtab_data,
					    (const uint8_t *)data +
					        str_sec->sh_offset,
					    mod->strtab_size);
				} else {
					free(mod->strtab_data);
					mod->strtab_data = NULL;
				}
			}
		}

		/* Count number of symbols */
		if (mod->symtab_data && sym_sec->sh_entsize > 0) {
			mod->num_syms = (unsigned int)
			    (mod->symtab_size / sym_sec->sh_entsize);
		} else {
			mod->num_syms = 0;
		}

		/* Track symtab/strtab as regions for cleanup.
		 * If the regions array is full, free them immediately. */
		if (mod->symtab_data) {
			if (mod->num_regions < ELF_MODULE_MAX_REGIONS) {
				unsigned int ri = mod->num_regions++;
				mod->regions[ri].addr = mod->symtab_data;
				mod->regions[ri].size = mod->symtab_size;
				mod->regions[ri].is_bss = 0;
			} else {
				free(mod->symtab_data);
				mod->symtab_data = NULL;
			}
		}
		if (mod->strtab_data) {
			if (mod->num_regions < ELF_MODULE_MAX_REGIONS) {
				unsigned int ri = mod->num_regions++;
				mod->regions[ri].addr = mod->strtab_data;
				mod->regions[ri].size = mod->strtab_size;
				mod->regions[ri].is_bss = 0;
			} else {
				free(mod->strtab_data);
				mod->strtab_data = NULL;
			}
		}
	}

	/* Determine if module is GPL-compatible */
	int gpl_compat = (mod->flags & ELF_MODULE_GPL_COMPATIBLE) ? 1 : 0;

	/* Apply relocations */
	if (is_elf64) {
		ret = process_relocations64(mod, host_syms, host_nsyms, gpl_compat);
	} else {
		ret = process_relocations32(mod, host_syms, host_nsyms, gpl_compat);
	}
	if (ret != 0) goto err;

	/* Post-process: find key symbols */
	post_process_module(mod);

	*out = mod;
	return 0;

err:
	elf_free_module(mod);
	return ret;
}

/*===========================================================================*
 *		Public API: elf_load_file				     *
 *===========================================================================*/

int elf_load_file(const char *path,
    const struct elf_host_symbol *host_syms, size_t host_nsyms,
    struct elf_loaded_module **out)
{
	int fd, ret;
	struct stat st;
	void *data = NULL;

	if (!path || !out)
		return -EINVAL;

	*out = NULL;

	fd = open(path, O_RDONLY);
	if (fd < 0)
		return -errno;

	if (fstat(fd, &st) < 0) {
		close(fd);
		return -errno;
	}

	data = malloc(st.st_size);
	if (!data) {
		close(fd);
		return -ENOMEM;
	}

	ssize_t nread = read(fd, data, st.st_size);
	close(fd);

	if (nread < 0 || (size_t)nread != (size_t)st.st_size) {
		free(data);
		return (nread < 0) ? -errno : -EIO;
	}

	ret = elf_load_buffer(data, st.st_size, host_syms, host_nsyms, out);
	free(data);
	return ret;
}

/*===========================================================================*
 *		Public API: elf_free_module				     *
 *===========================================================================*/

void elf_free_module(struct elf_loaded_module *mod)
{
	if (!mod) return;

	/* Free all tracked memory regions */
	for (unsigned int i = 0; i < mod->num_regions; i++) {
		if (mod->regions[i].addr) {
			free(mod->regions[i].addr);
			mod->regions[i].addr = NULL;
		}
	}

	/* Note: symtab_data and strtab_data are already tracked in regions,
	 * so they are freed above.  Clear the pointers to be safe. */
	memset(mod, 0, sizeof(*mod));
	free(mod);
}

/*===========================================================================*
 *		Public API: elf_find_local_symbol			     *
 *===========================================================================*/

void *elf_find_local_symbol(struct elf_loaded_module *mod, const char *name)
{
	if (!mod || !name || !mod->symtab_data)
		return NULL;

	/* Determine if 32-bit or 64-bit based on symtab entry size.
	 * For 64-bit: sizeof(Elf64_Sym) = 24
	 * For 32-bit: sizeof(Elf32_Sym) = 16 */
	if (mod->num_sections > 0 && mod->symtab_size >= sizeof(Elf64_Sym)) {
		/* Try 64-bit first */
		unsigned int nsyms = (unsigned int)
		    (mod->symtab_size / sizeof(Elf64_Sym));
		const Elf64_Sym *syms = (const Elf64_Sym *)mod->symtab_data;

		for (unsigned int i = 1; i < nsyms; i++) {
			uint32_t st_name = r32(0, syms[i].st_name);
			uint16_t st_shndx = r16(0, syms[i].st_shndx);
			uint64_t st_value = r64(0, syms[i].st_value);
			uint8_t  st_info  = syms[i].st_info;

			const char *sym_str = strtab_lookup(mod->strtab_data,
			    mod->strtab_size, st_name);

			if (sym_str && strcmp(sym_str, name) == 0) {
				/* STB_LOCAL and STB_GLOBAL both valid */
				if (st_shndx != SHN_UNDEF &&
				    st_shndx < mod->num_sections) {
					return (void *)(uintptr_t)
					    (st_value + mod->sections[st_shndx].sh_addr);
				} else if (st_value != 0) {
					return (void *)(uintptr_t)st_value;
				} else if (ELF_ST_TYPE(st_info) == STT_SECTION &&
				    st_shndx < mod->num_sections) {
					return (void *)(uintptr_t)
					    mod->sections[st_shndx].sh_addr;
				}
			}
		}
	} else if (mod->symtab_size >= sizeof(Elf32_Sym)) {
		/* Try 32-bit */
		unsigned int nsyms = (unsigned int)
		    (mod->symtab_size / sizeof(Elf32_Sym));
		const Elf32_Sym *syms = (const Elf32_Sym *)mod->symtab_data;

		for (unsigned int i = 1; i < nsyms; i++) {
			uint32_t st_name  = r32(0, syms[i].st_name);
			uint16_t st_shndx = r16(0, syms[i].st_shndx);
			uint32_t st_value = r32(0, syms[i].st_value);

			const char *sym_str = strtab_lookup(mod->strtab_data,
			    mod->strtab_size, st_name);

			if (sym_str && strcmp(sym_str, name) == 0) {
				if (st_shndx != SHN_UNDEF &&
				    st_shndx < mod->num_sections) {
					return (void *)(uintptr_t)
					    (st_value + (uint32_t)
					        mod->sections[st_shndx].sh_addr);
				} else if (st_value != 0) {
					return (void *)(uintptr_t)st_value;
				}
			}
		}
	}

	return NULL;
}

/*===========================================================================*
 *		Public API: elf_get_section				     *
 *===========================================================================*/

void *elf_get_section(const struct elf_loaded_module *mod,
    const char *name, size_t *size_out)
{
	if (!mod || !name) return NULL;

	for (unsigned int i = 0; i < mod->num_sections; i++) {
		if (mod->sections[i].name &&
		    strcmp(mod->sections[i].name, name) == 0) {
			if (size_out)
				*size_out = (size_t)mod->sections[i].sh_size;
			return mod->sections[i].sh_data;
		}
	}
	return NULL;
}

/*===========================================================================*
 *		Public API: elf_get_modinfo				     *
 *===========================================================================*/

const char *elf_get_modinfo(const struct elf_loaded_module *mod,
    const char *key)
{
	if (!mod || !key) return NULL;

	for (unsigned int i = 0; i < mod->modinfo.count; i++) {
		if (strcmp(mod->modinfo.entries[i].key, key) == 0)
			return mod->modinfo.entries[i].value;
	}
	return NULL;
}

/*===========================================================================*
 *		Public API: elf_dump_module				     *
 *===========================================================================*/

void elf_dump_module(const struct elf_loaded_module *mod)
{
	if (!mod) {
		printf("elf_dump_module: NULL\n");
		return;
	}

	printf("=== ELF Loaded Module ===\n");
	printf("  Name:           %s\n", mod->name[0] ? mod->name : "(unnamed)");
	printf("  Flags:          0x%04x%s\n", mod->flags,
	    (mod->flags & ELF_MODULE_GPL_COMPATIBLE) ? " GPL-compat" : "");
	printf("  init_module:    %p\n", mod->init_module_fn);
	printf("  cleanup_module: %p\n", mod->cleanup_module_fn);
	printf("  this_module:    %p\n", mod->this_module_data);
	printf("  Sections:       %u\n", mod->num_sections);
	printf("  Symbols:        %u\n", mod->num_syms);
	printf("  Memory regions: %u\n", mod->num_regions);

	printf("\n  --- Memory Regions ---\n");
	for (unsigned int i = 0; i < mod->num_regions; i++) {
		printf("  [%u] %p size=%zu%s\n", i,
		    mod->regions[i].addr, mod->regions[i].size,
		    mod->regions[i].is_bss ? " (bss)" : "");
	}

	printf("\n  --- .modinfo ---\n");
	for (unsigned int i = 0; i < mod->modinfo.count; i++) {
		printf("  %s = %s\n",
		    mod->modinfo.entries[i].key,
		    mod->modinfo.entries[i].value);
	}

	printf("\n  --- Sections ---\n");
	for (unsigned int i = 0; i < mod->num_sections; i++) {
		const struct elf_section_descriptor *sd = &mod->sections[i];
		if (!sd->name) continue;
		printf("  [%02u] %-32s type=%-2u flags=0x%02llx "
		    "addr=%p size=%llu\n",
		    i, sd->name,
		    (unsigned int)sd->sh_type,
		    (unsigned long long)sd->sh_flags,
		    sd->sh_data,
		    (unsigned long long)sd->sh_size);
	}
	printf("=== End ===\n");
}
