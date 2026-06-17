
#ifndef _PT_H
#define _PT_H 1

#include <machine/vm.h>

#include "vm.h"
#include "pagetable.h"

/* A pagetable.
 *
 * pt_entry_t is defined in <machine/vm.h> via the arch-specific pagetable.h.
 * On i386 it is u32_t (32-bit PTE), on x86_64 it is u64_t (64-bit PTE).
 */
typedef struct {
	/* Directory entries in VM addr space - root of page table.  */
	pt_entry_t *pt_dir;	/* page aligned (ARCH_VM_DIR_ENTRIES) */
	u32_t pt_dir_phys;	/* physical address of pt_dir */

	/* Pointers to page tables in VM address space. */
	pt_entry_t *pt_pt[ARCH_VM_DIR_ENTRIES];

	/* When looking for a hole in virtual address space, start
	 * looking here. This is in linear addresses, i.e.,
	 * not as the process sees it but the position in the page
	 * page table. This is just a hint.
	 */
	u32_t pt_virtop;
} pt_t;

#define CLICKSPERPAGE (VM_PAGE_SIZE/CLICK_SIZE)

#endif
