/* ============================================================
 * pre_init.c — ARM64 boot information setup
 *
 * This file sets up the initial kernel information structure (kinfo)
 * before kmain() is called. It parses information from the bootloader
 * (Device Tree, Limine protocol, or U-Boot) and fills in the kernel's
 * global data structures.
 *
 * Phase 2: Minimal stub — returns a simple kinfo.
 * Phase 2+: Full implementation with Device Tree parsing,
 *           memory map setup, and module loading.
 * ============================================================ */

#include <minix/type.h>
#include <minix/param.h>
#include <kernel/kernel.h>
#include <string.h>

/* Imported from kernel global variables (glo.h) */
extern struct kinfo kinfo;
extern char *_kern_phys_base;
extern char *_kern_vbase;

/*
 * Minimal kinfo initialization for ARM64.
 *
 * In Phase 2, this just sets up the minimum needed for kmain()
 * to function. In later phases, this will:
 *   - Parse the Device Tree for memory layout
 *   - Count CPU cores
 *   - Set up boot module lists
 *   - Initialize GIC and timer
 */
static void setup_minimal_kinfo(void)
{
    /* Clear kinfo */
    memset(&kinfo, 0, sizeof(kinfo));

    /* Kernel base addresses */
    kinfo.kinfo_reloc = (vir_bytes)&_kern_phys_base;
    kinfo.kinfo_virt_base = (vir_bytes)&_kern_vbase;

    /* Architecture */
    kinfo.kinfo_arch = "aarch64";

    /* Boot console: PL011 UART on QEMU virt */
    kinfo.kinfo_console = "pl011";
    kinfo.kinfo_serial = 1;

    /* Memory: QEMU virt default 512MB
     * Phase 2+ will parse from Device Tree */
    kinfo.kinfo_mem_lower = 0;
    kinfo.kinfo_mem_upper = 512 * 1024 * 1024; /* 512MB */

    /* Modules: none in Phase 2
     * Phase 2+ will load boot modules */
    kinfo.kinfo_nr_modules = 0;
}

/*
 * ARM64 pre-initialization entry point.
 *
 * Called from head.S after MMU is enabled and stack is set up.
 * arm64_boot() in startup.c handles the initial output,
 * then this function sets up the kinfo structure and calls kmain().
 *
 * @param dtb_address  Address of Device Tree Blob (or 0)
 * @return              Pointer to kinfo (for kmain)
 */
struct kinfo *pre_init(unsigned long dtb_address)
{
    /* Set up minimal kernel info */
    setup_minimal_kinfo();

    /* Phase 2: return minimal kinfo
     * Phase 2+: parse DTB, set up modules, initialize GIC/timer */
    (void)dtb_address;  /* Unused in Phase 2 */

    return &kinfo;
}
