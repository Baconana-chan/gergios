/*	machine/memory.h — x86_64 memory layout constants	*/

#ifndef _MACHINE_MEMORY_H_
#define _MACHINE_MEMORY_H_

/* Virtual address space layout */
#define KERNEL_VBASE		0xFFFF800000000000UL

#define PAGE_SHIFT		12
#define PAGE_SIZE		(1 << PAGE_SHIFT)
#define PAGE_MASK		(PAGE_SIZE - 1)

#define LARGE_PAGE_SIZE		(2 * 1024 * 1024)	/* 2MB pages */

#define	SEGMENT_SIZE		0x10000000	/* 256MB segment */

#define CLICK_SHIFT		4		/* 16-byte clicks */
#define CLICK_SIZE		(1 << CLICK_SHIFT)

#define PHYS_MEM_START		0x100000	/* 1MB (skip BIOS/VGA area) */

/* BIOS and base memory layout (x86 machines) */
#define BIOS_MEM_BEGIN		0x00000
#define BIOS_MEM_END		0x004FF
#define BASE_MEM_TOP		0x090000
#define UPPER_MEM_END		0x0FFFFF

#endif /* _MACHINE_MEMORY_H_ */
