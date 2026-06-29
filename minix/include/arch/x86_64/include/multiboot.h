/*	$NetBSD: multiboot.h,v 1.8 2009/02/22 18:05:42 ahoka Exp $	*/

/*-
 * Copyright (c) 2005, 2006 The NetBSD Foundation, Inc.
 * All rights reserved.
 *
 * This code is derived from software contributed to The NetBSD Foundation
 * by Julio M. Merino Vidal.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE NETBSD FOUNDATION, INC. AND CONTRIBUTORS
 * ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
 * TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION OR CONTRIBUTORS
 * BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

#ifndef _X86_64_MULTIBOOT_H_
#define _X86_64_MULTIBOOT_H_

/* Multiboot header constants. */
#define MULTIBOOT_HEADER_MAGIC		0x1BADB002
#define MULTIBOOT_HEADER_MODS_ALIGNED	0x00000001
#define MULTIBOOT_HEADER_WANT_MEMORY	0x00000002
#define MULTIBOOT_HEADER_HAS_VBE	0x00000004
#define MULTIBOOT_HEADER_HAS_ADDR	0x00010000

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

/* MINIX-specific multiboot constants (used in head.S). */
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

/* Multiboot header structure. */
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

/* Multiboot info structure. */
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

#endif /* __ASSEMBLY__ */

#endif /* _X86_64_MULTIBOOT_H_ */
