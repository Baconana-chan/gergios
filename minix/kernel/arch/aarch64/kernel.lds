/* ============================================================
 * kernel.lds — ARM64 Kernel Linker Script
 *
 * ARM64 kernel memory layout:
 *
 *   Physical load address:  PHYS_BASE (0x00080000 for UEFI)
 *   Kernel virtual base:    KERNEL_VBASE (0xFFFF800000000000)
 *
 *   Sections:
 *     .unpaged (identity-mapped): stays at PHYS_BASE
 *       - head.o (boot entry, MMU init)
 *       - pre_init.o (early boot info parsing)
 *
 *     .text, .rodata, .data, .bss: linked at KERNEL_VBASE + offset
 *       - These are accessed via the high kernel mapping
 *
 * Reference: ARMv8-A Architecture Reference Manual
 * ============================================================ */

OUTPUT_ARCH(aarch64)
ENTRY(_start)

/* =========================================================================
 * Memory layout constants
 * =========================================================================
 * PHYS_BASE:     Physical address where the kernel is loaded
 *                - For UEFI/Limine: 0x00080000 (512KB, typical)
 *                - For QEMU -kernel: 0x40000000 (1GB, QEMU virt default)
 * KERNEL_VBASE:  Virtual address for kernel high mapping
 * ========================================================================= */

PHYS_BASE = 0x00080000;
KERNEL_VBASE = 0xFFFF800000000000;

/* =========================================================================
 * Sections
 * ========================================================================= */

SECTIONS
{
    /* ----------------------------------------------------------
     * Unpaged section: identity-mapped at PHYS_BASE
     * This section must be accessible before MMU is enabled
     * or while the MMU uses an identity map.
     * ---------------------------------------------------------- */
    . = PHYS_BASE;

    .unpaged : ALIGN(4096) {
        _unpaged_start = .;
        *(.unpaged.text)
        *(.unpaged.text.*)
        *(.unpaged.data)
        *(.unpaged.data.*)
        *(.unpaged.rodata)
        *(.unpaged.rodata.*)
        *(.unpaged.bss)
        _unpaged_end = .;
    }

    /* ----------------------------------------------------------
     * High-mapped sections: linked at KERNEL_VBASE + offset
     * The MMU maps these at their virtual addresses.
     * ---------------------------------------------------------- */

    /* Text section */
    . = KERNEL_VBASE + PHYS_BASE + SIZEOF(.unpaged);

    .text : AT(ADDR(.unpaged) + SIZEOF(.unpaged)) {
        _text_start = .;
        *(.text)
        *(.text.*)
        *(.limine_requests)  /* Limine protocol request section */
        . = ALIGN(4096);
        _text_end = .;
    }

    /* Read-only data */
    .rodata : AT(LOADADDR(.text) + SIZEOF(.text)) {
        _rodata_start = .;
        *(.rodata)
        *(.rodata.*)
        . = ALIGN(4096);
        _rodata_end = .;
    }

    /* Data section */
    .data : AT(LOADADDR(.rodata) + SIZEOF(.rodata)) {
        _data_start = .;
        *(.data)
        *(.data.*)
        . = ALIGN(4096);
        _data_end = .;
    }

    /* BSS section (zero-initialized) */
    .bss : AT(LOADADDR(.data) + SIZEOF(.data)) {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss)
        *(.bss.*)
        *(COMMON)
        . = ALIGN(4096);
        __bss_end = .;
        _bss_end = .;
    }

    /* ----------------------------------------------------------
     * Physical-address BSS symbols (for clearing via identity map)
     *
     * __bss_start / __bss_end are at VMA = KERNEL_VBASE + offset.
     * But after MMU is enabled with identity-mapping only, we
     * cannot access KERNEL_VBASE addresses. These physical symbols
     * use LOADADDR which resolves to the physical address (within
     * the identity-mapped 0–2 GB range).
     * ---------------------------------------------------------- */
    __bss_start_phys = LOADADDR(.bss);
    __bss_end_phys = LOADADDR(.bss) + SIZEOF(.bss);

    /* ----------------------------------------------------------
     * Kernel symbols
     * ---------------------------------------------------------- */
    _kern_phys_base = PHYS_BASE;
    _kern_vbase = KERNEL_VBASE;
    _kern_size = SIZEOF(.unpaged) + SIZEOF(.text) + SIZEOF(.rodata) +
                 SIZEOF(.data) + SIZEOF(.bss);
    _end = .;

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
    }
}
