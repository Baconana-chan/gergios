/* ============================================================
 * kernel.lds — ARM64 Kernel Linker Script
 *
 * ARM64 kernel memory layout:
 *
 *   Physical base:    PHYS_BASE (0x00080000 for UEFI / -kernel)
 *   Virtual base:     KERNEL_VBASE (0xFFFF800000000000)
 *   Offset:           KERNEL_OFFSET (virtual - physical)
 *
 *   Layout:
 *     Low memory (identity-mapped, 0x00080000..):
 *       .unpaged_text  — boot code (head.S, early startup)
 *       .unpaged_data  — boot data + rodata (pg tables, stacks, strings)
 *       .unpaged_bss   — boot BSS
 *
 *     High memory (KERNEL_VBASE + offset..):
 *       .usermapped_glo — kernel globals accessible from userland
 *       .usermapped     — user-mappable kernel data
 *       .text           — main kernel code
 *       .data           — kernel data + read-only data
 *       .bss            — zero-initialized data
 *
 *   After MMU is enabled with identity map, the kernel:
 *     1. Sets up the high mapping (TTBR1_EL1)
 *     2. Jumps to the high-mapped address (KERNEL_VBASE + offset)
 *     3. Removes the identity mapping for low addresses
 *
 * Reference: ARMv8-A Architecture Reference Manual (DDI 0487)
 * ============================================================ */

OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* =========================================================================
 * Memory layout constants
 * =========================================================================
 *
 * PHYS_BASE:     Physical load address of the kernel.
 *                - QEMU -kernel:  0x00080000 (standard ARM64 kernel addr)
 *                - U-Boot:        configured per platform
 *                - UEFI/Limine:   determined by bootloader
 *
 * KERNEL_VBASE: Virtual address base for the kernel high mapping.
 *               ARM64 uses TTBR1_EL1 for the kernel half of the address
 *               space (0xFFFF800000000000 - 0xFFFFFFFFFFFFFFFF with 48-bit VA).
 *
 * KERNEL_OFFSET: Difference between virtual and physical base.
 *                Used to convert between VMA and LMA.
 * ========================================================================= */

_kern_phys_base = 0x00080000;
_kern_vir_base  = 0xFFFF800000000000;
_kern_offset = (_kern_vir_base - _kern_phys_base);

__k_unpaged__kern_offset = _kern_offset;
__k_unpaged__kern_vir_base = _kern_vir_base;
__k_unpaged__kern_phys_base = _kern_phys_base;

/* Compatibility aliases for C code.
 * pre_init.c references _kern_vbase (without _vir_), and _kern_phys_base.
 * These aliases ensure the symbols exist regardless of naming convention. */
_kern_vbase = _kern_vir_base;
_kern_phys = _kern_phys_base;

/* =========================================================================
 * Sections
 * ========================================================================= */

SECTIONS
{
    /* ----------------------------------------------------------
     * Unpaged section: identity-mapped at PHYS_BASE
     *
     * These sections must be accessible before the MMU is enabled
     * (when the CPU uses physical addresses directly) and during
     * the initial identity-mapped phase after MMU enablement.
     *
     * The linker places these at their LMA = VMA = PHYS_BASE.
     * ---------------------------------------------------------- */

    . = _kern_phys_base;
    __k_unpaged__kern_unpaged_start = .;

    /* Unpaged text: boot code, exception vectors, context switching */
    .unpaged_text ALIGN(4096) : {
        *(.unpaged.text)
        *(.unpaged.text.*)
    }

    /* Unpaged data: page tables, boot stacks, diagnostic strings,
     * and any other read-only or read-write data needed during boot */
    .unpaged_data ALIGN(4096) : {
        *(.unpaged.data)
        *(.unpaged.data.*)
        *(.unpaged.rodata)
        *(.unpaged.rodata.*)
    }
    __k_unpaged__kern_unpaged_edata = .;

    /* Unpaged BSS: zero-initialized data for boot phase */
    .unpaged_bss  ALIGN(4096) : {
        boot_stack_end = .;
        *(.unpaged.bss)
        *(.unpaged.bss.*)
        pagetable_l1 = .;
        . += 4096;   /* Reserve 1 page (4KB) for L1 page table */
        pagetable_l2 = .;
        . += 4096;   /* Reserve 1 page (4KB) for L2 page table */
    }
    __k_unpaged__kern_unpaged_end = .;

    /* ----------------------------------------------------------
     * Limine boot protocol requests
     *
     * If Limine boot protocol support is added, this section
     * contains the request structures that the bootloader scans.
     * Must be accessible at boot time (in low memory).
     * ---------------------------------------------------------- */

    .limine_requests ALIGN(64) : {
        *(.limine_requests)
    }

    /* ----------------------------------------------------------
     * High-mapped sections: linked at KERNEL_VBASE + offset
     *
     * After MMU enable, we jump to the high virtual address.
     * These sections are placed at VMA = KERNEL_VBASE + offset,
     * but their LMA (physical location) is at PHYS_BASE + offset,
     * which is within the identity-mapped range.
     *
     * The formula: VMA = KERNEL_VBASE + LMA_offset
     *             LMA = PHYS_BASE + LMA_offset
     *             LMA = VMA - KERNEL_OFFSET
     *             => AT(ADDR(.section) - KERNEL_OFFSET)
     * ---------------------------------------------------------- */

    . += _kern_offset;
    . = ALIGN(4096);

    /* User-mappable globals: accessed from userland via single page */
    usermapped_start = .;
    .usermapped_glo : AT(ADDR(.usermapped_glo) - _kern_offset) {
        *(.usermapped_glo)
    }
    . = ALIGN(4096);
    usermapped_nonglo_start = .;
    .usermapped : AT(ADDR(.usermapped) - _kern_offset) {
        *(.usermapped)
    }
    . = ALIGN(4096);
    usermapped_end = .;

    /* Main kernel text */
    .text : AT(ADDR(.text) - _kern_offset) {
        *(.text*)
    }

    /* Read-only data */
    .rodata ALIGN(4096) : AT(ADDR(.rodata) - _kern_offset) {
        *(.rodata)
        *(.rodata.*)
    }

    /* Data section (includes .rodata for compatibility) */
    .data ALIGN(4096) : AT(ADDR(.data) - _kern_offset) {
        *(.data)
        *(.data.*)
    }

    /* BSS section (zero-initialized) */
    .bss ALIGN(4096) : AT(ADDR(.bss) - _kern_offset) {
        __bss_start = .;
        *(.bss)
        *(.bss.*)
        *(COMMON)
        . = ALIGN(4096);
        __bss_end = .;
        __bss_end__ = .;
        _bss_end = .;

        /* Kernel size: total size from virtual base to end of BSS */
        __k_unpaged__kern_size = . - _kern_vir_base;
        _kern_size = __k_unpaged__kern_size;
    }

    _end = .;
    __k_unpaged__end = . - _kern_offset;

    /* ----------------------------------------------------------
     * Physical-address BSS markers (for identity-mapped clearing)
     *
     * After MMU is enabled with only the identity map active,
     * we cannot access __bss_start/__bss_end (which are at
     * KERNEL_VBASE + offset virtual addresses). These symbols
     * use LOADADDR which resolves to the physical address
     * (within the identity-mapped range).
     * ---------------------------------------------------------- */
    __bss_start_phys = LOADADDR(.bss);
    __bss_end_phys = LOADADDR(.bss) + SIZEOF(.bss);

    /* ----------------------------------------------------------
     * Discard unnecessary sections
     * ---------------------------------------------------------- */
    /DISCARD/ : {
        *(.comment)
        *(.note.*)
        *(.ARM.attributes)
        *(.debug)
        *(.debug.*)
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.gnu_debuglink)
        *(.gnu.lto_*)
    }
}
