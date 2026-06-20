/* ============================================================
 * head.S — ARM64 kernel entry point
 *
 * Entry point for GergiOS on ARM64 (AArch64).
 *
 * Boot flow:
 *   Bootloader (Limine AAC64 / U-Boot / QEMU -kernel)
 *     └→ _start (EL2 or EL1)
 *         ├→ Drop to EL1 if entered at EL2/EL3
 *         ├→ Set up exception vectors (VBAR_EL1)
 *         ├→ Set up page tables (identity map)
 *         ├→ Configure MMU (TCR_EL1, MAIR_EL1, SCTLR_EL1)
 *         ├→ Enable MMU
 *         ├→ Set up kernel stack (SP_EL1)
 *         └→ Call arm64_boot() in startup.c
 *
 * ARM64 boot requirements:
 *   - Kernel loaded at PHYS_BASE (0x00080000 for UEFI, 0x40000000 for QEMU -kernel)
 *   - CPU in EL2 or EL1
 *   - MMU off (or identity mapped)
 *   - All exception levels use SP_ELx
 *
 * Register usage at entry:
 *   x0 = Device Tree Blob address (if entered via UEFI/Limine)
 *   x1 = 0 (or magic number)
 *   All other registers are undefined
 *
 * Reference: ARM Architecture Reference Manual ARMv8-A (DDI 0487)
 * ============================================================ */

#include <machine/asm.h>
#include <machine/vm.h>

/* =========================================================================
 * Constants
 * ========================================================================= */

/* Exception level masks for CurrentEL register */
#define CURRENT_EL_MASK         0x0000000C
#define CURRENT_EL_SHIFT        2
#define EL1_VALUE               (1 << CURRENT_EL_SHIFT)  /* 0x4 */
#define EL2_VALUE               (2 << CURRENT_EL_SHIFT)  /* 0x8 */
#define EL3_VALUE               (3 << CURRENT_EL_SHIFT)  /* 0xC */

/* System Register Encodings */
#define SCTLR_EL1_RES1         0x30C50838  /* Reserved 1 bits */
#define SCTLR_EL1_M            (1 << 0)    /* MMU Enable */
#define SCTLR_EL1_A            (1 << 1)    /* Alignment check */
#define SCTLR_EL1_C            (1 << 2)    /* Cache enable */
#define SCTLR_EL1_SA           (1 << 3)    /* Stack alignment */
#define SCTLR_EL1_I            (1 << 12)   /* Instruction cache */
#define SCTLR_EL1_WXN          (1 << 19)   /* Write permission implies XN */
#define SCTLR_EL1_EE           (1 << 25)   /* Endianness (0=little) */
#define SCTLR_EL1_EOS          (1 << 11)   /* Exclusives */

/* TCR (Translation Control Register) for 4KB pages, 48-bit VA, 48-bit PA */
#define TCR_TG0_4KB            (0 << 14)   /* 4KB granule for TTBR0 */
#define TCR_TG1_4KB            (2 << 30)   /* 4KB granule for TTBR1 */
#define TCR_SH0_IS             (3 << 12)   /* Inner shareable TTBR0 */
#define TCR_SH1_IS             (3 << 28)   /* Inner shareable TTBR1 */
#define TCR_ORGN0_WB           (1 << 10)   /* Outer write-back TTBR0 */
#define TCR_IRGN0_WB           (1 << 8)    /* Inner write-back TTBR0 */
#define TCR_ORGN1_WB           (1 << 26)   /* Outer write-back TTBR1 */
#define TCR_IRGN1_WB           (1 << 24)   /* Inner write-back TTBR1 */
#define TCR_T0SZ_48            (16)        /* 48-bit VA space (64-48=16) */
#define TCR_T1SZ_48            (16)        /* 48-bit VA space (64-48=16) */
#define TCR_PS_40BIT           (2 << 16)   /* 40-bit physical address */
#define TCR_PS_48BIT           (5 << 16)   /* 48-bit physical address */

#define TCR_FLAGS              (TCR_TG0_4KB | TCR_TG1_4KB |            \
                                TCR_SH0_IS | TCR_SH1_IS |              \
                                TCR_ORGN0_WB | TCR_IRGN0_WB |          \
                                TCR_ORGN1_WB | TCR_IRGN1_WB |          \
                                TCR_T0SZ_48 | TCR_T1SZ_48 |            \
                                TCR_PS_48BIT)

/* MAIR (Memory Attribute Indirection Register)
 * Attr0: Normal memory, Outer/Inner Write-Back, Non-Transient
 * Attr1: Device-nGnRE (non-gathering, non-reordering, early acknowledges)
 */
#define MAIR_ATTR0             (0xFF << 0)   /* Normal WB */
#define MAIR_ATTR1             (0x04 << 8)   /* Device-nGnRE */

#define MAIR_FLAGS             (MAIR_ATTR0 | MAIR_ATTR1)

/* Page table entry flags */
#define PTE_TYPE_MASK          (3 << 0)
#define PTE_TYPE_TABLE         (3 << 0)    /* Table descriptor */
#define PTE_TYPE_BLOCK         (1 << 0)    /* Block descriptor */
#define PTE_TYPE_PAGE          (3 << 0)    /* Page descriptor */
#define PTE_BLOCK_AF           (1 << 10)   /* Access Flag */
#define PTE_BLOCK_UXN          (1 << 54)   /* Unprivileged Execute Never */
#define PTE_BLOCK_PXN          (1 << 53)   /* Privileged Execute Never */
#define PTE_BLOCK_NG           (1 << 11)   /* Not Global */
#define PTE_BLOCK_SH_IS        (3 << 8)    /* Inner Shareable */
#define PTE_BLOCK_AP_RW_EL1    (0 << 6)    /* RW at EL1, none at EL0 */
#define PTE_BLOCK_ATTR_INDEX_0 (0 << 2)    /* Attr0: Normal WB */
#define PTE_BLOCK_ATTR_INDEX_1 (1 << 2)    /* Attr1: Device */

#define PTE_BLOCK_FLAGS        (PTE_TYPE_BLOCK | PTE_BLOCK_AF |        \
                                PTE_BLOCK_SH_IS |                      \
                                PTE_BLOCK_AP_RW_EL1 |                  \
                                PTE_BLOCK_ATTR_INDEX_0)

#define PTE_TABLE_FLAGS        (PTE_TYPE_TABLE)

/* =========================================================================
 * Entry point
 * ========================================================================= */

.section .unpaged.text, "ax"
.balign 0x800

ENTRY(_start)
    /* ----------------------------------------------------------
     * Save Device Tree Blob address (from bootloader, passed in x0)
     * x19 is callee-saved in ARM64 C ABI, safe to use here.
     * ---------------------------------------------------------- */
    mov     x19, x0             /* Save DTB address */

    /* ----------------------------------------------------------
     * Step 1: Determine current exception level and drop to EL1
     * ----------------------------------------------------------
     * We may enter at EL3 (QEMU -cpu max), EL2 (QEMU default),
     * or EL1. We need to be at EL1 for the kernel.
     */
    mrs     x0, CurrentEL
    and     x0, x0, #CURRENT_EL_MASK
    cmp     x0, #EL3_VALUE
    b.lo    1f

    /* We're at EL3. Set up SCR_EL3 and drop to EL2. */
    mov     x0, #0              /* SCR_EL3: no traps, non-secure */
    msr     scr_el3, x0
    mov     x0, #0x3C9          /* SPSR_EL3: EL2h, all interrupts masked */
    msr     spsr_el3, x0
    adr     x0, 1f              /* Return to 1f at EL2 */
    msr     elr_el3, x0
    eret                         /* Drop to EL2 */

1:  /* Now at EL2 or EL1 */
    mrs     x0, CurrentEL
    and     x0, x0, #CURRENT_EL_MASK
    cmp     x0, #EL2_VALUE
    b.lo    2f

    /* We're at EL2. Set up HCR_EL2 and drop to EL1. */
    mov     x0, #(1 << 31)      /* HCR_EL2.RW: AArch64 at EL1 */
    msr     hcr_el2, x0

    /* Generic Timer: allow EL1 access to CNTP */
    mrs     x0, cnthctl_el2
    orr     x0, x0, #3          /* EL0/EL1 access to physical timer */
    msr     cnthctl_el2, x0
    msr     cntvoff_el2, xzr    /* No virtual offset */

    /* Set SPSR_EL2 for return to EL1h */
    mov     x0, #0x3C5          /* SPSR_EL2: EL1h, all interrupts masked */
    msr     spsr_el2, x0
    adr     x0, 2f              /* Return to 2f at EL1 */
    msr     elr_el2, x0
    eret                         /* Drop to EL1 */

    /* ----------------------------------------------------------
     * Step 2: We're at EL1. Set up system registers.
     * ---------------------------------------------------------- */
2:
    /* Set up Translation Control Register */
    ldr     x0, =TCR_FLAGS
    msr     tcr_el1, x0

    /* Set up Memory Attribute Indirection Register */
    ldr     x0, =MAIR_FLAGS
    msr     mair_el1, x0

    /* Set up System Control Register:
     * Start with MMU off, but set recommended RES1 bits */
    ldr     x0, =SCTLR_EL1_RES1
    msr     sctlr_el1, x0

    /* ----------------------------------------------------------
     * Step 3: Set up identity page tables
     * ----------------------------------------------------------
     * Identity map the first 2GB of physical memory using block
     * entries (2MB each). This gives us access to:
     *   - Kernel code at PHYS_BASE
     *   - UART at 0x09000000
     *   - GIC at 0x08000000
     *   - RAM at 0x40000000
     *
     * Page table structure (4KB pages, 48-bit VA, 3 levels):
     *   Level 0: 512GB block (not used — 1 entry at level 1)
     *   Level 1: 1GB block (512 entries, 1GB each)
     *   Level 2: 2MB block (512 entries, 2MB each)
     *   Level 3: 4KB page (not used for identity map)
     *
     * For identity mapping:
     *   Level 1[0] = Level 2 table (covers 0x0000000000 - 0x4000000000)
     *   Level 2[0..1023] = 2MB block entries
     */

    /* Level 1 table: one entry pointing to Level 2 table */
    adr     x0, pagetable_l1
    adr     x1, pagetable_l2
    orr     x1, x1, #PTE_TABLE_FLAGS
    str     x1, [x0]

    /* Level 2 table: 512 entries of 2MB blocks */
    adr     x0, pagetable_l2
    mov     x1, #0              /* Physical address = 0 */
    mov     x2, #512            /* 512 entries */

3:  /* Fill level 2 entry */
    orr     x3, x1, #PTE_BLOCK_FLAGS
    str     x3, [x0], #8
    add     x1, x1, #0x200000   /* Next 2MB block */
    subs    x2, x2, #1
    b.ne    3b

    /* ----------------------------------------------------------
     * Step 4: Load page table base and enable MMU
     * ---------------------------------------------------------- */

    /* Set TTBR0_EL1 to Level 1 table (identity map) */
    adr     x0, pagetable_l1
    msr     ttbr0_el1, x0

    /* Set TTBR1_EL1 to same table (kernel high map — temporary) */
    msr     ttbr1_el1, x0

    /* Ensure all previous writes are visible */
    dsb     sy
    isb

    /* Enable MMU in SCTLR_EL1 */
    mrs     x0, sctlr_el1
    orr     x0, x0, #SCTLR_EL1_M    /* MMU enable */
    orr     x0, x0, #SCTLR_EL1_C    /* Cache enable */
    orr     x0, x0, #SCTLR_EL1_I    /* Instruction cache */
    msr     sctlr_el1, x0
    isb

    /* ----------------------------------------------------------
     * Step 5: Install exception vector table (VBAR_EL1)
     * ----------------------------------------------------------
     * Set VBAR_EL1 to the base of the vector table.
     * The table is in .unpaged.text, accessible via identity map.
     * We're running identity-mapped, so adr gives the correct address.
     */
    adr     x0, arm64_vector_table
    msr     vbar_el1, x0
    isb

    /* ----------------------------------------------------------
     * Step 6: Set up kernel stack
     * ---------------------------------------------------------- */

    /* Set SP_EL1 to the boot stack */
    adr     x0, boot_stack_end
    mov     sp, x0

    /* ----------------------------------------------------------
     * Step 7: Clear BSS (using physical addresses)
     * ----------------------------------------------------------
     * __bss_start_phys / __bss_end_phys are defined in kernel.lds
     * as the LOADADDR of .bss. These are physical addresses
     * within the identity-mapped 0–2 GB range.
     *
     * We use ldr x0,=symbol (literal pool) instead of adr because
     * adr has only ±1 MB range and the virtual-address BSS
     * symbols (__bss_start/__bss_end at KERNEL_VBASE + offset)
     * are not identity-mapped.
     * ---------------------------------------------------------- */
    ldr     x0, =__bss_start_phys
    ldr     x1, =__bss_end_phys
    sub     x1, x1, x0          /* Size in bytes */
    cbz     x1, 5f
    lsr     x2, x1, #3          /* Size in qwords */
    mov     x1, xzr
4:  str     x1, [x0], #8
    subs    x2, x2, #1
    b.ne    4b
5:

    /* ----------------------------------------------------------
     * Step 8: Call C code — arm64_boot(x0 = dtb_address)
     * ---------------------------------------------------------- */
    mov     x0, x19             /* Pass saved DTB address */
    mov     x29, #0             /* Clear frame pointer */
    bl      arm64_boot

    /* ----------------------------------------------------------
     * Should never reach here
     * ---------------------------------------------------------- */
hang:
    wfi
    b       hang

/* =========================================================================
 * Page tables (identity mapping, 3 levels)
 * ========================================================================= */

.section .unpaged.data, "aw"
.balign 4096

pagetable_l1:
    .space 4096                  /* Level 1: 512 × 8 bytes = 4KB */
pagetable_l2:
    .space 4096                  /* Level 2: 512 × 8 bytes = 4KB */

/* =========================================================================
 * Boot stack
 * ========================================================================= */

.section .unpaged.data, "aw"
.balign 16
boot_stack:
    .space 8192                  /* 8KB boot stack */
boot_stack_end:

/* =========================================================================
 * Note: _kern_phys_base and _kern_size are provided by kernel.lds
 * as linker symbols, NOT defined here. The linker script calculates
 * them based on the section layout (PHYS_BASE, KERNEL_VBASE, etc.).
 *
 * Do NOT define them in assembly — it would cause duplicate symbols.
 * ========================================================================= */

/* =========================================================================
 * Boot-time stack for C code (identity-mapped, in .unpaged.data)
 *
 * Defined here rather than in mpx.S so that head.S (which runs first)
 * has a clean bootstrap stack. mpx.S provides the main kernel stacks
 * (k_initial_stack, per-CPU stacks) which are accessed via the high
 * kernel mapping after the transition.
 *
 * Stack layout:
 *   boot_stack:   8192 bytes (8KB)
 *   boot_stack_end: top of stack (SP is set to point here)
 * ========================================================================= */

.section .unpaged.data, "aw"
.balign 16
boot_stack:
    .space 8192                  /* 8KB boot stack */
boot_stack_end:

/* =========================================================================
 * Page tables (identity mapping, 3 levels)
 *
 * These are placed in .unpaged.data so they're identity-mapped and
 * accessible before/after MMU enablement.
 *
 * Layout:
 *   pagetable_l1: Level 1 table (512 × 8 = 4096 bytes)
 *     Entry 0 → pagetable_l2 (covers 0x0000000000 - 0x4000000000)
 *   pagetable_l2: Level 2 table (512 × 8 = 4096 bytes)
 *     Entries 0..511: 2MB block entries covering 0 - 1GB
 * ========================================================================= */

.section .unpaged.data, "aw"
.balign 4096

pagetable_l1:
    .space 4096                  /* Level 1: 512 × 8 bytes = 4KB */
pagetable_l2:
    .space 4096                  /* Level 2: 512 × 8 bytes = 4KB */
