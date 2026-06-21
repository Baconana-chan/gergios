/* ============================================================
 * protect.c — ARM64 MMU initialization and process setup
 *
 * This file provides the architecture-specific initialization
 * for the ARM64 memory management unit (MMU) and process boot
 * infrastructure. It corresponds to the ARM32 (earm) protect.c.
 *
 * ARM64 MMU state after head.S:
 *   - TCR_EL1:  configured for 4KB pages, 48-bit VA, 48-bit PA
 *   - MAIR_EL1: Attr0=Normal WB, Attr1=Device-nGnRE
 *   - SCTLR_EL1: MMU+caches enabled, RES1 bits set
 *   - VBAR_EL1:  set to arm64_vector_table
 *   - TTBR0_EL1: identity map covering first 1GB (2MB blocks)
 *   - TTBR1_EL1: same as TTBR0 (temporary, replaced by pg_load)
 *
 * What prot_init() adds:
 *   - Re-installs the exception vector table (VBAR_EL1)
 *   - Re-initializes page tables via pg_utils (pg_identity +
 *     pg_mapkernel + pg_load) for both identity and kernel maps
 *   - Marks protection init as complete
 *
 * Process boot (arch_boot_proc):
 *   - Phase 2: Minimal stub — boot modules are not yet available.
 *   - Phase 3+: Load ELF binaries via libexec_load_elf.
 *
 * References:
 *   ARM DDI 0487 — Chapter D5: VMSAv8-64 Translation Tables
 *   ARM DDI 0487 — Chapter D13: System Register Descriptions
 *   minix/kernel/arch/earm/protect.c — Original ARM32 version
 * ============================================================ */

#include <assert.h>
#include <string.h>

#include <machine/multiboot.h>

#include "kernel/kernel.h"
#include "arch_proto.h"

#include <sys/exec.h>
#if 0
# include <libexec.h>  /* ELF loader — Phase 3+ */
#endif

/* =========================================================================
 * Memory barrier helper
 *
 * ARM64 ISB (Instruction Synchronization Barrier) flushes the
 * pipeline and ensures all instructions before it complete before
 * any instruction after it is fetched. Required after system
 * register writes that affect MMU or exception handling.
 * ========================================================================= */

#define arm64_isb()  __asm__ __volatile__("isb" : : : "memory")

/* =========================================================================
 * Global state
 * ========================================================================= */

int prot_init_done = 0;

/* =========================================================================
 * VBAR_EL1 write helper
 *
 * ARM64 requires VBAR_EL1 to be 2KB-aligned (the vector table
 * alignment requirement). The ISB ensures the new table is used
 * for subsequent exceptions.
 *
 * Note: vir2phys() is NOT defined here — it is provided by klib.S
 * using the ARM64 AT instruction (ats1e1r). Do NOT add a C version
 * as it would cause a duplicate symbol at link time.
 * ========================================================================= */

static void write_vbar(uint64_t vbar)
{
	__asm__ volatile("msr vbar_el1, %0" : : "r"(vbar) : "memory");
	arm64_isb();
}

/* =========================================================================
 * bootmod — Find boot module by process number (Phase 3+)
 *
 * Searches the boot image table for the given process number and
 * returns the corresponding multiboot module descriptor.
 * Phase 2: Boot modules are not set up — this function panics.
 * ========================================================================= */

static multiboot_module_t *bootmod(int pnr)
{
	(void)pnr;
	panic("bootmod: boot modules not available in Phase 2");
}

/* =========================================================================
 * tss_init — Per-CPU kernel stack initialization
 *
 * ARM64 does not have a Task State Segment (TSS) like x86. The
 * kernel stack pointer (SP_EL1) is either:
 *   - Set once at boot and never changed (single-core Phase 2)
 *   - Switched per-CPU via tpidr_el1 + SP_EL1 (SMP Phase 5+)
 *
 * This function is a stub; per-CPU stack management is handled
 * by the kernel stack switching code in mpx.S / klib.S.
 *
 * Parameters:
 *   cpu           CPU number (0 for BSP in Phase 2)
 *   kernel_stack  Pointer to the top of the kernel stack
 *
 * Returns: 0 on success (always, for now).
 * ========================================================================= */

int tss_init(unsigned cpu, void *kernel_stack)
{
	/* ARM64 doesn't have TSS. Kernel stacks are per-CPU via
	 * TPIDR_EL1. This function is kept for compatibility with
	 * the common kernel code that calls tss_init().
	 *
	 * FIXME(Phase 5+): For SMP, store per-CPU stack top in
	 * the cpu-local variables area (accessed via TPIDR_EL1). */
	(void)cpu;
	(void)kernel_stack;

	return 0;
}

/* =========================================================================
 * prot_init — Architecture-specific MMU initialization
 *
 * Called from kmain() during kernel initialization to set up the
 * page tables for the post-relocation kernel. This function:
 *
 *   1. Sets VBAR_EL1 to the exception vector table
 *   2. Clears and reinitializes page tables:
 *      a. pg_clear() — zero out all boot page tables
 *      b. pg_identity(&kinfo) — set up identity map (first 2GB)
 *      c. pg_mapkernel() — set up kernel high VMA mapping
 *      d. pg_load() — load new TTBR0_EL1
 *   3. Marks prot_init as complete
 *
 * Note: The MMU is already enabled from head.S (TCR, MAIR, SCTLR).
 * This function only replaces the page table contents and the
 * exception vector base. System register configuration (TCR, MAIR,
 * SCTLR) remains unchanged from head.S.
 * ========================================================================= */

void prot_init(void)
{
	/* Set exception vector table.
	 * arm64_vector_table is defined in vectors.S (.unpaged.text).
	 * It's accessible via the identity map at this point. */
	extern int arm64_vector_table;

	write_vbar((uint64_t)&arm64_vector_table);

	/* Clear and reinitialize page tables.
	 * pg_clear() zeros all boot page tables.
	 * pg_identity() sets up the L0→L1 identity map (first 2GB).
	 * pg_mapkernel() sets up the kernel high VMA map.
	 * pg_load() loads the new L0 table into TTBR0_EL1. */
	pg_clear();
	pg_identity(&kinfo);
	pg_mapkernel();
	pg_load();

	/* Ensure all table writes are visible before continuing */
	arm64_isb();

	prot_init_done = 1;
}

/* =========================================================================
 * arch_post_init — Post-process-initialization setup
 *
 * Called after the process table and VM process have been set up.
 * This sets the current per-CPU ptproc to the VM process and
 * records the VM process's page table addresses (physical and
 * virtual). This allows the kernel to use the VM process's page
 * table for temporary mappings.
 * ========================================================================= */

void arch_post_init(void)
{
	struct proc *vm;

	vm = proc_addr(VM_PROC_NR);
	get_cpulocal_var(ptproc) = vm;
	pg_info(&vm->p_seg.p_ttbr, &vm->p_seg.p_ttbr_v);
}

/* =========================================================================
 * arch_boot_proc — Boot a process from a boot image
 *
 * Loads and initializes a boot process from an ELF binary. In the
 * full MINIX system, this:
 *   1. Finds the boot module for the given process
 *   2. Parses and loads the ELF binary into memory
 *   3. Sets up a ps_strings struct on the stack
 *   4. Calls arch_proc_init to set up the initial register state
 *
 * Phase 2: Minimal stub — boot modules are not yet available.
 * The function calls arch_proc_init with zeroed addresses and
 * returns immediately.
 *
 * Phase 3+: Full ELF loading via libexec_load_elf (as in earm).
 * ========================================================================= */

void arch_boot_proc(struct boot_image *ip, struct proc *rp)
{
	/* Phase 2: No boot modules available yet. Just initialize
	 * the process with default state and let the startup code
	 * handle it.
	 *
	 * FIXME(Phase 3+): Implement full ELF loading:
	 *   1. Call bootmod(rp->p_nr) to find the module
	 *   2. For VM_PROC_NR: load ELF via libexec_load_elf
	 *   3. Set up ps_strings on the user stack
	 *   4. Call arch_proc_init with proper PC/SP/PS_STR */
	(void)ip;

	arch_proc_init(rp, 0, 0, 0, ip->proc_name);
}
