#ifndef __X86_64_SCONST_H__
#define __X86_64_SCONST_H__

#include "kernel/const.h"
#include "procoffsets.h"

/* K_STACK_SIZE — must be visible in assembly (arch_proto.h is inside
 * #ifndef __ASSEMBLY__ in kernel.h, so not available to .S files). */
#define K_STACK_SIZE		4096

/*
 * CURR_PROC_PTR: offset from %rsp (kernel stack) to the current
 * process pointer stored at the top of the kernel stack.
 * Value = -X86_STACK_TOP_RESERVED = -(2 * sizeof(reg_t)) = -16.
 * Using a literal to avoid sizeof() which is a C-only operator.
 */
#define CURR_PROC_PTR		(-16)

/*
 * Tests whether the interrupt was triggered in kernel mode.
 * In x86_64, CS is saved at displ+8 from the exception/interrupt frame.
 * For exceptions (with error code): displ+8+8
 * The kernel CS selector has lower 3 bits zeroed.
 */
#define TEST_INT_IN_KERNEL(displ, label)	\
	cmpq	$KERN_CS_SELECTOR, displ+8(%rsp)	;\
	je	label

/*
 * Saves the basic interrupt context (no error code) to the process structure.
 *
 * x86_64 interrupt stack (from CPU, no CPL change):
 *   [RSP]   -> RIP
 *   [RSP+8] -> CS
 *   [RSP+16]-> RFLAGS
 *
 * The macro reads in CPU push order: RIP at lowest address, RFLAGS at highest.
 * displ: extra displacement (e.g., for error code or exception number).
 */
#define SAVE_TRAP_CTX(displ, pptr, tmp)		\
	movq	(0 + displ)(%rsp), tmp			;\
	movq	tmp, PCREG(pptr)			;\
	movq	(8 + displ)(%rsp), tmp			;\
	movq	tmp, CSREG(pptr)			;\
	movq	(16 + displ)(%rsp), tmp			;\
	movq	tmp, PSWREG(pptr)

/*
 * For user->kernel transitions via interrupt/exception, the CPU pushes:
 *   [RSP]   -> RIP      (lowest address, pushed last by CPU)
 *   [RSP+8] -> CS
 *   [RSP+16]-> RFLAGS
 *   [RSP+24]-> RSP (user stack pointer)
 *   [RSP+32]-> SS       (highest address, pushed first by CPU)
 *
 * The macro reads in CPU push order matching the stack layout.
 */
#define SAVE_TRAP_CTX_USER(displ, pptr, tmp)	\
	movq	(0 + displ)(%rsp), tmp			;\
	movq	tmp, PCREG(pptr)			;\
	movq	(8 + displ)(%rsp), tmp			;\
	movq	tmp, CSREG(pptr)			;\
	movq	(16 + displ)(%rsp), tmp			;\
	movq	tmp, PSWREG(pptr)			;\
	movq	(24 + displ)(%rsp), tmp			;\
	movq	tmp, SPREG(pptr)			;\
	movq	(32 + displ)(%rsp), tmp			;\
	movq	tmp, SSREG(pptr)

/*
 * Restore kernel segments on entry.
 * In x86_64 long mode, segmentation is largely disabled:
 * - %cs and %ss are the only segments with meaning
 * - %ds, %es, %fs, %gs are ignored in 64-bit mode (base = 0)
 * - We still set them to kernel DS for safety
 */
#define RESTORE_KERNEL_SEGS				\
	mov	$KERN_DS_SELECTOR, %si			;\
	mov	%si, %ds				;\
	mov	%si, %es				;\
	xor	%si, %si				;\
	mov	%si, %fs				;\
	mov	%si, %gs

/*
 * Save general-purpose registers to the process structure.
 * x86_64 has 16 GPRs: RAX, RCX, RDX, RBX, RSI, RDI, R8-R15.
 * RSP and RBP are saved separately.
 */
#define SAVE_GP_REGS(pptr)				\
	movq	%rax, AXREG(pptr)			;\
	movq	%rcx, CXREG(pptr)			;\
	movq	%rdx, DXREG(pptr)			;\
	movq	%rbx, BXREG(pptr)			;\
	movq	%rsi, SIREG(pptr)			;\
	movq	%rdi, DIREG(pptr)			;\
	movq	%r8,  R8REG(pptr)			;\
	movq	%r9,  R9REG(pptr)			;\
	movq	%r10, R10REG(pptr)			;\
	movq	%r11, R11REG(pptr)			;\
	movq	%r12, R12REG(pptr)			;\
	movq	%r13, R13REG(pptr)			;\
	movq	%r14, R14REG(pptr)			;\
	movq	%r15, R15REG(pptr)

#define RESTORE_GP_REGS(pptr)				\
	movq	AXREG(pptr), %rax			;\
	movq	CXREG(pptr), %rcx			;\
	movq	DXREG(pptr), %rdx			;\
	movq	BXREG(pptr), %rbx			;\
	movq	SIREG(pptr), %rsi			;\
	movq	DIREG(pptr), %rdi			;\
	movq	R8REG(pptr), %r8			;\
	movq	R9REG(pptr), %r9			;\
	movq	R10REG(pptr), %r10			;\
	movq	R11REG(pptr), %r11			;\
	movq	R12REG(pptr), %r12			;\
	movq	R13REG(pptr), %r13			;\
	movq	R14REG(pptr), %r14			;\
	movq	R15REG(pptr), %r15

/*
 * NOTE on ; vs \\ in macros:
 * The \\ at end of line is consumed by the C preprocessor to join lines.
 * The ; separator is needed to separate multiple instructions that end up
 * on the SAME logical line after the \\ are consumed.
 *
 * However, when a macro calls another macro (like SAVE_PROCESS_CTX calling
 * SAVE_GP_REGS which already ends with ;), the calling macro should NOT
 * add another ; after the call, to avoid creating ;; (empty statement).
 * This is handled in mpx.S where hwint_master/hwint_slave macros are
 * defined without trailing ; after inner macro calls.
 */

/*
 * Save the full process context (interrupt/exception entry from user).
 *
 * For x86_64, the stack after CPU interrupt (CPL change) is:
 *   SS, RSP, RFLAGS, CS, RIP  (5 values, 40 bytes)
 * Plus error code for some exceptions.
 *
 * The kernel stack top has:
 *   [stack_top - 8] = current process pointer
 *   [stack_top - 16] = current CPU id (unused for now)
 */
#define SAVE_PROCESS_CTX(displ, trapcode)		\
	cld						;\
	pushq	%rbp					;\
	movq	CURR_PROC_PTR(%rsp), %rbp		;\
	SAVE_GP_REGS(%rbp)				;\
	movq	$trapcode, P_KERN_TRAP_STYLE(%rbp)	;\
	popq	%rsi					;\
	movq	%rsi, BPREG(%rbp)			;\
	RESTORE_KERNEL_SEGS				;\
	SAVE_TRAP_CTX_USER(displ, %rbp, %rsi)

/*
 * Save process context for kernel-entry (no SS/RSP pushed by CPU).
 * This is used for exceptions that occur in kernel mode.
 */
#define SAVE_PROCESS_CTX_KERNEL(displ, trapcode)	\
	cld						;\
	pushq	%rbp					;\
	movq	CURR_PROC_PTR(%rsp), %rbp		;\
	SAVE_GP_REGS(%rbp)				;\
	movq	$trapcode, P_KERN_TRAP_STYLE(%rbp)	;\
	popq	%rsi					;\
	movq	%rsi, BPREG(%rbp)			;\
	SAVE_TRAP_CTX(displ, %rbp, %rsi)

/*
 * Clear IF flag in RFLAGS stored on the stack.
 */
#define CLEAR_IF(where)					\
	movq	where, %rax				;\
	andq	$0xfffffffffffffdff, %rax		;\
	movq	%rax, where

#endif /* __X86_64_SCONST_H__ */
