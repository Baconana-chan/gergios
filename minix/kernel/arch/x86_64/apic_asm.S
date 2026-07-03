/* x86_64 APIC interrupt handlers.
 *
 * 64-bit versions of APIC interrupt handlers. These replace the 8259 PIC
 * handlers when the APIC is enabled.
 *
 * Key differences from i386:
 *   - 64-bit registers (rax, rbx, ... r15)
 *   - No pusha/popa — save registers individually
 *   - iretq instead of iretd
 *   - lapic_eoi() is a C function or direct MMIO write
 *
 * Each handler:
 *   1. Tests if we interrupted kernel mode (TEST_INT_IN_KERNEL)
 *   2. If from user: SAVE_PROCESS_CTX → context_stop → irq_handle → switch_to_user
 *   3. If from kernel: save regs → context_stop_idle → irq_handle → iretq
 *
 * The handlers here are:
 *   apic_hwint0-63  — IOAPIC interrupt vectors (via gate_table_ioapic)
 *   lapic_timer_int_handler — LAPIC timer tick
 *   apic_spurios_intr       — Spurious interrupt
 *   apic_error_intr         — APIC error interrupt
 *   apic_ipi_sched_intr     — SMP scheduling IPI (CONFIG_SMP)
 *   apic_ipi_halt_intr      — SMP halt IPI (CONFIG_SMP)
 */

#include "kernel/kernel.h"
#include <machine/asm.h>
#include <machine/interrupt.h>
#include "archconst.h"
#include "kernel/const.h"
#include "kernel/proc.h"
#include "sconst.h"

/* Import C handler functions */
IMPORT(irq_handle)
IMPORT(context_stop)
IMPORT(context_stop_idle)
IMPORT(switch_to_user)

/* ========================================================================= */
/*                Macro: APIC interrupt handler for IOAPIC IRQs              */
/* ========================================================================= */
/*
 * Each IOAPIC interrupt (IRQ 0-63) is dispatched through apic_hwintN.
 *
 * Stack layout at entry (CPL change from user to kernel):
 *   [RSP]    = (saved by CPU) SS
 *   [RSP+8]  = RSP (user stack)
 *   [RSP+16] = RFLAGS
 *   [RSP+24] = CS
 *   [RSP+32] = RIP
 *
 * For kernel→kernel interrupts, only CS:RIP:RFLAGS are pushed.
 *
 * The macro:
 *   - Pushes IRQ number as argument to irq_handle()
 *   - Calls apic_eoi() AFTER irq_handle() — managed by irq_handle internals
 *     or the ioapic_eoi() function
 */

#define APIC_IRQ_HANDLER(irq)						\
	TEST_INT_IN_KERNEL(0, 0f)					;\
	SAVE_PROCESS_CTX(0, KTS_INT_HARD)				;\
	pushq	%rbp							;\
	movq	$0, %rbp	/* for stack trace */			;\
	call	_C_LABEL(context_stop)					;\
	add	$8, %rsp	/* pop rbp */				;\
	pushq	$irq							;\
	call	_C_LABEL(irq_handle)					;\
	add	$8, %rsp	/* pop irq */				;\
	jmp	_C_LABEL(switch_to_user)				;\
0:	/* Interrupted in kernel mode */				\
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
	pushq	$irq							;\
	call	_C_LABEL(irq_handle)					;\
	add	$8, %rsp						;\
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

/* ========================================================================= */
/*                IOAPIC interrupt vectors 0-63                             */
/* ========================================================================= */

ENTRY(apic_hwint0)	APIC_IRQ_HANDLER(0)
ENTRY(apic_hwint1)	APIC_IRQ_HANDLER(1)
ENTRY(apic_hwint2)	APIC_IRQ_HANDLER(2)
ENTRY(apic_hwint3)	APIC_IRQ_HANDLER(3)
ENTRY(apic_hwint4)	APIC_IRQ_HANDLER(4)
ENTRY(apic_hwint5)	APIC_IRQ_HANDLER(5)
ENTRY(apic_hwint6)	APIC_IRQ_HANDLER(6)
ENTRY(apic_hwint7)	APIC_IRQ_HANDLER(7)
ENTRY(apic_hwint8)	APIC_IRQ_HANDLER(8)
ENTRY(apic_hwint9)	APIC_IRQ_HANDLER(9)
ENTRY(apic_hwint10)	APIC_IRQ_HANDLER(10)
ENTRY(apic_hwint11)	APIC_IRQ_HANDLER(11)
ENTRY(apic_hwint12)	APIC_IRQ_HANDLER(12)
ENTRY(apic_hwint13)	APIC_IRQ_HANDLER(13)
ENTRY(apic_hwint14)	APIC_IRQ_HANDLER(14)
ENTRY(apic_hwint15)	APIC_IRQ_HANDLER(15)
ENTRY(apic_hwint16)	APIC_IRQ_HANDLER(16)
ENTRY(apic_hwint17)	APIC_IRQ_HANDLER(17)
ENTRY(apic_hwint18)	APIC_IRQ_HANDLER(18)
ENTRY(apic_hwint19)	APIC_IRQ_HANDLER(19)
ENTRY(apic_hwint20)	APIC_IRQ_HANDLER(20)
ENTRY(apic_hwint21)	APIC_IRQ_HANDLER(21)
ENTRY(apic_hwint22)	APIC_IRQ_HANDLER(22)
ENTRY(apic_hwint23)	APIC_IRQ_HANDLER(23)
ENTRY(apic_hwint24)	APIC_IRQ_HANDLER(24)
ENTRY(apic_hwint25)	APIC_IRQ_HANDLER(25)
ENTRY(apic_hwint26)	APIC_IRQ_HANDLER(26)
ENTRY(apic_hwint27)	APIC_IRQ_HANDLER(27)
ENTRY(apic_hwint28)	APIC_IRQ_HANDLER(28)
ENTRY(apic_hwint29)	APIC_IRQ_HANDLER(29)
ENTRY(apic_hwint30)	APIC_IRQ_HANDLER(30)
ENTRY(apic_hwint31)	APIC_IRQ_HANDLER(31)
ENTRY(apic_hwint32)	APIC_IRQ_HANDLER(32)
ENTRY(apic_hwint33)	APIC_IRQ_HANDLER(33)
ENTRY(apic_hwint34)	APIC_IRQ_HANDLER(34)
ENTRY(apic_hwint35)	APIC_IRQ_HANDLER(35)
ENTRY(apic_hwint36)	APIC_IRQ_HANDLER(36)
ENTRY(apic_hwint37)	APIC_IRQ_HANDLER(37)
ENTRY(apic_hwint38)	APIC_IRQ_HANDLER(38)
ENTRY(apic_hwint39)	APIC_IRQ_HANDLER(39)
ENTRY(apic_hwint40)	APIC_IRQ_HANDLER(40)
ENTRY(apic_hwint41)	APIC_IRQ_HANDLER(41)
ENTRY(apic_hwint42)	APIC_IRQ_HANDLER(42)
ENTRY(apic_hwint43)	APIC_IRQ_HANDLER(43)
ENTRY(apic_hwint44)	APIC_IRQ_HANDLER(44)
ENTRY(apic_hwint45)	APIC_IRQ_HANDLER(45)
ENTRY(apic_hwint46)	APIC_IRQ_HANDLER(46)
ENTRY(apic_hwint47)	APIC_IRQ_HANDLER(47)
ENTRY(apic_hwint48)	APIC_IRQ_HANDLER(48)
ENTRY(apic_hwint49)	APIC_IRQ_HANDLER(49)
ENTRY(apic_hwint50)	APIC_IRQ_HANDLER(50)
ENTRY(apic_hwint51)	APIC_IRQ_HANDLER(51)
ENTRY(apic_hwint52)	APIC_IRQ_HANDLER(52)
ENTRY(apic_hwint53)	APIC_IRQ_HANDLER(53)
ENTRY(apic_hwint54)	APIC_IRQ_HANDLER(54)
ENTRY(apic_hwint55)	APIC_IRQ_HANDLER(55)
ENTRY(apic_hwint56)	APIC_IRQ_HANDLER(56)
ENTRY(apic_hwint57)	APIC_IRQ_HANDLER(57)
ENTRY(apic_hwint58)	APIC_IRQ_HANDLER(58)
ENTRY(apic_hwint59)	APIC_IRQ_HANDLER(59)
ENTRY(apic_hwint60)	APIC_IRQ_HANDLER(60)
ENTRY(apic_hwint61)	APIC_IRQ_HANDLER(61)
ENTRY(apic_hwint62)	APIC_IRQ_HANDLER(62)
ENTRY(apic_hwint63)	APIC_IRQ_HANDLER(63)

/* ========================================================================= */
/*                   LAPIC timer interrupt handler                          */
/* ========================================================================= */
/*
 * The LAPIC timer fires at APIC_TIMER_INT_VECTOR (0xf0).
 * Calls timer_int_handler() to process clock ticks.
 * After the handler, writes 0 to LAPIC EOI register via apic_eoi().
 *
 * We use a simpler lapic_intr macro since timer handlers don't go
 * through irq_handle() — they call timer_int_handler() directly.
 */

#define LAPIC_INTR_HANDLER(func)					\
	call	*%rax	/* call the actual handler */			;\
	/* Write 0 to LAPIC EOI register */				;\
	/* Use RIP-relative addressing — kernel virtual address		\
	 * (0xFFFF8000F0100000+) is outside 32-bit signed range,	\
	 * so absolute 32-bit addressing (R_X86_64_32S) fails.	\
	 * RIP-relative (R_X86_64_PC32) works because the kernel	\
	 * binary is small and all symbols are within 2GB of each other. */	;\
	movq	_C_LABEL(lapic_eoi_addr)(%rip), %rax			;\
	movl	$0, (%rax)

#define lapic_intr(func)						\
	TEST_INT_IN_KERNEL(0, 0f)					;\
	SAVE_PROCESS_CTX(0, KTS_INT_HARD)				;\
	pushq	%rbp							;\
	movq	$0, %rbp						;\
	call	_C_LABEL(context_stop)					;\
	add	$8, %rsp						;\
	movabsq	$func, %rax						;\
	LAPIC_INTR_HANDLER(func)					;\
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
	movabsq	$func, %rax						;\
	LAPIC_INTR_HANDLER(func)					;\
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

IMPORT(timer_int_handler)
IMPORT(apic_spurios_intr_handler)
IMPORT(apic_error_intr_handler)

/* LAPIC timer tick handler */
ENTRY(lapic_timer_int_handler)
	lapic_intr(_C_LABEL(timer_int_handler))

/* Spurious interrupt handler */
ENTRY(apic_spurios_intr)
	lapic_intr(_C_LABEL(apic_spurios_intr_handler))

/* APIC error interrupt handler */
ENTRY(apic_error_intr)
	lapic_intr(_C_LABEL(apic_error_intr_handler))

#ifdef CONFIG_SMP

IMPORT(smp_ipi_sched_handler)
IMPORT(smp_ipi_halt_handler)

/* SMP scheduling IPI handler */
ENTRY(apic_ipi_sched_intr)
	lapic_intr(_C_LABEL(smp_ipi_sched_handler))

/* SMP CPU halt IPI handler */
ENTRY(apic_ipi_halt_intr)
	lapic_intr(_C_LABEL(smp_ipi_halt_handler))

#endif /* CONFIG_SMP */
