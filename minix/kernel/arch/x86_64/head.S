/* Dual-boot kernel entry point for x86_64.
 *
 * Supports two bootloaders:
 *
 * A) GRUB / Multiboot (32-bit protected mode entry):
 *    - Standard Multiboot v1 protocol
 *    - Enters in 32-bit protected mode at multiboot_init
 *    - Sets up page tables, transitions to long mode
 *    - Calls pre_init(mbi, magic) with multiboot info
 *
 * B) Limine (64-bit long mode entry):
 *    - ELF entry point starts in 64-bit long mode (set up by Limine)
 *    - Limine protocol provides memory map, modules, framebuffer via
 *      request/response structures placed in .limine_requests section
 *    - Calls limine_pre_init() which reads the responses
 *
 * Both paths converge: pg_clear() → pg_identity() → kmain()
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

#ifdef USE_LIMINE
IMPORT(limine_pre_init)
#endif

/* =========================================================================
 * Limine Entry Point (64-bit long mode)
 *
 * Limine loads the kernel as ELF64 and jumps to the ELF entry point
 * in 64-bit long mode. No 32→64 bit transition is needed.
 *
 * At entry:
 *   - CPU is in 64-bit long mode (CS.L=1, paging enabled)
 *   - Stack is undefined
 *   - All registers are undefined
 *
 * We call limine_pre_init() which reads responses from the
 * .limine_requests section and fills the kinfo struct.
 *
 * NOTE: This entry point serves dual purpose:
 *   - Limine enters here directly in 64-bit mode
 *   - The multiboot header's HAS_ADDR+mh_entry_addr directs GRUB
 *     to multiboot_init (32-bit) instead of this ELF entry
 * =========================================================================
 */
.section .unpaged.text, "ax"
.code64

ENTRY(MINIX)
#ifdef USE_LIMINE
	/* Set up stack (identity-mapped during early boot) */
	movabs	$load_stack_start, %rsp
	mov	$0, %rbp
	push	$0
	popf

	/* Set up kernel segments */
	mov	$0x10, %ax		/* kernel data selector */
	mov	%ax, %ds
	mov	%ax, %es
	mov	%ax, %ss
	xor	%ax, %ax
	mov	%ax, %fs
	mov	%ax, %gs

	/* Initialize KERNEL_GS_BASE for SWAPGS-based syscall entry */
	lea	load_stack_start(%rip), %rcx
	subq	$64, %rcx
	movq	%rcx, (%rcx)
	movq	%rcx, %rax
	shrq	$32, %rcx
	movq	%rcx, %rdx
	mov	$0xC0000102, %ecx	/* MSR IA32_KERNEL_GS_BASE */
	wrmsr

	/* Call limine_pre_init() — reads Limine responses, sets up pagetables */
	call	limine_pre_init

	/* Switch to high-mapped stack and jump to kmain */
	movabs	$k_initial_stktop, %rsp
	push	$0
	mov	%rax, %rdi
	call	kmain
#else
	/* USE_LIMINE not set — this entry point should never be reached
	 * because the multiboot header's HAS_ADDR+mh_entry_addr directs
	 * GRUB to multiboot_init (32-bit entry) directly. If we somehow
	 * end up here in 64-bit mode, we cannot simply jmp to 32-bit
	 * code — halt instead.
	 */
	hlt
	jmp	.-2
#endif

	/* Not reached */
hang_limine:
	jmp	hang_limine

/* Page table structures for long mode transition (multiboot path).
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
	.word 0
	.word 0
	.byte 0
	.byte 0x98		/* present, ring 0, code, non-conforming */
	.byte 0x20		/* long mode (L=1), db=0 for 64-bit */
	.byte 0
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

/* =========================================================================
 * Multiboot entry (32-bit protected mode)
 *
 * GRUB uses the multiboot header's mh_entry_addr field (set to
 * multiboot_init) to enter in 32-bit protected mode, NOT the ELF
 * entry point (which is MINIX above, for Limine).
 * =========================================================================
 */
.section .unpaged.text
.code32

/* Multiboot header with HAS_ADDR flag so mh_entry_addr is honoured.
 * This allows Limine to use the ELF entry (64-bit) while GRUB uses
 * the multiboot entry (32-bit at multiboot_init).
 */
.balign 8

#define MULTIBOOT_FLAGS (MULTIBOOT_HEADER_WANT_MEMORY | \
			 MULTIBOOT_HEADER_MODS_ALIGNED | \
			 MULTIBOOT_HEADER_HAS_ADDR)

multiboot_magic:
	.long MULTIBOOT_HEADER_MAGIC
multiboot_flags:
	.long MULTIBOOT_FLAGS
multiboot_checksum:
	.long -(MULTIBOOT_HEADER_MAGIC + MULTIBOOT_FLAGS)
multiboot_header_addr:
	.long multiboot_magic		/* physical addr of this header */
multiboot_load_addr:
	.long _kern_phys_base		/* physical load address */
multiboot_load_end_addr:
	.long _kern_end		/* load end */
multiboot_bss_end_addr:
	.long _kern_end		/* BSS end */
multiboot_entry_addr:
	.long multiboot_init		/* GRUB jumps here (32-bit!) */
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
