/* x86_64 page table utilities — adapted from i386:
 * - 512 entries per page table (PML4/PDP/PD/PT)
 * - 2MB large pages
 * - 64-bit PTE entries
 */

#include <minix/cpufeature.h>

#include <assert.h>
#include <machine/vm.h>
#include "kernel/kernel.h"
#include "arch_proto.h"

#include <string.h>

extern char _kern_vir_base, _kern_phys_base, _kern_size;

/* MSVC: &extern is not a compile-time constant — init in pg_identity/pg_mapkernel */
static phys_bytes kern_vir_start, kern_phys_start, kern_kernlen;
static int pg_utils_inited;

static void pg_utils_init_if_needed(void)
{
	if (!pg_utils_inited) {
		kern_vir_start = (phys_bytes) &_kern_vir_base;
		kern_phys_start = (phys_bytes) &_kern_phys_base;
		kern_kernlen = (phys_bytes) &_kern_size;
		pg_utils_inited = 1;
	}
}

/*
 * Page directory must be 4KB-aligned.
 * MSVC: __declspec(align()) for static arrays with C2099 workaround:
 * use a separate alignment declaration via __alignof.
 */
#ifdef _MSC_VER
__declspec(align(4096)) u64_t pagedir[512];
#else
_Alignas(4096) u64_t pagedir[512];
#endif

void print_memmap(kinfo_t *cbi)
{
        int m;
        assert(cbi->mmap_size < MAXMEMMAP);
        for(m = 0; m < cbi->mmap_size; m++) {
		phys_bytes addr = cbi->memmap[m].mm_base_addr, endit = cbi->memmap[m].mm_base_addr + cbi->memmap[m].mm_length;
                printf("%08lx-%08lx ",addr, endit);
        }
        printf("\nsize %08lx\n", cbi->mmap_size);
}

void cut_memmap(kinfo_t *cbi, phys_bytes start, phys_bytes end)
{
        int m;
        phys_bytes o;

        if((o=start % X86_64_PAGE_SIZE))
                start -= o;
        if((o=end % X86_64_PAGE_SIZE))
                end += X86_64_PAGE_SIZE - o;

	assert(kernel_may_alloc);

        for(m = 0; m < cbi->mmap_size; m++) {
                phys_bytes substart = start, subend = end;
                phys_bytes memaddr = cbi->memmap[m].mm_base_addr,
                        memend = cbi->memmap[m].mm_base_addr + cbi->memmap[m].mm_length;

                if(substart < memaddr) substart = memaddr;
                if(subend > memend) subend = memend;
                if(substart >= subend) continue;

                cbi->memmap[m].mm_base_addr = cbi->memmap[m].mm_length = 0;
                if(substart > memaddr)
                        add_memmap(cbi, memaddr, substart-memaddr);
                if(subend < memend)
                        add_memmap(cbi, subend, memend-subend);
        }
}

phys_bytes alloc_lowest(kinfo_t *cbi, phys_bytes len)
{
	int m;
#define EMPTY 0xffffffffffffffffULL
	phys_bytes lowest = EMPTY;
	assert(len > 0);
	len = roundup(len, X86_64_PAGE_SIZE);

	assert(kernel_may_alloc);

	for(m = 0; m < cbi->mmap_size; m++) {
		if(cbi->memmap[m].mm_length < len) continue;
		if(cbi->memmap[m].mm_base_addr < lowest) lowest = cbi->memmap[m].mm_base_addr;
	}
	assert(lowest != EMPTY);
	cut_memmap(cbi, lowest, len);
	cbi->kernel_allocated_bytes_dynamic += len;
	return lowest;
}

/*===========================================================================*
 *                              pg_remap_page                                 *
 *===========================================================================*/
/* Remap an existing page table entry to point to a different physical address.
 * Used by irq_thread_set_mmio() to map device MMIO regions into kernel space
 * for interrupt fast-ack. The page tables must already exist (set up by a
 * previous pg_map call or alloc_pagetable). Returns OK on success. */
int pg_remap_page(vir_bytes vaddr, phys_bytes new_phys)
{
    int pde = X86_64_VM_PDE(vaddr);
    int pte_idx = X86_64_VM_PTE(vaddr);
    extern u64_t pagedir[];
    u64_t pde_entry = pagedir[pde];

    if (!(pde_entry & X86_64_VM_PRESENT))
        return EFAULT;

    /* Convert the page table physical address back to virtual.
     * The page table was allocated from the static pagetables[] array
     * (in kernel data section), so virtual = physical + kernel_offset.
     * vir2phys() gives: physical = virtual - offset.
     * So: virtual = physical + offset. */
    extern char _kern_vir_base, _kern_phys_base;
    u64_t kernel_off = (vir_bytes)&_kern_vir_base - (vir_bytes)&_kern_phys_base;
    u64_t *pt = (u64_t *)(uintptr_t)((pde_entry & X86_64_VM_ADDR_MASK) + kernel_off);

    /* Set the new PTE (uncacheable for MMIO). */
    pt[pte_idx] = (new_phys & X86_64_VM_ADDR_MASK) |
                   X86_64_VM_PRESENT | X86_64_VM_WRITE |
                   X86_64_VM_PWT | X86_64_VM_PCD;

    /* Flush TLB for this single page. */
    __asm__ volatile("invlpg (%0)" : : "r" (vaddr) : "memory");

    return OK;
}

/*===========================================================================*
 *                              pg_unmap_page                                 *
 *===========================================================================*/
/* Unmap a page by clearing its PTE and flushing the TLB.
 * Used by irq_thread_unregister() to clean up MMIO mappings when a driver
 * removes its IRQ policy. The PDE must exist (present); if not, returns OK
 * (nothing to unmap). Returns OK on success. */
int pg_unmap_page(vir_bytes vaddr)
{
    int pde = X86_64_VM_PDE(vaddr);
    int pte_idx = X86_64_VM_PTE(vaddr);
    extern u64_t pagedir[];
    u64_t pde_entry = pagedir[pde];

    if (!(pde_entry & X86_64_VM_PRESENT))
        return OK;  /* nothing mapped — nothing to unmap */

    extern char _kern_vir_base, _kern_phys_base;
    u64_t kernel_off = (vir_bytes)&_kern_vir_base - (vir_bytes)&_kern_phys_base;
    u64_t *pt = (u64_t *)(uintptr_t)((pde_entry & X86_64_VM_ADDR_MASK) + kernel_off);

    /* Clear the PTE (mark as not present). */
    pt[pte_idx] = 0;

    /* Flush TLB for this single page. */
    __asm__ volatile("invlpg (%0)" : : "r" (vaddr) : "memory");

    return OK;
}

void add_memmap(kinfo_t *cbi, u64_t addr, u64_t len)
{
        int m;
#define LIMIT 0xFFFFFFFFFFF00000ULL
        if(addr > LIMIT) return;
        if(addr + len > LIMIT) {
                len -= (addr + len - LIMIT);
        }
        assert(cbi->mmap_size < MAXMEMMAP);
        if(len == 0) return;
	addr = roundup(addr, X86_64_PAGE_SIZE);
	len = rounddown(len, X86_64_PAGE_SIZE);

	assert(kernel_may_alloc);

        for(m = 0; m < MAXMEMMAP; m++) {
		phys_bytes highmark;
                if(cbi->memmap[m].mm_length) continue;
                cbi->memmap[m].mm_base_addr = addr;
                cbi->memmap[m].mm_length = len;
                cbi->memmap[m].mm_type = MULTIBOOT_MEMORY_AVAILABLE;
                if(m >= cbi->mmap_size)
                        cbi->mmap_size = m+1;
		highmark = addr + len;
		if(highmark > cbi->mem_high_phys) {
			cbi->mem_high_phys = highmark;
		}
                return;
        }

        panic("no available memmap slot");
}

u64_t *alloc_pagetable(phys_bytes *ph)
{
	u64_t *ret;
#define PG_PAGETABLES 6
	_Alignas(4096) static u64_t pagetables[PG_PAGETABLES][512];
	static int pt_inuse = 0;
	if(pt_inuse >= PG_PAGETABLES) panic("no more pagetables");
	assert(sizeof(pagetables[pt_inuse]) == X86_64_PAGE_SIZE);
	ret = pagetables[pt_inuse++];
	*ph = vir2phys(ret);
	return ret;
}

#define PAGE_KB (X86_64_PAGE_SIZE / 1024)

phys_bytes pg_alloc_page(kinfo_t *cbi)
{
	int m;
	multiboot_memory_map_t *mmap;

	assert(kernel_may_alloc);

	for(m = cbi->mmap_size-1; m >= 0; m--) {
		mmap = &cbi->memmap[m];
		if(!mmap->mm_length) continue;
		assert(mmap->mm_length > 0);
		assert(!(mmap->mm_length % X86_64_PAGE_SIZE));
		assert(!(mmap->mm_base_addr % X86_64_PAGE_SIZE));

		mmap->mm_length -= X86_64_PAGE_SIZE;

                cbi->kernel_allocated_bytes_dynamic += X86_64_PAGE_SIZE;

		return mmap->mm_base_addr + mmap->mm_length;
	}

	panic("can't find free memory");
}

void pg_identity(kinfo_t *cbi)
{
	uint32_t i;
	phys_bytes phys;

	pg_utils_init_if_needed();
	assert(cbi->mem_high_phys);

        for(i = 0; i < X86_64_VM_DIR_ENTRIES; i++) {
		u64_t flags = X86_64_VM_PRESENT | X86_64_VM_BIGPAGE
			| X86_64_VM_USER
			| X86_64_VM_WRITE;
                phys = i * X86_64_BIG_PAGE_SIZE;
		if((cbi->mem_high_phys & X86_64_VM_ADDR_MASK_2MB)
			<= (phys & X86_64_VM_ADDR_MASK_2MB)) {
			flags |= X86_64_VM_PWT | X86_64_VM_PCD;
		}
                pagedir[i] =  phys | flags;
        }
}

int pg_mapkernel(void)
{
	int pde;
	u64_t mapped = 0, kern_phys = kern_phys_start;

	pg_utils_init_if_needed();
        assert(!(kern_vir_start % X86_64_BIG_PAGE_SIZE));
        assert(!(kern_phys % X86_64_BIG_PAGE_SIZE));
        pde = kern_vir_start / X86_64_BIG_PAGE_SIZE;
	while(mapped < kern_kernlen) {
	        pagedir[pde] = kern_phys | X86_64_VM_PRESENT |
			X86_64_VM_BIGPAGE | X86_64_VM_WRITE;
		mapped += X86_64_BIG_PAGE_SIZE;
		kern_phys += X86_64_BIG_PAGE_SIZE;
		pde++;
	}
	return pde;
}

void vm_enable_paging(void)
{
        u64_t cr0, cr4;
        int pgeok;

        pgeok = _cpufeature(_CPUF_I386_PGE);

        cr0= read_cr0();
        cr4= read_cr4();

	assert(cr0 & X86_64_CR0_PE);

        write_cr0(cr0 & ~X86_64_CR0_PG);
        write_cr4(cr4 & ~(X86_64_CR4_PGE | X86_64_CR4_PAE));

        cr0= read_cr0();
        cr4= read_cr4();

        cr4 |= X86_64_CR4_PAE;
        write_cr4(cr4);

        cr0 |= X86_64_CR0_PG;
        write_cr0(cr0);
        cr0 |= X86_64_CR0_WP;
        write_cr0(cr0);

        if(pgeok)
                cr4 |= X86_64_CR4_PGE;

        write_cr4(cr4);
}

phys_bytes pg_load(void)
{
	phys_bytes phpagedir = vir2phys(pagedir);
        write_cr3(phpagedir);
	return phpagedir;
}

void pg_clear(void)
{
	memset(pagedir, 0, sizeof(pagedir));
}

phys_bytes pg_rounddown(phys_bytes b)
{
	phys_bytes o;
	if(!(o = b % X86_64_PAGE_SIZE))
		return b;
	return b - o;
}

void pg_map(phys_bytes phys, vir_bytes vaddr, vir_bytes vaddr_end,
	kinfo_t *cbi)
{
	static int mapped_pde = -1;
	static u64_t *pt = NULL;
	int pde, pte;

	assert(kernel_may_alloc);

	if(phys == PG_ALLOCATEME) {
		assert(!(vaddr % X86_64_PAGE_SIZE));
	} else  {
		assert((vaddr % X86_64_PAGE_SIZE) == (phys % X86_64_PAGE_SIZE));
		vaddr = pg_rounddown(vaddr);
		phys = pg_rounddown(phys);
	}
	assert(vaddr < kern_vir_start);

	while(vaddr < vaddr_end) {
		phys_bytes source = phys;
		assert(!(vaddr % X86_64_PAGE_SIZE));
		if(phys == PG_ALLOCATEME) {
			source = pg_alloc_page(cbi);
		} else {
			assert(!(phys % X86_64_PAGE_SIZE));
		}
		assert(!(source % X86_64_PAGE_SIZE));
		pde = X86_64_VM_PDE(vaddr);
		pte = X86_64_VM_PTE(vaddr);
		if(mapped_pde < pde) {
			phys_bytes ph;
			pt = alloc_pagetable(&ph);
			pagedir[pde] = (ph & X86_64_VM_ADDR_MASK)
		                | X86_64_VM_PRESENT | X86_64_VM_USER | X86_64_VM_WRITE;
			mapped_pde = pde;
		}
		assert(pt);
		pt[pte] = (source & X86_64_VM_ADDR_MASK) |
			X86_64_VM_PRESENT | X86_64_VM_USER | X86_64_VM_WRITE;
		vaddr += X86_64_PAGE_SIZE;
		if(phys != PG_ALLOCATEME)
			phys += X86_64_PAGE_SIZE;
	}
}

void pg_info(phys_bytes *pagedir_ph, u64_t **pagedir_v)
{
	*pagedir_ph = vir2phys(pagedir);
	*pagedir_v = pagedir;
}
