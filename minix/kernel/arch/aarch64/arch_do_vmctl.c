/* ============================================================
 * arch_do_vmctl.c — ARM64 VM control operations
 *
 * Implements architecture-specific VM control kernel calls.
 * Handles getting/setting page table base registers and
 * TLB flushing.
 *
 * The kernel call implemented in this file:
 *   m_type:  SYS_VMCTL
 *   SVMCTL_WHO:   which process
 *   SVMCTL_PARAM: VMCTL_* parameter
 *   SVMCTL_VALUE: value
 * ============================================================ */

#include "kernel/system.h"
#include "kernel/proc.h"
#include <assert.h>
#include <minix/type.h>

#include "arch_proto.h"

/* =========================================================================
 * set_ttbr — Set process TTBR (page table base register)
 *
 * Updates the process's page table pointer and reloads TTBR0_EL1
 * if the process is the current page table process.
 *
 * Parameters:
 *   p     Process whose page table to set.
 *   ttbr  Physical address of L0 page table (TTBR0_EL1 value).
 *   v     Virtual address of L0 page table (for kernel access).
 * ========================================================================= */

static void set_ttbr(struct proc *p, u64_t ttbr, u64_t *v)
{
	/* Set process TTBR */
	p->p_seg.p_ttbr = ttbr;
	assert(p->p_seg.p_ttbr);
	p->p_seg.p_ttbr_v = v;

	/* If this process is the current page table process,
	 * reload TTBR0_EL1 immediately. */
	if (p == get_cpulocal_var(ptproc)) {
		write_ttbr0(p->p_seg.p_ttbr);
	}

	/* If this is the VM process, enable paging now.
	 * This triggers the callbacks for all pending phys_map entries. */
	if (p->p_nr == VM_PROC_NR) {
		if (arch_enable_paging(p) != OK)
			panic("arch_enable_paging failed");
	}

	/* Clear the VMINHIBIT flag — process can now run */
	RTS_UNSET(p, RTS_VMINHIBIT);
}

/* =========================================================================
 * arch_do_vmctl — Handle VM control kernel call
 *
 * Handles the following VMCTL_* parameters:
 *   VMCTL_GET_PDBR:     Get the process's page directory base (TTBR).
 *   VMCTL_SETADDRSPACE: Set the process's address space (TTBR + virt ptr).
 *   VMCTL_FLUSHTLB:     Flush the TLB (reload TTBR0_EL1).
 *
 * Parameters:
 *   m_ptr  Pointer to the SYS_VMCTL request message.
 *   p      Process making the call (unused).
 *
 * Returns:
 *   OK on success, EINVAL for unknown parameters.
 * ========================================================================= */

int arch_do_vmctl(register message *m_ptr, struct proc *p)
{
	switch (m_ptr->SVMCTL_PARAM) {

	case VMCTL_GET_PDBR:
		/* Get process page directory base register (TTBR0_EL1). */
		m_ptr->SVMCTL_VALUE = (uint64_t)p->p_seg.p_ttbr;
		return OK;

	case VMCTL_SETADDRSPACE:
		/* Set process address space from VM.
		 * SVMCTL_PTROOT   = physical address of L0 page table
		 * SVMCTL_PTROOT_V = virtual address of L0 page table */
		set_ttbr(p,
			 (uint64_t)m_ptr->SVMCTL_PTROOT,
			 (uint64_t *)m_ptr->SVMCTL_PTROOT_V);
		return OK;

	case VMCTL_FLUSHTLB:
		/* Flush TLB using architecturally guaranteed method.
		 * tlb_flush_all() executes TLBI Vmalle1 + DSB SY + ISB,
		 * which invalidates all EL1&EL0 non-global TLB entries. */
	{
		tlb_flush_all();
		return OK;
	}
	}

	printf("arch_do_vmctl: unknown parameter %d\n", m_ptr->SVMCTL_PARAM);
	return EINVAL;
}
