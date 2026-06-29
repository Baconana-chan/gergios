/*	$NetBSD$	*/
/* Multiboot header — multi-architecture self-contained.
 *
 * For x86 (x86_64, i386): assembly-visible constants + C structures for
 *   the Multiboot v1 boot protocol.
 *
 * For AArch64: minimal stub with just enough types for kernel compilation.
 */

#ifndef _MACHINE_MULTIBOOT_H_
#define _MACHINE_MULTIBOOT_H_

#if defined(__x86_64__) || defined(__i386__)

/* Multiboot header constants. */
#define MULTIBOOT_HEADER_MAGIC		0x1BADB002
#define MULTIBOOT_HEADER_MODS_ALIGNED	0x00000001
#define MULTIBOOT_HEADER_WANT_MEMORY	0x00000002
#define MULTIBOOT_HEADER_HAS_VBE	0x00000004
#define MULTIBOOT_HEADER_HAS_ADDR	0x00010000

/* Multiboot info flags. */
#define MULTIBOOT_INFO_MAGIC		0x2BADB002
#define MULTIBOOT_INFO_HAS_MEMORY	0x00000001
#define MULTIBOOT_INFO_HAS_BOOT_DEVICE	0x00000002
#define MULTIBOOT_INFO_HAS_CMDLINE	0x00000004
#define MULTIBOOT_INFO_HAS_MODS		0x00000008
#define MULTIBOOT_INFO_HAS_AOUT_SYMS	0x00000010
#define MULTIBOOT_INFO_HAS_ELF_SYMS	0x00000020
#define MULTIBOOT_INFO_HAS_MMAP		0x00000040
#define MULTIBOOT_INFO_HAS_DRIVES	0x00000080
#define MULTIBOOT_INFO_HAS_CONFIG_TABLE	0x00000100
#define MULTIBOOT_INFO_HAS_LOADER_NAME	0x00000200
#define MULTIBOOT_INFO_HAS_APM_TABLE	0x00000400
#define MULTIBOOT_INFO_HAS_VBE		0x00000800

/* MINIX-specific multiboot constants. */
#define MULTIBOOT_BOOTLOADER_MAGIC	0x2BADB002
#define MULTIBOOT_PAGE_ALIGN		0x00000001
#define MULTIBOOT_MEMORY_INFO		0x00000002
#define MULTIBOOT_VIDEO_MODE		0x00000004
#define MULTIBOOT_AOUT_KLUDGE		0x00010000
#define MULTIBOOT_VIDEO_MODE_EGA	1
#define MULTIBOOT_VIDEO_BUFFER		0xB8000
#define MULTIBOOT_LOWER_MEM_MAX		0x7f800
#define MULTIBOOT_CONSOLE_LINES		25
#define MULTIBOOT_CONSOLE_COLS		80
#define MULTIBOOT_VIDEO_BUFFER_BYTES	(25 * 80 * 2)
#define MULTIBOOT_STACK_SIZE		4096
#define MULTIBOOT_PARAM_BUF_SIZE	1024
#define MULTIBOOT_MAX_MODS		20
#define MULTIBOOT_INFO_MEMORY		0x00000001
#define MULTIBOOT_INFO_MEM_MAP		0x00000040
#define MULTIBOOT_INFO_BOOTDEV		0x00000002
#define MULTIBOOT_INFO_CMDLINE		0x00000004
#define MULTIBOOT_INFO_MODS		0x00000008
#define MULTIBOOT_HIGH_MEM_BASE		0x100000

#ifndef __ASSEMBLY__
#include <stdint.h>

struct multiboot_header {
	uint32_t	mh_magic;
	uint32_t	mh_flags;
	uint32_t	mh_checksum;
	uint32_t	mh_header_addr;
	uint32_t	mh_load_addr;
	uint32_t	mh_load_end_addr;
	uint32_t	mh_bss_end_addr;
	uint32_t	mh_entry_addr;
	uint32_t	mh_mode_type;
	uint32_t	mh_width;
	uint32_t	mh_height;
	uint32_t	mh_depth;
};

struct multiboot_info {
	uint32_t	mi_flags;
	uint32_t	mi_mem_lower;
	uint32_t	mi_mem_upper;
	uint32_t	mi_boot_device;
	uint32_t	mi_cmdline;
	uint32_t	mi_mods_count;
	uint32_t	mi_mods_addr;
	uint32_t	mi_syms[4];
	uint32_t	mi_mmap_length;
	uint32_t	mi_mmap_addr;
	uint32_t	mi_drives_length;
	uint32_t	mi_drives_addr;
	uint32_t	mi_config_table;
	uint32_t	mi_boot_loader_name;
	uint32_t	mi_apm_table;
	uint32_t	mi_vbe_control_info;
	uint32_t	mi_vbe_mode_info;
	uint16_t	mi_vbe_mode;
	uint16_t	mi_vbe_interface_seg;
	uint16_t	mi_vbe_interface_off;
	uint16_t	mi_vbe_interface_len;
};

#define MULTIBOOT_MEMORY_AVAILABLE	1
#define MULTIBOOT_MEMORY_RESERVED	2

/* I386_PAGE_SIZE alias for code ported from i386 that uses this name. */
#define I386_PAGE_SIZE		4096

struct multiboot_mmap {
	uint32_t	mm_size;
	uint64_t	mm_base_addr;
	uint64_t	mm_length;
	uint32_t	mm_type;
} __attribute__((packed));

struct multiboot_module {
	uint32_t	mod_start;
	uint32_t	mod_end;
	uint32_t	mod_cmdline;
	uint32_t	mod_pad;
};

/* Typedefs for param.h compatibility (same names as AArch64 branch). */
typedef struct multiboot_info multiboot_info_t;
typedef struct multiboot_module multiboot_module_t;
typedef struct multiboot_mmap multiboot_memory_map_t;

#endif /* __ASSEMBLY__ */

#else
/* AArch64 (and other non-x86): minimal stub for kernel compilation. */
#ifndef __ASSEMBLY__
#include <stdint.h>

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
#endif

#endif /* _MACHINE_MULTIBOOT_H_ */
