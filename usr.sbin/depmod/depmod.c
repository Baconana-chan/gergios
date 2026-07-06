/* depmod — GergiOS Module Dependency Generator
 *
 * Scans /lib/modules/gergios/ for .ko files, parses .modinfo sections
 * to extract module name, dependencies, PCI aliases, license, and
 * parameters, then generates:
 *
 *   modules.dep        — Dependency graph: module.ko: dep1.ko dep2.ko
 *   modules.alias      — PCI alias map:     pci:v... module_name
 *   modules.symbols    — Exported symbol map: sym_name module_name
 *   modules.softdep    — Soft dependency hints
 *
 * Additionally, generates modprobe JSON config entries for each
 * module with alias mappings, so modprobe_by_device() works.
 *
 * Usage:
 *   depmod -a              # Process all modules in /lib/modules/gergios/
 *   depmod -a -o /alt/path # Output to alternate directory
 *   depmod -n              # Dry run — print to stdout only
 *   depmod -A              # Only generate alias map, skip dep file
 *   depmod e1000e ahci     # Process specific modules
 *
 * Output files are written to /lib/modules/gergios/ (or -o dir).
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <getopt.h>
#include <stdint.h>
#include <ctype.h>

/*===========================================================================*
 *		ELF constants (minimal subset)                       *
 *===========================================================================*/

#define EI_NIDENT		16
#define ELFMAG			"\177ELF"
#define ELFCLASS64		2
#define ELFDATA2LSB		1
#define ET_REL			1

/* x86_64 / i386 */
#define EM_X86_64		62
#define EM_386			3

/* Section header types */
#define SHT_NULL		0
#define SHT_PROGBITS		1
#define SHT_SYMTAB		2
#define SHT_STRTAB		3
#define SHT_NOBITS		8

/* Section header flags */
#define SHF_ALLOC		0x2

/* Symbol bindings (st_info >> 4) */
#define STB_LOCAL		0
#define STB_GLOBAL		1
#define STB_WEAK		2

/* Symbol visibility (st_other & 0x3) */
#define STV_DEFAULT		0

/* Special section indices */
#define SHN_UNDEF		0
#define SHN_ABS			0xFFF1

/*===========================================================================*
 *		ELF64 structures (little-endian)                     *
 *===========================================================================*/

struct elf64_ehdr {
	unsigned char e_ident[EI_NIDENT];
	uint16_t e_type;
	uint16_t e_machine;
	uint32_t e_version;
	uint64_t e_entry;
	uint64_t e_phoff;
	uint64_t e_shoff;
	uint32_t e_flags;
	uint16_t e_ehsize;
	uint16_t e_phentsize;
	uint16_t e_phnum;
	uint16_t e_shentsize;
	uint16_t e_shnum;
	uint16_t e_shstrndx;
};

struct elf64_shdr {
	uint32_t sh_name;
	uint32_t sh_type;
	uint64_t sh_flags;
	uint64_t sh_addr;
	uint64_t sh_offset;
	uint64_t sh_size;
	uint32_t sh_link;
	uint32_t sh_info;
	uint64_t sh_addralign;
	uint64_t sh_entsize;
};

struct elf64_sym {
	uint32_t st_name;
	unsigned char st_info;
	unsigned char st_other;
	uint16_t st_shndx;
	uint64_t st_value;
	uint64_t st_size;
};

/*===========================================================================*
 *		Module info database                                *
 *===========================================================================*/

#define MAX_MODULES		1024
#define MAX_ALIASES_PER_MODULE	32
#define MAX_DEPS_PER_MODULE	32
#define MAX_PARMS_PER_MODULE	32
#define MAX_SYMBOLS_PER_MODULE	64
#define MAX_MODNAME_LEN		64
#define MAX_PATH_LEN		256
#define MAX_FILE_LEN		128	/* filename (not full path) */
#define MAX_VAL_LEN		256

struct modinfo_entry {
	char key[MAX_MODNAME_LEN];
	char val[MAX_VAL_LEN];
};

struct module_info {
	char name[MAX_MODNAME_LEN];		/* Module name (e.g. "e1000") */
	char filename[MAX_FILE_LEN];		/* File name (e.g. "e1000.ko") */
	char path[MAX_PATH_LEN];		/* Full path */
	int  is_ko;				/* 1 = .ko, 0 = .so */

	/* Parsed .modinfo */
	char license[MAX_VAL_LEN];
	char vermagic[MAX_VAL_LEN];
	int  num_deps;
	char deps[MAX_DEPS_PER_MODULE][MAX_MODNAME_LEN];
	int  num_aliases;
	char aliases[MAX_ALIASES_PER_MODULE][MAX_PATH_LEN];
	int  num_parms;
	char parms[MAX_PARMS_PER_MODULE][MAX_VAL_LEN];
	int  num_symbols;
	char symbols[MAX_SYMBOLS_PER_MODULE][MAX_VAL_LEN];

	int  parsed;	/* 1 = .modinfo was parsed */
};

static struct module_info g_modules[MAX_MODULES];
static int g_num_modules = 0;
static int g_dry_run = 0;
static int g_alias_only = 0;
static const char *g_output_dir = "/lib/modules/gergios";

/*===========================================================================*
 *		ELF .modinfo parser (lightweight, no relocation)    *
 *===========================================================================*/

/* Read a file into a malloc'd buffer. */
static char *read_file(const char *path, size_t *out_size)
{
	int fd;
	struct stat st;
	char *data;

	fd = open(path, O_RDONLY);
	if (fd < 0) return NULL;
	if (fstat(fd, &st) < 0) { close(fd); return NULL; }
	if (st.st_size == 0) { close(fd); return NULL; }

	data = malloc(st.st_size);
	if (!data) { close(fd); return NULL; }

	if (read(fd, data, st.st_size) != st.st_size) {
		free(data);
		close(fd);
		return NULL;
	}
	close(fd);

	*out_size = (size_t)st.st_size;
	return data;
}

/* Validate ELF64 header for a .ko file.
 * Returns 0 on success, -1 on error. */
static int validate_elf64(const char *data, size_t size,
    const struct elf64_ehdr **ehdr_out)
{
	const struct elf64_ehdr *ehdr;

	if (size < sizeof(struct elf64_ehdr))
		return -1;

	ehdr = (const struct elf64_ehdr *)data;

	/* Magic */
	if (memcmp(ehdr->e_ident, ELFMAG, 4) != 0)
		return -1;

	/* 64-bit, little-endian, relocatable object */
	if (ehdr->e_ident[4] != ELFCLASS64)
		return -1;
	if (ehdr->e_ident[5] != ELFDATA2LSB)
		return -1;
	if (ehdr->e_type != ET_REL)
		return -1;
	if (ehdr->e_machine != EM_X86_64 && ehdr->e_machine != EM_386)
		return -1;

	/* Sanity check section header table */
	if (ehdr->e_shentsize != sizeof(struct elf64_shdr))
		return -1;
	if (ehdr->e_shoff + (uint64_t)ehdr->e_shnum * sizeof(struct elf64_shdr) > size)
		return -1;
	if (ehdr->e_shstrndx >= ehdr->e_shnum)
		return -1;

	*ehdr_out = ehdr;
	return 0;
}

/* Find a section by name in the ELF. */
static const struct elf64_shdr *find_section(const char *name,
    const struct elf64_ehdr *ehdr,
    const struct elf64_shdr *shdr,
    const char *shstrtab)
{
	for (int i = 0; i < ehdr->e_shnum; i++) {
		const char *sname = shstrtab + shdr[i].sh_name;
		if (strcmp(sname, name) == 0)
			return &shdr[i];
	}
	return NULL;
}

/* Parse a .modinfo key=value string.
 * Modinfo entries are NUL-separated strings in format "key=value". */
static int parse_modinfo(const char *modinfo_data, size_t modinfo_size,
    struct module_info *mod)
{
	const char *p = modinfo_data;
	const char *end = modinfo_data + modinfo_size;

	while (p < end) {
		const char *eq = memchr(p, '=', (size_t)(end - p));
		if (!eq) break;

		size_t klen = (size_t)(eq - p);
		const char *val_start = eq + 1;
		size_t vlen = strnlen(val_start, (size_t)(end - val_start));

		/* Copy key */
		char key[64];
		if (klen >= sizeof(key)) klen = sizeof(key) - 1;
		memcpy(key, p, klen);
		key[klen] = '\0';

		/* Copy value */
		char val[MAX_VAL_LEN];
		if (vlen >= sizeof(val)) vlen = sizeof(val) - 1;
		memcpy(val, val_start, vlen);
		val[vlen] = '\0';

		/* Process known keys */
		if (strcmp(key, "name") == 0) {
			strncpy(mod->name, val, sizeof(mod->name) - 1);
		} else if (strcmp(key, "license") == 0) {
			strncpy(mod->license, val, sizeof(mod->license) - 1);
		} else if (strcmp(key, "vermagic") == 0) {
			strncpy(mod->vermagic, val, sizeof(mod->vermagic) - 1);
		} else if (strcmp(key, "depends") == 0 && val[0]) {
			/* Comma-separated list */
			char buf[MAX_VAL_LEN];
			strncpy(buf, val, sizeof(buf) - 1);
			char *save;
			char *tok = strtok_r(buf, ",", &save);
			while (tok && mod->num_deps < MAX_DEPS_PER_MODULE) {
				/* Trim spaces */
				while (*tok == ' ' || *tok == '\t') tok++;
				char *endp = tok + strlen(tok) - 1;
				while (endp > tok && (*endp == ' ' || *endp == '\t'))
					*endp-- = '\0';
				if (tok[0]) {
					strncpy(mod->deps[mod->num_deps], tok,
					    sizeof(mod->deps[0]) - 1);
					mod->num_deps++;
				}
				tok = strtok_r(NULL, ",", &save);
			}
		} else if (strcmp(key, "alias") == 0) {
			if (mod->num_aliases < MAX_ALIASES_PER_MODULE) {
				/* Only store PCI aliases */
				if (strncmp(val, "pci:", 4) == 0) {
					strncpy(mod->aliases[mod->num_aliases],
					    val, sizeof(mod->aliases[0]) - 1);
					mod->num_aliases++;
				}
			}
		} else if (strcmp(key, "parm") == 0) {
			if (mod->num_parms < MAX_PARMS_PER_MODULE) {
				strncpy(mod->parms[mod->num_parms], val,
				    sizeof(mod->parms[0]) - 1);
				mod->num_parms++;
			}
		} else if (strncmp(key, "intree.", 7) == 0) {
			/* "intree." prefixed — skip, used by Linux build system */
		}

		/* Advance to next entry (skip NUL) */
		p = val_start + vlen + 1;
	}

	mod->parsed = 1;
	return 0;
}

/* Parse .symtab section to extract exported (STB_GLOBAL) symbols.
 * Stores found symbol names in mod->symbols[]. */
static void parse_symtab(const char *data, size_t size,
    const struct elf64_ehdr *ehdr,
    const struct elf64_shdr *shdr,
    const char *shstrtab,
    struct module_info *mod)
{
	const struct elf64_shdr *symtab_sh;
	const struct elf64_shdr *strtab_sh;
	const char *symtab_data;
	const char *strtab_data;

	/* Find .symtab section */
	symtab_sh = find_section(".symtab", ehdr, shdr, shstrtab);
	if (!symtab_sh) return;

	/* .symtab's sh_link points to the linked .strtab */
	if (symtab_sh->sh_link >= ehdr->e_shnum) return;
	strtab_sh = &shdr[symtab_sh->sh_link];

	/* Validate section data bounds */
	if (symtab_sh->sh_offset + symtab_sh->sh_size > size) return;
	if (strtab_sh->sh_offset + strtab_sh->sh_size > size) return;

	symtab_data = data + symtab_sh->sh_offset;
	strtab_data = data + strtab_sh->sh_offset;

	/* Determine symbol entry size (use default if 0) */
	size_t entsize = symtab_sh->sh_entsize;
	if (entsize == 0) entsize = sizeof(struct elf64_sym);
	if (entsize < sizeof(struct elf64_sym)) return;

	unsigned int nsyms = (unsigned int)(symtab_sh->sh_size / entsize);

	for (unsigned int i = 0; i < nsyms; i++) {
		const struct elf64_sym *sym =
		    (const struct elf64_sym *)(symtab_data + i * entsize);

		/* Only consider defined global symbols */
		unsigned char bind = sym->st_info >> 4;
		uint16_t shndx = sym->st_shndx;

		/* Skip undefined, section-only, or absolute symbols */
		if (bind != STB_GLOBAL) continue;
		if (shndx == SHN_UNDEF) continue;
		if (shndx == SHN_ABS) continue;

		/* Skip symbols with no name or starting with __ */
		if (sym->st_name == 0) continue;

		const char *sym_name = strtab_data + sym->st_name;

		/* Skip known-internal names only — genuine __-prefixed exports
		 * (e.g. __register_chrdev, __alloc_skb) must be kept. */
		if (strcmp(sym_name, "__this_module") == 0) continue;
		if (strncmp(sym_name, "__crc_", 6) == 0) continue;
		if (strncmp(sym_name, "__ksymtab_", 10) == 0) continue;
		if (strncmp(sym_name, "__UNIQUE_ID_", 12) == 0) continue;

		/* Skip STT_SECTION symbols (always STB_LOCAL in practice) */
		unsigned char type = sym->st_info & 0x0f;
		if (type == 3) continue;  /* STT_SECTION */

		/* Also skip module param symbols (parm_*) */
		if (strncmp(sym_name, "parm_", 5) == 0) continue;

		/* Store this as an exported symbol */
		if (mod->num_symbols >= MAX_SYMBOLS_PER_MODULE) break;

		size_t nlen = strlen(sym_name);
		if (nlen >= sizeof(mod->symbols[0]))
			nlen = sizeof(mod->symbols[0]) - 1;
		memcpy(mod->symbols[mod->num_symbols], sym_name, nlen);
		mod->symbols[mod->num_symbols][nlen] = '\0';
		mod->num_symbols++;
	}
}

/* Parse a single .ko file's .modinfo section and fill module_info. */
static int parse_ko_file(const char *path, struct module_info *mod)
{
	const struct elf64_ehdr *ehdr;
	const struct elf64_shdr *shdr;
	const char *shstrtab;
	const struct elf64_shdr *modinfo_shdr;
	const char *modinfo_data;
	char *data;
	size_t size;
	int ret = -1;

	data = read_file(path, &size);
	if (!data) return -1;

	/* Validate ELF header */
	if (validate_elf64(data, size, &ehdr) != 0)
		goto done;

	/* Locate section header table and string table */
	shdr = (const struct elf64_shdr *)(data + ehdr->e_shoff);
	shstrtab = data + shdr[ehdr->e_shstrndx].sh_offset;

	/* Find .modinfo section */
	modinfo_shdr = find_section(".modinfo", ehdr, shdr, shstrtab);
	if (!modinfo_shdr)
		goto done;  /* No .modinfo — not a kernel module */

	/* Read .modinfo data */
	modinfo_data = data + modinfo_shdr->sh_offset;

	/* Parse key=value pairs */
	parse_modinfo(modinfo_data, modinfo_shdr->sh_size, mod);

	/* Parse .symtab for exported symbols */
	parse_symtab(data, size, ehdr, shdr, shstrtab, mod);

	ret = 0;
done:
	free(data);
	return ret;
}

/*===========================================================================*
 *		Directory scanning                                 *
 *===========================================================================*/

/* Scan /lib/modules/gergios/ for .ko and .so files.
 * Also scans hierarchical kernel/drivers/*/ subdirectories. */
static int scan_module_dir(void)
{
	DIR *dir;
	struct dirent *dent;
	int count = 0;

	/* Static list of hierarchical subdirectories to scan */
	static const char *hier_dirs[] = {
	    "/lib/modules/gergios/kernel/drivers/ata",
	    "/lib/modules/gergios/kernel/drivers/nvme",
	    "/lib/modules/gergios/kernel/drivers/net",
	    "/lib/modules/gergios/kernel/drivers/block",
	    "/lib/modules/gergios/kernel/drivers/usb",
	    "/lib/modules/gergios/kernel/drivers/audio",
	    "/lib/modules/gergios/kernel/drivers/video",
	    "/lib/modules/gergios/kernel/drivers/scsi",
	    "/lib/modules/gergios/kernel/drivers/pci",
	    "/lib/modules/gergios/kernel/drivers/hid",
	    "/lib/modules/gergios/kernel/drivers/input",
	    "/lib/modules/gergios/kernel/drivers/char",
	    "/lib/modules/gergios/kernel/drivers/mmc",
	    "/lib/modules/gergios/kernel/drivers/mtd",
	    "/lib/modules/gergios/kernel/drivers/virtio",
	    "/lib/modules/gergios/kernel/drivers/extra",
	    NULL
	};

	dir = opendir(g_output_dir);
	if (dir) {
		while ((dent = readdir(dir)) != NULL && g_num_modules < MAX_MODULES) {
			size_t len = strlen(dent->d_name);
			if (dent->d_name[0] == '.') continue;

			int is_ko = (len >= 3 && strcmp(dent->d_name + len - 3, ".ko") == 0);
			int is_so = (len >= 3 && strcmp(dent->d_name + len - 3, ".so") == 0);
			if (!is_ko && !is_so) continue;

			if (len >= MAX_FILE_LEN) {
				printf("depmod: warning — filename too long: '%s'\n", dent->d_name);
				continue;
			}

			struct module_info *mod = &g_modules[g_num_modules];
			memset(mod, 0, sizeof(*mod));

			strncpy(mod->filename, dent->d_name, sizeof(mod->filename) - 1);
			snprintf(mod->path, sizeof(mod->path), "%s/%s", g_output_dir, dent->d_name);
			mod->is_ko = is_ko;

			int suffix = is_ko ? 3 : 3;
			size_t nlen = len - suffix;
			if (nlen >= sizeof(mod->name)) nlen = sizeof(mod->name) - 1;
			memcpy(mod->name, dent->d_name, nlen);
			mod->name[nlen] = '\0';

			if (is_ko) {
				int r = parse_ko_file(mod->path, mod);
				if (r != 0)
					printf("depmod: warning — failed to parse '%s'\n", mod->filename);
			}

			g_num_modules++;
			count++;
		}
		closedir(dir);

		printf("depmod: scanned '%s' — added %d module(s)\n", g_output_dir, count);
	} else {
		printf("depmod: warning — '%s' does not exist\n", g_output_dir);
	}

	/* Scan hierarchical subdirectories */
	for (int d = 0; hier_dirs[d]; d++) {
		DIR *hdir = opendir(hier_dirs[d]);
		if (!hdir) continue;  /* Subdirectory doesn't exist — skip */

		int subcount = 0;
		while ((dent = readdir(hdir)) != NULL && g_num_modules < MAX_MODULES) {
			size_t len = strlen(dent->d_name);
			if (dent->d_name[0] == '.') continue;

			int is_ko = (len >= 3 && strcmp(dent->d_name + len - 3, ".ko") == 0);
			int is_so = (len >= 3 && strcmp(dent->d_name + len - 3, ".so") == 0);
			if (!is_ko && !is_so) continue;

			if (len >= MAX_FILE_LEN) continue;

			/* Check for duplicate (module already registered from flat dir) */
			char modname[MAX_MODNAME_LEN];
			int suffix = is_ko ? 3 : 3;
			size_t nlen = len - suffix;
			if (nlen >= sizeof(modname)) nlen = sizeof(modname) - 1;
			memcpy(modname, dent->d_name, nlen);
			modname[nlen] = '\0';

			int dup = 0;
			for (int j = 0; j < g_num_modules; j++) {
				if (strcmp(g_modules[j].name, modname) == 0) {
					dup = 1;
					break;
				}
			}
			if (dup) continue;

			struct module_info *mod = &g_modules[g_num_modules];
			memset(mod, 0, sizeof(*mod));

			strncpy(mod->name, modname, sizeof(mod->name) - 1);
			strncpy(mod->filename, dent->d_name, sizeof(mod->filename) - 1);
			snprintf(mod->path, sizeof(mod->path), "%s/%s", hier_dirs[d], dent->d_name);
			mod->is_ko = is_ko;

			if (is_ko) {
				int r = parse_ko_file(mod->path, mod);
				if (r != 0)
					printf("depmod: warning — failed to parse '%s/%s'\n", hier_dirs[d], dent->d_name);
			}

			g_num_modules++;
			subcount++;
		}
		closedir(hdir);

		if (subcount > 0) {
			printf("depmod: scanned '%s' — added %d module(s)\n", hier_dirs[d], subcount);
			count += subcount;
		}
	}

	return count;
}

/*===========================================================================*
 *		Output file generation                            *
 *===========================================================================*/

/* Write modules.dep — format: module.ko: dep1.ko dep2.ko\n */
static int write_modules_dep(FILE *out)
{
	int count = 0;

	fprintf(out, "# GergiOS modules.dep — generated by depmod\n");
	fprintf(out, "# Format: module_filename.ko: dep1.ko dep2.ko ...\n\n");

	for (int i = 0; i < g_num_modules; i++) {
		struct module_info *mod = &g_modules[i];
		if (!mod->is_ko) continue;  /* .so files don't go in dep map */

		fprintf(out, "%s:", mod->filename);

		for (int j = 0; j < mod->num_deps; j++) {
			/* Look up the dependency's filename */
			const char *depfile = NULL;
			for (int k = 0; k < g_num_modules; k++) {
				if (strcmp(g_modules[k].name, mod->deps[j]) == 0) {
					depfile = g_modules[k].filename;
					break;
				}
			}
			/* Fallback: construct filename from module name */
			if (!depfile) {
				char fallback[64];
				snprintf(fallback, sizeof(fallback), "%s.ko",
				    mod->deps[j]);
				fprintf(out, " %s", fallback);
			} else {
				fprintf(out, " %s", depfile);
			}
		}

		fprintf(out, "\n");
		count++;
	}

	return count;
}

/* Write modules.alias — format: alias module_name\n */
static int write_modules_alias(FILE *out)
{
	int count = 0;

	fprintf(out, "# GergiOS modules.alias — generated by depmod\n");
	fprintf(out, "# Format: pci:v... module_name\n\n");

	for (int i = 0; i < g_num_modules; i++) {
		struct module_info *mod = &g_modules[i];
		if (!mod->is_ko) continue;

		/* Write PCI aliases from .modinfo */
		for (int j = 0; j < mod->num_aliases; j++) {
			fprintf(out, "%s %s\n",
			    mod->aliases[j], mod->name);
			count++;
		}

		/* If module has no PCI aliases but has a name, write
		 * a generic wildcard alias for class-based matching */
		if (mod->num_aliases == 0 && mod->name[0]) {
			fprintf(out, "* %s\n", mod->name);
			count++;
		}
	}

	/* Also add .so modules with generic aliases */
	for (int i = 0; i < g_num_modules; i++) {
		struct module_info *mod = &g_modules[i];
		if (mod->is_ko) continue;
		if (mod->name[0]) {
			fprintf(out, "* %s\n", mod->name);
			count++;
		}
	}

	return count;
}

/* Write modules.symbols — format: symbol_name module_name\n */
static int write_modules_symbols(FILE *out)
{
	int count = 0;

	fprintf(out, "# GergiOS modules.symbols — generated by depmod\n");
	fprintf(out, "# Format: symbol_name module_name\n\n");

	for (int i = 0; i < g_num_modules; i++) {
		struct module_info *mod = &g_modules[i];
		for (int j = 0; j < mod->num_symbols; j++) {
			fprintf(out, "%s %s\n",
			    mod->symbols[j], mod->name);
			count++;
		}
	}

	return count;
}

/* Write modules.softdep — format: softdep module: pre: dep1 dep2\n */
static int write_modules_softdep(FILE *out)
{
	int count = 0;

	fprintf(out, "# GergiOS modules.softdep — generated by depmod\n");
	fprintf(out, "# Format: softdep module: pre: predep1 predep2\n\n");

	/* Currently, softdeps are not embedded in .modinfo.
	 * This file is a placeholder for manual configuration. */
	fprintf(out, "# No soft dependencies found\n");

	return count;
}

/* Write JSON modprobe config entries for each module with PCI aliases.
 * Output: /etc/gergios/modprobe.d/00-depmod-generated.json */
static int write_modprobe_json(FILE *out)
{
	int count = 0;

	fprintf(out, "{\n");
	fprintf(out, "  \"modprobe\": [\n");

	for (int i = 0; i < g_num_modules; i++) {
		struct module_info *mod = &g_modules[i];

		/* Each alias gets one entry */
		int alias_count = (mod->num_aliases > 0) ? mod->num_aliases : 1;

		for (int j = 0; j < alias_count; j++) {
			if (count > 0)
				fprintf(out, ",\n");

			fprintf(out, "    {\n");
			fprintf(out, "      \"alias\": \"%s\",\n",
			    alias_count > 1 ? mod->aliases[j] : "*");
			fprintf(out, "      \"driver\": \"%s\",\n",
			    mod->name);

			/* Use .ko path for LKM modules, /sbin/ for native */
			if (mod->is_ko) {
				fprintf(out, "      \"path\": \"%s\"\n",
				    mod->path);
			} else {
				fprintf(out, "      \"path\": \"/sbin/%s\"\n",
				    mod->name);
			}

			fprintf(out, "    }");
			count++;
		}
	}

	if (count > 0)
		fprintf(out, "\n");

	fprintf(out, "  ]\n");
	fprintf(out, "}\n");

	return count;
}

/*===========================================================================*
 *		File writing helpers                               *
 *===========================================================================*/

static int write_output_file(const char *filename,
    int (*writer)(FILE *), const char *description)
{
	char path[MAX_PATH_LEN];
	FILE *f;

	if (g_dry_run) {
		printf("\n=== %s (dry run, stdout) ===\n", filename);
		writer(stdout);
		return 0;
	}

	snprintf(path, sizeof(path), "%s/%s", g_output_dir, filename);

	f = fopen(path, "w");
	if (!f) {
		printf("depmod: error — failed to write '%s': %s\n",
		    path, strerror(errno));
		return -1;
	}

	int count = writer(f);
	fclose(f);

	printf("depmod: wrote %s (%d entries)\n", path, count);
	return 0;
}

/*===========================================================================*
 *		Main                                                  *
 *===========================================================================*/

static void usage(void)
{
	fprintf(stderr,
	    "Usage: depmod [-a] [-A] [-n] [-o dir] [module_name ...]\n"
	    "\n"
	    "Options:\n"
	    "  -a           Scan all modules in /lib/modules/gergios/\n"
	    "  -A           Only generate alias map (skip modules.dep)\n"
	    "  -n           Dry run (print to stdout, don't write files)\n"
	    "  -o dir       Output directory (default: /lib/modules/gergios/)\n"
	    "\n"
	    "If module names are given, only those modules are processed.\n"
	    "Output files generated in output directory:\n"
	    "  modules.dep      — Dependency graph\n"
	    "  modules.alias    — PCI alias map\n"
	    "  modules.symbols  — Export symbol map\n"
	    "  modules.softdep  — Soft dependencies\n"
	    "\n"
	    "Modprobe JSON config written to:\n"
	    "  /etc/gergios/modprobe.d/00-depmod-generated.json\n"
	);
}

int main(int argc, char *argv[])
{
	int opt;
	int scan_all = 0;
	int specific_count = 0;

	/* Parse options */
	while ((opt = getopt(argc, argv, "aAno:")) != -1) {
		switch (opt) {
		case 'a':
			scan_all = 1;
			break;
		case 'A':
			g_alias_only = 1;
			break;
		case 'n':
			g_dry_run = 1;
			break;
		case 'o':
			g_output_dir = optarg;
			break;
		default:
			usage();
			return 1;
		}
	}

	if (!scan_all && optind >= argc) {
		fprintf(stderr, "depmod: error — specify -a or module names\n");
		usage();
		return 1;
	}

	printf("depmod: GergiOS Module Dependency Generator\n");
	printf("depmod: output directory: %s\n", g_output_dir);
	printf("depmod: dry run: %s\n", g_dry_run ? "yes" : "no");

	/* Scan module directory */
	if (scan_all) {
		int found = scan_module_dir();
		printf("depmod: found %d module(s) in '%s'\n",
		    found, g_output_dir);
	}

	/* Process specific module names (optional additional) */
	for (int i = optind; i < argc; i++) {
		const char *name = argv[i];
		if (g_num_modules >= MAX_MODULES) break;

		/* Check if already in database */
		int found = 0;
		for (int j = 0; j < g_num_modules; j++) {
			if (strcmp(g_modules[j].name, name) == 0) {
				found = 1;
				break;
			}
		}
		if (found) continue;

		/* Try as .ko path */
		struct stat st;
		char path[MAX_PATH_LEN];
		snprintf(path, sizeof(path), "%s/%s.ko", g_output_dir, name);

		struct module_info *mod = &g_modules[g_num_modules];
		memset(mod, 0, sizeof(*mod));
		strncpy(mod->name, name, sizeof(mod->name) - 1);
		snprintf(mod->filename, sizeof(mod->filename), "%s.ko", name);
		strncpy(mod->path, path, sizeof(mod->path) - 1);
		mod->is_ko = 1;

		if (stat(path, &st) == 0 && S_ISREG(st.st_mode)) {
			parse_ko_file(path, mod);
		} else {
			/* Try .so */
			snprintf(path, sizeof(path), "%s/%s.so",
			    g_output_dir, name);
			if (stat(path, &st) == 0 && S_ISREG(st.st_mode)) {
				mod->is_ko = 0;
				snprintf(mod->filename, sizeof(mod->filename),
				    "%s.so", name);
				strncpy(mod->path, path, sizeof(mod->path) - 1);
			} else {
				printf("depmod: warning — module '%s' not found\n",
				    name);
				continue;
			}
		}

		g_num_modules++;
		specific_count++;
	}

	printf("depmod: total %d module(s) in database\n", g_num_modules);

	/* Summarise what we found */
	for (int i = 0; i < g_num_modules; i++) {
		struct module_info *mod = &g_modules[i];
		printf("  [%3d] %-20s %-8s %-9s syms=%d%s%s\n",
		    i, mod->name,
		    mod->is_ko ? ".ko" : ".so",
		    mod->parsed ? "modinfo" : "no-modinfo",
		    mod->num_symbols,
		    mod->num_aliases > 0 ? " aliases=" : "",
		    mod->num_aliases > 0 ? mod->aliases[0] : "");
		if (mod->num_deps > 0) {
			printf("       deps: ");
			for (int j = 0; j < mod->num_deps; j++)
				printf("%s%s", j > 0 ? ", " : "", mod->deps[j]);
			printf("\n");
		}
	}

	if (g_num_modules == 0) {
		printf("depmod: no modules to process\n");
		return 0;
	}

	/* Ensure output directory exists (for non-dry-run) */
	if (!g_dry_run) {
		struct stat st;
		if (stat(g_output_dir, &st) != 0) {
			if (mkdir(g_output_dir, 0755) != 0 && errno != EEXIST) {
				printf("depmod: error — cannot create '%s': %s\n",
				    g_output_dir, strerror(errno));
				return 1;
			}
		}
	}

	/* Write output files */
	if (!g_alias_only) {
		write_output_file("modules.dep", write_modules_dep,
		    "dependency graph");
	}

	write_output_file("modules.alias", write_modules_alias,
	    "alias map");

	write_output_file("modules.symbols", write_modules_symbols,
	    "symbol map");

	write_output_file("modules.softdep", write_modules_softdep,
	    "soft dependencies");

	/* Write modprobe JSON config */
	if (!g_dry_run) {
		char json_path[MAX_PATH_LEN];
		snprintf(json_path, sizeof(json_path),
		    "/etc/gergios/modprobe.d/00-depmod-generated.json");

		/* Ensure directory exists */
		struct stat st;
		if (stat("/etc/gergios/modprobe.d", &st) != 0) {
			mkdir("/etc/gergios/modprobe.d", 0755);
		}

		FILE *f = fopen(json_path, "w");
		if (f) {
			int count = write_modprobe_json(f);
			fclose(f);
			printf("depmod: wrote %s (%d entries)\n",
			    json_path, count);
		} else {
			printf("depmod: error — failed to write '%s': %s\n",
			    json_path, strerror(errno));
		}
	} else {
		printf("\n=== /etc/gergios/modprobe.d/00-depmod-generated.json (dry run) ===\n");
		write_modprobe_json(stdout);
	}

	printf("depmod: done\n");
	return 0;
}
