/* relocate.c — ELF relocation processing for Virtual KASLR.
 *
 * Phase 3: Virtual KASLR. When the kernel is linked as PIE (-fPIE -pie),
 * the linker generates .rela.dyn with R_X86_64_RELATIVE relocation entries.
 * These must be processed at boot time to adjust absolute addresses when
 * the kernel is loaded at a different virtual address than linked.
 *
 * This file implements apply_relocations() in the .unpaged.text section,
 * called from head.S before jumping to the high VMA. It runs from the
 * identity-mapped low memory and directly patches absolute address
 * references in the kernel's .text, .data, and other paged sections.
 *
 * R_X86_64_RELATIVE (type 8):
 *   For each entry, the value at virtual address r_offset is a pointer
 *   that was linked at (_kern_vir_base + r_addend). After relocating by
 *   delta, it should point to (_kern_vir_base + delta + r_addend).
 *   We simply add delta to the 64-bit value at the physical address
 *   corresponding to r_offset.
 *
 * The function MUST NOT access any global/static variables — it only
 * uses parameters passed in registers (RDI, RSI, RDX in sysv amd64 ABI).
 * It is placed in .unpaged.text so it runs at physical addresses before
 * paging is fully set up.
 */

/* NOTE: This file uses minimal C — no includes that reference global
 * variables. The type definitions below are standalone and do not
 * rely on any external headers, because this code runs BEFORE
 * relocations are applied (accessing global vars requires working
 * absolute addresses).
 */

/* ELF64 Rela entry (standalone definition, no ELF headers needed) */
struct elf64_rela {
	unsigned long long r_offset;	/* virtual address of location to patch */
	unsigned long long r_info;	/* type (low 32 bits) + symbol index */
	long long r_addend;		/* constant addend */
};

#define RELA_SIZE sizeof(struct elf64_rela)

/* Extract relocation type from r_info (low 32 bits). */
static inline unsigned int rela_type(unsigned long long info)
{
	return (unsigned int)(info & 0xffffffffULL);
}

/* R_X86_64_RELATIVE relocation type value */
#define R_X86_64_RELATIVE  8

/* =========================================================================
 * apply_relocations — process .rela.dyn entries
 *
 * Called from head.S to adjust kernel absolute addresses by a delta
 * offset. First call (from head.S before pre_init/limine_pre_init):
 *   delta=0 (no-op, infrastructure verification).
 * Second call (from head.S after pre_init/limine_pre_init returns):
 *   delta=kaslr_virt_offset (real VMA randomization).
 *
 * Parameters (sysv amd64 ABI):
 *   RDI (arg1): physical address of .rela.dyn start
 *   RSI (arg2): number of relocation entries (count)
 *   RDX (arg3): delta (new_virt_base - linked_virt_base)
 *
 * For each R_X86_64_RELATIVE entry, adds delta to the 64-bit value
 * at the entry's r_offset virtual address. The page tables map both
 * the identity (low) VMA and the high kernel VMA, so accessing
 * r_offset directly as a virtual address works.
 *
 * IMPORTANT: Entries with r_offset below 0xFFFF800000000000 are in
 * the unpaged section (low physical VMA). These are SKIPPED to avoid
 * self-modifying code issues (the apply_relocations function itself
 * is in .unpaged.text). Unpaged section addresses don't need
 * relocation — they run at fixed physical addresses.
 * =========================================================================
 */
__attribute__((section(".unpaged.text")))
void apply_relocations(unsigned long long rela_phys,
		       unsigned long long rela_count,
		       unsigned long long delta)
{
	volatile struct elf64_rela *rela;
	unsigned long long i;

	/* Early exit: no relocations to process, or no VMA shift needed */
	if (rela_phys == 0 || rela_count == 0 || delta == 0)
		return;

	rela = (volatile struct elf64_rela *)(unsigned long)rela_phys;

	for (i = 0; i < rela_count; i++) {
		volatile unsigned long long *target;

		if (rela_type(rela[i].r_info) != R_X86_64_RELATIVE)
			continue;

		/* Skip entries in the unpaged section range (low VMA).
		 * These have r_offset < 0xFFFF800000000000 (the lowest
		 * high VMA). Unpaged sections run at fixed physical
		 * addresses and DON'T need VMA relocation. Skipping
		 * them also avoids self-modifying code issues, since
		 * this function itself is in the unpaged section. */
		if (rela[i].r_offset < 0xFFFF800000000000ULL)
			continue;

		/* r_offset is the virtual address to patch.
		 * The page tables map both the identity-mapped low
		 * VMA and the high VMA, so we can access r_offset
		 * directly. After relocation, this value becomes
		 * linked_value + delta. */
		target = (volatile unsigned long long *)
			(unsigned long)rela[i].r_offset;

		/* Add delta: new_addr = old_addr + delta.
		 * The old value at r_offset is (linked_VMA + addend).
		 * We need it to be (linked_VMA + delta + addend).
		 * Volatile ensures the write goes to memory immediately. */
		*target += delta;
	}
}
