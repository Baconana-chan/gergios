/* ============================================================
 * pg_utils.c — ARM64 page table utilities
 *
 * Provides early boot page table setup and management for the
 * ARM64 kernel. This file implements the identity map, kernel
 * high VMA map, page allocation from the memory map, and other
 * boot-time MMU helpers.
 *
 * ARM64 page table structure (4KB pages, 48-bit VA, 4 levels):
 *   L0 (root):  512 entries × 8 bytes = 4KB, each covers 512GB
 *   L1:         512 entries × 8 bytes = 4KB, each covers 1GB
 *   L2 (PDE):   512 entries × 8 bytes = 4KB, each covers 2MB  ← VM PDE level
 *   L3 (PTE):   512 entries × 8 bytes = 4KB, each covers 4KB  ← VM PTE level
 *
 * VM server abstraction: PDE = L2, PTE = L3
 * L0 and L1 are managed by the kernel (this file).
 *
 * Block sizes:
 *   L0 block: 512GB (bit 1 = 0, not typically used)
 *   L1 block: 1GB   (bit 1 = 0)
 *   L2 block: 2MB   (bit 1 = 0)
 *   L3 page:  4KB   (bits[1:0] = 11)
 *
 * Descriptor type bits:
 *   Block:  bit 1 = 0 (at L0-L2)
 *   Table:  bit 1 = 1 (at L0-L2, points to next level)
 *   Page:   bits[1:0] = 11 (at L3 only)
 *   Invalid: bit 0 = 0
 *
 * TTBR split (48-bit VA, T0SZ=T1SZ=16):
 *   TTBR0_EL1:  VA[47] = 0  (lower half: 0x0000... - 0x7FFF...)
 *   TTBR1_EL1:  VA[47] = 1  (upper half: 0x8000... - 0xFFFF...)
 *
 *   Kernel at KERNEL_VBASE (0xFFFF800000000000):
 *   - Uses TTBR1_EL1 (VA[47] = 1)
 *   - Effective VA[47:0] for table walk = 0x800000000000
 *   - L0 index = bits[47:39] = 256
 *   - L1 index = bits[38:30] = 0
 *
 * References:
 *   ARM DDI 0487, Chapter D5 — VMSAv8-64 Translation Tables
 * ============================================================ */

#include <minix/type.h>
#include <minix/cpufeature.h>
#include <minix/board.h>
#include <assert.h>
#include <string.h>

#include "kernel/kernel.h"
#include "kernel/proc.h"
#include "arch_proto.h"
#include <machine/vm.h>

/* =========================================================================
 * Linker symbols (defined in kernel.lds)
 * ========================================================================= */

extern char _kern_vir_base, _kern_phys_base, _kern_size;

static phys_bytes kern_vir_start  = (phys_bytes)&_kern_vir_base;
static phys_bytes kern_phys_start = (phys_bytes)&_kern_phys_base;
static phys_bytes kern_kernlen    = (phys_bytes)&_kern_size;

/* =========================================================================
 * Page table storage
 *
 * We need three levels of tables for boot-time identity + kernel mapping:
 *
 *   L0 table (root): 512 entries × 8B = 4KB
 *     L0[0]   → L1_identity  (identity map for lower 512GB)
 *     L0[256] → L1_kernel    (kernel high map at 0xFFFF800000000000)
 *
 *   L1_identity table: 512 entries × 8B = 4KB
 *     L1[i] = 1GB block entry for physical addr i*1GB
 *
 *   L1_kernel table: 512 entries × 8B = 4KB
 *     L1[0]   → L2_kernel    (kernel at L1 offset 0 within 0xFFFF800...)
 *
 *   L2_kernel table: 512 entries × 8B = 4KB
 *     L2[i] = 2MB block entry for kernel section i
 *
 * Total: 4 tables × 4KB = 16KB for boot-time page tables
 * ========================================================================= */

#define PAGEDIR_ALIGN   4096

/* L0 root table */
_Alignas(PAGEDIR_ALIGN) static u64_t pagetable_l0[512];

/* L1 identity table (1GB blocks for identity map) */
_Alignas(PAGEDIR_ALIGN) static u64_t pagetable_l1_id[512];

/* L1 kernel table (for the kernel high mapping region) */
_Alignas(PAGEDIR_ALIGN) static u64_t pagetable_l1_kern[512];

/* L2 kernel table (2MB blocks for kernel sections) */
_Alignas(PAGEDIR_ALIGN) static u64_t pagetable_l2_kern[512];

/* Pre-allocated L2/L3 tables for dynamic mappings (pg_map) */
#define PG_PAGETABLES   24
_Alignas(PAGEDIR_ALIGN) static u64_t pagetable_pool[PG_PAGETABLES][512];

static int pt_pool_used = 0;

/* =========================================================================
 * alloc_pagetable — Allocate a page table from the static pool
 *
 * Returns a virtual pointer and fills *ph with the physical address.
 * The table is zeroed before return.
 * ========================================================================= */

static u64_t *alloc_pagetable(phys_bytes *ph)
{
	u64_t *ret;

	if (pt_pool_used >= PG_PAGETABLES) {
		panic("pg_utils: no more pre-allocated page tables");
	}

	ret = pagetable_pool[pt_pool_used++];
	memset(ret, 0, 512 * sizeof(u64_t));

	if (ph) {
		*ph = vir2phys(ret);
	}

	return ret;
}

/* =========================================================================
 * pg_roundup / pg_rounddown — Page alignment helpers
 * ========================================================================= */

phys_bytes pg_roundup(phys_bytes b)
{
	phys_bytes o;
	if ((o = b % AARCH64_PAGE_SIZE) == 0)
		return b;
	return b + AARCH64_PAGE_SIZE - o;
}

static phys_bytes pg_rounddown(phys_bytes b)
{
	phys_bytes o;
	if ((o = b % AARCH64_PAGE_SIZE) == 0)
		return b;
	return b - o;
}

/* =========================================================================
 * add_memmap — Add a memory region to the kernel memory map
 * ========================================================================= */

void add_memmap(kinfo_t *cbi, u64_t addr, u64_t len)
{
	int m;
#define LIMIT 0xFFFFF000UL

	if (addr > LIMIT) return;
	if (addr + len > LIMIT)
		len -= (addr + len - LIMIT);

	assert(cbi->mmap_size < MAXMEMMAP);
	if (len == 0) return;

	addr = pg_roundup((phys_bytes)addr);
	len  = (phys_bytes)(len & ~(AARCH64_PAGE_SIZE - 1));

	assert(kernel_may_alloc);

	for (m = 0; m < MAXMEMMAP; m++) {
		phys_bytes highmark;
		if (cbi->memmap[m].mm_length) continue;
		cbi->memmap[m].mm_base_addr = addr;
		cbi->memmap[m].mm_length    = len;
		cbi->memmap[m].type         = MULTIBOOT_MEMORY_AVAILABLE;
		if (m >= cbi->mmap_size)
			cbi->mmap_size = m + 1;
		highmark = addr + len;
		if (highmark > cbi->mem_high_phys)
			cbi->mem_high_phys = highmark;
		return;
	}
	panic("add_memmap: no available memmap slot");
}

/* =========================================================================
 * cut_memmap — Remove a memory region from the kernel memory map
 * ========================================================================= */

void cut_memmap(kinfo_t *cbi, phys_bytes start, phys_bytes end)
{
	int m;
	phys_bytes o;

	if ((o = start % AARCH64_PAGE_SIZE)) start -= o;
	if ((o = end   % AARCH64_PAGE_SIZE)) end += AARCH64_PAGE_SIZE - o;

	assert(kernel_may_alloc);

	for (m = 0; m < cbi->mmap_size; m++) {
		phys_bytes substart = start, subend = end;
		phys_bytes memaddr = cbi->memmap[m].mm_base_addr;
		phys_bytes memend  = memaddr + cbi->memmap[m].mm_length;

		if (substart < memaddr) substart = memaddr;
		if (subend   > memend)   subend   = memend;
		if (substart >= subend) continue;

		cbi->memmap[m].mm_base_addr = 0;
		cbi->memmap[m].mm_length    = 0;
		if (substart > memaddr)
			add_memmap(cbi, memaddr, substart - memaddr);
		if (subend < memend)
			add_memmap(cbi, subend, memend - subend);
	}
}

/* =========================================================================
 * pg_alloc_page — Allocate one physical page from the memory map
 * ========================================================================= */

static phys_bytes pg_alloc_page(kinfo_t *cbi)
{
	int m;
	multiboot_memory_map_t *mmap;

	assert(kernel_may_alloc);

	for (m = 0; m < cbi->mmap_size; m++) {
		mmap = &cbi->memmap[m];
		if (!mmap->mm_length) continue;
		assert(mmap->mm_length > 0);
		assert(!(mmap->mm_length      % AARCH64_PAGE_SIZE));
		assert(!(mmap->mm_base_addr   % AARCH64_PAGE_SIZE));

		phys_bytes addr = mmap->mm_base_addr;
		mmap->mm_base_addr += AARCH64_PAGE_SIZE;
		mmap->mm_length    -= AARCH64_PAGE_SIZE;
		cbi->kernel_allocated_bytes_dynamic += AARCH64_PAGE_SIZE;
		return addr;
	}
	panic("pg_alloc_page: no free memory");
}

/* =========================================================================
 * alloc_lowest — Allocate contiguous physical memory (lowest available)
 * ========================================================================= */

phys_bytes alloc_lowest(kinfo_t *cbi, phys_bytes len)
{
	int m;
	phys_bytes aligned = pg_roundup(len);

	assert(kernel_may_alloc);

	for (m = 0; m < cbi->mmap_size; m++) {
		multiboot_memory_map_t *mmap = &cbi->memmap[m];
		if (mmap->mm_length < aligned) continue;
		phys_bytes addr = mmap->mm_base_addr;
		mmap->mm_base_addr += aligned;
		mmap->mm_length    -= aligned;
		cbi->kernel_allocated_bytes_dynamic += aligned;
		return addr;
	}
	panic("alloc_lowest: no contiguous memory");
}

/* =========================================================================
 * pg_identity — Create identity page table
 *
 * Sets up the 4-level page table hierarchy:
 *   L0[0] → L1_identity
 *   L1_identity[0..1] → 1GB block entries covering first 2GB
 *
 * This identity-maps the first 2GB of physical memory so the kernel
 * can access all boot-critical hardware (UART at 0x09000000, GIC at
 * 0x08000000, RAM at 0x40000000, kernel code at 0x00080000).
 * ========================================================================= */

void pg_identity(kinfo_t *cbi)
{
	int i;

	assert(cbi->mem_high_phys);

	/* Clear all tables */
	memset(pagetable_l0,      0, sizeof(pagetable_l0));
	memset(pagetable_l1_id,   0, sizeof(pagetable_l1_id));
	memset(pagetable_l1_kern, 0, sizeof(pagetable_l1_kern));
	memset(pagetable_l2_kern, 0, sizeof(pagetable_l2_kern));

	/* ---- Link L0[0] → L1_identity (for identity-mapped lower half) ---- */
	phys_bytes l1_id_phys = vir2phys(pagetable_l1_id);
	pagetable_l0[0] = l1_id_phys
	                | AARCH64_VM_PRESENT
	                | AARCH64_VM_TABLE      /* bit 1 = 1: table descriptor */
	                | AARCH64_VM_AF;

	/* ---- Link L0[256] → L1_kernel (for kernel high mapping) ---- */
	/* L0[256] = VA[47:39] for address 0x800000000000 (KERNEL_VBASE stripped) */
	phys_bytes l1_kern_phys = vir2phys(pagetable_l1_kern);
	pagetable_l0[256] = l1_kern_phys
	                  | AARCH64_VM_PRESENT
	                  | AARCH64_VM_TABLE
	                  | AARCH64_VM_AF;

	/* ---- Fill L1_identity with 1GB block entries ---- */
	u64_t id_flags = AARCH64_VM_PRESENT       /* Valid */
	               | AARCH64_VM_BLOCK         /* Block type (bit 1 = 0) */
	               | AARCH64_VM_AF            /* Access Flag */
	               | AARCH64_VM_NORMAL        /* Attr0: Normal WB */
	               | AARCH64_VM_SH_IS         /* Inner Shareable */
	               | ((u64_t)1 << 53)         /* PXN */
	               | ((u64_t)1 << 54);        /* UXN */

	for (i = 0; i < 2; i++) {   /* First 2GB */
		phys_bytes phys = (phys_bytes)i * 0x40000000ULL;  /* 1GB */
		pagetable_l1_id[i] = phys | id_flags;
	}

	/* Mark rest of L1_identity as invalid */
	for (i = 2; i < 512; i++)
		pagetable_l1_id[i] = 0;
}

/* =========================================================================
 * pg_mapkernel — Map kernel sections in high VMA space
 *
 * Structure:
 *   L0[256] → L1_kernel (already set up by pg_identity)
 *   L1_kernel[0] → L2_kernel (2MB block table)
 *   L2_kernel[i] → 2MB block for kernel section i
 *
 * Returns:
 *   Number of 2MB PDE entries used (for VM bookkeeping).
 * ========================================================================= */

int pg_mapkernel(void)
{
	int pde;
	phys_bytes mapped = 0;
	phys_bytes kern_phys = kern_phys_start;

	assert(!(kern_vir_start % AARCH64_BIG_PAGE_SIZE));
	assert(!(kern_phys_start % AARCH64_BIG_PAGE_SIZE));

	/* Link L1_kernel[0] → L2_kernel table */
	phys_bytes l2_phys = vir2phys(pagetable_l2_kern);
	pagetable_l1_kern[0] = l2_phys
	                     | AARCH64_VM_PRESENT
	                     | AARCH64_VM_TABLE
	                     | AARCH64_VM_AF;

	/* Fill L2_kernel with 2MB block entries */
	u64_t block_flags = AARCH64_VM_PRESENT
	                  | AARCH64_VM_BLOCK
	                  | AARCH64_VM_AF
	                  | AARCH64_VM_NORMAL
	                  | AARCH64_VM_SH_IS;

	pde = 0;
	while (mapped < kern_kernlen && pde < 512) {
		pagetable_l2_kern[pde] = (kern_phys & AARCH64_VM_ADDR_MASK_2MB)
		                        | block_flags;
		mapped    += AARCH64_BIG_PAGE_SIZE;
		kern_phys += AARCH64_BIG_PAGE_SIZE;
		pde++;
	}

	return pde;
}

/* =========================================================================
 * vm_enable_paging — Enable virtual memory (MMU)
 *
 * Loads TTBR0_EL1 with the L0 page table and enables MMU if disabled.
 * If MMU is already enabled (from head.S), this is a no-op.
 * ========================================================================= */

void vm_enable_paging(void)
{
	phys_bytes ttbr0_val = vir2phys(pagetable_l0);

	write_ttbr0(ttbr0_val);
	dsb_sy();
	isb();

	uint64_t sctlr;
	__asm__ volatile("mrs %0, sctlr_el1" : "=r"(sctlr));

	if (!(sctlr & 1)) {
		sctlr |= 1;              /* M: MMU enable */
		sctlr |= 4;              /* C: data cache */
		sctlr |= (1 << 12);      /* I: instruction cache */
		__asm__ volatile("msr sctlr_el1, %0" : : "r"(sctlr));
		isb();
	}
}

/* =========================================================================
 * pg_load — Load L0 page directory into TTBR0_EL1
 *
 * Returns physical address of L0 table.
 * ========================================================================= */

phys_bytes pg_load(void)
{
	phys_bytes phpagedir = vir2phys(pagetable_l0);
	write_ttbr0(phpagedir);
	return phpagedir;
}

/* =========================================================================
 * pg_clear — Clear all boot-time page tables
 * ========================================================================= */

void pg_clear(void)
{
	memset(pagetable_l0,      0, sizeof(pagetable_l0));
	memset(pagetable_l1_id,   0, sizeof(pagetable_l1_id));
	memset(pagetable_l1_kern, 0, sizeof(pagetable_l1_kern));
	memset(pagetable_l2_kern, 0, sizeof(pagetable_l2_kern));
}

/* =========================================================================
 * pg_info — Return physical and virtual addresses of L0 page directory
 * ========================================================================= */

void pg_info(reg_t *pagedir_ph, u32_t **pagedir_v)
{
	*pagedir_ph = (reg_t)vir2phys(pagetable_l0);
	*pagedir_v  = (u32_t *)pagetable_l0;
}

/* =========================================================================
 * pg_map — Map physical memory at a given virtual address (4KB pages)
 *
 * Creates L3 page entries for fine-grained mapping. Virtual addresses
 * must be in the lower half (TTBR0_EL1 space, VA[47]=0).
 * ========================================================================= */

#define PG_ALLOCATEME ((phys_bytes)-1)

void pg_map(phys_bytes phys, vir_bytes vaddr, vir_bytes vaddr_end,
	    kinfo_t *cbi)
{
	static int mapped_l0_idx = -1;
	static u64_t *l1_table  = NULL;
	static int mapped_l1_idx = -1;
	static u64_t *l2_table  = NULL;

	assert(kernel_may_alloc);

	if (phys == PG_ALLOCATEME) {
		assert(!(vaddr % AARCH64_PAGE_SIZE));
	} else {
		assert((vaddr % AARCH64_PAGE_SIZE) == (phys % AARCH64_PAGE_SIZE));
		vaddr = pg_rounddown(vaddr);
		phys  = pg_rounddown(phys);
	}
	assert(vaddr < kern_vir_start);

	while (vaddr < vaddr_end) {
		phys_bytes source = phys;
		assert(!(vaddr % AARCH64_PAGE_SIZE));
		if (phys == PG_ALLOCATEME) {
			source = pg_alloc_page(cbi);
		} else {
			assert(!(phys % AARCH64_PAGE_SIZE));
		}
		assert(!(source % AARCH64_PAGE_SIZE));

		int l0_idx = AARCH64_VM_L0E(vaddr);
		int l1_idx = AARCH64_VM_L1E(vaddr);
		int l2_idx = AARCH64_VM_L2E(vaddr);   /* PDE */
		int l3_idx = AARCH64_VM_L3E(vaddr);   /* PTE */

		/* Allocate L1 table if needed */
		if (mapped_l0_idx != l0_idx) {
			phys_bytes l1_phys;
			l1_table = alloc_pagetable(&l1_phys);
			pagetable_l0[l0_idx] = l1_phys
			                    | AARCH64_VM_PRESENT
			                    | AARCH64_VM_TABLE
			                    | AARCH64_VM_AF;
			mapped_l0_idx = l0_idx;
			mapped_l1_idx = -1;  /* Force L2 recheck */
		}

		/* Allocate L2 table if needed */
		if (mapped_l1_idx != l1_idx) {
			phys_bytes l2_phys;
			l2_table = alloc_pagetable(&l2_phys);
			l1_table[l1_idx] = l2_phys
			                 | AARCH64_VM_PRESENT
			                 | AARCH64_VM_TABLE
			                 | AARCH64_VM_AF;
			mapped_l1_idx = l1_idx;
		}

		/* Allocate L3 table if needed */
		if (!(l2_table[l2_idx] & AARCH64_VM_PRESENT)) {
			phys_bytes l3_phys;
			u64_t *l3_table = alloc_pagetable(&l3_phys);
			l2_table[l2_idx] = l3_phys
			                 | AARCH64_VM_PRESENT
			                 | AARCH64_VM_TABLE
			                 | AARCH64_VM_AF;
		}

		/* Set L3 page entry */
		u64_t page_flags = AARCH64_VM_PRESENT
		                 | AARCH64_VM_PAGE       /* Page type (bits[1:0]=11) */
		                 | AARCH64_VM_AF
		                 | AARCH64_VM_NORMAL
		                 | AARCH64_VM_SH_IS
		                 | AARCH64_VM_USER
		                 | AARCH64_VM_RW;

		u64_t *l3_base = (u64_t *)(uintptr_t)(l2_table[l2_idx]
		                    & AARCH64_VM_ADDR_MASK);
		l3_base[l3_idx] = (source & AARCH64_VM_ADDR_MASK) | page_flags;

		vaddr += AARCH64_PAGE_SIZE;
		if (phys != PG_ALLOCATEME)
			phys += AARCH64_PAGE_SIZE;
	}
}

/* =========================================================================
 * pg_mapproc — Set up initial page table for a user process
 *
 * Allocates an L0 table for the process and copies the kernel
 * high mapping (L0[256:511]) from the kernel's boot page tables.
 * The user half (L0[0:255]) is left empty — it will be filled by
 * the VM server during exec().
 *
 * Parameters:
 *   p   Process to set up page tables for.
 *   ip  Boot image descriptor.
 *   cbi Kernel boot info.
 *
 * FIXME (Phase 3+): Full user process page table setup with proper
 * user mappings for text, data, stack segments.
 * ========================================================================= */

void pg_mapproc(struct proc *p, struct boot_image *ip, kinfo_t *cbi)
{
	phys_bytes l0_phys;
	u64_t *l0_table = alloc_pagetable(&l0_phys);

	/* Copy kernel high mapping (L0[256:511]) from boot tables.
	 * L0 indices 256-511 cover the kernel space (VA[47]=1).
	 * These entries (L1 table pointers) are shared with the kernel. */
	memcpy(&l0_table[256], &pagetable_l0[256], 256 * sizeof(u64_t));

	/* Store the page table base in the process structure.
	 * p_seg.p_ttbr is the TTBR0_EL1 value for this process.
	 * p_seg.p_ttbr_v is the virtual address for kernel access. */
	p->p_seg.p_ttbr   = l0_phys;
	p->p_seg.p_ttbr_v = l0_table;

	(void)ip;   /* TODO: parse ip for text/data/bss segments */
	(void)cbi;
}
