/* x86_64 protected mode initialization.
 * Adapted from i386 with 64-bit GDT/IDT descriptor handling.
 */

#include <assert.h>
#include <string.h>

#include <minix/cpufeature.h>
#include <sys/types.h>
#include "kernel/kernel.h"

#include "arch_proto.h"

#include <sys/exec.h>
#include <libexec.h>

#define INT_GATE_TYPE	14	/* 64-bit interrupt gate */
#define TSS_TYPE	9	/* 64-bit available TSS */

char *video_mem = (char *) MULTIBOOT_VIDEO_BUFFER;

_Alignas(DESC_SIZE) struct segdesc_s gdt[GDT_SIZE];
_Alignas(DESC_SIZE) struct gatedesc_s idt[IDT_SIZE];
struct tss_s tss[CONFIG_MAX_CPUS];

u64_t k_percpu_stacks[CONFIG_MAX_CPUS];

int prot_init_done = 0;

phys_bytes vir2phys(void *vir)
{
	extern char _kern_vir_base, _kern_phys_base;
	u64_t offset = (vir_bytes) &_kern_vir_base -
		(vir_bytes) &_kern_phys_base;
	return (phys_bytes)vir - offset;
}

void enable_iop(struct proc *pp)
{
  pp->p_reg.psw |= 0x3000;
}

void sdesc(struct segdesc_s *segdp, phys_bytes base, vir_bytes size)
{
  segdp->base_low = base;
  segdp->base_middle = base >> BASE_MIDDLE_SHIFT;
  segdp->base_high = base >> BASE_HIGH_SHIFT;

  --size;
  if (size > BYTE_GRAN_MAX) {
	segdp->limit_low = size >> PAGE_GRAN_SHIFT;
	segdp->granularity = GRANULAR | (size >>
			     (PAGE_GRAN_SHIFT + GRANULARITY_SHIFT));
  } else {
	segdp->limit_low = size;
	segdp->granularity = size >> GRANULARITY_SHIFT;
  }
  segdp->granularity |= DEFAULT;
}

void init_param_dataseg(register struct segdesc_s *segdp,
	phys_bytes base, vir_bytes size, const int privilege)
{
	sdesc(segdp, base, size);
	segdp->access = (privilege << DPL_SHIFT) | (PRESENT | SEGMENT |
		WRITEABLE | ACCESSED);
}

void init_dataseg(int index, const int privilege)
{
	init_param_dataseg(&gdt[index], 0, 0xFFFFFFFF, privilege);
}

static void init_codeseg(int index, int privilege)
{
	sdesc(&gdt[index], 0, 0xFFFFFFFF);
	gdt[index].access = (privilege << DPL_SHIFT)
	        | (PRESENT | SEGMENT | EXECUTABLE | READABLE);
}

static struct gate_table_s gate_table_pic[] = {
	{ hwint00, VECTOR( 0), INTR_PRIVILEGE },
	{ hwint01, VECTOR( 1), INTR_PRIVILEGE },
	{ hwint02, VECTOR( 2), INTR_PRIVILEGE },
	{ hwint03, VECTOR( 3), INTR_PRIVILEGE },
	{ hwint04, VECTOR( 4), INTR_PRIVILEGE },
	{ hwint05, VECTOR( 5), INTR_PRIVILEGE },
	{ hwint06, VECTOR( 6), INTR_PRIVILEGE },
	{ hwint07, VECTOR( 7), INTR_PRIVILEGE },
	{ hwint08, VECTOR( 8), INTR_PRIVILEGE },
	{ hwint09, VECTOR( 9), INTR_PRIVILEGE },
	{ hwint10, VECTOR(10), INTR_PRIVILEGE },
	{ hwint11, VECTOR(11), INTR_PRIVILEGE },
	{ hwint12, VECTOR(12), INTR_PRIVILEGE },
	{ hwint13, VECTOR(13), INTR_PRIVILEGE },
	{ hwint14, VECTOR(14), INTR_PRIVILEGE },
	{ hwint15, VECTOR(15), INTR_PRIVILEGE },
	{ NULL, 0, 0}
};

static struct gate_table_s gate_table_exceptions[] = {
	{ divide_error, DIVIDE_VECTOR, INTR_PRIVILEGE },
	{ single_step_exception, DEBUG_VECTOR, INTR_PRIVILEGE },
	{ nmi, NMI_VECTOR, INTR_PRIVILEGE },
	{ breakpoint_exception, BREAKPOINT_VECTOR, USER_PRIVILEGE },
	{ overflow, OVERFLOW_VECTOR, USER_PRIVILEGE },
	{ bounds_check, BOUNDS_VECTOR, INTR_PRIVILEGE },
	{ inval_opcode, INVAL_OP_VECTOR, INTR_PRIVILEGE },
	{ copr_not_available, COPROC_NOT_VECTOR, INTR_PRIVILEGE },
	{ double_fault, DOUBLE_FAULT_VECTOR, INTR_PRIVILEGE },
	{ copr_seg_overrun, COPROC_SEG_VECTOR, INTR_PRIVILEGE },
	{ inval_tss, INVAL_TSS_VECTOR, INTR_PRIVILEGE },
	{ segment_not_present, SEG_NOT_VECTOR, INTR_PRIVILEGE },
	{ stack_exception, STACK_FAULT_VECTOR, INTR_PRIVILEGE },
	{ general_protection, PROTECTION_VECTOR, INTR_PRIVILEGE },
	{ page_fault, PAGE_FAULT_VECTOR, INTR_PRIVILEGE },
	{ copr_error, COPROC_ERR_VECTOR, INTR_PRIVILEGE },
	{ alignment_check, ALIGNMENT_CHECK_VECTOR, INTR_PRIVILEGE },
	{ machine_check, MACHINE_CHECK_VECTOR, INTR_PRIVILEGE },
	{ simd_exception, SIMD_EXCEPTION_VECTOR, INTR_PRIVILEGE },
	{ ipc_entry_softint_orig, IPC_VECTOR_ORIG, USER_PRIVILEGE },
	{ kernel_call_entry_orig, KERN_CALL_VECTOR_ORIG, USER_PRIVILEGE },
	{ ipc_entry_softint_um, IPC_VECTOR_UM, USER_PRIVILEGE },
	{ kernel_call_entry_um, KERN_CALL_VECTOR_UM, USER_PRIVILEGE },
	{ NULL, 0, 0}
};

int tss_init(unsigned cpu, void * kernel_stack)
{
	struct tss_s * t = &tss[cpu];
	int index = TSS_INDEX(cpu);
	struct segdesc_s *tssgdt;

	tssgdt = &gdt[index];

	init_param_dataseg(tssgdt, (phys_bytes) t,
			sizeof(struct tss_s), INTR_PRIVILEGE);
	tssgdt->access = PRESENT | (INTR_PRIVILEGE << DPL_SHIFT) | TSS_TYPE;

	memset(t, 0, sizeof(*t));
	t->ds = t->es = t->fs = t->gs = t->ss0 = KERN_DS_SELECTOR;
	t->cs = KERN_CS_SELECTOR;
	t->iobase = sizeof(struct tss_s);

	k_percpu_stacks[cpu] = t->sp0 = ((u64_t) kernel_stack) - X86_STACK_TOP_RESERVED;
	*((reg_t *)(t->sp0 + 1 * sizeof(reg_t))) = cpu;

	/* Set up AMD SYSCALL support (x86_64 primary IPC path). */
	if(minix_feature_flags & MKF_I386_AMD_SYSCALL) {
		u32_t msr_lo, msr_hi;

		/* Enable SYSCALL in EFER */
		ia32_msr_read(AMD_MSR_EFER, &msr_hi, &msr_lo);
		msr_lo |= AMD_EFER_SCE;
		ia32_msr_write(AMD_MSR_EFER, msr_hi, msr_lo);

		/* Set STAR (32-bit compat syscall) */
		ia32_msr_write(AMD_MSR_STAR,
		  ((u32_t)USER_CS_SELECTOR << 16) | (u32_t)KERN_CS_SELECTOR,
		  (u32_t) ipc_entry_syscall_cpu0);

		/* Set LSTAR (64-bit syscall entry) */
		/* LSTAR — 64-bit syscall entry point (split for wrmsr) */
		{
			u64_t syscall_addr = (u64_t)ipc_entry_syscall;
			ia32_msr_write(AMD_MSR_LSTAR,
				(u32_t)(syscall_addr >> 32),
				(u32_t)syscall_addr);
		}
		/* Set SF_MASK (clear IF in user RFLAGS) */
		ia32_msr_write(AMD_MSR_SF_MASK, 0, 0x200);
	}

	return SEG_SELECTOR(index);
}

phys_bytes init_segdesc(int gdt_index, void *base, int size)
{
	struct desctableptr_s *dtp = (struct desctableptr_s *) &gdt[gdt_index];
	dtp->limit = size - 1;
	dtp->base = (phys_bytes) base;
	return (phys_bytes) dtp;
}

void int_gate(struct gatedesc_s *tab,
	unsigned vec_nr, vir_bytes offset, unsigned dpl_type)
{
  register struct gatedesc_s *idp;

  idp = &tab[vec_nr];
  idp->offset_low = offset;
  idp->selector = KERN_CS_SELECTOR;
  idp->ist = 0;			/* No Interrupt Stack Table */
  idp->p_dpl_type = dpl_type;
  idp->offset_middle = offset >> 16;
  idp->offset_high = (u64_t)offset >> 32;
  idp->reserved = 0;
}

void int_gate_idt(unsigned vec_nr, vir_bytes offset, unsigned dpl_type)
{
	int_gate(idt, vec_nr, offset, dpl_type);
}

void idt_copy_vectors(struct gate_table_s * first)
{
	struct gate_table_s *gtp;
	for (gtp = first; gtp->gate; gtp++) {
		int_gate(idt, gtp->vec_nr, (vir_bytes) gtp->gate,
				PRESENT | INT_GATE_TYPE |
				(gtp->privilege << DPL_SHIFT));
	}
}

void idt_copy_vectors_pic(void)
{
	idt_copy_vectors(gate_table_pic);
}

void idt_init(void)
{
	idt_copy_vectors_pic();
	idt_copy_vectors(gate_table_exceptions);
}

struct desctableptr_s gdt_desc, idt_desc;

void idt_reload(void)
{
	x86_lidt(&idt_desc);
}

multiboot_module_t *bootmod(int pnr)
{
	int i;

	assert(pnr >= 0);

	for(i = NR_TASKS; i < NR_BOOT_PROCS; i++) {
		int p;
		p = i - NR_TASKS;
		if(image[i].proc_nr == pnr) {
			assert(p < MULTIBOOT_MAX_MODS);
			assert(p < kinfo.mbi.mi_mods_count);
			return &kinfo.module_list[p];
		}
	}

	panic("boot module %d not found", pnr);
}

int booting_cpu = 0;

void prot_load_selectors(void)
{
  x86_lgdt(&gdt_desc);
  idt_init();
  idt_reload();
  x86_lldt(LDT_SELECTOR);
  x86_ltr(TSS_SELECTOR(booting_cpu));

  x86_load_kerncs();
  x86_load_ds(KERN_DS_SELECTOR);
  x86_load_es(KERN_DS_SELECTOR);
  x86_load_fs(KERN_DS_SELECTOR);
  x86_load_gs(KERN_DS_SELECTOR);
  x86_load_ss(KERN_DS_SELECTOR);
}

void prot_init(void)
{
  extern char k_boot_stktop;

  if(_cpufeature(_CPUF_I386_SYSENTER))
	minix_feature_flags |= MKF_I386_INTEL_SYSENTER;
  if(_cpufeature(_CPUF_I386_SYSCALL))
	minix_feature_flags |= MKF_I386_AMD_SYSCALL;

  memset(gdt, 0, sizeof(gdt));
  memset(idt, 0, sizeof(idt));

  gdt_desc.base = (u64_t) gdt;
  gdt_desc.limit = sizeof(gdt)-1;
  idt_desc.base = (u64_t) idt;
  idt_desc.limit = sizeof(idt)-1;
  tss_init(0, &k_boot_stktop);

  init_param_dataseg(&gdt[LDT_INDEX],
    (phys_bytes) 0, 0, INTR_PRIVILEGE);
  gdt[LDT_INDEX].access = PRESENT | LDT;
  init_codeseg(KERN_CS_INDEX, INTR_PRIVILEGE);
  init_dataseg(KERN_DS_INDEX, INTR_PRIVILEGE);
  init_codeseg(USER_CS_INDEX, USER_PRIVILEGE);
  init_dataseg(USER_DS_INDEX, USER_PRIVILEGE);

  prot_load_selectors();

  pg_clear();
  pg_identity(&kinfo);
  pg_mapkernel();
  pg_load();

  prot_init_done = 1;
}

static int alloc_for_vm = 0;

void arch_post_init(void)
{
  struct proc *vm;
  vm = proc_addr(VM_PROC_NR);
  get_cpulocal_var(ptproc) = vm;
  pg_info(&vm->p_seg.p_cr3, (u64_t **)&vm->p_seg.p_cr3_v);
}

static int libexec_pg_alloc(struct exec_info *execi, vir_bytes vaddr, size_t len)
{
        pg_map(PG_ALLOCATEME, vaddr, vaddr+len, &kinfo);
  	pg_load();
        memset((char *) vaddr, 0, len);
	alloc_for_vm += len;
        return OK;
}

void arch_boot_proc(struct boot_image *ip, struct proc *rp)
{
	multiboot_module_t *mod;
	struct ps_strings *psp;
	char *sp;

	if(rp->p_nr < 0) return;

	mod = bootmod(rp->p_nr);

	if(rp->p_nr == VM_PROC_NR) {
		struct exec_info execi;

		memset(&execi, 0, sizeof(execi));

		execi.stack_high = kinfo.user_sp;
		execi.stack_size = 64 * 1024;
		execi.proc_e = ip->endpoint;
		execi.hdr = (char *) mod->mod_start;
		execi.filesize = execi.hdr_len = mod->mod_end - mod->mod_start;
		strlcpy(execi.progname, ip->proc_name, sizeof(execi.progname));
		execi.frame_len = 0;

		execi.copymem = libexec_copy_memcpy;
		execi.clearmem = libexec_clear_memset;
		execi.allocmem_prealloc_junk = libexec_pg_alloc;
		execi.allocmem_prealloc_cleared = libexec_pg_alloc;
		execi.allocmem_ondemand = libexec_pg_alloc;
		execi.clearproc = NULL;

		if(libexec_load_elf(&execi) != OK)
			panic("VM loading failed");

		sp = (char *)execi.stack_high;
		sp -= sizeof(struct ps_strings);
		psp = (struct ps_strings *) sp;

		sp -= (sizeof(void *) + sizeof(void *) + sizeof(int));

		psp->ps_argvstr = (char **)(sp + sizeof(int));
		psp->ps_nargvstr = 0;
		psp->ps_envstr = psp->ps_argvstr + sizeof(void *);
		psp->ps_nenvstr = 0;

		arch_proc_init(rp, execi.pc, (vir_bytes)sp,
			execi.stack_high - sizeof(struct ps_strings),
			ip->proc_name);

		add_memmap(&kinfo, mod->mod_start, mod->mod_end-mod->mod_start);
		mod->mod_end = mod->mod_start = 0;

		kinfo.vm_allocated_bytes = alloc_for_vm;
	}
}
