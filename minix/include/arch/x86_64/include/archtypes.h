#ifndef _X86_64_TYPES_H
#define _X86_64_TYPES_H

#include <minix/sys_config.h>
#include <machine/stackframe.h>
#include <machine/fpu.h>
#include <sys/cdefs.h>

struct segdesc_s {		/* segment descriptor for protected/long mode */
  u16_t limit_low;
  u16_t base_low;
  u8_t base_middle;
  u8_t access;			/* |P|DL|1|X|E|R|A| */
  u8_t granularity;		/* |G|L|0|A|LIMT| */
  u8_t base_high;
} __attribute__((packed));

struct gatedesc_s {
  u16_t offset_low;
  u16_t selector;
  u8_t ist;                     /* Interrupt Stack Table (x86_64: bits 0-2) */
  u8_t p_dpl_type;              /* |P|DL|0|TYPE| */
  u16_t offset_middle;
  u32_t offset_high;
  u32_t reserved;
} __attribute__((packed));

/* Descriptor table pointer for LGDT/LIDT.
 * On x86_64, the base is 64-bit (8 bytes + 2 byte limit = 10 bytes total).
 */
struct desctableptr_s {
  u16_t limit;
  u64_t base;
} __attribute__((packed));

typedef struct segframe {
	reg_t	p_cr3;		/* page table root (CR3) */
	u64_t	*p_cr3_v;	/* virtual address of CR3 value */
	char	*fpu_state;
	int	p_kern_trap_style;
} segframe_t;

struct cpu_info {
	u8_t	vendor;
	u8_t	family;
	u8_t	model;
	u8_t	stepping;
	u32_t	freq;		/* in MHz */
	u32_t	flags[2];
};

typedef u32_t atomic_t;	/* access to an aligned 32bit value is atomic on x86_64 */

#endif /* #ifndef _X86_64_TYPES_H */
