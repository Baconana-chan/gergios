/* x86_64 SMP support.
 *
 * This file handles the initialization of Application Processors (APs)
 * on x86_64 systems. Key differences from i386:
 *   - phys_bytes is 64-bit (unsigned long long)
 *   - PML4 page tables instead of page directories
 *   - No PSE (CR4.PSE) — use PAE instead (required for long mode)
 *   - EFER.LME MSR for long mode enable
 *   - startup_ap_64 entry point (in mpx.S) instead of startup_ap_32
 *
 * Changes:
 *   Apr 1, 2008    Added SMP support (original i386).
 *   Jun 17, 2026   Ported to x86_64.
 */

#define _SMP

#include <unistd.h>
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <machine/cmos.h>
#include <machine/bios.h>

#include "kernel/spinlock.h"
#include "kernel/smp.h"
#include "apic.h"
#include "acpi.h"
#include "kernel/clock.h"

#include "kernel/kernel.h"

void trampoline(void);

/*
 * Arguments for trampoline. We need to pass the logical CPU id, page table
 * base (PML4 physical address), GDT and IDT.
 * They have to be in a location which is reachable using absolute addressing
 * in 16-bit mode. On x86_64, the trampoline is still in low memory (< 1MB).
 *
 * __ap_pt is 64-bit on x86_64 (phys_bytes) to support > 4GB page tables.
 */
extern volatile u64_t __ap_id;
extern volatile u64_t __ap_pt;
extern volatile struct desctableptr_s __ap_gdt, __ap_idt;
extern u64_t __ap_gdt_tab[], __ap_idt_tab[];
extern void * __trampoline_end;

extern u32_t busclock[CONFIG_MAX_CPUS];
extern int panicking;

static volatile int ap_cpu_ready;
static volatile int cpu_down;

/* There can be at most 255 local APIC ids, each fits in 8 bits. */
static unsigned char apicid2cpuid[255];
unsigned char cpuid2apicid[CONFIG_MAX_CPUS];

SPINLOCK_DEFINE(smp_cpu_lock)
SPINLOCK_DEFINE(dispq_lock)

static void smp_reinit_vars(void);

/* These are initialized in protect.c */
extern struct segdesc_s gdt[GDT_SIZE];
extern struct gatedesc_s idt[IDT_SIZE];
extern struct tss_s tss[CONFIG_MAX_CPUS];
extern int prot_init_done;	/* Indicates they are ready */

int booting_cpu = 0;	/* tell protect.c what to do */

static phys_bytes trampoline_base;

static phys_bytes ap_lin_addr(void *vaddr)
{
	assert(trampoline_base);
	return (phys_bytes) vaddr - (phys_bytes) &trampoline + trampoline_base;
}

/*
 * Copies the 16-bit AP trampoline code to the first 1M of memory.
 * On x86_64, the trampoline includes both 16-bit and 64-bit code,
 * and the data area includes 64-bit variables (__ap_id, __ap_pt).
 */
void copy_trampoline(void)
{
	phys_bytes tramp_start = (phys_bytes)&trampoline;
	phys_bytes tramp_size;

	/* The trampoline code/data is made to be page-aligned. */
	assert(!(tramp_start % I386_PAGE_SIZE));

	tramp_size = (phys_bytes) &__trampoline_end - tramp_start;
	trampoline_base = alloc_lowest(&kinfo, (phys_bytes) tramp_size);

	/* The memory allocator finds the lowest available memory.
	 * Verify it's low enough for the SIPI vector (page-aligned).
	 */
	assert(trampoline_base + tramp_size < (1 << 20));

	/* Prepare GDT and IDT for the new CPUs; make copies
	 * of both the tables and the descriptors of them
	 * in their boot addressing environment.
	 *
	 * On x86_64:
	 *   - GDT entries are still 8 bytes each (same format)
	 *   - IDT entries are 16 bytes each (64-bit gate format)
	 *   - pseudo-descriptors have a 64-bit base (10 bytes total)
	 */
	assert(prot_init_done);

	/* Copy GDT to AP trampoline area */
	memcpy((void*)&__ap_gdt_tab, gdt, sizeof(gdt));

	/* Copy IDT to AP trampoline area.
	 * On x86_64, IDT entries are 16 bytes each (struct gatedesc_s
	 * has offset_low + selector + ist + p_dpl_type + offset_middle +
	 * offset_high + reserved). */
	memcpy((void*)&__ap_idt_tab, idt, sizeof(idt));

	/* Set up 10-byte pseudo-descriptors for LGDT/LIDT in 64-bit mode.
	 * Format: [2 bytes limit][8 bytes base address] */
	__ap_gdt.limit = sizeof(gdt) - 1;
	__ap_gdt.base = (u64_t) ap_lin_addr(&__ap_gdt_tab);

	__ap_idt.limit = sizeof(idt) - 1;
	__ap_idt.base = (u64_t) ap_lin_addr(&__ap_idt_tab);

	phys_copy((phys_bytes) trampoline, trampoline_base, (phys_bytes) tramp_size);
}

extern int booting_cpu;	/* tell protect.c what to do */

static void smp_start_aps(void)
{
	unsigned cpu;
	u32_t biosresetvector;
	phys_bytes __ap_id_phys;
	struct proc *bootstrap_pt = get_cpulocal_var(ptproc);

	/* Save the BIOS reset vector so we can restore it later */
	phys_copy(0x467, (phys_bytes) &biosresetvector, sizeof(u32_t));

	/* Set the BIOS shutdown code to 0xA (warm reset) */
	outb(RTC_INDEX, 0xF);
	outb(RTC_IO, 0xA);

	assert(bootstrap_pt);
	assert(bootstrap_pt->p_seg.p_cr3);
	__ap_pt = (u64_t) bootstrap_pt->p_seg.p_cr3;
	assert(__ap_pt);

	copy_trampoline();

	/* Physical address of __ap_id within the trampoline copy */
	__ap_id_phys = trampoline_base +
		(phys_bytes) &__ap_id - (phys_bytes)&trampoline;

	/* Setup the warm reset vector to point to our trampoline base */
	phys_copy((phys_bytes) &trampoline_base, 0x467, sizeof(u32_t));

	/* Okay, we're ready to go. Boot all of the APs now.
	 * We loop through using the processor's APIC id values.
	 */
	for (cpu = 0; cpu < ncpus; cpu++) {
		ap_cpu_ready = -1;

		/* Don't send INIT/SIPI to boot CPU. */
		if ((apicid() == cpuid2apicid[cpu]) &&
				(apicid() == bsp_lapic_id)) {
			continue;
		}

		__ap_id = (u64_t) (booting_cpu = cpu);
		phys_copy((phys_bytes) &__ap_id, __ap_id_phys,
			  (phys_bytes) sizeof(__ap_id));
		mfence();

		if (apic_send_init_ipi(cpu, trampoline_base) ||
				apic_send_startup_ipi(cpu, trampoline_base)) {
			printf("WARNING cannot boot cpu %d\n", cpu);
			continue;
		}

		/* Wait for 5 secs for the processors to boot */
		lapic_set_timer_one_shot(5000000);

		while (lapic_read(LAPIC_TIMER_CCR)) {
			if (ap_cpu_ready == (int) cpu) {
				cpu_set_flag(cpu, CPU_IS_READY);
				break;
			}
		}
		if (ap_cpu_ready == -1) {
			printf("WARNING : CPU %d didn't boot\n", cpu);
		}
	}

	/* Restore the BIOS reset vector */
	phys_copy((phys_bytes) &biosresetvector, 0x467, sizeof(u32_t));

	outb(RTC_INDEX, 0xF);
	outb(RTC_IO, 0);

	bsp_finish_booting();
	NOT_REACHABLE;
}

void smp_halt_cpu(void)
{
	NOT_IMPLEMENTED;
}

void smp_shutdown_aps(void)
{
	unsigned cpu;

	if (ncpus == 1)
		goto exit_shutdown_aps;

	/* We must let the other CPUs enter the kernel mode */
	BKL_UNLOCK();

	for (cpu = 0; cpu < ncpus; cpu++) {
		if (cpu == cpuid)
			continue;
		if (!cpu_test_flag(cpu, CPU_IS_READY)) {
			printf("CPU %d didn't boot\n", cpu);
			continue;
		}

		cpu_down = -1;
		barrier();
		apic_send_ipi(APIC_SMP_CPU_HALT_VECTOR, cpu, APIC_IPI_DEST);
		/* Wait for the cpu to be down */
		while (cpu_down != (int) cpu);
		printf("CPU %d is down\n", cpu);
		cpu_clear_flag(cpu, CPU_IS_READY);
	}

exit_shutdown_aps:
	ioapic_disable_all();

	lapic_disable();

	ncpus = 1; /* hopefully !!! */
	lapic_addr = lapic_eoi_addr = 0;
	return;
}

static void ap_finish_booting(void)
{
	unsigned cpu = cpuid;

	/* Inform the world of our presence */
	ap_cpu_ready = (int) cpu;

	/*
	 * Finish processor initialization. CPUs must be excluded from running.
	 * LAPIC timer calibration locks and unlocks the BKL because of the
	 * nested interrupts used for calibration. Therefore BKL is not good
	 * enough, the boot_lock must be held.
	 */
	spinlock_lock(&boot_lock);
	BKL_LOCK();

	printf("CPU %d is up\n", cpu);

	cpu_identify();

	lapic_enable(cpu);
	fpu_init();

	if (app_cpu_init_timer(system_hz)) {
		panic("FATAL : failed to initialize timer interrupts CPU %d, "
			"cannot continue without any clock source!", cpu);
	}

	/* Assign CPU local idle structure */
	get_cpulocal_var(proc_ptr) = get_cpulocal_var_ptr(idle_proc);
	get_cpulocal_var(bill_ptr) = get_cpulocal_var_ptr(idle_proc);

	ap_boot_finished(cpu);
	spinlock_unlock(&boot_lock);

	switch_to_user();
	NOT_REACHABLE;
}

void smp_ap_boot(void)
{
	switch_k_stack((char *)get_k_stack_top(__ap_id) -
			X86_STACK_TOP_RESERVED, ap_finish_booting);
}

static void smp_reinit_vars(void)
{
	lapic_addr = lapic_eoi_addr = 0;
	ioapic_enabled = 0;

	ncpus = 1;
}

static void tss_init_all(void)
{
	unsigned cpu;

	for (cpu = 0; cpu < ncpus; cpu++)
		tss_init(cpu, get_k_stack_top(cpu));
}

static int discover_cpus(void)
{
	struct acpi_madt_lapic *cpu_entry;

	while (ncpus < CONFIG_MAX_CPUS &&
	       (cpu_entry = acpi_get_lapic_next())) {
		apicid2cpuid[cpu_entry->apic_id] = (unsigned char) ncpus;
		cpuid2apicid[ncpus] = (unsigned char) cpu_entry->apic_id;
		printf("CPU %3d local APIC id %3d\n", ncpus,
		       cpu_entry->apic_id);
		ncpus++;
	}

	return ncpus;
}

void smp_init(void)
{
	/* Read the MP configuration */
	if (!discover_cpus()) {
		ncpus = 1;
		goto uniproc_fallback;
	}

	lapic_addr = LOCAL_APIC_DEF_ADDR;
	ioapic_enabled = 0;

	tss_init_all();

	/*
	 * We still run on the boot stack and we cannot use cpuid as its value
	 * wasn't set yet. apicid2cpuid is initialized in discover_cpus().
	 */
	bsp_cpu_id = apicid2cpuid[apicid()];

	if (!lapic_enable(bsp_cpu_id)) {
		printf("ERROR : failed to initialize BSP Local APIC\n");
		goto uniproc_fallback;
	}

	bsp_lapic_id = apicid();

	acpi_init();

	if (!detect_ioapics()) {
		lapic_disable();
		lapic_addr = 0x0;
		goto uniproc_fallback;
	}

	ioapic_enable_all();

	if (ioapic_enabled)
		machine.apic_enabled = 1;

	/* Set SMP IDT entries */
	apic_idt_init(0); /* Not a reset! */
	idt_reload();

	BOOT_VERBOSE(printf("SMP initialized\n"));

	switch_k_stack((char *)get_k_stack_top(bsp_cpu_id) -
			X86_STACK_TOP_RESERVED, smp_start_aps);

	return;

uniproc_fallback:
	apic_idt_init(1); /* Reset to PIC IDT! */
	idt_reload();
	smp_reinit_vars(); /* Revert to a single proc system */
	intr_init(0); /* No auto EOI */
	printf("WARNING : SMP initialization failed\n");
}

void arch_smp_halt_cpu(void)
{
	/* Say that we are down */
	cpu_down = cpuid;
	barrier();
	/* Unlock the BKL and don't continue */
	BKL_UNLOCK();
	for (;;)
		arch_pause();
}

void arch_send_smp_schedule_ipi(unsigned cpu)
{
	apic_send_ipi(APIC_SMP_SCHED_PROC_VECTOR, cpu, APIC_IPI_DEST);
}
