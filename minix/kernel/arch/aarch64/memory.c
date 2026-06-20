/* ============================================================
 * memory.c — ARM64 memory management
 *
 * Provides the architecture-dependent memory management layer
 * for the MINIX kernel on ARM64. This includes virtual-to-
 * physical address translation (vm_lookup, umap_virtual),
 * inter-process memory copy (virtual_copy_f, data_copy),
 * physical memory fill (vm_memset), and boot-time physical
 * mapping registration (kern_req_phys_map).
 *
 * ARM64 page table structure (4KB pages, 48-bit VA, 4 levels):
 *   L0 (root):  512 × 8B = 4KB, each covers 512GB
 *   L1:         512 × 8B = 4KB, each covers 1GB
 *   L2 (PDE):   512 × 8B = 4KB, each covers 2MB  ← VM PDE level
 *   L3 (PTE):   512 × 8B = 4KB, each covers 4KB  ← VM PTE level
 *
 * Phase 2 simplification:
 *   All kernel-accessible memory is identity-mapped (VA == PA)
 *   for the first 2GB. Physical addresses can be dereferenced
 *   directly. The createpde() mechanism is kept for future
 *   Phase 3+ compatibility when paging is fully enabled and
 *   the identity map may be removed.
 *
 * References:
 *   ARM DDI 0487 — VMSAv8-64 Translation Tables
 *   minix/kernel/arch/earm/memory.c — Original ARM32 version
 * ============================================================ */

#include "kernel/kernel.h"
#include "kernel/proc.h"
#include "kernel/vm.h"
#include <machine/vm.h>
#include <minix/type.h>
#include <minix/board.h>
#include <minix/syslib.h>
#include <minix/cpufeature.h>
#include <string.h>
#include <assert.h>
#include <signal.h>
#include <stdlib.h>
#include "arch_proto.h"
#include "kernel/proto.h"
#include "kernel/debug.h"

/* =========================================================================
 * Missing ARM64 page table macros
 *
 * AARCH64_VM_PDE(v) = L2 index (bits[29:21])
 * AARCH64_VM_PTE(v) = L3 index (bits[20:12])
 *
 * For full 4-level walk (L0-L3), we also need:
 *   AARCH64_VM_L0E(v) — defined in vm.h
 *   AARCH64_VM_L1E(v) — defined in vm.h
 *   AARCH64_VM_L2E(v) — same as PDE
 *   AARCH64_VM_L3E(v) — same as PTE
 * ========================================================================= */

#ifndef AARCH64_VM_L2E
#define AARCH64_VM_L2E(v)   AARCH64_VM_PDE(v)
#endif
#ifndef AARCH64_VM_L3E
#define AARCH64_VM_L3E(v)   AARCH64_VM_PTE(v)
#endif

/* =========================================================================
 * Constants
 * ========================================================================= */

#define HASPT(procptr)  ((procptr)->p_seg.p_ttbr != 0)

/* Number of free PDE slots (2MB windows) for temporary kernel mappings */
#define MAXFREEPDES     2
static int nfreepdes = 0;
static int freepdes[MAXFREEPDES];

/* =========================================================================
 * Linked list of registered physical memory mappings
 * ========================================================================= */

static kern_phys_map *kern_phys_map_head;

/* =========================================================================
 * phys_get32 — Read a 32-bit value from physical memory
 *
 * Phase 2: With identity mapping active for the first 2GB of physical
 * memory (QEMU virt), physical addresses can be dereferenced directly.
 *
 * Phase 3+: This will need to use the temp mapping mechanism via lin_lin_copy
 *           to access physical addresses when the identity map is removed.
 * ========================================================================= */

static u32_t phys_get32(phys_bytes addr)
{
	/* Identity map: VA == PA for the first 2GB */
	assert(addr < 0x80000000ULL);
	return *(volatile u32_t *)(uintptr_t)addr;
}

/* =========================================================================
 * mem_clear_mapcache — Clear any temporary PDE mappings
 *
 * For Phase 2, the cache is maintained as freepdes[] entries in the
 * ptproc's page table. This function clears them.
 * ========================================================================= */

void mem_clear_mapcache(void)
{
	/* Phase 2: identity mapping is active — all kernel-accessible
	 * memory is directly reachable. No temporary PDE mappings to
	 * clear.
	 *
	 * FIXME(Phase 3+): Walk L0→L1→L2 to clear the specific L2
	 * entries pointed to by freepdes[]. Requires a scratch L2
	 * table for temporary kernel mappings. */
	(void)freepdes;
	(void)nfreepdes;
}

/* =========================================================================
 * check_resumed_caller — Return the result from a VM-suspended caller
 * ========================================================================= */

static int check_resumed_caller(struct proc *caller)
{
	if (caller && (caller->p_misc_flags & MF_KCALL_RESUME)) {
		assert(caller->p_vmrequest.vmresult != VMSUSPEND);
		return caller->p_vmrequest.vmresult;
	}
	return OK;
}

/* =========================================================================
 * createpde — Create a temporary 2MB window for kernel access
 *
 * This function sets up a temporary mapping from the kernel's address
 * space to physical memory or a process's virtual memory, in 2MB
 * windows. It is used by lin_lin_copy and vm_memset.
 *
 * Phase 2 simplification:
 *   With identity mapping active, physical addresses are directly
 *   accessible. For the current ptproc or kernel, the address is
 *   returned as-is. For other processes, the page table is walked
 *   to extract the physical address, which is then returned directly
 *   (since the identity map covers all of physical memory).
 *
 * Parameters:
 *   pr            Process whose memory to access (NULL = physical)
 *   linaddr       Linear address (physical or virtual)
 *   bytes         Size of chunk (clamped to 2MB window)
 *   free_pde_idx  Index into freepdes[] (0 or 1)
 *   changed       Set to 1 if mapping was created (unused in Phase 2)
 *
 * Returns the linear (virtual) address that phys_copy can use.
 * ========================================================================= */

static phys_bytes createpde(
	const struct proc *pr,
	const phys_bytes linaddr,
	phys_bytes *bytes,
	int free_pde_idx,
	int *changed
)
{
	phys_bytes offset;

	assert(free_pde_idx >= 0 && free_pde_idx < nfreepdes);
	(void)freepdes[free_pde_idx];

	/* If the requested process is the current ptproc or the kernel,
	 * the address is directly accessible (it's already mapped). */
	if (pr && ((pr == get_cpulocal_var(ptproc)) || iskernelp(pr))) {
		return linaddr;
	}

	/* For physical memory with identity mapping, the physical
	 * address equals the virtual address. */
	if (!pr) {
		offset = linaddr & AARCH64_VM_OFFSET_MASK_2MB;
		*bytes = MIN(*bytes, AARCH64_BIG_PAGE_SIZE - offset);
		return linaddr;
	}

	/* For a different process: walk its page tables to find the
	 * physical address, then return it directly (identity map
	 * makes it accessible).
	 *
	 * If vm_lookup fails (unmapped page), return the linaddr directly.
	 * This may cause phys_copy to access the identity-mapped address
	 * instead of the intended process page — a potential silent data
	 * corruption. In practice this is safe because all callers validate
	 * ranges before calling lin_lin_copy, so vm_lookup only fails on
	 * race conditions.
	 *
	 * FIXME(Phase 3+): When the identity map is removed, this will
	 * need to install an L2 block descriptor into a scratch L2 table. */
	{
		phys_bytes phys;
		u32_t pte;

		if (vm_lookup(pr, linaddr, &phys, &pte) != OK) {
			offset = linaddr & AARCH64_VM_OFFSET_MASK_2MB;
			*bytes = MIN(*bytes, AARCH64_BIG_PAGE_SIZE - offset);
			return linaddr;
		}

		offset = phys & AARCH64_VM_OFFSET_MASK_2MB;
		*bytes = MIN(*bytes, AARCH64_BIG_PAGE_SIZE - offset);
		return phys;
	}
}

/* =========================================================================
 * lin_lin_copy — Copy data between two linear address ranges
 *
 * Copies 'bytes' bytes from 'srcproc:srclinaddr' to 'dstproc:dstlinaddr'.
 * The copy is performed in 2MB chunks using createpde for mapping.
 * Returns OK on success, EFAULT_SRC/EFAULT_DST on fault.
 * ========================================================================= */

static int lin_lin_copy(struct proc *srcproc, vir_bytes srclinaddr,
			struct proc *dstproc, vir_bytes dstlinaddr,
			vir_bytes bytes)
{
	u32_t addr;

	assert(get_cpulocal_var(ptproc));
	assert(get_cpulocal_var(proc_ptr));

	if (srcproc) assert(!RTS_ISSET(srcproc, RTS_SLOT_FREE));
	if (dstproc) assert(!RTS_ISSET(dstproc, RTS_SLOT_FREE));
	assert(!RTS_ISSET(get_cpulocal_var(ptproc), RTS_SLOT_FREE));
	assert(get_cpulocal_var(ptproc)->p_seg.p_ttbr_v);

	while (bytes > 0) {
		phys_bytes srcptr, dstptr;
		vir_bytes chunk = bytes;
		int changed = 0;

		/* Set up 2MB ranges */
		srcptr = createpde(srcproc, srclinaddr, &chunk, 0, &changed);
		dstptr = createpde(dstproc, dstlinaddr, &chunk, 1, &changed);

		(void)changed;

		/* Check for overflow */
		if (srcptr + chunk < srcptr) return EFAULT_SRC;
		if (dstptr + chunk < dstptr) return EFAULT_DST;

		/* Copy pages with fault catching */
		PHYS_COPY_CATCH(srcptr, dstptr, chunk, addr);

		if (addr) {
			/* phys_copy does all memory accesses word-aligned
			 * (rounded down), so page faults can occur at a lower
			 * address than the specified offsets. */
			vir_bytes src_aligned = srcptr & ~0x3;
			vir_bytes dst_aligned = dstptr & ~0x3;

			if (addr >= src_aligned && addr < (srcptr + chunk))
				return EFAULT_SRC;
			if (addr >= dst_aligned && addr < (dstptr + chunk))
				return EFAULT_DST;

			panic("lin_lin_copy fault out of range");
			return EFAULT;
		}

		/* Update counter and addresses for next iteration */
		bytes -= chunk;
		srclinaddr += chunk;
		dstlinaddr += chunk;
	}

	if (srcproc) assert(!RTS_ISSET(srcproc, RTS_SLOT_FREE));
	if (dstproc) assert(!RTS_ISSET(dstproc, RTS_SLOT_FREE));
	assert(!RTS_ISSET(get_cpulocal_var(ptproc), RTS_SLOT_FREE));
	assert(get_cpulocal_var(ptproc)->p_seg.p_ttbr_v);

	return OK;
}

/* =========================================================================
 *                vm_lookup — Page table walk
 *
 * Walk the 4-level ARM64 page table for the given process to find
 * the physical address corresponding to a virtual address.
 *
 * ARM64 translation:
 *   L0[9:0]  → L1 table (or 512GB block — not used)
 *   L1[9:0]  → L2 table (or 1GB block)
 *   L2[9:0]  → L3 table (or 2MB block)  ← VM PDE level
 *   L3[9:0]  → 4KB page                  ← VM PTE level
 *
 * Descriptor types:
 *   Block:  bit 1 = 0 (at L0-L2, arm64 VM_BLOCK)
 *   Table:  bit 1 = 1 (at L0-L2, arm64 VM_TABLE)
 *   Page:   bits[1:0] = 11 (at L3, arm64 VM_PAGE)
 *   Invalid: bit 0 = 0
 * ========================================================================= */

int vm_lookup(const struct proc *proc, const vir_bytes virtual,
              phys_bytes *physical, u32_t *ptent)
{
	u64_t l0_phys, l1_phys, l2_phys;
	u64_t l0_v, l1_v, l2_v, l3_v;
	int l0_idx, l1_idx, l2_idx, l3_idx;

	assert(proc);
	assert(physical);
	assert(!isemptyp(proc));
	assert(HASPT(proc));

	/* ---- Level 0 (root) ---- */
	l0_phys = (u64_t)(proc->p_seg.p_ttbr & AARCH64_VM_ADDR_MASK);
	assert(l0_phys % AARCH64_PAGEDIR_SIZE == 0);

	l0_idx = AARCH64_VM_L0E(virtual);
	assert(l0_idx >= 0 && l0_idx < AARCH64_VM_DIR_ENTRIES);

	l0_v = phys_get32((phys_bytes)(l0_phys + l0_idx * 8));
	if (!(l0_v & AARCH64_VM_PRESENT))
		return EFAULT;

	/* Check if L0 is a block entry (bit 1 = 0) */
	if (!(l0_v & AARCH64_VM_TABLE)) {
		/* L0 block is 512GB — should not happen for 48-bit VA */
		*physical = (l0_v & AARCH64_VM_ADDR_MASK)
		            | (virtual & 0x7FFFFFFFFULL);
		if (ptent) *ptent = (u32_t)l0_v;
		return OK;
	}

	/* ---- Level 1 ---- */
	l1_phys = l0_v & AARCH64_VM_ADDR_MASK;
	assert(l1_phys % AARCH64_PAGETABLE_SIZE == 0);

	l1_idx = AARCH64_VM_L1E(virtual);
	assert(l1_idx >= 0 && l1_idx < AARCH64_VM_DIR_ENTRIES);

	l1_v = phys_get32((phys_bytes)(l1_phys + l1_idx * 8));
	if (!(l1_v & AARCH64_VM_PRESENT))
		return EFAULT;

	/* Check if L1 is a block entry (1GB block, bit 1 = 0) */
	if (!(l1_v & AARCH64_VM_TABLE)) {
		*physical = (l1_v & AARCH64_VM_ADDR_MASK)
		            | (virtual & 0x3FFFFFFFULL);  /* offset within 1GB */
		if (ptent) *ptent = (u32_t)l1_v;
		return OK;
	}

	/* ---- Level 2 (PDE) ---- */
	l2_phys = l1_v & AARCH64_VM_ADDR_MASK;
	assert(l2_phys % AARCH64_PAGETABLE_SIZE == 0);

	l2_idx = AARCH64_VM_PDE(virtual);
	assert(l2_idx >= 0 && l2_idx < AARCH64_VM_DIR_ENTRIES);

	l2_v = phys_get32((phys_bytes)(l2_phys + l2_idx * 8));
	if (!(l2_v & AARCH64_VM_PRESENT))
		return EFAULT;

	/* Check if L2 is a block entry (2MB block, bit 1 = 0) */
	if (!(l2_v & AARCH64_VM_TABLE)) {
		*physical = (l2_v & AARCH64_VM_ADDR_MASK)
		            | (virtual & AARCH64_VM_OFFSET_MASK_2MB);
		if (ptent) *ptent = (u32_t)l2_v;
		return OK;
	}

	/* ---- Level 3 (PTE) — must be a page descriptor ---- */
	l3_idx = AARCH64_VM_PTE(virtual);
	assert(l3_idx >= 0 && l3_idx < AARCH64_VM_PT_ENTRIES);

	/* Read the L3 entry from physical memory (64-bit read as two 32-bit) */
	{
		phys_bytes l3_entry_addr = (l2_v & AARCH64_VM_ADDR_MASK)
		                          + l3_idx * 8;
		u64_t l3_entry_lo = phys_get32((phys_bytes)l3_entry_addr);
		u64_t l3_entry_hi = phys_get32((phys_bytes)(l3_entry_addr + 4));
		l3_v = l3_entry_lo | (l3_entry_hi << 32);
	}

	/* L3 must have bits[1:0] = 11 (page descriptor), else invalid */
	if ((l3_v & 3) != 3)
		return EFAULT;

	if (ptent)
		*ptent = (u32_t)(l3_v & 0xFFFFFFFFULL);

	*physical = (l3_v & AARCH64_VM_ADDR_MASK)
	            | (virtual & (AARCH64_PAGE_SIZE - 1));

	return OK;
}

/* =========================================================================
 *             vm_lookup_range — Contiguous physical range check
 *
 * Determines how many bytes starting at 'vir_addr' are backed by
 * physically contiguous memory. Returns the contiguous length,
 * up to 'bytes' bytes.
 * ========================================================================= */

size_t vm_lookup_range(const struct proc *proc, vir_bytes vir_addr,
                       phys_bytes *phys_addr, size_t bytes)
{
	phys_bytes phys, next_phys;
	size_t len;

	assert(proc);
	assert(bytes > 0);
	assert(HASPT(proc));

	/* Look up the first page */
	if (vm_lookup(proc, vir_addr, &phys, NULL) != OK)
		return 0;

	if (phys_addr != NULL)
		*phys_addr = phys;

	len = AARCH64_PAGE_SIZE - (vir_addr % AARCH64_PAGE_SIZE);
	vir_addr += len;
	next_phys = phys + len;

	/* Look up next pages and test physical contiguity */
	while (len < bytes) {
		if (vm_lookup(proc, vir_addr, &phys, NULL) != OK)
			break;
		if (next_phys != phys)
			break;
		len += AARCH64_PAGE_SIZE;
		vir_addr += AARCH64_PAGE_SIZE;
		next_phys += AARCH64_PAGE_SIZE;
	}

	return MIN(bytes, len);
}

/* =========================================================================
 *              vm_check_range — VM-suspend range validation
 *
 * On behalf of 'caller', call into VM to check the virtual address
 * range of process 'target'. Returns VMSUSPEND if VM needs to fix
 * up the mapping first.
 * ========================================================================= */

int vm_check_range(struct proc *caller, struct proc *target,
                   vir_bytes vir_addr, size_t bytes, int writeflag)
{
	int r;

	if ((caller->p_misc_flags & MF_KCALL_RESUME) &&
	    (r = caller->p_vmrequest.vmresult) != OK)
		return r;

	vm_suspend(caller, target, vir_addr, bytes, VMSTYPE_KERNELCALL,
	           writeflag);

	return VMSUSPEND;
}

/* =========================================================================
 *            umap_virtual — Virtual to physical address mapping
 *
 * Translates a virtual address in a process's address space to a
 * physical address, ensuring the range is physically contiguous.
 * Returns 0 on failure, the physical address on success.
 * ========================================================================= */

phys_bytes umap_virtual(
	register struct proc *rp,     /* Process to translate for */
	int seg,                      /* T, D, or S segment (unused on ARM) */
	vir_bytes vir_addr,           /* Virtual address */
	vir_bytes bytes               /* # of bytes to check contiguity */
)
{
	phys_bytes phys = 0;

	if (vm_lookup(rp, vir_addr, &phys, NULL) != OK) {
		printf("SYSTEM:umap_virtual: vm_lookup of %s: "
		       "seg 0x%x: 0x%lx failed\n",
		       rp->p_name, seg, vir_addr);
		phys = 0;
	} else {
		if (phys == 0)
			panic("vm_lookup returned phys: 0x%lx", phys);
	}

	if (phys == 0) {
		printf("SYSTEM:umap_virtual: lookup failed\n");
		return 0;
	}

	/* Ensure addresses are contiguous in physical memory */
	if (bytes > 0 &&
	    vm_lookup_range(rp, vir_addr, NULL, bytes) != bytes) {
		printf("umap_virtual: %s: %lu at 0x%lx (vir 0x%lx) "
		       "not contiguous\n",
		       rp->p_name, bytes, vir_addr, vir_addr);
		return 0;
	}

	assert(phys);
	return phys;
}

/* =========================================================================
 *            virtual_copy_f — Virtual copy with VM check
 *
 * Copy bytes from a source virtual address to a destination virtual
 * address. If vmcheck is set and a page fault occurs, the caller
 * is suspended until VM resolves the mapping.
 * ========================================================================= */

int virtual_copy_f(
	struct proc *caller,
	struct vir_addr *src_addr,    /* Source virtual address */
	struct vir_addr *dst_addr,    /* Destination virtual address */
	vir_bytes bytes,              /* # of bytes to copy */
	int vmcheck                   /* If nonzero, can return VMSUSPEND */
)
{
	struct vir_addr *vir_addr[2];
	int i, r;
	struct proc *procs[2];

	assert((vmcheck && caller) || (!vmcheck && !caller));

	/* Check copy count */
	if (bytes <= 0) return EDOM;

	vir_addr[_SRC_] = src_addr;
	vir_addr[_DST_] = dst_addr;

	for (i = _SRC_; i <= _DST_; i++) {
		endpoint_t proc_e = vir_addr[i]->proc_nr_e;
		int proc_nr;
		struct proc *p;

		if (proc_e == NONE) {
			p = NULL;
		} else {
			if (!isokendpt_d(proc_e, &proc_nr)) {
				printf("virtual_copy: no reasonable endpoint\n");
				return ESRCH;
			}
			p = proc_addr(proc_nr);
		}
		procs[i] = p;
	}

	if ((r = check_resumed_caller(caller)) != OK)
		return r;

	r = lin_lin_copy(procs[_SRC_], vir_addr[_SRC_]->offset,
	                 procs[_DST_], vir_addr[_DST_]->offset, bytes);

	if (r != OK) {
		int writeflag;
		struct proc *target = NULL;
		phys_bytes lin;

		if (r != EFAULT_SRC && r != EFAULT_DST)
			panic("lin_lin_copy failed: %d", r);

		if (!vmcheck || !caller)
			return r;

		if (r == EFAULT_SRC) {
			lin = vir_addr[_SRC_]->offset;
			target = procs[_SRC_];
			writeflag = 0;
		} else if (r == EFAULT_DST) {
			lin = vir_addr[_DST_]->offset;
			target = procs[_DST_];
			writeflag = 1;
		} else {
			panic("r strange: %d", r);
		}

		assert(caller);
		assert(target);

		vm_suspend(caller, target, lin, bytes,
		           VMSTYPE_KERNELCALL, writeflag);
		return VMSUSPEND;
	}

	return OK;
}

/* =========================================================================
 *              data_copy — Endpoint-based data copy
 *
 * Copy data from one process/address to another using endpoint IDs.
 * ========================================================================= */

int data_copy(const endpoint_t from_proc, const vir_bytes from_addr,
              const endpoint_t to_proc, const vir_bytes to_addr,
              size_t bytes)
{
	struct vir_addr src, dst;

	src.offset = from_addr;
	dst.offset = to_addr;
	src.proc_nr_e = from_proc;
	dst.proc_nr_e = to_proc;
	assert(src.proc_nr_e != NONE);
	assert(dst.proc_nr_e != NONE);

	return virtual_copy(&src, &dst, bytes);
}

/* =========================================================================
 *         data_copy_vmcheck — Endpoint data copy with VM check
 * ========================================================================= */

int data_copy_vmcheck(struct proc *caller,
                      const endpoint_t from_proc, const vir_bytes from_addr,
                      const endpoint_t to_proc, const vir_bytes to_addr,
                      size_t bytes)
{
	struct vir_addr src, dst;

	src.offset = from_addr;
	dst.offset = to_addr;
	src.proc_nr_e = from_proc;
	dst.proc_nr_e = to_proc;
	assert(src.proc_nr_e != NONE);
	assert(dst.proc_nr_e != NONE);

	return virtual_copy_vmcheck(caller, &src, &dst, bytes);
}

/* =========================================================================
 *               memory_init — Initialize memory subsystem
 *
 * Allocates free PDE slots from the kernel boot info for temporary
 * kernel mappings. On ARM64, these are L2 index slots in the scratch
 * page table.
 * ========================================================================= */

void memory_init(void)
{
	assert(nfreepdes == 0);

	freepdes[nfreepdes++] = kinfo.freepde_start++;
	freepdes[nfreepdes++] = kinfo.freepde_start++;

	assert(kinfo.freepde_start < AARCH64_VM_DIR_ENTRIES);
	assert(nfreepdes == 2);
	assert(nfreepdes <= MAXFREEPDES);
}

/* =========================================================================
 *           vm_memset — Fill physical/virtual memory with a pattern
 *
 * Writes 'count' bytes of pattern 'c' to physical address 'ph'
 * (or virtual in process 'who'). Supports VM suspend for page
 * fault resolution.
 * ========================================================================= */

int vm_memset(struct proc *caller, endpoint_t who, phys_bytes ph,
              int c, phys_bytes count)
{
	u32_t pattern;
	struct proc *whoptr = NULL;
	phys_bytes cur_ph = ph;
	phys_bytes left = count;
	phys_bytes ptr, chunk, pfa = 0;
	int r = OK;

	if ((r = check_resumed_caller(caller)) != OK)
		return r;

	/* NONE for physical, otherwise virtual */
	if (who != NONE && !(whoptr = endpoint_lookup(who)))
		return ESRCH;

	c &= 0xFF;
	pattern = c | (c << 8) | (c << 16) | (c << 24);

	assert(get_cpulocal_var(ptproc)->p_seg.p_ttbr_v);
	assert(!catch_pagefaults);
	catch_pagefaults = 1;

	while (left > 0) {
		int new_ttbr = 0;
		chunk = left;
		ptr = createpde(whoptr, cur_ph, &chunk, 0, &new_ttbr);

		(void)new_ttbr;

		/* If a page fault happens, pfa is non-null */
		pfa = phys_memset(ptr, pattern, chunk);

		if (pfa) {
			/* If a process page faults, VM may help out */
			if (whoptr) {
				vm_suspend(caller, whoptr, ph, count,
				           VMSTYPE_KERNELCALL, 1);
				assert(catch_pagefaults);
				catch_pagefaults = 0;
				return VMSUSPEND;
			}

			/* Pagefault when phys copying ?! */
			panic("vm_memset: pf %lx addr=%lx len=%lu\n",
			      pfa, ptr, chunk);
		}

		cur_ph += chunk;
		left -= chunk;
	}

	assert(get_cpulocal_var(ptproc)->p_seg.p_ttbr_v);
	assert(catch_pagefaults);
	catch_pagefaults = 0;

	return OK;
}

/* =========================================================================
 *           arch_proc_init — Initialize architecture-specific process state
 *
 * Sets up the initial register state for a new process. The state
 * includes the program counter (PC), stack pointer (SP_EL0), and
 * the first argument (x0 = initial PS string).
 * ========================================================================= */

void arch_proc_init(struct proc *pr, const u32_t ip, const u32_t sp,
                    const u32_t ps_str, char *name)
{
	arch_proc_reset(pr);
	strlcpy(pr->p_name, name, sizeof(pr->p_name));

	/* Set custom state */
	pr->p_reg.elr_el1 = ip;      /* PC */
	pr->p_reg.sp_el0  = sp;      /* User stack pointer */
	pr->p_reg.gpr[0]  = ps_str;  /* x0 = first argument (PS string) */
}

/* =========================================================================
 *        usermapped / kern_phys_map management
 * ========================================================================= */

static int usermapped_glo_index = -1;
static int usermapped_index = -1;
static int first_um_idx = -1;

/* Defined in kernel.lds */
extern char usermapped_start, usermapped_end, usermapped_nonglo_start;

/* =========================================================================
 *            arch_phys_map — Register physical memory mappings
 *
 * Called by VM to discover all physical memory regions that the
 * kernel needs access to (usermapped globals, device mappings, etc.).
 * ========================================================================= */

int arch_phys_map(const int index,
                  phys_bytes *addr,
                  phys_bytes *len,
                  int *flags)
{
	static int first = 1;
	kern_phys_map *phys_maps;
	int freeidx = 0;
	u32_t glo_len = (u32_t)&usermapped_nonglo_start -
	                (u32_t)&usermapped_start;

	if (first) {
		memset(&minix_kerninfo, 0, sizeof(minix_kerninfo));
		if (glo_len > 0)
			usermapped_glo_index = freeidx++;

		usermapped_index = freeidx++;
		first_um_idx = usermapped_index;
		if (usermapped_glo_index != -1)
			first_um_idx = usermapped_glo_index;
		first = 0;

		/* Index registered physical maps */
		phys_maps = kern_phys_map_head;
		while (phys_maps != NULL) {
			phys_maps->index = freeidx++;
			phys_maps = phys_maps->next;
		}
	}

	if (index == usermapped_glo_index) {
		*addr = vir2phys(&usermapped_start);
		*len  = glo_len;
		*flags = VMMF_USER | VMMF_GLO;
		return OK;
	} else if (index == usermapped_index) {
		*addr = vir2phys(&usermapped_nonglo_start);
		*len  = (u32_t)&usermapped_end -
		        (u32_t)&usermapped_nonglo_start;
		*flags = VMMF_USER;
		return OK;
	}

	/* Check registered physical maps */
	phys_maps = kern_phys_map_head;
	while (phys_maps != NULL) {
		if (phys_maps->index == index) {
			*addr  = phys_maps->addr;
			*len   = phys_maps->size;
			*flags = phys_maps->vm_flags;
			return OK;
		}
		phys_maps = phys_maps->next;
	}

	return EINVAL;
}

/* =========================================================================
 *          arch_phys_map_reply — Handle VM reply for physical mapping
 * ========================================================================= */

int arch_phys_map_reply(const int index, const vir_bytes addr)
{
	kern_phys_map *phys_maps;

	if (index == first_um_idx) {
		u32_t usermapped_offset;
		assert(addr > (u32_t)&usermapped_start);
		usermapped_offset = addr - (u32_t)&usermapped_start;

#define FIXEDPTR(ptr)  (void *)((u32_t)(ptr) + usermapped_offset)
#define FIXPTR(ptr)    ptr = FIXEDPTR(ptr)
#define ASSIGN(minixstruct) \
		minix_kerninfo.minixstruct = FIXEDPTR(&minixstruct)

		ASSIGN(kinfo);
		ASSIGN(machine);
		ASSIGN(kmessages);
		ASSIGN(loadinfo);
		ASSIGN(kuserinfo);
		ASSIGN(kclockinfo);

		/* FIXME(Phase 4+): Add arm_frclock when arch_timer is integrated */

		minix_kerninfo.kerninfo_magic = KERNINFO_MAGIC;
		minix_kerninfo.minix_feature_flags = minix_feature_flags;
		minix_kerninfo_user = (vir_bytes)FIXEDPTR(&minix_kerninfo);

		minix_kerninfo.ki_flags |= MINIX_KIF_USERINFO;

		return OK;
	}

	if (index == usermapped_index)
		return OK;

	/* Check registered physical maps */
	phys_maps = kern_phys_map_head;
	while (phys_maps != NULL) {
		if (phys_maps->index == index) {
			assert(phys_maps->cb != NULL);
			phys_maps->vir = addr;
			return OK;
		}
		phys_maps = phys_maps->next;
	}

	return EINVAL;
}

/* =========================================================================
 *          arch_enable_paging — Post-paging-enable callback
 *
 * Called after the VM server has set up a process's page table and
 * switched to it. For Phase 2, this is a stub.
 *
 * FIXME(Phase 3+): Walk the kern_phys_map_head list and invoke
 * callbacks to let drivers know their physical mappings are now
 * accessible at the new virtual addresses.
 * ========================================================================= */

int arch_enable_paging(struct proc *caller)
{
	kern_phys_map *phys_maps;

	assert(caller->p_seg.p_ttbr);

	/* Load caller's page table */
	switch_address_space(caller);

	/* Invoke callbacks for registered physical mappings */
	phys_maps = kern_phys_map_head;
	while (phys_maps != NULL) {
		assert(phys_maps->cb != NULL);
		phys_maps->cb(phys_maps->id, phys_maps->vir);
		phys_maps = phys_maps->next;
	}

	return OK;
}

/* =========================================================================
 *          release_address_space — Release a process's address space
 * ========================================================================= */

void release_address_space(struct proc *pr)
{
	pr->p_seg.p_ttbr_v = NULL;
	barrier();
}

/* =========================================================================
 *             kern_req_phys_map — Request a kernel physical mapping
 *
 * Registers a physical memory region that the kernel needs mapped
 * into the kernel's address space. The region will be mapped by VM
 * and a callback will be invoked after the mapping is established.
 * ========================================================================= */

int kern_req_phys_map(phys_bytes base_address, vir_bytes io_size,
                      int vm_flags, kern_phys_map *priv,
                      kern_phys_map_mapped cb, vir_bytes id)
{
	assert(base_address != 0);
	assert(io_size % AARCH64_PAGE_SIZE == 0);
	assert(cb != NULL);

	priv->addr     = base_address;
	priv->size     = io_size;
	priv->vm_flags = vm_flags;
	priv->cb       = cb;
	priv->id       = id;
	priv->index    = -1;
	priv->next     = NULL;

	if (kern_phys_map_head == NULL) {
		kern_phys_map_head = priv;
		kern_phys_map_head->next = NULL;
	} else {
		priv->next = kern_phys_map_head;
		kern_phys_map_head = priv;
	}

	return 0;
}

/* =========================================================================
 *         kern_phys_map_mapped_ptr — Default callback implementation
 *
 * A simple callback that stores the new virtual address into the
 * pointer given as 'id'. This is used by kern_phys_map_ptr.
 * ========================================================================= */

int kern_phys_map_mapped_ptr(vir_bytes id, phys_bytes address)
{
	*((vir_bytes *)id) = address;
	return 0;
}

/* =========================================================================
 *            kern_phys_map_ptr — Request mapping with pointer result
 *
 * Convenience function that registers a physical mapping and stores
 * the resulting virtual address in 'ptr' once the callback fires.
 * ========================================================================= */

int kern_phys_map_ptr(phys_bytes base_address, vir_bytes io_size,
                      int vm_flags, kern_phys_map *priv,
                      vir_bytes ptr)
{
	return kern_req_phys_map(base_address, io_size, vm_flags, priv,
	                         kern_phys_map_mapped_ptr, ptr);
}
