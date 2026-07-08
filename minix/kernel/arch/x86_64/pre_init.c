/* x86_64 pre_init — boot-time initialization.
 * Called from head.S (multiboot or Limine path).
 * Sets up kinfo struct, page tables, and enables paging.
 */

#define UNPAGED 1

/* __unpaged attribute: places function in .unpaged.text section.
 * MINIX's patched GCC recognizes this natively; for Clang/GCC we
 * define it as an explicit section attribute. The build system also
 * uses objcopy --prefix-symbols=__k_unpaged_ on the resulting .o.
 */
#ifndef __unpaged
#define __unpaged __attribute__((section(".unpaged.text")))
#endif

#include <assert.h>
#include <stdlib.h>
#include <minix/minlib.h>
#include <minix/board.h>
#include <sys/reboot.h>
#include <machine/partition.h>
#include <machine/vm.h>
#include "string.h"
#include "direct_utils.h"
#include "serial.h"
#include <machine/multiboot.h>
#include "kernel/kernel.h"

#if USE_SYSDEBUG
#define MULTIBOOT_VERBOSE 1
#endif

kinfo_t kinfo;
struct kmessages kmessages;

phys_bytes vir2phys(void *addr) { return (phys_bytes) addr; }

char *video_mem = (char *) MULTIBOOT_VIDEO_BUFFER;

#define ITOA_BUFFER_SIZE 20

int kernel_may_alloc = 1;

int mb_set_param(char *bigbuf, char *name, char *value, kinfo_t *cbi)
{
	char *p = bigbuf;
	char *bufend = bigbuf + MULTIBOOT_PARAM_BUF_SIZE;
	char *q;
	int namelen = strlen(name);
	int valuelen = strlen(value);

	if(!strcmp(name, SERVARNAME)) { cbi->do_serial_debug = 1; }
	if(!strcmp(name, SERBAUDVARNAME)) { cbi->serial_debug_baud = atoi(value); }

	while (*p) {
		if (strncmp(p, name, namelen) == 0 && p[namelen] == '=') {
			q = p;
			while (*q) q++;
			for (q++; q < bufend; q++, p++)
				*p = *q;
			break;
		}
		while (*p++)
			;
		p++;
	}

	for (p = bigbuf; p < bufend && (*p || *(p + 1)); p++)
		;
	if (p > bigbuf) p++;

	if (p + namelen + valuelen + 3 > bufend)
		return -1;

	strcpy(p, name);
	p[namelen] = '=';
	strcpy(p + namelen + 1, value);
	p[namelen + valuelen + 1] = 0;
	p[namelen + valuelen + 2] = 0;
	return 0;
}

/* Forward declarations for functions defined later or in pg_utils.c */
void add_memmap(kinfo_t *cbi, u64_t addr, u64_t len);
void cut_memmap(kinfo_t *cbi, phys_bytes start, phys_bytes end);
/* Local typedef for multiboot types (not always available through
 * param.h when _MINIX_SYSTEM is undefined). */
typedef struct multiboot_module multiboot_module_t;
typedef struct multiboot_mmap multiboot_memory_map_t;

int __unpaged overlaps(multiboot_module_t *mod, int n, int cmp_mod)
{
	multiboot_module_t *cmp = &mod[cmp_mod];
	int m;

#define INRANGE(mod, v) ((v) >= mod->mod_start && (v) < mod->mod_end)
#define OVERLAP(mod1, mod2) (INRANGE(mod1, mod2->mod_start) || \
			INRANGE(mod1, mod2->mod_end-1))
	for(m = 0; m < n; m++) {
		struct multiboot_module *thismod = &mod[m];
		if(m == cmp_mod) continue;
		if(OVERLAP(thismod, cmp))
			return 1;
	}
	return 0;
}

void get_parameters(u64_t ebx, kinfo_t *cbi)
{
	multiboot_memory_map_t *mmap;
	multiboot_info_t *mbi = &cbi->mbi;
	int var_i,value_i, m, k;
	char *p;
	extern char _kern_phys_base, _kern_vir_base, _kern_size,
		_kern_unpaged_start, _kern_unpaged_end;
	phys_bytes kernbase = (phys_bytes) &_kern_phys_base,
		kernsize = (phys_bytes) &_kern_size;
#define BUF 1024
	static char cmdline[BUF];

	memcpy((void *) mbi, (void *) (u64_t) ebx, sizeof(*mbi));

	cbi->mem_high_phys = 0;
	cbi->user_sp = (vir_bytes) &_kern_vir_base;
	cbi->vir_kern_start = (vir_bytes) &_kern_vir_base;
	cbi->bootstrap_start = (vir_bytes) &_kern_unpaged_start;
	cbi->bootstrap_len = (vir_bytes) &_kern_unpaged_end -
		cbi->bootstrap_start;
	cbi->kmess = &kmess;

	cbi->do_serial_debug = 0;
	cbi->serial_debug_baud = 115200;

	if (mbi->mi_flags & MULTIBOOT_INFO_HAS_CMDLINE) {
		static char var[BUF];
		static char value[BUF];

		memcpy(cmdline, (void *) (u64_t) mbi->mi_cmdline, BUF);
		p = cmdline;
		while (*p) {
			var_i = 0;
			value_i = 0;
			while (*p == ' ') p++;
			if (!*p) break;
			while (*p && *p != '=' && *p != ' ' && var_i < BUF - 1)
				var[var_i++] = *p++ ;
			var[var_i] = 0;
			if (*p++ != '=') continue;
			while (*p && *p != ' ' && value_i < BUF - 1)
				value[value_i++] = *p++ ;
			value[value_i] = 0;

			mb_set_param(cbi->param_buf, var, value, cbi);
		}
	}

	mb_set_param(cbi->param_buf, ARCHVARNAME, (char *)get_board_arch_name(BOARD_ID_INTEL), cbi);
	mb_set_param(cbi->param_buf, BOARDVARNAME, (char *)get_board_name(BOARD_ID_INTEL), cbi);

	cbi->user_sp = USR_STACKTOP;
	cbi->user_end = USR_DATATOP;

	kinfo.kernel_allocated_bytes = (phys_bytes) &_kern_size;
	kinfo.kernel_allocated_bytes -= cbi->bootstrap_len;

	assert(!(cbi->bootstrap_start % X86_64_PAGE_SIZE));
	cbi->bootstrap_len = rounddown(cbi->bootstrap_len, X86_64_PAGE_SIZE);
	assert(mbi->mi_flags & MULTIBOOT_INFO_HAS_MODS);
	assert(mbi->mi_mods_count < MULTIBOOT_MAX_MODS);
	assert(mbi->mi_mods_count > 0);
	memcpy(&cbi->module_list, (void *) (u64_t) mbi->mi_mods_addr,
		mbi->mi_mods_count * sizeof(multiboot_module_t));

	memset(cbi->memmap, 0, sizeof(cbi->memmap));
	if(mbi->mi_flags & MULTIBOOT_INFO_HAS_MMAP) {
		cbi->mmap_size = 0;
	        for (mmap = (multiboot_memory_map_t *) (u64_t) mbi->mi_mmap_addr;
      	     (unsigned long) mmap < mbi->mi_mmap_addr + mbi->mi_mmap_length;
      	       mmap = (multiboot_memory_map_t *)
		     	((unsigned long) mmap + mmap->mm_size + sizeof(mmap->mm_size))) {
			if(mmap->mm_type != MULTIBOOT_MEMORY_AVAILABLE) continue;
			add_memmap(cbi, mmap->mm_base_addr, mmap->mm_length);
		}
	} else {
		assert(mbi->mi_flags & MULTIBOOT_INFO_HAS_MEMORY);
		add_memmap(cbi, 0, mbi->mi_mem_lower*1024);
		add_memmap(cbi, 0x100000, mbi->mi_mem_upper*1024);
	}

	k = mbi->mi_mods_count;
	assert(k < MULTIBOOT_MAX_MODS);
	cbi->module_list[k].mod_start = kernbase;
	cbi->module_list[k].mod_end = kernbase + kernsize;
	cbi->mods_with_kernel = mbi->mi_mods_count+1;
	cbi->kern_mod = k;

	for(m = 0; m < cbi->mods_with_kernel; m++) {
		if(overlaps(cbi->module_list, cbi->mods_with_kernel, m))
			panic("overlapping boot modules/kernel");
		cut_memmap(cbi,
			cbi->module_list[m].mod_start,
			cbi->module_list[m].mod_end);
	}
}

/* Check if RDRAND instruction is available via CPUID.
 * ECX[30] = RDRAND support bit.
 */
static int __unpaged rdrand_available(void)
{
	u32_t eax, ebx, ecx, edx;
	__asm__("cpuid"
		: "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
		: "a"(1), "c"(0));
	return (ecx & (1 << 30)) != 0;
}

/* Read a 64-bit random value from RDRAND (x86_64 native instruction).
 * Returns 0 if RDRAND fails (CF=0 after instruction).
 * Must only be called if rdrand_available() returns true.
 */
static u64_t __unpaged rdrand_read(void)
{
	u64_t val;
	unsigned char ok;
	/* RDRAND: rdrand %0 → CF=1 if valid, then setc captures CF */
	__asm__ volatile(
		"rdrand %0\n\t"
		"setc %1\n\t"
		: "=r" (val), "=qm" (ok)
		:
		: "cc");
	if (!ok) return 0;
	return val;
}

/* Declared in head.S (.unpaged.data) — read by head.S to get
 * the computed KASLR virtual offset for the second relocation call. */
extern u64_t kaslr_virt_offset_slot;

kinfo_t *pre_init(u64_t ebx, u64_t magic)
{
	assert(magic == MULTIBOOT_INFO_MAGIC);

	get_parameters(ebx, &kinfo);

	/* Acquire KASLR entropy from RDRAND if available.
	 * Falls back to 0 (no randomization) on CPUs without RDRAND
	 * or if KASLR is not enabled (seed stays 0 from BSS init).
	 */
#if defined(KASLR) && KASLR == 1
	if (rdrand_available()) {
		kinfo.kaslr_seed = rdrand_read();
		if (kinfo.kaslr_seed != 0) {
			direct_print("KASLR: RDRAND seed acquired\n");
		}

		/* Compute Virtual KASLR offset from seed.
		 * Range: 0 to 1022MB in 2MB steps (PDE granularity).
		 * The linked VMA is 0xFFFF8000F0100000, so the kernel
		 * can be mapped anywhere in the 0xFFFF8000F0100000 to
		 * 0xFFFF8000F0100000 + 1022MB range, giving 511 possible
		 * locations (0 + 511 * 2MB). The offset is 2MB-aligned
		 * for compatibility with the existing big-page mapping.
		 * This field is used by apply_relocations() (called from
		 * head.S before we reach here) and by pg_mapkernel(). */
		kinfo.kaslr_virt_offset = (kinfo.kaslr_seed & 0x1FF) * X86_64_BIG_PAGE_SIZE;

#if defined(KASLR_PIE) && KASLR_PIE == 1
		{
			char dbg[128];
			snprintf(dbg, sizeof(dbg),
				"KASLR: PIE build, virt_offset=0x%lx\n",
				(unsigned long)kinfo.kaslr_virt_offset);
			/* Only print non-zero offset */
			if (kinfo.kaslr_virt_offset != 0)
				direct_print(dbg);
		}
#endif
	} else {
		direct_print("KASLR: RDRAND not available, KASLR disabled\n");
	}
#else
	/* KASLR not enabled — seed stays 0 from BSS init */
#endif

	/* Store the computed virt_offset in the unpaged data slot
	 * so head.S can read it for the second apply_relocations() call. */
	kaslr_virt_offset_slot = kinfo.kaslr_virt_offset;

	pg_clear();
	pg_identity(&kinfo);
	/* Pass virt_offset to pg_mapkernel so it maps BOTH the linked
	 * VMA (for current execution) and the new VMA (for post-relocation).
	 * If offset is 0, only the linked VMA is mapped (no KASLR). */
	kinfo.freepde_start = pg_mapkernel(kinfo.kaslr_virt_offset);
	pg_load();
	vm_enable_paging();

	return &kinfo;
}

void send_diag_sig(void) { }
void minix_shutdown(int how) { arch_shutdown(how); }
void busy_delay_ms(int x) { }
int raise(int sig) { panic("raise(%d)\n", sig); }
