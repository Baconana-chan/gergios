/* The kernel call implemented in this file:
 *   m_type:	SYS_VMCTL
 *
 * The parameters for this kernel call are:
 *   SVMCTL_WHO	which process
 *   SVMCTL_PARAM	set this setting (VMCTL_*)
 *   SVMCTL_VALUE	to this value
 *
 * x86_64 version (same as i386 — CR3/TLB operations identical).
 */

#include "kernel/system.h"
#include <assert.h>

#include "arch_proto.h"

extern phys_bytes video_mem_vaddr;
extern char *video_mem;

/* On x86_64 LP64, CR3 is a 64-bit value (reg_t = unsigned long).
 * Use u64_t/u64_t* to match p_cr3 (reg_t) and p_cr3_v (u64_t*).
 */
static void setcr3(struct proc *p, u64_t cr3, u64_t *v)
{
	p->p_seg.p_cr3 = cr3;
	assert(p->p_seg.p_cr3);
	p->p_seg.p_cr3_v = v;
	if(p == get_cpulocal_var(ptproc)) {
		write_cr3(p->p_seg.p_cr3);
	}
	if(p->p_nr == VM_PROC_NR) {
		if (arch_enable_paging(p) != OK)
			panic("arch_enable_paging failed");
	}
	RTS_UNSET(p, RTS_VMINHIBIT);
}

int arch_do_vmctl(
  register message *m_ptr,
  struct proc *p
)
{
  switch(m_ptr->SVMCTL_PARAM) {
	case VMCTL_GET_PDBR:
		m_ptr->SVMCTL_VALUE = p->p_seg.p_cr3;
		return OK;
	case VMCTL_SETADDRSPACE:
		setcr3(p, m_ptr->SVMCTL_PTROOT, (u64_t *) m_ptr->SVMCTL_PTROOT_V);
		return OK;
	case VMCTL_FLUSHTLB:
	{
		reload_cr3();
		return OK;
	}
	case VMCTL_I386_INVLPG:
	{
		i386_invlpg(m_ptr->SVMCTL_VALUE);
		return OK;
	}
  }

  printf("arch_do_vmctl: strange param %d\n", m_ptr->SVMCTL_PARAM);
  return EINVAL;
}
