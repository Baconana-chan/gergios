#ifndef _X86_64_PROTO_H
#define _X86_64_PROTO_H

#include <machine/vm.h>
#include "archconst.h"

#define K_STACK_SIZE	X86_64_PAGE_SIZE

#ifndef __ASSEMBLY__

/* Hardware interrupt handlers. */
void hwint00(void);
void hwint01(void);
void hwint02(void);
void hwint03(void);
void hwint04(void);
void hwint05(void);
void hwint06(void);
void hwint07(void);
void hwint08(void);
void hwint09(void);
void hwint10(void);
void hwint11(void);
void hwint12(void);
void hwint13(void);
void hwint14(void);
void hwint15(void);

/* Exception handlers. */
void divide_error(void);
void single_step_exception(void);
void nmi(void);
void breakpoint_exception(void);
void overflow(void);
void bounds_check(void);
void inval_opcode(void);
void copr_not_available(void);
void double_fault(void);
void copr_seg_overrun(void);
void inval_tss(void);
void segment_not_present(void);
void stack_exception(void);
void general_protection(void);
void page_fault(void);
void copr_error(void);
void alignment_check(void);
void machine_check(void);
void simd_exception(void);

/* Context restore functions. */
void restore_user_context_int(struct proc *);
void restore_user_context_syscall(struct proc *);

/* IPC entry points. */
void ipc_entry_syscall(void);
void ipc_entry_syscall_cpu0(void);
void ipc_entry_softint_orig(void);
void ipc_entry_softint_um(void);
void kernel_call_entry_orig(void);
void kernel_call_entry_um(void);

/* exception.c */
struct exception_frame {
	reg_t	vector;		/* which interrupt vector was triggered */
	reg_t	errcode;	/* zero if no exception does not push err code */
	reg_t	eip;		/* _NOT_ rip — keep i386 name for compatibility */
	reg_t	cs;
	reg_t	eflags;		/* _NOT_ rflags — keep i386 name for compatibility */
	reg_t	esp;		/* undefined if trap is nested */
	reg_t	ss;		/* undefined if trap is nested */
};

void exception(struct exception_frame * frame);

/* klib.S */
_Noreturn void reset(void);
_Noreturn void poweroff_vmware_clihlt(void);
_Noreturn void x86_triplefault(void);
reg_t read_cr0(void);
reg_t read_cr2(void);
reg_t read_cr3(void);
reg_t read_cr4(void);
void write_cr0(unsigned long value);
void write_cr3(unsigned long value);
void write_cr4(unsigned long value);
unsigned long read_cpu_flags(void);
phys_bytes vir2phys(void *);
void phys_insb(u16_t port, phys_bytes buf, size_t count);
void phys_insw(u16_t port, phys_bytes buf, size_t count);
void phys_outsb(u16_t port, phys_bytes buf, size_t count);
void phys_outsw(u16_t port, phys_bytes buf, size_t count);
void reload_cr3(void);
void i386_invlpg(phys_bytes linaddr);
vir_bytes phys_memset(phys_bytes ph, u64_t c, phys_bytes bytes);
void ia32_msr_read(u32_t reg, u32_t * hi, u32_t * lo);
void ia32_msr_write(u32_t reg, u32_t hi, u32_t lo);
void fninit(void);
void clts(void);
void fxsave(void *);
void fnsave(void *);
int fxrstor(void *);
int __fxrstor_end(void *);
int frstor(void *);
int __frstor_end(void *);
int __frstor_failure(void *);
unsigned short fnstsw(void);
void fnstcw(unsigned short* cw);
void x86_lgdt(void *);
void x86_lldt(u32_t);
void x86_ltr(u32_t);
void x86_lidt(void *);
void x86_load_kerncs(void);
void x86_load_ds(u32_t);
void x86_load_ss(u32_t);
void x86_load_es(u32_t);
void x86_load_fs(u32_t);
void x86_load_gs(u32_t);
void eoi_8259_master(void);
void eoi_8259_slave(void);

/* phys_copy fault labels (used by exception.c) */
void phys_copy_fault(void);
void phys_copy_fault_in_kernel(void);
void memset_fault(void);
void memset_fault_in_kernel(void);

/* copy_msg fault labels (used by exception.c) */
void __copy_msg_from_user_end(void);
void __copy_msg_to_user_end(void);
void __user_copy_msg_pointer_failure(void);

/* mfence / barrier */
void mfence(void);
#define barrier()	do { mfence(); } while(0)

/* ipc functions */
int usermapped_send_softint(endpoint_t dest, message *m_ptr);
int usermapped_receive_softint(endpoint_t src, message *m_ptr, int *status_ptr);
int usermapped_sendrec_softint(endpoint_t src_dest, message *m_ptr);
int usermapped_sendnb_softint(endpoint_t dest, message *m_ptr);
int usermapped_notify_softint(endpoint_t dest);
int usermapped_do_kernel_call_softint(message *m_ptr);
int usermapped_senda_softint(asynmsg_t *table, size_t count);

int usermapped_send_syscall(endpoint_t dest, message *m_ptr);
int usermapped_receive_syscall(endpoint_t src, message *m_ptr, int *status_ptr);
int usermapped_sendrec_syscall(endpoint_t src_dest, message *m_ptr);
int usermapped_sendnb_syscall(endpoint_t dest, message *m_ptr);
int usermapped_notify_syscall(endpoint_t dest);
int usermapped_do_kernel_call_syscall(message *m_ptr);
int usermapped_senda_syscall(asynmsg_t *table, size_t count);

void switch_k_stack(void * esp, void (* continuation)(void));

void __switch_address_space(struct proc * p, struct proc ** __ptproc);
#define switch_address_space(proc)	\
	__switch_address_space(proc, get_cpulocal_var_ptr(ptproc))

void refresh_tlb(void);

/* protect.c */
struct tss_s {
  reg_t backlink;
  reg_t sp0;                    /* stack pointer to use during interrupt */
  reg_t ss0;                    /*   "   segment  "  "    "        "     */
  reg_t sp1;
  reg_t ss1;
  reg_t sp2;
  reg_t ss2;
  reg_t cr3;
  reg_t ip;
  reg_t flags;
  reg_t ax;
  reg_t cx;
  reg_t dx;
  reg_t bx;
  reg_t sp;
  reg_t bp;
  reg_t si;
  reg_t di;
  reg_t es;
  reg_t cs;
  reg_t ss;
  reg_t ds;
  reg_t fs;
  reg_t gs;
  reg_t ldt;
  u16_t trap;
  u16_t iobase;
} __attribute__((packed));

void enable_iop(struct proc *pp);
u32_t read_cs(void);
u32_t read_ds(void);
u32_t read_ss(void);

void add_memmap(kinfo_t *cbi, u64_t addr, u64_t len);
phys_bytes alloc_lowest(kinfo_t *cbi, phys_bytes len);
void vm_enable_paging(void);
void cut_memmap(kinfo_t *cbi, phys_bytes start, phys_bytes end);
phys_bytes pg_roundup(phys_bytes b);
void pg_info(phys_bytes *, u64_t **);
void pg_clear(void);
void pg_identity(kinfo_t *);
phys_bytes pg_load(void);
void pg_map(phys_bytes phys, vir_bytes vaddr, vir_bytes vaddr_end, kinfo_t *cbi);
int pg_mapkernel(void);
void pg_mapproc(struct proc *p, struct boot_image *ip, kinfo_t *cbi);

/* gate table for IDT init */
struct gate_table_s {
 void(*gate) (void);
	unsigned char vec_nr;
	unsigned char privilege;
};

void idt_copy_vectors(struct gate_table_s * first);
void idt_copy_vectors_pic(void);
void idt_reload(void);

EXTERN void * k_stacks_start;
extern void * k_stacks;

#define get_k_stack_top(cpu)	((void *)(((char*)(k_stacks)) \
					+ 2 * ((cpu) + 1) * K_STACK_SIZE))

#define get_stack_frame(__X)	((reg_t)__builtin_frame_address(0))

int tss_init(unsigned cpu, void * kernel_stack);
void int_gate_idt(unsigned vec_nr, vir_bytes offset, unsigned dpl_type);

int platform_tbl_checksum_ok(void *ptr, unsigned int length);
int platform_tbl_ptr(phys_bytes start, phys_bytes end, unsigned
	increment, void * buff, unsigned size, phys_bytes * phys_addr, int ((*
	cmp_f)(void *)));

/* breakpoints.c */
int breakpoint_set(phys_bytes linaddr, int bp, const int flags);
#define BREAKPOINT_COUNT		4
#define BREAKPOINT_FLAG_RW_MASK		(3 << 0)
#define BREAKPOINT_FLAG_RW_EXEC		(0 << 0)
#define BREAKPOINT_FLAG_RW_WRITE	(1 << 0)
#define BREAKPOINT_FLAG_RW_RW		(2 << 0)
#define BREAKPOINT_FLAG_LEN_MASK	(3 << 2)
#define BREAKPOINT_FLAG_LEN_1		(0 << 2)
#define BREAKPOINT_FLAG_LEN_2		(1 << 2)
#define BREAKPOINT_FLAG_LEN_4		(2 << 2)
#define BREAKPOINT_FLAG_MODE_MASK	(3 << 4)
#define BREAKPOINT_FLAG_MODE_OFF	(0 << 4)
#define BREAKPOINT_FLAG_MODE_LOCAL	(1 << 4)
#define BREAKPOINT_FLAG_MODE_GLOBAL	(2 << 4)

/* functions defined in architecture-independent kernel source. */
#include "kernel/proto.h"

#endif /* __ASSEMBLY__ */

#endif /* _X86_64_PROTO_H */
