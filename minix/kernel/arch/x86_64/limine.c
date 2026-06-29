/* Limine boot protocol support for GergiOS x86_64.
 *
 * This file implements the Limine boot info parser, which reads the
 * request/response structures populated by the Limine bootloader and
 * fills the MINIX kinfo struct with the same information that the
 * multiboot-based pre_init would provide.
 *
 * The pagetable setup (pg_identity, pg_mapkernel, pg_load) is shared
 * with the multiboot boot path.
 *
 * Called from head.S when Limine is detected as the bootloader.
 */

#define UNPAGED 1	/* for proper kmain() prototype */

#include <assert.h>
#include <stdlib.h>
#include <stdio.h>
#include <minix/minlib.h>
#include <minix/board.h>
#include <sys/reboot.h>
#include <machine/partition.h>
#include <machine/multiboot.h>
#include <sys/types.h>
#include <minix/type.h>
#include <machine/vm.h>
#include <minix/param.h>
#include "string.h"
#include "direct_utils.h"
#include "serial.h"
#include "kernel/kernel.h"
/* Function declarations shared with pre_init.c (i386 arch).
 * These are normally declared in arch/<arch>/include/arch_proto.h
 * but x86_64 doesn't have its own arch_proto.h and uses i386's.
 * We declare them here to keep this file self-contained.
 */
int mb_set_param(char *bigbuf, char *name, char *value, kinfo_t *cbi);
int overlaps(multiboot_module_t *mod, int n, int cmp_mod);
void add_memmap(kinfo_t *cbi, u64_t addr, u64_t len);
void cut_memmap(kinfo_t *cbi, phys_bytes start, phys_bytes end);
void pg_clear(void);
void pg_identity(kinfo_t *);
int pg_mapkernel(void);
phys_bytes pg_load(void);
void vm_enable_paging(void);

/* Limine protocol definitions */
#include <machine/limine.h>

#if USE_SYSDEBUG
#define LIMINE_VERBOSE 1
#endif

/* =========================================================================
 * Limine request structures
 *
 * These MUST be placed in the .limine_requests section so that the
 * bootloader can find them and populate the response pointers.
 * =========================================================================
 */

/* Declare base revision 1 */
__attribute__((used, section(".limine_requests")))
static volatile struct limine_base_revision _limine_base_rev = {
    .id = LIMINE_BASE_REVISION_ID,
    .revision = 1
};

/* Memory map request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_memmap_request _limine_memmap_req = {
    .id = LIMINE_MEMMAP_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Boot time request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_boot_time_request _limine_boot_time_req = {
    .id = LIMINE_BOOT_TIME_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Kernel address request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_kernel_address_request _limine_kern_addr_req = {
    .id = LIMINE_KERNEL_ADDRESS_REQUEST,
    .revision = 0,
    .response = NULL
};

/* HHDM request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_hhdm_request _limine_hhdm_req = {
    .id = LIMINE_HHDM_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Module request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_module_request _limine_module_req = {
    .id = LIMINE_MODULE_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Framebuffer request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_framebuffer_request _limine_fb_req = {
    .id = LIMINE_FRAMEBUFFER_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Bootloader info request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_bootloader_info_request _limine_bootloader_req = {
    .id = LIMINE_BOOTLOADER_INFO_REQUEST,
    .revision = 0,
    .response = NULL
};

/* RSDP (ACPI) request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_rsdp_request _limine_rsdp_req = {
    .id = LIMINE_RSDP_REQUEST,
    .revision = 0,
    .response = NULL
};

/* SMP request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_smp_request _limine_smp_req = {
    .id = LIMINE_SMP_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Terminator for the request list */
__attribute__((used, section(".limine_requests")))
static volatile uint64_t _limine_requests_end[4] = { 0, 0, 0, 0 };

/* =========================================================================
 * Helper: convert a Limine memory map entry type to what MINIX expects
 * =========================================================================
 */
static int limine_memtype_to_minix(uint64_t limine_type)
{
	switch (limine_type) {
	case LIMINE_MEMMAP_USABLE:
		return MULTIBOOT_MEMORY_AVAILABLE;
	case LIMINE_MEMMAP_RESERVED:
	case LIMINE_MEMMAP_BAD_MEMORY:
	case LIMINE_MEMMAP_ACPI_NVS:
		return MULTIBOOT_MEMORY_RESERVED;
	case LIMINE_MEMMAP_ACPI_RECLAIMABLE:
	case LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE:
	case LIMINE_MEMMAP_KERNEL_AND_MODULES:
	case LIMINE_MEMMAP_FRAMEBUFFER:
	default:
		/* Treat as reserved for now; kernel will free ACPI later */
		return MULTIBOOT_MEMORY_RESERVED;
	}
}

/* =========================================================================
 * Helper: convert Limine module info to multiboot_module_t
 * =========================================================================
 */
static void limine_module_to_mb(
	const struct limine_file *lmod,
	multiboot_module_t *mbmod,
	int index)
{
	mbmod->mod_start = (u32_t)(lmod->address & 0xFFFFFFFF);
	mbmod->mod_end = (u32_t)((lmod->address + lmod->size) & 0xFFFFFFFF);
	mbmod->mod_cmdline = (u32_t)(uintptr_t)lmod->cmdline;
	mbmod->mod_pad = 0;

#if LIMINE_VERBOSE
	/* Debug: print loaded module info */
	{
		char dbg[256];
		snprintf(dbg, sizeof(dbg),
			"limine: mod%d addr=0x%lx size=%lu path=%s\n",
			index, (unsigned long)lmod->address,
			(unsigned long)lmod->size,
			lmod->path ? lmod->path : "(null)");
		direct_print(dbg);
	}
#endif
}

/* =========================================================================
 * Convert cmdline string to MINIX param_buf format
 *
 * MINIX param_buf is a list of key=value pairs separated by \0,
 * terminated by double \0.
 * =========================================================================
 */
static void limine_parse_cmdline(const char *cmdline, kinfo_t *cbi)
{
	char *p;
	int var_i, value_i;
#define BUF 1024
	static char cmdline_buf[BUF];
	static char var[BUF];
	static char value[BUF];

	if (!cmdline || !*cmdline)
		return;

	/* Copy the cmdline locally (it may be in high memory) */
	memcpy(cmdline_buf, cmdline, BUF);
	cmdline_buf[BUF - 1] = '\0';

	p = cmdline_buf;
	while (*p) {
		var_i = 0;
		value_i = 0;
		while (*p == ' ') p++;
		if (!*p) break;
		while (*p && *p != '=' && *p != ' ' && var_i < BUF - 1)
			var[var_i++] = *p++;
		var[var_i] = 0;
		if (*p++ != '=') continue;
		while (*p && *p != ' ' && value_i < BUF - 1)
			value[value_i++] = *p++;
		value[value_i] = 0;

		/* Set serial debug parameters */
		if (strcmp(var, "cttyline") == 0) {
			/* Console TTY line — handled by higher layers */
		}
		if (strcmp(var, "ttybaud") == 0) {
			cbi->serial_debug_baud = atoi(value);
		}
		if (strcmp(var, "console") == 0) {
			cbi->do_serial_debug = 1;
		}

		mb_set_param(cbi->param_buf, var, value, cbi);
	}
}

/* =========================================================================
 * Fill kinfo from Limine responses
 *
 * This is analogous to get_parameters() in the multiboot pre_init,
 * but reads from Limine's request/response structures instead.
 * =========================================================================
 */
void limine_get_parameters(kinfo_t *cbi)
{
	extern char _kern_phys_base, _kern_vir_base, _kern_size,
		_kern_unpaged_start, _kern_unpaged_end;
	phys_bytes kernbase = (phys_bytes) &_kern_phys_base,
		kernsize = (phys_bytes) &_kern_size;

	int m, k;

	/* =================================================================
	 * 1. Basic kernel info (same as multiboot path)
	 * =================================================================
	 */
	cbi->mem_high_phys = 0;
	cbi->user_sp = (vir_bytes) &_kern_vir_base;
	cbi->vir_kern_start = (vir_bytes) &_kern_vir_base;
	cbi->bootstrap_start = (vir_bytes) &_kern_unpaged_start;
	cbi->bootstrap_len = (vir_bytes) &_kern_unpaged_end -
		cbi->bootstrap_start;
	cbi->kmess = &kmess;

	/* Serial debug defaults */
	cbi->do_serial_debug = 0;
	cbi->serial_debug_baud = 115200;

	/* =================================================================
	 * 2. Parse cmdline
	 *
	 * Limine doesn't pass the kernel cmdline directly via the
	 * protocol. Instead, we try to get it from:
	 *   a) First boot module's cmdline field (if modules exist)
	 *   b) Bootloader info (name/version — not useful for cmdline)
	 *
	 * For now, we pass an empty cmdline. The kernel will use
	 * built-in defaults. In Phase 1+, we'll add a proper cmdline
	 * mechanism (e.g., embed cmdline in a dedicated module).
	 * =================================================================
	 */
	{
		const char *cmdline = NULL;

		/* Try to get cmdline from first module */
		if (_limine_module_req.response &&
		    _limine_module_req.response->module_count > 0) {
			struct limine_file *mod =
				_limine_module_req.response->modules[0];
			if (mod->cmdline && *mod->cmdline)
				cmdline = mod->cmdline;
		}

		if (cmdline)
			limine_parse_cmdline(cmdline, cbi);
	}

	/* Add architecture/board info */
	mb_set_param(cbi->param_buf, ARCHVARNAME,
		(char *)get_board_arch_name(BOARD_ID_INTEL), cbi);
	mb_set_param(cbi->param_buf, BOARDVARNAME,
		(char *)get_board_name(BOARD_ID_INTEL), cbi);

	/* Set up user stack/data boundaries */
	cbi->user_sp = USR_STACKTOP;
	cbi->user_end = USR_DATATOP;

	/* Kernel allocated bytes */
	kinfo.kernel_allocated_bytes = (phys_bytes) &_kern_size;
	kinfo.kernel_allocated_bytes -= cbi->bootstrap_len;

	assert(!(cbi->bootstrap_start % X86_64_PAGE_SIZE));
	cbi->bootstrap_len = rounddown(cbi->bootstrap_len, X86_64_PAGE_SIZE);

	/* =================================================================
	 * 3. Load modules from Limine module response
	 *
	 * Limine modules are stored as struct limine_file entries.
	 * We convert them to multiboot_module_t format for compatibility
	 * with the rest of the kernel.
	 * =================================================================
	 */
	memset(&cbi->module_list, 0, sizeof(cbi->module_list));

	if (_limine_module_req.response &&
	    _limine_module_req.response->module_count > 0) {
		u64_t count = _limine_module_req.response->module_count;
		u64_t i;

		if (count > MULTIBOOT_MAX_MODS)
			count = MULTIBOOT_MAX_MODS;

		cbi->mbi.mi_mods_count = (u32_t)count;

		for (i = 0; i < count; i++) {
			limine_module_to_mb(
				_limine_module_req.response->modules[i],
				&cbi->module_list[i],
				(int)i);
		}
	} else {
		/* No modules provided by bootloader — use defaults */
		cbi->mbi.mi_mods_count = 0;
	}

	/* =================================================================
	 * 4. Load memory map from Limine memmap response
	 *
	 * We convert Limine memory map entries to the multiboot-compatible
	 * format that the kernel expects.
	 * =================================================================
	 */
	memset(cbi->memmap, 0, sizeof(cbi->memmap));
	cbi->mmap_size = 0;

	if (_limine_memmap_req.response &&
	    _limine_memmap_req.response->entry_count > 0) {
		u64_t i;

		for (i = 0; i < _limine_memmap_req.response->entry_count; i++) {
			struct limine_memmap_entry *entry =
				_limine_memmap_req.response->entries[i];

			if (limine_memtype_to_minix(entry->type) !=
			    MULTIBOOT_MEMORY_AVAILABLE)
				continue;

			add_memmap(cbi, entry->base, entry->length);
		}
	} else {
		/* Fallback: no memory map — use sensible defaults */
		add_memmap(cbi, 0x100000, 128ULL * 1024 * 1024); /* 1MB-129MB */
	}

	/* =================================================================
	 * 5. Sanity check: kernel and modules must not overlap
	 *
	 * Pretend the kernel is an extra module for the overlap check.
	 * =================================================================
	 */
	k = (int)cbi->mbi.mi_mods_count;
	assert(k < MULTIBOOT_MAX_MODS);

	cbi->module_list[k].mod_start = (u32_t)kernbase;
	cbi->module_list[k].mod_end = (u32_t)(kernbase + kernsize);
	cbi->mods_with_kernel = k + 1;
	cbi->kern_mod = k;

	for (m = 0; m < cbi->mods_with_kernel; m++) {
		if (overlaps(cbi->module_list, cbi->mods_with_kernel, m))
			panic("limine: overlapping boot modules/kernel");
		/* Remove occupied memory from the available memory map */
		cut_memmap(cbi,
			cbi->module_list[m].mod_start,
			cbi->module_list[m].mod_end);
	}

	/* =================================================================
	 * 6. HHDM offset — needed for physical→virtual address translation
	 *
	 * Limine can tell us the higher-half direct map offset.
	 * We store it for later use by the memory manager.
	 * =================================================================
	 */
	if (_limine_hhdm_req.response) {
		/* HHDM offset is available if needed */
	}

#if LIMINE_VERBOSE
	{
		char dbg[256];
		snprintf(dbg, sizeof(dbg),
			"limine: booted with %d modules, %d memmap entries\n",
			cbi->mbi.mi_mods_count, cbi->mmap_size);
		direct_print(dbg);
	}
#endif
}

/* =========================================================================
 * limine_pre_init — main entry point called from head.S
 *
 * This is analogous to pre_init() in the multiboot path.
 * It fills the kinfo struct and sets up pagetables.
 *
 * Limine enters the kernel in 64-bit long mode directly,
 * so no 32→64 bit transition is needed.
 * =========================================================================
 */
kinfo_t *limine_pre_init(void)
{
	/* Re-initialize the kinfo struct */
	memset(&kinfo, 0, sizeof(kinfo));
	memset(&kmessages, 0, sizeof(kmessages));

	/* Fill kinfo from Limine responses */
	limine_get_parameters(&kinfo);

	/* Set up pagetables (shared with multiboot path):
	 *   1. Clear existing page tables
	 *   2. Create identity mapping for low memory (unpaged code)
	 *   3. Map kernel to its high virtual address
	 *   4. Load the new page tables
	 *   5. Enable paging
	 */
	pg_clear();
	pg_identity(&kinfo);
	kinfo.freepde_start = pg_mapkernel();
	pg_load();
	vm_enable_paging();

	/* Return boot info for kmain() */
	return &kinfo;
}
