/* sections */

#include <minix/config.h>
#include <minix/const.h>
#include <machine/asm.h>
#include <machine/interrupt.h>
#include <machine/vm.h>
#include "archconst.h"
#include "kernel/const.h"
#include "sconst.h"
#include <machine/multiboot.h>

/* x86_64 calling convention (System V AMD64 ABI):
 *   1st arg: RDI
 *   2nd arg: RSI
 *   3rd arg: RDX
 *   4th arg: RCX
 *   5th arg: R8
 *   6th arg: R9
 *   Return: RAX
 *   Stack: RSP, 8-byte return address pushed by CALL
 *
 * All registers are 64-bit. Use movq for 64-bit operations,
 * movl for 32-bit zero-extending operations (clears upper 32 bits).
 */

/* Make a function of the form func(arg) */
#define ARG_ACTION(FUNCTION, ACTION)				;\
ENTRY(FUNCTION)							;\
	pushq	%rbp						;\
	movq	%rsp, %rbp					;\
	ACTION							;\
	popq	%rbp						;\
	ret

/* Make a function that reads a system register into RAX */
#define READ_SREG(FUNCTION, SREG)				;\
ENTRY(FUNCTION)							;\
	movq	SREG, %rax					;\
	ret

/* Make a function that writes RAX to a system register from the stack arg */
#define WRITE_SREG(FUNCTION, SREG)				;\
ENTRY(FUNCTION)							;\
	pushq	%rbp						;\
	movq	%rsp, %rbp					;\
	movq	16(%rbp), %rax	/* first argument */		;\
	movq	%rax, SREG					;\
	popq	%rbp						;\
	ret

/* For 16-bit segment register loads */
#define WRITE_SEG(FUNCTION, SREG)				;\
ENTRY(FUNCTION)							;\
	pushq	%rbp						;\
	movq	%rsp, %rbp					;\
	movl	16(%rbp), %eax	/* first argument (32-bit) */	;\
	movw	%ax, SREG					;\
	popq	%rbp						;\
	ret

/*
 * This file contains assembly code utility routines needed by the kernel.
 */

ENTRY(__main)
	ret

/*===========================================================================*/
/*				phys_insw				     */
/*===========================================================================*/
/*
 * PUBLIC void phys_insw(Port_t port, phys_bytes buf, size_t count);
 * Input an array from an I/O port.  Absolute address version of insw().
 */
ENTRY(phys_insw)
	pushq	%rbp
	movq	%rsp, %rbp
	cld
	pushq	%rdi

	movl	%edi, %edx	/* port to read from (1st arg) */
	movq	%rsi, %rdi	/* destination addr (2nd arg) */
	movq	%rdx, %rcx	/* byte count (3rd arg) */
	shrq	$1, %rcx	/* word count */
	rep insw
	popq	%rdi
	popq	%rbp
	ret

/*===========================================================================*/
/*				phys_insb				     */
/*===========================================================================*/
/*
 * PUBLIC void phys_insb(Port_t port, phys_bytes buf, size_t count);
 * Input an array from an I/O port.  Absolute address version of insb().
 */
ENTRY(phys_insb)
	pushq	%rbp
	movq	%rsp, %rbp
	cld
	pushq	%rdi

	movl	%edi, %edx	/* port */
	movq	%rsi, %rdi	/* destination */
	movq	%rdx, %rcx	/* count */
	rep insb
	popq	%rdi
	popq	%rbp
	ret

/*===========================================================================*/
/*				phys_outsw				     */
/*===========================================================================*/
/*
 * PUBLIC void phys_outsw(Port_t port, phys_bytes buf, size_t count);
 * Output an array to an I/O port.  Absolute address version of outsw().
 */
ENTRY(phys_outsw)
	pushq	%rbp
	movq	%rsp, %rbp
	cld
	pushq	%rsi

	movl	%edi, %edx	/* port */
	movq	%rsi, %rsi	/* source */
	movq	%rdx, %rcx	/* count */
	shrq	$1, %rcx	/* word count */
	rep outsw
	popq	%rsi
	popq	%rbp
	ret

/*===========================================================================*/
/*				phys_outsb				     */
/*===========================================================================*/
/*
 * PUBLIC void phys_outsb(Port_t port, phys_bytes buf, size_t count);
 * Output an array to an I/O port.  Absolute address version of outsb().
 */
ENTRY(phys_outsb)
	pushq	%rbp
	movq	%rsp, %rbp
	cld
	pushq	%rsi

	movl	%edi, %edx	/* port */
	movq	%rsi, %rsi	/* source */
	movq	%rdx, %rcx	/* count */
	rep outsb
	popq	%rsi
	popq	%rbp
	ret

/*===========================================================================*/
/*				phys_copy				     */
/*===========================================================================*/
/*
 * PUBLIC phys_bytes phys_copy(phys_bytes source, phys_bytes destination,
 *				phys_bytes bytecount);
 * Copy a block of data from anywhere to anywhere in physical memory.
 *
 * x86_64: physical addresses may be up to 48 bits (or more), but we
 * access them directly (identity-mapped during early boot, or via
 * the kernel high mapping after paging). phys_bytes is 64-bit.
 */
ENTRY(phys_copy)
	pushq	%rbp
	movq	%rsp, %rbp

	cld
	pushq	%rsi
	pushq	%rdi

	/* xchg swap: RSI=src (1st arg RDI), RDI=dst (2nd arg RSI) */
	xchgq	%rsi, %rdi
	movq	%rdx, %rax	/* count (3rd arg) */

	cmpq	$10, %rax	/* avoid align overhead for small counts */
	jb	pc_small

	movq	%rsi, %rcx	/* align source */
	negq	%rcx
	andq	$7, %rcx	/* 8-byte alignment for 64-bit */
	subq	%rcx, %rax

	rep movsb		/* alignment bytes */
	movq	%rax, %rcx
	shrq	$3, %rcx	/* count of 8-byte qwords */

	rep movsq		/* copy qwords */
	andq	$7, %rax

pc_small:
	xchgq	%rax, %rcx	/* remainder */

	rep movsb

	movq	$0, %rax	/* 0 means: no fault */
LABEL(phys_copy_fault)
	popq	%rdi
	popq	%rsi
	popq	%rbp
	ret

LABEL(phys_copy_fault_in_kernel)
	popq	%rdi
	popq	%rsi
	popq	%rbp
	movq	%cr2, %rax
	ret

/*===========================================================================*/
/*				copy_msg_from_user			     */
/*===========================================================================*/
/*
 * int copy_msg_from_user(message * user_mbuf, message * dst);
 *
 * Copies a message of 64 bytes from user process space to a kernel buffer.
 * This function assumes the process address space is installed (cr3 loaded).
 *
 * x86_64: message size is still 64 bytes (like MINIX 3),
 * but we copy as 8 qwords of 8 bytes each.
 */
ENTRY(copy_msg_from_user)
	movq	%rdi, %rcx	/* source (user message) */
	movq	%rsi, %rdx	/* dest (kernel buffer) */

	movq	0*8(%rcx), %rax
	movq	%rax, 0*8(%rdx)
	movq	1*8(%rcx), %rax
	movq	%rax, 1*8(%rdx)
	movq	2*8(%rcx), %rax
	movq	%rax, 2*8(%rdx)
	movq	3*8(%rcx), %rax
	movq	%rax, 3*8(%rdx)
	movq	4*8(%rcx), %rax
	movq	%rax, 4*8(%rdx)
	movq	5*8(%rcx), %rax
	movq	%rax, 5*8(%rdx)
	movq	6*8(%rcx), %rax
	movq	%rax, 6*8(%rdx)
	movq	7*8(%rcx), %rax
	movq	%rax, 7*8(%rdx)

LABEL(__copy_msg_from_user_end)
	movq	$0, %rax
	ret

/*===========================================================================*/
/*				copy_msg_to_user			     */
/*===========================================================================*/
/*
 * void copy_msg_to_user(message * src, message * user_mbuf);
 *
 * Copies a message of 64 bytes to user process space from a kernel buffer.
 */
ENTRY(copy_msg_to_user)
	movq	%rdi, %rcx	/* source (kernel buffer) */
	movq	%rsi, %rdx	/* dest (user message) */

	movq	0*8(%rcx), %rax
	movq	%rax, 0*8(%rdx)
	movq	1*8(%rcx), %rax
	movq	%rax, 1*8(%rdx)
	movq	2*8(%rcx), %rax
	movq	%rax, 2*8(%rdx)
	movq	3*8(%rcx), %rax
	movq	%rax, 3*8(%rdx)
	movq	4*8(%rcx), %rax
	movq	%rax, 4*8(%rdx)
	movq	5*8(%rcx), %rax
	movq	%rax, 5*8(%rdx)
	movq	6*8(%rcx), %rax
	movq	%rax, 6*8(%rdx)
	movq	7*8(%rcx), %rax
	movq	%rax, 7*8(%rdx)

LABEL(__copy_msg_to_user_end)
	movq	$0, %rax
	ret

ENTRY(__user_copy_msg_pointer_failure)
	movq	$-1, %rax
	ret

/*===========================================================================*/
/*				phys_memset				     */
/*===========================================================================*/
/*
 * PUBLIC void phys_memset(phys_bytes dst, unsigned long pattern,
 *	phys_bytes bytecount);
 * Fill a block of physical memory with pattern.
 */
ENTRY(phys_memset)
	pushq	%rbp
	movq	%rsp, %rbp
	pushq	%rdi
	cld

	/* RDI already holds dst (1st arg) per calling convention */
	movq	%rdx, %rcx	/* bytecount (3rd arg) */
	movq	%rsi, %rax	/* pattern (2nd arg) */
	shrq	$3, %rcx	/* qword count */
	rep stosq		/* fill 8 bytes at a time */

	/* Any remaining bytes? */
	movq	%rdx, %rcx	/* restore original count */
	andq	$7, %rcx	/* remainder */
	rep stosb

LABEL(memset_fault)
	movq	$0, %rax	/* 0 means: no fault */
	popq	%rdi
	popq	%rbp
	ret

LABEL(memset_fault_in_kernel)
	popq	%rdi
	popq	%rbp
	movq	%cr2, %rax
	ret

/*===========================================================================*/
/*				x86_triplefault				     */
/*===========================================================================*/
/*
 * PUBLIC void x86_triplefault();
 * Reset the system by loading IDT with offset 0 and interrupting.
 */
ENTRY(x86_triplefault)
	lidtq	idt_zero
	int	$3

.data
idt_zero:
.quad	0, 0
.text

/*===========================================================================*/
/*				halt_cpu				     */
/*===========================================================================*/
/*
 * PUBLIC void halt_cpu(void);
 * Re-enables interrupts and halts the CPU.
 */
ENTRY(halt_cpu)
	sti
	hlt
	ret

/*===========================================================================*/
/*			poweroff_vmware_clihlt				     */
/*===========================================================================*/
/*
 * PUBLIC void poweroff_vmware_clihlt(void);
 * VMware detection and poweroff.
 */
ENTRY(poweroff_vmware_clihlt)
#ifndef NO_VMWARE_DETECTION
	movl	$1, %eax
	cpuid
	testl	$[1<<31], %ecx
	jz	1f
	movl	$0x40000000, %eax
	cpuid
	cmpl	$0x61774D56, %ebx	/* "VMwa" */
	jne	1f
	cmpl	$0x4D566572, %ecx	/* "reVM" */
	jne	1f
	cmpl	$0x65726177, %edx	/* "ware" */
	jne	1f
#endif
	cli
	hlt
1:
	ret

/*===========================================================================*/
/*				read_cpu_flags				     */
/*===========================================================================*/
/*
 * PUBLIC unsigned long read_cpu_flags(void);
 * Read CPU RFLAGS register.
 */
ENTRY(read_cpu_flags)
	pushfq
	popq	%rax
	ret

ENTRY(read_ds)
	xorl	%eax, %eax
	movw	%ds, %ax
	ret

ENTRY(read_cs)
	xorl	%eax, %eax
	movw	%cs, %ax
	ret

ENTRY(read_ss)
	xorl	%eax, %eax
	movw	%ss, %ax
	ret

/*===========================================================================*/
/*				fpu_routines				     */
/*===========================================================================*/

ENTRY(fninit)
	fninit
	ret

ENTRY(clts)
	clts
	ret

ENTRY(fnstsw)
	xorl	%eax, %eax
	fnstsw	%ax
	ret

/*===========================================================================*/
/*				fxrstor / frstor			     */
/*===========================================================================*/
ENTRY(fxrstor)
	movq	%rdi, %rax
	fxrstorq	(%rax)
ENTRY(__fxrstor_end)
	xorl	%eax, %eax
	ret

ENTRY(frstor)
	movq	%rdi, %rax
	frstor	(%rax)
ENTRY(__frstor_end)
	xorl	%eax, %eax
	ret

ENTRY(__frstor_failure)
	movl	$1, %eax
	ret

/*===========================================================================*/
/*			Control register access				     */
/*===========================================================================*/
ENTRY(read_cr0)
	movq	%cr0, %rax
	ret

ENTRY(read_cr2)
	movq	%cr2, %rax
	ret

ENTRY(read_cr3)
	movq	%cr3, %rax
	ret

ENTRY(read_cr4)
	movq	%cr4, %rax
	ret

ENTRY(write_cr4)
	movq	%rdi, %rax
	movq	%rax, %cr4
	ret

ENTRY(write_cr0)
	movq	%rdi, %rax
	movq	%rax, %cr0
	ret

ENTRY(write_cr3)
	movq	%rdi, %rax
	movq	%rax, %cr3
	ret

/*===========================================================================*/
/*			Descriptor table operations			     */
/*===========================================================================*/
/*
 * x86_ltr: Load Task Register (argument is selector value)
 */
ENTRY(x86_ltr)
	movq	%rdi, %rax
	ltr	%ax
	ret

/*
 * x86_lidt: Load IDT (argument is pointer to 6-byte pseudo-descriptor)
 * In 64-bit mode, the lidt operand is a 10-byte pseudo-descriptor
 * (2 bytes limit, 8 bytes base).
 */
ENTRY(x86_lidt)
	/* RDI points to a struct { u16_t limit; u64_t base; } */
	lidtq	(%rdi)
	ret

/*
 * x86_lgdt: Load GDT (argument is pointer to 10-byte pseudo-descriptor)
 */
ENTRY(x86_lgdt)
	lgdtq	(%rdi)
	ret

/*
 * x86_lldt: Load LDT (argument is selector value)
 */
ENTRY(x86_lldt)
	movq	%rdi, %rax
	lldt	%ax
	ret

/*
 * x86_sgdt: Store GDT (argument is pointer to 10-byte buffer)
 */
ENTRY(x86_sgdt)
	sgdtq	(%rdi)
	ret

/*
 * x86_sidt: Store IDT (argument is pointer to 10-byte buffer)
 */
ENTRY(x86_sidt)
	sidtq	(%rdi)
	ret

/*===========================================================================*/
/*			Segment register loads				     */
/*===========================================================================*/
ENTRY(x86_load_ds)
	movw	%di, %ds
	ret

ENTRY(x86_load_es)
	movw	%di, %es
	ret

ENTRY(x86_load_fs)
	movw	%di, %fs
	ret

ENTRY(x86_load_gs)
	movw	%di, %gs
	ret

ENTRY(x86_load_ss)
	movw	%di, %ss
	ret

/*===========================================================================*/
/*			FPU state save/restore				     */
/*===========================================================================*/
ENTRY(fnsave)
	fnsave	(%rdi)
	fwait
	ret

ENTRY(fxsave)
	fxsaveq	(%rdi)
	ret

ENTRY(fnstcw)
	fnstcw	(%rdi)
	ret

/*===========================================================================*/
/*				invlpg					     */
/*===========================================================================*/
ENTRY(x86_invlpg)
	invlpg	(%rdi)
	ret

/*===========================================================================*/
/*				x86_load_kerncs				     */
/*===========================================================================*/
/*
 * void x86_load_kerncs(void);
 * Far jump to reload CS with kernel code segment.
 * In 64-bit mode, a far jump (ljmp) loads CS from the descriptor,
 * but the target must be a 64-bit code segment.
 */
ENTRY(x86_load_kerncs)
	pushq	%rbp
	movq	%rsp, %rbp
	/* Push new CS and return address for far return */
	pushq	$KERN_CS_SELECTOR
	pushq	$newcs64
	lretq
newcs64:
	popq	%rbp
	ret

/*===========================================================================*/
/*			MSR read/write					     */
/*===========================================================================*/
/*
 * Read MSR
 * void ia32_msr_read(u32_t reg, u64_t *value)
 * On x86_64, the MSR value can be returned as a single 64-bit value.
 * We also maintain the old split interface for compatibility.
 */
ENTRY(ia32_msr_read)
	/* Old interface: void ia32_msr_read(u32_t reg, u32_t *hi, u32_t *lo) */
	pushq	%rbp
	movq	%rsp, %rbp

	movl	%edi, %ecx	/* reg */
	rdmsr
	movq	%rsi, %rcx	/* hi ptr (2nd arg) */
	movl	%edx, (%rcx)
	movq	%rdx, %rcx	/* lo ptr (3rd arg) */
	movl	%eax, (%rcx)

	popq	%rbp
	ret

/*
 * Write MSR
 * void ia32_msr_write(u32_t reg, u32_t hi, u32_t lo)
 */
ENTRY(ia32_msr_write)
	pushq	%rbp
	movq	%rsp, %rbp

	movl	%edx, %eax	/* lo (3rd arg) */
	movl	%esi, %edx	/* hi (2nd arg) */
	movl	%edi, %ecx	/* reg (1st arg) */
	wrmsr

	popq	%rbp
	ret

/*===========================================================================*/
/*			__switch_address_space				     */
/*===========================================================================*/
/*
 * PUBLIC void __switch_address_space(struct proc *p, struct proc **ptproc)
 *
 * Sets CR3 to the supplied value if not already set to the same value.
 * This avoids unnecessary TLB flushes.
 *
 * x86_64: CR3 is 64-bit (pML4 base address).
 */
ENTRY(__switch_address_space)
	movq	%rdi, %rdx		/* proc pointer (1st arg) */
	movq	P_CR3(%rdx), %rax	/* new CR3 value */
	testq	%rax, %rax
	je	0f

	movq	%cr3, %rcx
	cmpq	%rcx, %rax
	je	0f
	movq	%rax, %cr3

	/* Update ptproc */
	movq	%rsi, %rax		/* ptproc pointer (2nd arg) */
	movq	%rdx, (%rax)		/* *ptproc = p */
0:
	ret

/*===========================================================================*/
/*			PIC EOI functions				     */
/*===========================================================================*/
ENTRY(eoi_8259_master)
	movb	$END_OF_INT, %al
	outb	$INT_CTL
	ret

ENTRY(eoi_8259_slave)
	movb	$END_OF_INT, %al
	outb	$INT_CTL
	outb	$INT2_CTL
	ret

ENTRY(refresh_tlb)
	movq	%cr3, %rax
	movq	%rax, %cr3
	ret

/*===========================================================================*/
/*			mfence / arch_pause				     */
/*===========================================================================*/
ENTRY(mfence)
	mfence
	ret

ENTRY(arch_pause)
	pause
	ret

/*===========================================================================*/
/*				read_ebp				     */
/*===========================================================================*/
/*
 * PUBLIC u16_t read_ebp(void);
 * Returns the current base pointer (RBP on x86_64).
 */
ENTRY(read_ebp)
	movq	%rbp, %rax
	ret

ENTRY(interrupts_enable)
	sti
	ret

ENTRY(interrupts_disable)
	cli
	ret

/*===========================================================================*/
/*				cpuid					     */
/*===========================================================================*/
/*
 * PUBLIC void cpuid(u32_t leaf, u32_t *eax, u32_t *ebx, u32_t *ecx, u32_t *edx)
 * Execute CPUID instruction.
 */
ENTRY(cpuid)
	pushq	%rbx			/* must be preserved */
	/* Save ecx_ptr (4th arg, RCX) before CPUID clobbers it */
	movq	%rcx, %r9		/* save ecx_ptr in R9 */
	movl	%edi, %eax		/* leaf (1st arg) */
	cpuid
	movl	%eax, (%rsi)		/* *eax_ptr (2nd arg) */
	movl	%ebx, (%rdx)		/* *ebx_ptr (3rd arg) */
	movl	%ecx, (%r9)		/* *ecx_ptr (saved from RCX) */
	movl	%edx, (%r8)		/* *edx_ptr (5th arg) */
	popq	%rbx
	ret

/*===========================================================================*/
/*			switch_k_stack					     */
/*===========================================================================*/
/*
 * void switch_k_stack(void *rsp, void (*continuation)(void));
 *
 * Sets the current stack pointer to the given value and continues
 * execution at the given address.
 */
ENTRY(switch_k_stack)
	movq	%rsi, %rax	/* continuation (2nd arg) */
	movq	%rdi, %rcx	/* new RSP (1st arg) */
	movq	$0, %rbp	/* reset frame pointer for stack trace */
	movq	%rcx, %rsp	/* switch stack */
	jmp	*%rax		/* jump to continuation */

0:	jmp	0b		/* NOT REACHED */

.data
idt_ptr:
	.short 0x3ff
	.long 0x0
	.long 0x0		/* 64-bit IDT base high 32 bits */

ldtsel:
	.long LDT_SELECTOR

