/* x86_64 kernel entry point.
 *
 * Boot sequence:
 * 1. Multiboot-compliant bootloader (GRUB) loads kernel at _kern_phys_base
 * 2. We enter in 32-bit protected mode (standard multiboot)
 * 3. We immediately set up the page tables for long mode:
 *    - Identity map low 4MB (so we can execute while transitioning)
 *    - Map kernel to high canonical address (0xFFFF8000F0400000)
 * 4. Enable PAE (required for long mode), then EFER.LME (long mode enable)
 * 5. Load 64-bit GDT with a 64-bit code segment
 * 6. Set CR0.PG to enable paging (this enters long mode)
 * 7. Far jump to 64-bit code segment
 * 8. In 64-bit mode: set up stack, jump to pre_init, then kmain
 */

#include "kernel/kernel.h"

#include <machine/vm.h>
#include "kernel/kernel.h"
#include <minix/config.h>
#include <minix/const.h>
#include <minix/com.h>
#include <machine/asm.h>
#include <machine/interrupt.h>
#include <machine/multiboot.h>
#include "archconst.h"
#include "sconst.h"

#ifdef CONFIG_SMP
#include "kernel/smp.h"
#endif

IMPORT(pre_init)
IMPORT(kmain)
IMPORT(switch_to_user)

/* Page table structures for long mode transition.
 * We need:
 * - PML4 table (1 entry: maps high kernel + identity)
 * - PDP table (2 entries: one for identity, one for kernel)
 * - PD table (2 entries: identity + kernel, 2MB pages)
 *
 * Using 2MB large pages for simplicity during boot.
 */
.section .unpaged.data, "aw"
.balign 4096
pml4_table:
	.space 4096
pdp_table:
	.space 4096
pd_table_low:
	.space 4096
pd_table_high:
	.space 4096

/* Temporary GDT for long mode transition. */
.balign 16
gdt64:
	/* Null descriptor */
	.quad 0
	/* Kernel code: 64-bit, ring 0, present, executable, long mode */
	.word 0			/* limit 15:0 */
	.word 0			/* base 15:0 */
	.byte 0			/* base 23:16 */
	.byte 0x98		/* present, ring 0, code, non-conforming */
	.byte 0x20		/* long mode (L=1), db=0 for 64-bit */
	.byte 0			/* base 31:24 */
	/* Kernel data: ring 0, present, writable */
	.word 0
	.word 0
	.byte 0
	.byte 0x92		/* present, ring 0, data, writable */
	.byte 0
	.byte 0
	/* User code: 64-bit, ring 3 */
	.word 0
	.word 0
	.byte 0
	.byte 0xfa		/* present, ring 3, code */
	.byte 0x20		/* long mode */
	.byte 0
	/* User data: ring 3, present, writable */
	.word 0
	.word 0
	.byte 0
	.byte 0xf2		/* present, ring 3, data, writable */
	.byte 0
	.byte 0

gdt64_ptr:
	.word (5 * 8) - 1	/* limit */
	.quad gdt64		/* base (64-bit address) */

.text
.code32

ENTRY(MINIX)
	jmp multiboot_init

/* Multiboot header. */
.balign 8

#define MULTIBOOT_FLAGS (MULTIBOOT_HEADER_WANT_MEMORY | MULTIBOOT_HEADER_MODS_ALIGNED)

multiboot_magic:
	.long MULTIBOOT_HEADER_MAGIC
multiboot_flags:
	.long MULTIBOOT_FLAGS
multiboot_checksum:
	.long -(MULTIBOOT_HEADER_MAGIC + MULTIBOOT_FLAGS)
	.long 0
	.long 0
	.long 0
	.long 0
	.long 0
multiboot_mode_type:
	.long MULTIBOOT_VIDEO_MODE_EGA
multiboot_width:
	.long MULTIBOOT_CONSOLE_COLS
multiboot_height:
	.long MULTIBOOT_CONSOLE_LINES
multiboot_depth:
	.long 0

multiboot_init:
	/* We are in 32-bit protected mode.
	 * Step 1: Identity map the first 4MB (kernel loaded here)
	 *         and map kernel to high address.
	 */
	
	/* Set up PML4 table:
	 * Entry 0: identity map (0 -> PDP table)
	 * Entry 256: kernel map at 0xFFFF800000000000 (entry 256 = 0xFFFF800000000000 >> 39)
	 *   Actually, kernel virtual base is 0xFFFF8000F0400000
	 *   PML4 index = (0xFFFF8000F0400000 >> 39) & 0x1FF = 0x100 = 256
	 *   PDP table is shared with identity mapping initially.
	 */
	 
	/* PML4[0] = PDP table (identity) */
	movl	$pdp_table, %eax
	orl	$0x03, %eax		/* present + writable */
	movl	%eax, pml4_table
	
	/* PML4[256] = same PDP table (kernel mapping) */
	movl	%eax, pml4_table + (256 * 8)
	
	/* PML4[511] = PML4 table itself (recursive mapping for VM) */
	movl	$pml4_table, %eax
	orl	$0x03, %eax
	movl	%eax, pml4_table + (511 * 8)
	
	/* Set up PDP table:
	 * Entry 0: maps 0-1GB -> PD table low
	 * Entry 0x100 (actually...)
	 *   Kernel is at 0xFFFF8000F0400000
	 *   PDP index = (0xFFFF8000F0400000 >> 30) & 0x1FF = 0x3C0
	 *   But for simplicity, identity map first 2MB and kernel at same physical
	 */
	 
	/* PDP[0] = PD table low (identity: 0-1GB) */
	movl	$pd_table_low, %eax
	orl	$0x03, %eax
	movl	%eax, pdp_table
	
	/* PDP[0x1C0] = PD table high (kernel: 0xFFFF8000C0000000+)
	 * Offset within PDP for 0xFFFF8000C0000000 = (0xFFFF8000C0000000 >> 30) & 0x1FF
	 * For kernel at 0xFFFF8000F0400000: PDP index = 0x1C0 ( ((0xFFFF8000F0400000 >> 30) & 0x1FF) )
	 * Actually for 0xFFFF8000F0400000:
	 *   PML4=256, PDP=0x1C0, PD=2, PT=0x40
	 * But for simplicity, we identity-map where we are.
	 */
	/* Actually, let's keep it simple: identity map bottom 4MB */
	
	/* PD table low[0] = 0-2MB (2MB page) */
	movl	$0x00, %eax
	orl	$0x83, %eax		/* present + writable + huge page (2MB) */
	movl	%eax, pd_table_low
	movl	$0x00, pd_table_low + 4
	
	/* PD table low[1] = 2-4MB (2MB page) */
	movl	$0x200000, %eax
	orl	$0x83, %eax
	movl	%eax, pd_table_low + 16
	movl	$0x00, pd_table_low + 20

	/* Step 2: Load PML4 base into CR3 */
	movl	$pml4_table, %eax
	movl	%eax, %cr3
	
	/* Step 3: Enable PAE in CR4 */
	movl	%cr4, %eax
	orl	$0x20, %eax		/* CR4.PAE = bit 5 */
	movl	%eax, %cr4
	
	/* Step 4: Enable long mode in EFER */
	movl	$0xC0000080, %ecx	/* MSR EFER */
	rdmsr
	orl	$0x100, %eax		/* EFER.LME = bit 8 */
	wrmsr
	
	/* Step 5: Enable paging (this enters long mode) */
	movl	%cr0, %eax
	orl	$0x80000000, %eax	/* CR0.PG = bit 31 */
	andl	$0x9FFFFFFF, %eax	/* clear EM, MP for FPU */
	movl	%eax, %cr0
	
	/* Step 6: Far jump into 64-bit mode.
	 * This sets CS to gdt64+8 (kernel code segment with L=1).
	 */
	ljmp	$0x08, $long_mode_entry

.code64
long_mode_entry:
	/* Now in 64-bit long mode. */
	
	/* Load 64-bit GDT */
	lgdt	gdt64_ptr
	
	/* Set up kernel data segments */
	mov	$0x10, %ax		/* kernel data selector */
	mov	%ax, %ds
	mov	%ax, %es
	mov	%ax, %ss
	xor	%ax, %ax
	mov	%ax, %fs
	mov	%ax, %gs
	
	/* Set up stack (physical address, will map high later) */
	movabs	$load_stack_start, %rsp
	mov	$0, %rbp
	push	$0
	popf
	
	/* Initialize KERNEL_GS_BASE for SWAPGS-based syscall entry.
	 * Use the identity-mapped load_stack area for per-CPU data.
	 * Per-CPU data layout:
	 *   [offset 0]: kernel stack pointer (RSP0)
	 *
	 * The per-CPU area is at the TOP of load_stack, which is
	 * identity-mapped and accessible during early boot.
	 *
	 * We set only IA32_KERNEL_GS_BASE (0xC0000102).
	 * IA32_GS_BASE (0xC0000101) stays at 0 (user mode value).
	 * SWAPGS exchanges GS.base with KERNEL_GS_BASE:
	 *   Entry: GS.base=0 ↔ KERNEL_GS_BASE=per-CPU
	 *   After: GS.base=per-CPU (kernel), KERNEL_GS_BASE=0
	 */
	lea	load_stack_start(%rip), %rcx	/* identity-mapped address */
	subq	$64, %rcx	/* per-CPU data at top-64 */
	/* Store kernel stack pointer (the top of per-CPU area) */
	movq	%rcx, (%rcx)
	/* Split into EDX:EAX for wrmsr */
	movq	%rcx, %rax
	shrq	$32, %rcx
	movq	%rcx, %rdx
	mov	$0xC0000102, %ecx	/* MSR IA32_KERNEL_GS_BASE */
	wrmsr
	
	/* IA32_GS_BASE (0xC0000101) — NOT set, stays at 0 for user mode */
	
	/* Push multiboot info for pre_init */
	mov	%rbx, %rdi		/* multiboot info struct */
	mov	%rax, %rsi		/* multiboot magic (0x2BADB002) */
	
	call	pre_init
	
	/* pre_init returns the boot info pointer in %rax.
	 * Switch to high-mapped stack and jump to kmain.
	 */
	movabs	$k_initial_stktop, %rsp
	push	$0
	mov	%rax, %rdi
	call	kmain
	
	/* Not reached */
hang:
	jmp hang

.data
load_stack:
	.space 4096
load_stack_start:

/* High-mapped kernel stack (in BSS, accessed via high mapping). */
.bss
k_initial_stack:
.space	K_STACK_SIZE
LABEL(__k_unpaged_k_initial_stktop)
