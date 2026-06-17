OUTPUT_ARCH("i386:x86-64")
ENTRY(__k_unpaged_MINIX)

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

	.unpaged_text : { unpaged_*.o(.text) }
	.unpaged_data ALIGN(4096) : { unpaged_*.o(.data .rodata*) }
	.unpaged_bss  ALIGN(4096)  : { unpaged_*.o(.bss COMMON) }
	__k_unpaged__kern_unpaged_end = .;

	. += _kern_offset;

	. = ALIGN(4096); usermapped_start = .;
	.usermapped_glo : AT(ADDR(.usermapped_glo) - _kern_offset) { *(.usermapped_glo) }
	. = ALIGN(4096); usermapped_nonglo_start = .;
	.usermapped : AT(ADDR(.usermapped) - _kern_offset) { *(.usermapped) }
	. = ALIGN(4096); usermapped_end = .;
	.text             : AT(ADDR(.text) - _kern_offset) { *(.text*) }
	.data ALIGN(4096) : AT(ADDR(.data) - _kern_offset) { *(.data .rodata* ) }
	. = ALIGN(4096);
	.bss ALIGN(4096)  : AT(ADDR(.bss) - _kern_offset) { *(.bss* COMMON)
		__k_unpaged__kern_size = . - _kern_vir_base;
		_kern_size = __k_unpaged__kern_size;
	}
	_end = .;
}
