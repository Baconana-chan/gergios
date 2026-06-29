/* This file is part of the lowest layer of the MINIX kernel.  (The other part
 * is "proc.c".)  The lowest layer does process switching and message handling.
 * Furthermore it contains the assembler startup code for Minix and the 64-bit
 * interrupt handlers.  It cooperates with the code in "start.c" to set up a
 * good environment for main().
 *
 * x86_64 version: uses long mode (64-bit), syscall/sysret for fast system
 * calls, iretq for interrupt/exception return.
 *
 * Kernel is entered either because of kernel-calls, ipc-calls, interrupts or
 * exceptions. The TSS is set so the kernel stack is loaded on privilege
 * change. The user context is saved to the proc table and the handler of the
 * event is called. Once the handler is done, switch_to_user() function is
 * called to pick a new process, finish what needs to be done for the next
 * process to run, sets its context and switch to userspace.
 */

#include "kernel/kernel.h"

#include <machine/vm.h>
#include "kernel/kernel.h"
#include <minix/config.h>
#include <minix/const.h>
#include <ipcconst.h>
#include <minix/com.h>
#include <machine/asm.h>
#include <machine/interrupt.h>
#include "archconst.h"
#include "kernel/const.h"
#include "kernel/proc.h"
#include "sconst.h"
#include <machine/multiboot.h>

#include "arch_proto.h"

#ifdef CONFIG_SMP
#include "kernel/smp.h"
#endif

IMPORT(usermapped_offset)
IMPORT(copr_not_available_handler)
IMPORT(params_size)
IMPORT(params_offset)
IMPORT(switch_to_user)
IMPORT(multiboot_init)

.text

/*===========================================================================*/
/*			interrupt handlers				     */
/*		interrupt handlers for x86_64 long mode			     */
/*===========================================================================*/

#define PIC_IRQ_HANDLER(irq)						\
	pushq	$irq							;\
	call	_C_LABEL(irq_handle)					;\
	add	$8, %rsp

/*===========================================================================*/
/*				hwint00 - 07				     */
/*===========================================================================*/
/*
 * Hardware interrupt from master PIC (IRQ 0-7).
 *
 * On entry, we are on the kernel stack (TSS set up this way).
 * The CPU has pushed SS, RSP, RFLAGS, CS, RIP on a CPL change.
 * For kernel->kernel interrupts, only CS:RIP:RFLAGS are pushed.
 *
 * We use TEST_INT_IN_KERNEL to check if we came from kernel mode.
 * If from kernel: save minimal context, handle, return.
 * If from user:  save full process context, handle, schedule.
 *
 * Note: In 64-bit mode, pusha/popa do not exist, so we save
 * registers individually.
 */
#define hwint_master(irq)						\
	TEST_INT_IN_KERNEL(0, 0f)					;\
	SAVE_PROCESS_CTX(0, KTS_INT_HARD)				;\
	pushq	%rbp							;\
	movq	$0, %rbp	/* for stack trace */			;\
	call	_C_LABEL(context_stop)					;\
	add	$8, %rsp						;\
	PIC_IRQ_HANDLER(irq)						;\
	movb	$END_OF_INT, %al					;\
	outb	$INT_CTL						;\
	jmp	_C_LABEL(switch_to_user)				;\
0:									\
	/* Interrupted in kernel mode */				\
	pushq	%rax							;\
	pushq	%rcx							;\
	pushq	%rdx							;\
	pushq	%rbx							;\
	pushq	%rsi							;\
	pushq	%rdi							;\
	pushq	%r8							;\
	pushq	%r9							;\
	pushq	%r10							;\
	pushq	%r11							;\
	pushq	%r12							;\
	pushq	%r13							;\
	pushq	%r14							;\
	pushq	%r15							;\
	call	_C_LABEL(context_stop_idle)				;\
	PIC_IRQ_HANDLER(irq)						;\
	movb	$END_OF_INT, %al					;\
	outb	$INT_CTL						;\
	popq	%r15							;\
	popq	%r14							;\
	popq	%r13							;\
	popq	%r12							;\
	popq	%r11							;\
	popq	%r10							;\
	popq	%r9							;\
	popq	%r8							;\
	popq	%rdi							;\
	popq	%rsi							;\
	popq	%rbx							;\
	popq	%rdx							;\
	popq	%rcx							;\
	popq	%rax							;\
	iretq

ENTRY(hwint00)
	hwint_master(0)

ENTRY(hwint01)
	hwint_master(1)

ENTRY(hwint02)
	hwint_master(2)

ENTRY(hwint03)
	hwint_master(3)

ENTRY(hwint04)
	hwint_master(4)

ENTRY(hwint05)
	hwint_master(5)

ENTRY(hwint06)
	hwint_master(6)

ENTRY(hwint07)
	hwint_master(7)

/*===========================================================================*/
/*				hwint08 - 15				     */
/*===========================================================================*/
#define hwint_slave(irq)						\
	TEST_INT_IN_KERNEL(0, 0f)					;\
	SAVE_PROCESS_CTX(0, KTS_INT_HARD)				;\
	pushq	%rbp							;\
	movq	$0, %rbp						;\
	call	_C_LABEL(context_stop)					;\
	add	$8, %rsp						;\
	PIC_IRQ_HANDLER(irq)						;\
	movb	$END_OF_INT, %al					;\
	outb	$INT_CTL						;\
	outb	$INT2_CTL						;\
	jmp	_C_LABEL(switch_to_user)				;\
0:									\
	pushq	%rax							;\
	pushq	%rcx							;\
	pushq	%rdx							;\
	pushq	%rbx							;\
	pushq	%rsi							;\
	pushq	%rdi							;\
	pushq	%r8							;\
	pushq	%r9							;\
	pushq	%r10							;\
	pushq	%r11							;\
	pushq	%r12							;\
	pushq	%r13							;\
	pushq	%r14							;\
	pushq	%r15							;\
	call	_C_LABEL(context_stop_idle)				;\
	PIC_IRQ_HANDLER(irq)						;\
	movb	$END_OF_INT, %al					;\
	outb	$INT_CTL						;\
	outb	$INT2_CTL						;\
	popq	%r15							;\
	popq	%r14							;\
	popq	%r13							;\
	popq	%r12							;\
	popq	%r11							;\
	popq	%r10							;\
	popq	%r9							;\
	popq	%r8							;\
	popq	%rdi							;\
	popq	%rsi							;\
	popq	%rbx							;\
	popq	%rdx							;\
	popq	%rcx							;\
	popq	%rax							;\
	iretq

ENTRY(hwint08)
	hwint_slave(8)

ENTRY(hwint09)
	hwint_slave(9)

ENTRY(hwint10)
	hwint_slave(10)

ENTRY(hwint11)
	hwint_slave(11)

ENTRY(hwint12)
	hwint_slave(12)

ENTRY(hwint13)
	hwint_slave(13)

ENTRY(hwint14)
	hwint_slave(14)

ENTRY(hwint15)
	hwint_slave(15)

/*===========================================================================*/
/*			System call entry (SYSCALL instruction)		     */
/*===========================================================================*/
/*
 * x86_64 uses the SYSCALL instruction for fast system calls (instead of
 * SYSENTER on 32-bit). SYSCALL saves RIP in RCX and RFLAGS in R11, then
 * loads new RIP and RFLAGS from MSRs (LSTAR and SF_MASK).
 *
 * Register usage on SYSCALL entry (from userland):
 *   RCX = return RIP (saved by CPU)
 *   R11 = return RFLAGS (saved by CPU)
 *   RAX = syscall number
 *   RDI = 1st arg  (endpoint)
 *   RSI = 2nd arg  (message pointer)
 *   RDX = 3rd arg  (status pointer or flags)
 *   R10 = 4th arg  (replacement for RCX, which is clobbered by SYSCALL)
 *
 * On return, SYSRET restores RIP from RCX and RFLAGS from R11.
 *
 * We save the return RIP/RFLAGS to the process structure, then dispatch
 * to the appropriate handler.
 */

/*
 * ipc_entry_syscall_cpu0 — Per-CPU SYSCALL entry point for CPU 0.
 *
 * On UP (uniprocessor) systems, this is an alias for ipc_entry_syscall.
 * The address is loaded into the LSTAR MSR to set up the SYSCALL
 * instruction target for the kernel on CPU 0.
 *
 * On SMP systems, ipc_entry_syscall_cpuN entries would point to
 * per-CPU entry points that load per-CPU kernel stacks. For now,
 * all CPUs share the same entry point.
 */
ENTRY(ipc_entry_syscall_cpu0)
	jmp	ipc_entry_syscall

ENTRY(ipc_entry_syscall)
	/* SYSCALL entry point for IPC.
	 * On entry (from userland via SYSCALL):
	 *   RCX = userspace RIP (return address)
	 *   R11 = userspace RFLAGS
	 *   RDI = dest endpoint
	 *   RSI = message pointer
	 *   RDX = status pointer / flags
	 *   RAX = IPC call type
	 *
	 * We need to:
	 * 1. Save the user context to the current process's proc struct
	 * 2. Set up kernel segments
	 * 3. Call the IPC handler
	 * 4. On return, schedule next process and restore context
	 */

	/* Switch to kernel stack.
	 * The TSS has kernel stack pointer (RSP0) set up per-CPU.
	 * We need to save the user RSP somewhere.
	 * Approach: use MSR-based kernel stack (like Linux does with
	 * MSR_KERNEL_GS_BASE to point per-CPU data area).
	 *
	 * For now, we use a simpler approach: the per-CPU kernel stack
	 * is set up via the TSS (RSP0). When an interrupt occurs, the
	 * CPU automatically switches stacks. But SYSCALL does NOT
	 * switch stacks - we must do it ourselves.
	 *
	 * We use the per-CPU kernel stack pointer stored in a scratch
	 * MSR or in a per-CPU data structure accessed via GS.base.
	 *
	 * For this port, we use Intel's recommended approach:
	 * - Set up kernel stack in TSS.RSP0
	 * - Use SWAPGS to get kernel GS base (per-CPU area)
	 * - Load kernel stack from per-CPU data
	 * - Copy user RSP to the kernel stack
	 */

	swapgs			/* GS.base = kernel per-CPU area */

	/* Save user stack pointer to a scratch location.
	 * We'll store it on the kernel stack. */
	movq	%rsp, %rdx	/* save user RSP */
	movq	%gs:0, %rsp	/* load kernel stack from per-CPU data */

	/* Push user state onto kernel stack (for restore_user_context) */
	pushq	%r11		/* user RFLAGS */
	pushq	%rcx		/* user RIP */
	pushq	%rdx		/* user RSP */

	/* Push a fake SS (user data segment selector with RPL=3) */
	pushq	%r11		/* user RFLAGS (again, for context saving) */
	pushq	%rcx		/* user RIP (again) */

	/* Now save user registers.
	 * We have: [kernel RSP] -> [saved RFLAGS], [saved RIP], [user RSP] */

	/* Get current process pointer from kernel stack top */
	movq	CURR_PROC_PTR(%rsp), %rbp

	/* Save general-purpose registers.
	 * RDI, RSI, RDX, R10 contain syscall args.
	 * RAX contains syscall number.
	 * RCX, R11 are saved above (return RIP, RFLAGS). */
	movq	%rdi, DIREG(%rbp)
	movq	%rsi, SIREG(%rbp)
	movq	%r10, CXREG(%rbp)	/* R10 holds orig RCX role */
	movq	%rdx, DXREG(%rbp)
	movq	%rax, AXREG(%rbp)
	movq	%rbx, BXREG(%rbp)
	/* R8, R9 stay as user set them */
	movq	%r8,  R8REG(%rbp)
	movq	%r9,  R9REG(%rbp)
	/* Save R12-R15 (caller preserved, but save for correctness) */
	movq	%r12, R12REG(%rbp)
	movq	%r13, R13REG(%rbp)
	movq	%r14, R14REG(%rbp)
	movq	%r15, R15REG(%rbp)

	/* Save RIP, RFLAGS, RSP from the stack */
	movq	8(%rsp), %rdi		/* saved RIP */
	movq	%rdi, PCREG(%rbp)
	movq	0(%rsp), %rdi		/* saved RFLAGS */
	movq	%rdi, PSWREG(%rbp)
	movq	16(%rsp), %rdi		/* saved user RSP */
	movq	%rdi, SPREG(%rbp)

	/* Record trap style */
	movq	$KTS_SYSCALL, P_KERN_TRAP_STYLE(%rbp)

	/* Set up kernel segments */
	mov	$KERN_DS_SELECTOR, %di
	mov	%di, %ds
	mov	%di, %es
	xor	%di, %di
	mov	%di, %fs
	/* GS remains with kernel base (swapgs already done) */

	/* Restore syscall args from saved regs for the handler */
	movq	DIREG(%rbp), %rdi	/* 1st arg: endpoint */
	movq	SIREG(%rbp), %rsi	/* 2nd arg: message ptr */
	movq	CXREG(%rbp), %rdx	/* 3rd arg: status/flags */
	movq	AXREG(%rbp), %rax	/* syscall type */

	/* Call context_stop to account for user CPU cycles */
	pushq	%rbp
	movq	$0, %rbp
	call	_C_LABEL(context_stop)
	add	$8, %rsp

	/* Check syscall type and dispatch */
	cmpq	$IPCVEC_UM, %rax
	jz	ipc_entry_syscall_common

	/* Unrecognized call type */
	movq	$-1, AXREG(%rbp)
	pushq	%rbp
	call	restore_user_context
	/* NOTREACHED */

ipc_entry_syscall_common:
	/* Call do_ipc(dest, msg_ptr, status_flags) */
	call	_C_LABEL(do_ipc)

	/* Save return value */
	movq	%rax, AXREG(%rbp)

	jmp	_C_LABEL(switch_to_user)

/*
 * IPC entry via software interrupt (int 0x80 / int 0x82 compat).
 * Also handles the "orig" and "um" entry points for non-syscall paths.
 */
ENTRY(ipc_entry_softint_orig)
	SAVE_PROCESS_CTX(0, KTS_INT_ORIG)
	jmp	ipc_entry_common

ENTRY(ipc_entry_softint_um)
	SAVE_PROCESS_CTX(0, KTS_INT_UM)
	jmp	ipc_entry_common

ENTRY(ipc_entry_common)
	/* Save the pointer to the current process */
	pushq	%rbp

	/* Pass the syscall arguments to the handler */
	pushq	%rbx		/* 3rd arg */
	pushq	%rax		/* 2nd arg (message ptr or similar) */
	pushq	%rcx		/* 1st arg (endpoint) */

	/* Stop user process cycles */
	pushq	%rbp
	movq	$0, %rbp
	call	_C_LABEL(context_stop)
	add	$8, %rsp

	call	_C_LABEL(do_ipc)

	/* Restore current process pointer and save return value */
	add	$24, %rsp
	popq	%rsi
	movq	%rax, AXREG(%rsi)

	jmp	_C_LABEL(switch_to_user)

/*
 * Kernel call entry (syscall type: kernel call, not IPC).
 */
ENTRY(kernel_call_entry_orig)
	SAVE_PROCESS_CTX(0, KTS_INT_ORIG)
	jmp	kernel_call_entry_common

ENTRY(kernel_call_entry_um)
	SAVE_PROCESS_CTX(0, KTS_INT_UM)
	jmp	kernel_call_entry_common

ENTRY(kernel_call_entry_common)
	/* Save the current process pointer */
	pushq	%rbp

	/* Pass the syscall args */
	pushq	%rax

	/* Stop user process cycles */
	pushq	%rbp
	movq	$0, %rbp
	call	_C_LABEL(context_stop)
	add	$8, %rsp

	call	_C_LABEL(kernel_call)

	/* Clean up */
	add	$16, %rsp

	jmp	_C_LABEL(switch_to_user)

/*===========================================================================*/
/*			Exception entry					     */
/*===========================================================================*/
.balign 16
exception_entry:
	/*
	 * Called by exception vector stubs.
	 * Stack layout (from CPU, with CPL change):
	 *   [RSP]     -> error code (or 0 if no error code pushed)
	 *   [RSP+8]   -> vector number (pushed by stub)
	 *   [RSP+16]  -> RIP
	 *   [RSP+24]  -> CS
	 *   [RSP+32]  -> RFLAGS
	 *   [RSP+40]  -> RSP (user stack, if CPL change)
	 *   [RSP+48]  -> SS (if CPL change)
	 *
	 * Check if nested trap by comparing CS with kernel CS.
	 * The vector number is at displ=0 (after error code) and
	 * CS is at displ+24 from RSP (below vector+err).
	 * After pushing error code and vector, total displacement
	 * for CS from current RSP = 0 + 8 + 24 = 32 (but we pushed
	 * nothing yet, so CS is at original RSP + 8 + 24).
	 *
	 * Wait - let's be precise about the stack:
	 * Before exception_entry is called, the stub pushes:
	 *   1. $0 (or nothing if error code exists) -- fake error code
	 *   2. $vector_number
	 * So at exception_entry:
	 *   [RSP]     = vector number
	 *   [RSP+8]   = error code (or 0)
	 *   [RSP+16]  = RIP (CPU pushed)
	 *   [RSP+24]  = CS
	 *   [RSP+32]  = RFLAGS
	 *   [RSP+40]  = RSP (if CPL change)
	 *   [RSP+48]  = SS (if CPL change)
	 *
	 * TEST_INT_IN_KERNEL checks CS at displ+8 from current stack
	 * after subtracting vector+err pushes. So from RSP, CS is
	 * at offset 24.
	 */
	TEST_INT_IN_KERNEL(16, exception_entry_nested)

exception_entry_from_user:
	SAVE_PROCESS_CTX(16, KTS_INT_HARD)
	/* The displacement (16) accounts for the vector number +
	 * error code pushed by the stub before the CPU interrupt frame. */

	/* Stop user process cycles */
	pushq	%rbp
	movq	$0, %rbp
	call	_C_LABEL(context_stop)
	add	$8, %rsp

	/* Push pointer to the interrupt frame and exception handler */
	pushq	%rsp
	pushq	$0	/* not a nested exception */
	call	_C_LABEL(exception_handler)

	jmp	_C_LABEL(switch_to_user)

exception_entry_nested:
	/* Save all registers manually (no pusha in 64-bit mode) */
	pushq	%rax
	pushq	%rcx
	pushq	%rdx
	pushq	%rbx
	pushq	%rsi
	pushq	%rdi
	pushq	%r8
	pushq	%r9
	pushq	%r10
	pushq	%r11
	pushq	%r12
	pushq	%r13
	pushq	%r14
	pushq	%r15

	/* Stack pointer to original frame */
	movq	%rsp, %rax
	add	$(15 * 8), %rax
	pushq	%rax
	pushq	$1	/* nested exception */
	call	_C_LABEL(exception_handler)
	add	$16, %rsp

	/* Restore all registers */
	popq	%r15
	popq	%r14
	popq	%r13
	popq	%r12
	popq	%r11
	popq	%r10
	popq	%r9
	popq	%r8
	popq	%rdi
	popq	%rsi
	popq	%rbx
	popq	%rdx
	popq	%rcx
	popq	%rax

	/* Clear error code and exception number */
	add	$16, %rsp
	/* Resume execution at the point of exception */
	iretq

/*===========================================================================*/
/*			restore_user_context				     */
/*===========================================================================*/
/*
 * Restore user context and return to userland.
 * Called with proc pointer as argument.
 *
 * For SYSCALL return, we use sysretq (restores RIP from RCX,
 * RFLAGS from R11). This requires:
 *   - RCX = return RIP
 *   - R11 = return RFLAGS
 *   - RSP already set to user stack
 *
 * For interrupt/exception return (iretq), we push:
 *   SS, RSP, RFLAGS, CS, RIP onto the kernel stack and iretq.
 *
 * For full context restore (KTS_FULLCONTEXT), we also restore
 * FS and GS bases via swapgs and wrmsr.
 */

ENTRY(restore_user_context)
	/* Called as: restore_user_context(proc_ptr) */
	/* Argument: RDI = proc pointer */
	movq	%rdi, %rbp

	/* Check trap style to determine return method */
	movq	P_KERN_TRAP_STYLE(%rbp), %rax

	cmpq	$KTS_SYSCALL, %rax
	je	restore_user_context_syscall

	/* Default: use iretq (for interrupts and exceptions) */
	jmp	restore_user_context_int

ENTRY(restore_user_context_syscall)
	/* Return via sysretq.
	 *
	 * On syscall entry, we saved:
	 *   RCX = user RIP
	 *   R11 = user RFLAGS
	 * User RSP is in SPREG(%rbp).
	 */
	movq	PCREG(%rbp), %rcx	/* sysretq restores RIP from RCX */
	movq	SPREG(%rbp), %rsp	/* restore user RSP */
	movq	PSWREG(%rbp), %r11	/* sysretq restores RFLAGS from R11 */
	movq	AXREG(%rbp), %rax	/* return value */
	movq	BXREG(%rbp), %rbx	/* secondary return value */

	/* Restore R8-R15 */
	movq	R8REG(%rbp), %r8
	movq	R9REG(%rbp), %r9
	movq	R10REG(%rbp), %r10
	movq	R12REG(%rbp), %r12
	movq	R13REG(%rbp), %r13
	movq	R14REG(%rbp), %r14
	movq	R15REG(%rbp), %r15

	/* Restore RSI, RDI from saved context */
	movq	SIREG(%rbp), %rsi
	movq	DIREG(%rbp), %rdi

	/* Restore RDX last (used for syscall args) */
	movq	DXREG(%rbp), %rdx

	swapgs			/* restore user GS base */
	sysretq			/* return to userspace (RIP=RCX, RFLAGS=R11) */

ENTRY(restore_user_context_int)
	/* Return via iretq.
	 *
	 * Stack for iretq:
	 *   [RSP]   -> SS (user data segment with RPL=3)
	 *   [RSP+8] -> RSP (user stack pointer)
	 *   [RSP+16]-> RFLAGS
	 *   [RSP+24]-> CS (user code segment with RPL=3)
	 *   [RSP+32]-> RIP
	 */

	/* Push SS */
	pushq	$USER_DS_SELECTOR
	/* Push RSP */
	movq	SPREG(%rbp), %rax
	pushq	%rax
	/* Push RFLAGS */
	movq	PSWREG(%rbp), %rax
	pushq	%rax
	/* Push CS */
	pushq	$USER_CS_SELECTOR
	/* Push RIP */
	movq	PCREG(%rbp), %rax
	pushq	%rax

	/* Restore segments as user expects them.
	 * In 64-bit mode, DS/ES/FS/GS are ignored for address generation,
	 * but we restore them for safety. */
	movw	$USER_DS_SELECTOR, %si
	movw	%si, %ds
	movw	%si, %es
	/* FS and GS: user can set base via MSR, keep zero for now */
	xorw	%si, %si
	movw	%si, %fs
	movw	%si, %gs

	/* Restore general-purpose registers */
	RESTORE_GP_REGS(%rbp)

	/* Restore RBP last */
	movq	BPREG(%rbp), %rbp

	iretq			/* continue in userspace */

/*===========================================================================*/
/*			Exception vector stubs				     */
/*===========================================================================*/
/*
 * Each exception vector stub pushes a fake error code (if needed) and
 * the vector number, then jumps to exception_entry.
 */

#define EXCEPTION_ERR_CODE(vector)					\
	pushq	$vector							;\
	jmp	exception_entry

#define EXCEPTION_NO_ERR_CODE(vector)					\
	pushq	$0			/* fake error code */		;\
	EXCEPTION_ERR_CODE(vector)

LABEL(divide_error)
	EXCEPTION_NO_ERR_CODE(DIVIDE_VECTOR)

LABEL(single_step_exception)
	EXCEPTION_NO_ERR_CODE(DEBUG_VECTOR)

LABEL(nmi)
#ifndef USE_WATCHDOG
	EXCEPTION_NO_ERR_CODE(NMI_VECTOR)
#else
	/* NMI watchdog handler.
	 * NMI can occur at any time. Save minimal context and call handler. */
	pushq	%rax
	pushq	%rcx
	pushq	%rdx
	pushq	%rbx
	pushq	%rsi
	pushq	%rdi
	pushq	%r8
	pushq	%r9
	pushq	%r10
	pushq	%r11
	pushq	%r12
	pushq	%r13
	pushq	%r14
	pushq	%r15

	/* Set kernel segments */
	mov	$KERN_DS_SELECTOR, %si
	mov	%si, %ds
	mov	%si, %es

	pushq	%rsp
	call	_C_LABEL(nmi_watchdog_handler)
	add	$8, %rsp

	popq	%r15
	popq	%r14
	popq	%r13
	popq	%r12
	popq	%r11
	popq	%r10
	popq	%r9
	popq	%r8
	popq	%rdi
	popq	%rsi
	popq	%rbx
	popq	%rdx
	popq	%rcx
	popq	%rax

	iretq
#endif

LABEL(breakpoint_exception)
	EXCEPTION_NO_ERR_CODE(BREAKPOINT_VECTOR)

LABEL(overflow)
	EXCEPTION_NO_ERR_CODE(OVERFLOW_VECTOR)

LABEL(bounds_check)
	EXCEPTION_NO_ERR_CODE(BOUNDS_VECTOR)

LABEL(inval_opcode)
	EXCEPTION_NO_ERR_CODE(INVAL_OP_VECTOR)

LABEL(copr_not_available)
	TEST_INT_IN_KERNEL(24, copr_not_available_in_kernel)
	cld
	SAVE_PROCESS_CTX(0, KTS_INT_HARD)
	pushq	%rbp
	movq	$0, %rbp
	call	_C_LABEL(context_stop)
	call	_C_LABEL(copr_not_available_handler)
	jmp	_C_LABEL(switch_to_user)

copr_not_available_in_kernel:
	pushq	$0
	pushq	$COPROC_NOT_VECTOR
	jmp	exception_entry_nested

LABEL(double_fault)
	EXCEPTION_ERR_CODE(DOUBLE_FAULT_VECTOR)

LABEL(copr_seg_overrun)
	EXCEPTION_NO_ERR_CODE(COPROC_SEG_VECTOR)

LABEL(inval_tss)
	EXCEPTION_ERR_CODE(INVAL_TSS_VECTOR)

LABEL(segment_not_present)
	EXCEPTION_ERR_CODE(SEG_NOT_VECTOR)

LABEL(stack_exception)
	EXCEPTION_ERR_CODE(STACK_FAULT_VECTOR)

LABEL(general_protection)
	EXCEPTION_ERR_CODE(PROTECTION_VECTOR)

LABEL(page_fault)
	EXCEPTION_ERR_CODE(PAGE_FAULT_VECTOR)

LABEL(copr_error)
	EXCEPTION_NO_ERR_CODE(COPROC_ERR_VECTOR)

LABEL(alignment_check)
	EXCEPTION_NO_ERR_CODE(ALIGNMENT_CHECK_VECTOR)

LABEL(machine_check)
	EXCEPTION_NO_ERR_CODE(MACHINE_CHECK_VECTOR)

LABEL(simd_exception)
	EXCEPTION_NO_ERR_CODE(SIMD_EXCEPTION_VECTOR)

/*===========================================================================*/
/*				reload_cr3				     */
/*===========================================================================*/
/* PUBLIC void reload_cr3(void); */
ENTRY(reload_cr3)
	pushq	%rbp
	movq	%rsp, %rbp
	movq	%cr3, %rax
	movq	%rax, %cr3
	popq	%rbp
	ret

#ifdef CONFIG_SMP
ENTRY(startup_ap_64)
	/* Entry point for Application Processors (APs) in 64-bit mode.
	 * We are in long mode with a flat 64-bit GDT loaded by the
	 * startup IPI trampoline code. */
	movw	$KERN_DS_SELECTOR, %ax
	movw	%ax, %ds
	movw	%ax, %es
	movw	%ax, %ss
	movabs	$k_boot_stktop, %rsp

	call	_C_LABEL(prot_load_selectors)
	call	_C_LABEL(smp_ap_boot)
	hlt
#endif

/*===========================================================================*/
/*				data					     */
/*===========================================================================*/

.data
.short	0x526F	/* magic number -- must be first data entry */

.bss
k_initial_stack:
.space	K_STACK_SIZE
LABEL(__k_unpaged_k_initial_stktop)

k_boot_stack:
.space	K_STACK_SIZE
LABEL(k_boot_stktop)

.balign K_STACK_SIZE
LABEL(k_stacks_start)

.space	2 * (K_STACK_SIZE * CONFIG_MAX_CPUS)
LABEL(k_stacks_end)

