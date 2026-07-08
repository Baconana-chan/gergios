
#ifndef _MINIX_PARAM_H
#define _MINIX_PARAM_H 1

#include <minix/com.h>
#include <minix/const.h>

/* Number of processes contained in the system image. */
#define NR_BOOT_PROCS   (NR_TASKS + LAST_SPECIAL_PROC_NR + 1)

#ifdef _MINIX_SYSTEM
/* This is used to obtain system information through SYS_GETINFO. */
#define MAXMEMMAP 40
/* NOTE: AArch64 uses the same kinfo struct with multiboot fields for
 * kernel compilation compatibility. At runtime, boot info comes from
 * Device Tree or UEFI, not Multiboot. */
typedef struct kinfo {
        /* Straight multiboot-provided info */
        multiboot_info_t        mbi;
        multiboot_module_t      module_list[MULTIBOOT_MAX_MODS];
        multiboot_memory_map_t  memmap[MAXMEMMAP]; /* free mem list */
        phys_bytes              mem_high_phys;
        int                     mmap_size;

        /* Multiboot-derived */
        int                     mods_with_kernel; /* no. of mods incl kernel */
        int                     kern_mod; /* which one is kernel */

        /* Minix stuff, started at bootstrap phase */
        int                     freepde_start;  /* lowest pde unused kernel pde */
        char                    param_buf[MULTIBOOT_PARAM_BUF_SIZE];

        /* Minix stuff */
        struct kmessages *kmessages;
        int do_serial_debug;    /* system serial output */
        int serial_debug_baud;  /* serial baud rate */
        int minix_panicing;     /* are we panicing? */
        vir_bytes               user_sp; /* where does kernel want stack set */
        vir_bytes               user_end; /* upper proc limit */
        vir_bytes               vir_kern_start; /* kernel addrspace starts */
        vir_bytes               bootstrap_start, bootstrap_len;
        struct boot_image       boot_procs[NR_BOOT_PROCS];
        int nr_procs;           /* number of user processes */
        int nr_tasks;           /* number of kernel tasks */
        char release[6];        /* kernel release number */
        char version[6];        /* kernel version number */
	int vm_allocated_bytes; /* allocated by kernel to load vm */
	int kernel_allocated_bytes;		/* used by kernel */
	int kernel_allocated_bytes_dynamic;	/* used by kernel (runtime) */

	/* KASLR: random seed from RDRAND at boot, 0 if not enabled.
	 * Used to randomize kernel layout, pagetable placement,
	 * stack positions, and for user-space ASLR entropy.
	 * Written once during pre_init/limine_pre_init.
	 */
	u64_t kaslr_seed;

	/* KASLR: physical offset applied by bootloader.
	 * If the bootloader loaded the kernel at a different physical
	 * address than the linked _kern_phys_base (0x100000), this
	 * field contains the difference: actual_phys - linked_phys.
	 * Zero when KASLR is not active or bootloader doesn't support
	 * physical randomization. Written during limine_pre_init().
	 * Used by pg_mapkernel() to map the kernel at its actual
	 * physical location, and by VM for user-space ASLR.
	 */
	u64_t kaslr_phys_offset;

	/* KASLR: virtual address offset for Virtual KASLR (Phase 3).
	 * When compiling with -fPIE -pie and processing relocations at
	 * boot, the kernel can run at a different virtual address than
	 * linked. This field stores the offset: new_virt_base -
	 * linked_virt_base. Zero when KASLR is not enabled, or when the
	 * PIE infrastructure is active but VMA randomization is deferred.
	 * Computed from kaslr_seed during pre_init/limine_pre_init.
	 * Used by apply_relocations() and pg_mapkernel().
	 */
	u64_t kaslr_virt_offset;
} kinfo_t;
#endif /* _MINIX_SYSTEM */
#endif
