OUTPUT_ARCH("i386:x86-64")
ENTRY(MINIX)

/* x86_64 kernel layout:
 *   Physical base: 0x00100000 (1MB — within identity-mapped 0-4MB range)
 *   Virtual base:  0xFFFF8000F0100000 (high canonical address)
 *   Offset: virtual - physical (maps kernel high)
 *   Start in low memory (unpaged), then jump to high mapping
 *
 * The kernel is loaded at 1MB so it fits within the 4MB identity mapping
 * (0x00000000-0x003FFFFF) set up in head.S during the 32→64 bit transition.
 */
_kern_phys_base = 0x00100000;
_kern_vir_base  = 0xFFFF8000F0100000;
_kern_offset = (_kern_vir_base - _kern_phys_base);

__k_unpaged__kern_offset = _kern_offset;
__k_unpaged__kern_vir_base = _kern_vir_base;
__k_unpaged__kern_phys_base = _kern_phys_base;

/* 64-bit page table entries need a temporary identity mapping before
 * jumping to the high address. The boot code sets up a 1:1 mapping
 * for the low 4MB + the high mapping simultaneously, then jumps to
 * the high address and removes the low mapping.
 */

SECTIONS
{
	. = _kern_phys_base;
	__k_unpaged__kern_unpaged_start = .;
	/* Alias for C code (MINIX strips __k_unpaged_ prefix at link time). */
	_kern_unpaged_start = __k_unpaged__kern_unpaged_start;

	/* Unpaged sections: code that runs before paging is enabled.
	 * head.S and klib.S use explicit .section .unpaged.text,
	 * .unpaged.data, .unpaged.bss directives for early boot code
	 * that must run at low physical addresses.
	 * C files (pre_init.c, limine.c, etc.) are NOT placed here —
	 * they use regular .text and go to the high VMA. Head.S calls
	 * them via absolute indirect jumps (movabs+call). */
	.unpaged_text : {
		*(.unpaged.text)
		*(.unpaged.text.*)
	}
	.unpaged_data ALIGN(4096) : {
		*(.unpaged.data)
		*(.unpaged.rodata)
		*(.unpaged.rodata.*)
	}
	.unpaged_bss  ALIGN(4096)  : {
		*(.unpaged.bss)
		*(.unpaged.COMMON)
	}
	__k_unpaged__kern_unpaged_end = .;
	_kern_unpaged_end = __k_unpaged__kern_unpaged_end;

	/* Limine boot protocol requests section.
	 * This section contains the request structures that the Limine
	 * bootloader scans to find which services the kernel needs.
	 * Must be accessible at boot time (in unpaged part).
	 */
	.limine_requests ALIGN(64) : { *(.limine_requests) }
	__k_unpaged__limine_requests_start = .;
	__k_unpaged__limine_requests_end = .;

	/* Jump to high virtual address for the paged kernel */
	. += _kern_offset;

	. = ALIGN(4096); usermapped_start = .;
	.usermapped_glo : AT(ADDR(.usermapped_glo) - _kern_offset) { *(.usermapped_glo) }
	. = ALIGN(4096); usermapped_nonglo_start = .;
	.usermapped : AT(ADDR(.usermapped) - _kern_offset) { *(.usermapped) }
	. = ALIGN(4096); usermapped_end = .;
	.text             : AT(ADDR(.text) - _kern_offset) {
		*(.text*)
		*(.ltext*)
	}
	.data ALIGN(4096) : AT(ADDR(.data) - _kern_offset) {
		*(.data .rodata*)
		*(.ldata* .lrodata*)
	}
	. = ALIGN(4096);
	.bss ALIGN(4096)  : AT(ADDR(.bss) - _kern_offset) {
		*(.bss* COMMON)
		*(.lbss*)
		__k_unpaged__kern_size = . - _kern_vir_base;
		_kern_size = __k_unpaged__kern_size;
	}
	_kern_end = _kern_phys_base + _kern_size;
	__k_unpaged__kern_end = _kern_end;
	_kern_end = __k_unpaged__kern_end;
	_end = .;

	/* PROVIDE aliases for symbols that C/assembly code references
	 * without the __k_unpaged_ prefix. These are defined by the
	 * build system in the original BSD Make (which auto-generated
	 * aliases), but must be explicit with ld.lld. */
	PROVIDE(k_initial_stktop = __k_unpaged_k_initial_stktop);
	PROVIDE(kmain = __k_unpaged_kmain);
}
