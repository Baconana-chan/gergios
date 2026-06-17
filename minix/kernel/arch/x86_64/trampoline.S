/* x86_64 SMP trampoline for Application Processors (APs).
 *
 * This code is copied to low memory (< 1MB) by the BSP and executed
 * by each AP after receiving an INIT-SIPI-SIPI sequence.
 *
 * Execution flow:
 *   1. Start in 16-bit real mode (after SIPI, CPU starts at CS:IP = 0000:page*100)
 *   2. Load GDT with 32-bit + 64-bit code segments
 *   3. Switch to 32-bit protected mode
 *   4. Set up page tables: CR4.PAE, load CR3 (PML4 base), EFER.LME
 *   5. Enable paging (this transitions to long mode)
 *   6. Far jump to 64-bit code segment
 *   7. In 64-bit mode: load temporary stack (k_boot_stktop)
 *   8. Jump to startup_ap_64 (in mpx.S) which sets per-CPU stack via smp_ap_boot()
 *
 * The page table used is the BSP's pml4_table (identity-mapped entry 0),
 * so the AP can execute from the trampoline copy in low physical memory.
 *
 * All variables (__ap_id, __ap_pt, etc.) are stored in 64-bit format
 * to support 64-bit physical addresses for the page table.
 */

#include <machine/asm.h>
#include <machine/vm.h>
#include "archconst.h"

/* Page table and control register flags */
#define X86_64_CR4_PAE		0x00000020	/* Physical Address Extension */
#define X86_64_CR4_PGE		0x00000080	/* Page Global Enable */
#define X86_64_EFER_LME		0x00000100	/* Long Mode Enable */
#define X86_64_CR0_PG		0x80000000	/* Paging Enable */

/* Segment selector for 64-bit kernel code in the trampoline GDT */
#define TRAMP_CS32_SEL		0x08		/* 32-bit code segment selector */
#define TRAMP_CS64_SEL		0x10		/* 64-bit code segment selector */
#define TRAMP_DS_SEL		0x18		/* data segment selector */

.balign 4096
.text
.code16

ENTRY(trampoline)
	cli
	cld

	/* %cs has some value and we must use the same for data */
	mov	%cs, %ax
	mov	%ax, %ds

	/* load gdt and idt prepared by bsp.
	 * Use 32-bit variants (lgdtl/lidtl) since we're in 16-bit mode
	 * but the operands are 32-bit offsets relative to trampoline */
	lgdtl	_C_LABEL(__ap_gdt) - _C_LABEL(trampoline)
	lidtl	_C_LABEL(__ap_idt) - _C_LABEL(trampoline)

	/* Switch to 32-bit protected mode */
	mov	%cr0, %eax
	orb	$1, %al			/* CR0.PE = 1 */
	mov	%eax, %cr0

	/* Far jump to flush prefetch and enter 32-bit mode */
	ljmp	$TRAMP_CS32_SEL, $trampoline_pm

.code32
trampoline_pm:
	/* Now in 32-bit protected mode */
	mov	$TRAMP_DS_SEL, %ax
	mov	%ax, %ds
	mov	%ax, %es
	mov	%ax, %ss

	/* Step 1: Enable PAE in CR4 (required for long mode) */
	movl	%cr4, %eax
	orl	$X86_64_CR4_PAE, %eax
	andl	$~X86_64_CR4_PGE, %eax	/* PGE off for now */
	movl	%eax, %cr4

	/* Step 2: Load CR3 with PML4 base address.
	 * In 32-bit compat mode (before far jump to CS64), we can only
	 * write lower 32 bits of CR3 via movl %eax, %cr3.
	 * The PML4 table is allocated via alloc_lowest() in low memory
	 * (< 4GB physical), so upper 32 bits are 0. alloc_lowest()
	 * guarantees addresses in the first 16MB of physical memory.
	 * When we later transition to long mode, the upper 32 bits
	 * remain 0, which is correct for physical addresses < 4GB. */
	movl	_C_LABEL(__ap_pt) - _C_LABEL(trampoline), %eax
	movl	%eax, %cr3

	/* Step 3: Enable Long Mode in EFER MSR */
	movl	$0xC0000080, %ecx	/* MSR EFER */
	rdmsr
	orl	$X86_64_EFER_LME, %eax	/* EFER.LME = 1 */
	wrmsr

	/* Step 4: Enable paging (this transitions to long mode).
	 * Since CR4.PAE is set and EFER.LME is set, setting CR0.PG
	 * puts the CPU in long mode (initially in compatibility mode). */
	movl	%cr0, %ecx
	orl	$X86_64_CR0_PG, %ecx
	movl	%ecx, %cr0

	/* Enable PGE after paging is enabled */
	movl	%cr4, %eax
	orl	$X86_64_CR4_PGE, %eax
	movl	%eax, %cr4

	/* Step 5: Far jump to 64-bit code segment.
	 * This transitions from compatibility mode to 64-bit long mode.
	 * The target trampoline_lm must be in 64-bit code.
	 * We use a 32-bit ljmp with a 6-byte descriptor:
	 *   2 bytes: offset low (16 bits)
	 *   4 bytes: CS selector + offset high (actually the CPU expects
	 *   a specific format in 32-bit mode for ljmp).
	 * Actually, in 32-bit mode, ljmp uses:
	 *   ljmp $CS_SEL, $offset32
	 * But after enabling paging with EFER.LME=1, the CPU is in
	 * compatibility mode, and ljmp to a 64-bit CS puts it into
	 * 64-bit mode. The offset is a 32-bit value (sign-extended?).
	 *
	 * On x86_64 hardware, when EFER.LME=1 and CR0.PG=1, the CPU
	 * operates in compatibility mode (32-bit instructions, 32-bit
	 * addressing) until a far jump to a 64-bit code segment is
	 * executed, after which it operates in 64-bit mode.
	 *
	 * Use a 32-bit offset for the far jump (the kernel is loaded
	 * below 4GB in physical memory, so the trampoline is at a
	 * low physical address). */
	.byte	0xea			/* far jump opcode */
	.long	trampoline_lm		/* 32-bit offset (sign-extended) */
	.word	TRAMP_CS64_SEL		/* 64-bit code segment selector */

.code64
trampoline_lm:
	/* Now in 64-bit long mode */
	/* Set up data segments (flat model) */
	mov	$TRAMP_DS_SEL, %ax
	mov	%ax, %ds
	mov	%ax, %es
	mov	%ax, %ss

	/* Get our CPU id from the BSP-set variable */
	movq	_C_LABEL(__ap_id) - _C_LABEL(trampoline), %rdi

	/* Use a temporary stack for the call to startup_ap_64.
	 * startup_ap_64 will call smp_ap_boot(), which calls
	 * switch_k_stack() to set up the proper per-CPU stack.
	 * The per-CPU stack (with cpu id at the top) is set up
	 * by tss_init() in protect.c during BSP initialization. */
	movabs	$k_boot_stktop, %rsp

	/* Call the 64-bit initialization code in mpx.S */
	movabs	$startup_ap_64, %rax
	jmp	*%rax

/* Data area — these must be at fixed offsets from trampoline
 * because the BSP accesses them relative to trampoline_base.
 * On x86_64, __ap_pt is 64-bit (8 bytes) to support > 4GB page tables. */
.balign 8
LABEL(__ap_id)
.quad 0			/* AP CPU id (set by BSP before SIPI) */
LABEL(__ap_pt)
.quad 0			/* Physical address of PML4 table (64-bit) */
LABEL(__ap_gdt)
.short 0		/* GDT limit (2 bytes) */
.quad 0			/* GDT base (8 bytes) — 10-byte pseudo-descriptor */
LABEL(__ap_idt)
.short 0		/* IDT limit (2 bytes) */
.quad 0			/* IDT base (8 bytes) — 10-byte pseudo-descriptor */

/* GDT table for AP boot.
 * Must be self-contained in the trampoline area.
 * Entries:
 *   0: null descriptor
 *   1: 32-bit code segment (CS32): ring 0, 32-bit, D=1
 *   2: 64-bit code segment (CS64): ring 0, 64-bit, L=1, D=0
 *   3: data segment: ring 0, writable
 */
.balign 8
LABEL(__ap_gdt_tab)
	.quad 0x0000000000000000	/* null */
	/* 32-bit code: base=0, limit=4G, ring 0, code, D=1 */
	.quad 0x00CF9A000000FFFF
	/* 64-bit code: base=0, limit=0, ring 0, code, L=1, D=0 */
	.quad 0x00209A0000000000
	/* data: base=0, limit=4G, ring 0, writable */
	.quad 0x00CF92000000FFFF

/* IDT table for AP boot (minimal — will be reloaded later) */
.balign 8
LABEL(__ap_idt_tab)
	.space IDT_SIZE * 16		/* 256 entries × 16 bytes (64-bit IDT) */

LABEL(__trampoline_end)
