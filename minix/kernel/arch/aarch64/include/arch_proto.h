/* ============================================================
 * arch_proto.h — ARM64 architecture prototypes
 *
 * Declares assembly functions implemented in klib.S, and
 * functions provided by other architecture-specific modules.
 *
 * Phase 2: Minimal set — only functions needed for bootstrap.
 * Phase 2+: Full set with all klib.S functions.
 * ============================================================ */

#ifndef _AARCH64_ARCH_PROTO_H
#define _AARCH64_ARCH_PROTO_H

#include <machine/vm.h>

/* Kernel stack size (ARM64 uses 4KB pages) */
#define K_STACK_SIZE	ARM64_PAGE_SIZE

#ifndef __ASSEMBLY__

#include <stdint.h>
#include <minix/type.h>
#include <minix/endpoint.h>
#include <machine/vm.h>

/* =========================================================================
 * klib.S — Basic architecture functions
 * ========================================================================= */

/* Physical memory operations */
phys_bytes phys_copy(phys_bytes source, phys_bytes destination,
		     phys_bytes bytecount);
phys_bytes phys_memset(phys_bytes dst, unsigned long pattern,
		       phys_bytes bytecount);

/* Virtual-to-physical address translation (AT instruction) */
phys_bytes vir2phys(void *vir_addr);

/* Message copy with fault recovery */
int copy_msg_from_user(message *user_mbuf, message *dst);
int copy_msg_to_user(message *src, message *user_mbuf);
void __user_copy_msg_pointer_failure(void);

/* Markers for fault recovery in message copy */
void __copy_msg_from_user_end(void);
void __copy_msg_to_user_end(void);

/* Address space switching (TTBR0_EL1) */
void __switch_address_space(struct proc *p, struct proc **__ptproc);
#define switch_address_space(proc)					\
	__switch_address_space(proc, get_cpulocal_var_ptr(ptproc))

/* Stack switching */
void switch_k_stack(void *sp, void (*continuation)(void));

/* System control */
_Noreturn void reset(void);
void halt_cpu(void);
/* intr_enable/intr_disable are declared in <minix/portio.h> */

/* System register access */
uint64_t read_ttbr0(void);
uint64_t read_ttbr1(void);
void write_ttbr0(uint64_t ttbr);

uint64_t read_esr(void);
uint64_t read_far(void);
uint64_t read_elr(void);
uint64_t read_spsr(void);

/* Per-CPU pointer (tpidr_el1) */
uint64_t read_tpidr_el1(void);
void write_tpidr_el1(uint64_t val);

/* TLB maintenance */
void tlb_flush_all(void);
void tlb_flush_addr(uint64_t addr);

/* Memory barriers */
void dmb_sy(void);
void dsb_sy(void);

/* Instruction synchronization barrier (used in pg_utils.c, arch_system.c, etc.) */
#define isb() __asm__ __volatile__("isb" : : : "memory")

/* Compiler barrier — prevents reordering across the barrier */
#define barrier() __asm__ __volatile__("dmb sy" : : : "memory")

/* Misc */
uint64_t read_current_sp(void);
void arch_pause(void);

/* =========================================================================
 * Other architecture-specific functions
 * ========================================================================= */

/* Memory mapping helpers */
phys_bytes pg_roundup(phys_bytes b);
void pg_info(reg_t *, uint32_t **);
void pg_clear(void);
void pg_identity(kinfo_t *);
phys_bytes pg_load(void);
void pg_map(phys_bytes phys, vir_bytes vaddr, vir_bytes vaddr_end,
	    kinfo_t *cbi);
int pg_mapkernel(void);
void pg_mapproc(struct proc *p, struct boot_image *ip, kinfo_t *cbi);
void add_memmap(kinfo_t *cbi, u64_t addr, u64_t len);
phys_bytes alloc_lowest(kinfo_t *cbi, phys_bytes len);
void vm_enable_paging(void);
void cut_memmap(kinfo_t *cbi, phys_bytes start, phys_bytes end);
int tss_init(unsigned cpu, void *kernel_stack);

/* Stack variables */
EXTERN void *k_stacks_start;
extern void *k_stacks;

#define get_k_stack_top(cpu)	((void *)(((char *)(k_stacks))		\
					+ 2 * ((cpu) + 1) * K_STACK_SIZE))

/* =========================================================================
 * Physical memory mapping helpers
 * ========================================================================= */

typedef int (*kern_phys_map_mapped)(vir_bytes id, vir_bytes new_addr);

typedef struct kern_phys_map {
	phys_bytes addr;	/* Physical address to map */
	vir_bytes size;		/* Size of the mapping */
	vir_bytes id;		/* ID passed to callback */
	int vm_flags;		/* Flags for vm_map */
	kern_phys_map_mapped cb;/* Callback when mapped */
	phys_bytes vir;		/* Virtual address after remap */
	int index;		/* Index */
	struct kern_phys_map *next; /* Next entry */
} kern_phys_map;

int kern_req_phys_map(phys_bytes base_address, vir_bytes io_size,
		      int vm_flags, kern_phys_map *priv,
		      kern_phys_map_mapped cb, vir_bytes id);

int kern_phys_map_ptr(phys_bytes base_address, vir_bytes io_size,
		      int vm_flags, kern_phys_map *priv,
		      vir_bytes ptr);

/* Architecture-independent kernel prototypes */
#include "kernel/proto.h"

#endif /* __ASSEMBLY__ */

#endif /* _AARCH64_ARCH_PROTO_H */
