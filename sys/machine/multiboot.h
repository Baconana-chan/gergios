/*	$NetBSD$	*/
/* Multiboot header — stub for AArch64.
 * ARM64 does NOT use the Multiboot boot protocol (uses device tree / UEFI).
 * However, the MINIX kernel code (kmain in main.c) references multiboot types
 * unconditionally. We provide minimal type definitions for compilation.
 */

#ifndef _MACHINE_MULTIBOOT_H_
#define _MACHINE_MULTIBOOT_H_

#ifndef __ASSEMBLY__
#include <stdint.h>

/* Minimal multiboot types for kernel compilation (not used at runtime). */
#define MULTIBOOT_MAX_MODS          32
#define MULTIBOOT_PARAM_BUF_SIZE    1024
#define MULTIBOOT_INFO_MEM_MAP      0x40

typedef struct {
    uint32_t mod_start;
    uint32_t mod_end;
    uint32_t cmdline;
    uint32_t padding;
} multiboot_module_t;

typedef struct {
    uint32_t    mi_flags;
    uint32_t    mi_mem_lower;
    uint32_t    mi_mem_upper;
    uint32_t    mi_boot_device;
    uint32_t    mi_cmdline;
    uint32_t    mi_mods_count;
    uint32_t    mi_mods_addr;
    /* ... many more fields, but these are all we need for compilation */
    uint32_t    _pad[16];
} multiboot_info_t;

#define MULTIBOOT_MEMORY_AVAILABLE  1

typedef struct {
    uint32_t    size;
    uint64_t    mm_base_addr;
    uint64_t    mm_length;
    uint32_t    type;
} __attribute__((packed)) multiboot_memory_map_t;

#endif /* __ASSEMBLY__ */
#endif /* _MACHINE_MULTIBOOT_H_ */
