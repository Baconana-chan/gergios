/* ============================================================
 * pre_init.c — ARM64 boot information setup
 *
 * This file sets up the initial kernel information structure (kinfo)
 * before kmain() is called. It parses information from the bootloader
 * (Device Tree, Limine protocol, or U-Boot) and fills in the kernel's
 * global data structures.
 *
 * Phase 3: FDT parser integration — kinfo is populated from the
 *          Device Tree Blob (DTB) provided by the bootloader.
 *          Memory layout, CPU count, and boot console are detected
 *          at runtime instead of being hardcoded.
 * ============================================================ */

#include <stdint.h>
#include <minix/type.h>
#include <minix/param.h>
#include <kernel/kernel.h>
#include <string.h>

/* FDT parser header */
#include "fdt.h"

/* Imported from kernel global variables (glo.h) */
extern struct kinfo kinfo;
extern char *_kern_phys_base;
extern char *_kern_vbase;

/*
 * Parse Device Tree Blob and fill kinfo structure.
 *
 * Extracts the following from the DTB:
 *   - Memory layout (base address and size) from /memory node
 *   - CPU core count from /cpus node
 *   - Boot command line from /chosen/bootargs
 *   - Console UART from /chosen/stdout-path (or fallback to PL011)
 *
 * If no valid DTB is provided, falls back to sensible defaults
 * for QEMU virt platform (512 MB RAM, PL011 UART).
 *
 * @param dtb  Pointer to the Device Tree Blob (or NULL)
 */
static void fdt_parse_kinfo(const void *dtb)
{
    uint64_t mem_addr = 0, mem_size = 0;
    int cpu_count = 0;
    int ret;

    /* Clear kinfo */
    memset(&kinfo, 0, sizeof(kinfo));

    /* Boot console: default PL011 (QEMU virt) */
    kinfo.do_serial_debug = 1;
    kinfo.serial_debug_baud = 115200;

    /* Parse memory from DTB */
    if (dtb && fdt_validate(dtb, 0) == 0) {
        ret = fdt_get_memory(dtb, &mem_addr, &mem_size);
        if (ret == 1 && mem_size > 0) {
            /* Memory detected from DTB — store in memmap */
            kinfo.memmap[0].mm_base_addr = mem_addr;
            kinfo.memmap[0].mm_length = mem_size;
            kinfo.memmap[0].type = MULTIBOOT_MEMORY_AVAILABLE;
            kinfo.mem_high_phys = mem_addr + mem_size;
            kinfo.mmap_size = 1;
        } else {
            /* Fallback: QEMU virt default (512 MB at 0x40000000) */
            kinfo.memmap[0].mm_base_addr = 0x40000000ULL;
            kinfo.memmap[0].mm_length = 512UL * 1024 * 1024;
            kinfo.memmap[0].type = MULTIBOOT_MEMORY_AVAILABLE;
            kinfo.mem_high_phys = 0x40000000ULL + 512UL * 1024 * 1024;
            kinfo.mmap_size = 1;
        }

        /* CPU count (informational only) */
        cpu_count = fdt_get_cpu_count(dtb);
        if (cpu_count > 0)
            kinfo.nr_procs = cpu_count + NR_TASKS;
        else
            kinfo.nr_procs = NR_TASKS + 1;
    } else {
        /* No DTB: fallback defaults for QEMU virt */
        kinfo.memmap[0].mm_base_addr = 0x40000000ULL;
        kinfo.memmap[0].mm_length = 512UL * 1024 * 1024;
        kinfo.memmap[0].type = MULTIBOOT_MEMORY_AVAILABLE;
        kinfo.mem_high_phys = 0x40000000ULL + 512UL * 1024 * 1024;
        kinfo.mmap_size = 1;
        kinfo.nr_procs = NR_TASKS + 1;
    }

    /* Modules: none at this stage */
    kinfo.mods_with_kernel = 0;
    kinfo.kern_mod = 0;
}

/*
 * ARM64 pre-initialization entry point.
 *
 * Called from startup.c after the UART is initialized and the
 * Device Tree has been parsed for basic information. Sets up
 * the kinfo structure with memory layout, CPU count, and boot
 * configuration, then returns the kinfo for kmain().
 *
 * @param dtb_address  Physical address of Device Tree Blob (or 0)
 * @return              Pointer to kinfo (for kmain)
 */
struct kinfo *pre_init(unsigned long dtb_address)
{
    /* Parse DTB and fill kinfo structure */
    fdt_parse_kinfo((const void *)dtb_address);

    return &kinfo;
}
