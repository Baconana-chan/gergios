/* x86_64 APIC stub — minimal definitions for kernel build.
 *
 * This file provides the APIC variables and functions declared in
 * apic.h but not yet implemented for the x86_64 port.
 *
 * TODO: Implement real APIC/IOAPIC support for x86_64.
 */

#include "kernel/kernel.h"
#include "apic.h"

/* ------------------------------------------------------------------ *
 * APIC global variables                                              *
 * ------------------------------------------------------------------ */

vir_bytes lapic_addr      = 0;   /* No local APIC by default */
vir_bytes lapic_eoi_addr  = 0;   /* No EOI address by default */
int       ioapic_enabled  = 0;   /* I/O APIC disabled by default */
int       bsp_lapic_id    = 0;   /* BSP LAPIC ID (unused in stub) */

struct io_apic io_apic[MAX_NR_IOAPICS];
unsigned      nioapics    = 0;

u32_t lapic_addr_vaddr    = 0;   /* Virtual address after paging (unused) */

/* ------------------------------------------------------------------ *
 * Stub functions                                                     *
 * ------------------------------------------------------------------ */

void dump_apic_irq_state(void)
{
	/* Not implemented */
}

int lapic_enable(unsigned cpu)
{
	return 0;  /* APIC not available */
}

void lapic_disable(void)
{
	/* Not implemented */
}

void ioapic_unmask_irq(unsigned irq)
{
	/* Not implemented */
}

void ioapic_mask_irq(unsigned irq)
{
	/* Not implemented */
}

void ioapic_reset_pic(void)
{
	/* Not implemented */
}

void lapic_microsec_sleep(unsigned count)
{
	/* Not implemented */
}

void ioapic_disable_irqs(u32_t irqs)
{
	/* Not implemented */
}

void ioapic_enable_irqs(u32_t irqs)
{
	/* Not implemented */
}

void ioapic_disable_all(void)
{
	/* Not implemented */
}

int ioapic_enable_all(void)
{
	return 0;
}

int detect_ioapics(void)
{
	return 0;  /* No IOAPICs detected */
}

void apic_idt_init(int reset)
{
	/* Not implemented */
}

#ifdef CONFIG_SMP
int apic_send_startup_ipi(unsigned cpu, phys_bytes trampoline)
{
	return 0;
}

int apic_send_init_ipi(unsigned cpu, phys_bytes trampoline)
{
	return 0;
}

unsigned int apicid(void)
{
	return 0;
}

void ioapic_set_id(u32_t addr, unsigned int id)
{
	/* Not implemented */
}
#else
int apic_single_cpu_init(void)
{
	return 0;  /* APIC init failed (legacy PIC used) */
}
#endif

void lapic_set_timer_periodic(const unsigned freq)
{
	/* Not implemented */
}

void lapic_set_timer_one_shot(const u32_t value)
{
	/* Not implemented */
}

void lapic_stop_timer(void)
{
	/* Not implemented */
}

void lapic_restart_timer(void)
{
	/* Not implemented */
}

void ioapic_set_irq(unsigned irq)
{
	/* Not implemented */
}

void ioapic_unset_irq(unsigned irq)
{
	/* Not implemented */
}

void ioapic_eoi(int irq)
{
	/* Not implemented */
}

void apic_send_ipi(unsigned vector, unsigned cpu, int type)
{
	/* Not implemented */
}

void apic_ipi_sched_intr(void)
{
	/* Not implemented */
}

void apic_ipi_halt_intr(void)
{
	/* Not implemented */
}
