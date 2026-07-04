#ifndef __APIC_X86_64_H__
#define __APIC_X86_64_H__

/* APIC register constants — shared with i386, same for x86_64. */

#define APIC_ENABLE		0x100
#define APIC_FOCUS_DISABLED	(1 << 9)
#define APIC_SIV		0xFF

#define APIC_TDCR_2	0x00
#define APIC_TDCR_4	0x01
#define APIC_TDCR_8	0x02
#define APIC_TDCR_16	0x03
#define APIC_TDCR_32	0x08
#define APIC_TDCR_64	0x09
#define APIC_TDCR_128	0x0a
#define APIC_TDCR_1	0x0b

#define APIC_LVTT_VECTOR_MASK	0x000000FF
#define APIC_LVTT_DS_PENDING	(1 << 12)
#define APIC_LVTT_MASK		(1 << 16)
#define APIC_LVTT_TM		(1 << 17)

#define APIC_LVT_IIPP_MASK	0x00002000
#define APIC_LVT_IIPP_AH	0x00002000
#define APIC_LVT_IIPP_AL	0x00000000

#define IOAPIC_REGSEL		0x0
#define IOAPIC_RW		0x10

#define APIC_ICR_DM_MASK		0x00000700
#define APIC_ICR_VECTOR			APIC_LVTT_VECTOR_MASK
#define APIC_ICR_DM_FIXED		(0 << 8)
#define APIC_ICR_DM_LOWEST_PRIORITY	(1 << 8)
#define APIC_ICR_DM_SMI		(2 << 8)
#define APIC_ICR_DM_RESERVED		(3 << 8)
#define APIC_ICR_DM_NMI		(4 << 8)
#define APIC_ICR_DM_INIT		(5 << 8)
#define APIC_ICR_DM_STARTUP		(6 << 8)
#define APIC_ICR_DM_EXTINT		(7 << 8)

#define APIC_ICR_DM_PHYSICAL		(0 << 11)
#define APIC_ICR_DM_LOGICAL		(1 << 11)

#define APIC_ICR_DELIVERY_PENDING	(1 << 12)

#define APIC_ICR_INT_POLARITY		(1 << 13)

#define APIC_ICR_LEVEL_ASSERT		(1 << 14)
#define APIC_ICR_LEVEL_DEASSERT		(0 << 14)

#define APIC_ICR_TRIGGER		(1 << 15)

#define APIC_ICR_INT_MASK		(1 << 16)

#define APIC_ICR_DEST_FIELD		(0 << 18)
#define APIC_ICR_DEST_SELF		(1 << 18)
#define APIC_ICR_DEST_ALL		(2 << 18)
#define APIC_ICR_DEST_ALL_BUT_SELF	(3 << 18)

#define LOCAL_APIC_DEF_ADDR	0xfee00000 /* default local apic address */
#define IO_APIC_DEF_ADDR	0xfec00000 /* default i/o apic address */

/* LAPIC register offsets (relative to lapic_addr) */
#define LAPIC_ID	(lapic_addr + 0x020)
#define LAPIC_VERSION	(lapic_addr + 0x030)
#define LAPIC_TPR	(lapic_addr + 0x080)
#define LAPIC_EOI	(lapic_addr + 0x0b0)
#define LAPIC_LDR	(lapic_addr + 0x0d0)
#define LAPIC_DFR	(lapic_addr + 0x0e0)
#define LAPIC_SIVR	(lapic_addr + 0x0f0)
#define LAPIC_ISR	(lapic_addr + 0x100)
#define LAPIC_TMR	(lapic_addr + 0x180)
#define LAPIC_IRR	(lapic_addr + 0x200)
#define LAPIC_ESR	(lapic_addr + 0x280)
#define LAPIC_ICR1	(lapic_addr + 0x300)
#define LAPIC_ICR2	(lapic_addr + 0x310)
#define LAPIC_LVTTR	(lapic_addr + 0x320)
#define LAPIC_LVTTMR	(lapic_addr + 0x330)
#define LAPIC_LVTPCR	(lapic_addr + 0x340)
#define LAPIC_LINT0	(lapic_addr + 0x350)
#define LAPIC_LINT1	(lapic_addr + 0x360)
#define LAPIC_LVTER	(lapic_addr + 0x370)
#define LAPIC_TIMER_ICR	(lapic_addr + 0x380)
#define LAPIC_TIMER_CCR	(lapic_addr + 0x390)
#define LAPIC_TIMER_DCR	(lapic_addr + 0x3e0)

/* IOAPIC register offsets */
#define IOAPIC_ID		0x0
#define IOAPIC_VERSION		0x1
#define IOAPIC_ARB		0x2
#define IOAPIC_REDIR_TABLE	0x10

/* APIC interrupt vector assignments */
#define APIC_TIMER_INT_VECTOR		0xf0
#define APIC_SMP_SCHED_PROC_VECTOR	0xf1
#define APIC_SMP_CPU_HALT_VECTOR	0xf2
#define APIC_ERROR_INT_VECTOR		0xfe
#define APIC_SPURIOUS_INT_VECTOR	0xff

/* MSR constants for APIC base register */
#define IA32_APIC_BASE		0x1b
#define IA32_APIC_BASE_ENABLE_BIT	11
#define IA32_APIC_BASE_XAPIC_ENABLE	(1 << 10)

#ifndef __ASSEMBLY__

#include "kernel/kernel.h"

/* LAPIC base address (virtual, after mapping). */
EXTERN vir_bytes lapic_addr;
/* LAPIC EOI register virtual address. */
EXTERN vir_bytes lapic_eoi_addr;
/* Whether IOAPIC is enabled. */
EXTERN int ioapic_enabled;
/* BSP local APIC ID. */
EXTERN int bsp_lapic_id;
EXTERN u32_t lapic_addr_vaddr;

#define MAX_NR_IOAPICS		32

struct io_apic {
	unsigned	id;
	vir_bytes	addr;		/* presently used address */
	phys_bytes	paddr;		/* physical address */
	vir_bytes	vaddr;		/* address after paging is on */
	unsigned	pins;
	unsigned	gsi_base;
};

EXTERN struct io_apic io_apic[MAX_NR_IOAPICS];
EXTERN unsigned nioapics;

/* LAPIC enable/disable */
int lapic_enable(unsigned cpu);
void lapic_disable(void);

/* IOAPIC initialization */
int detect_ioapics(void);
int ioapic_enable_all(void);
void ioapic_disable_all(void);
void ioapic_reset_pic(void);

/* IOAPIC IRQ routing */
void ioapic_set_irq(unsigned irq);
int ioapic_set_irq_affinity(unsigned irq, unsigned cpu);
void ioapic_unset_irq(unsigned irq);
void ioapic_mask_irq(unsigned irq);
void ioapic_unmask_irq(unsigned irq);

/* EOI */
void ioapic_eoi(int irq);
void arch_eoi(void);
#define apic_eoi() do { *((volatile u32_t *) lapic_eoi_addr) = 0; } while(0)

/* APIC IDT setup */
void apic_idt_init(int reset);

/* LAPIC timer */
void lapic_set_timer_one_shot(const u32_t usec);
void lapic_set_timer_periodic(const unsigned freq);
void lapic_stop_timer(void);
void lapic_restart_timer(void);
void lapic_microsec_sleep(unsigned count);

/* IPI */
void apic_send_ipi(unsigned vector, unsigned cpu, int type);

#define APIC_IPI_DEST		0
#define APIC_IPI_SELF		1
#define APIC_IPI_TO_ALL		2
#define APIC_IPI_TO_ALL_BUT_SELF 3

#define apic_send_ipi_single(vector, cpu) \
	apic_send_ipi(vector, cpu, APIC_IPI_DEST)
#define apic_send_ipi_self(vector) \
	apic_send_ipi(vector, 0, APIC_IPI_SELF)
#define apic_send_ipi_all(vector) \
	apic_send_ipi(vector, 0, APIC_IPI_TO_ALL)
#define apic_send_ipi_allbutself(vector) \
	apic_send_ipi(vector, 0, APIC_IPI_TO_ALL_BUT_SELF)

/* Assembly interrupt entry points (from apic_asm.S) */
void apic_hwint0(void);
void apic_hwint1(void);
void apic_hwint2(void);
void apic_hwint3(void);
void apic_hwint4(void);
void apic_hwint5(void);
void apic_hwint6(void);
void apic_hwint7(void);
void apic_hwint8(void);
void apic_hwint9(void);
void apic_hwint10(void);
void apic_hwint11(void);
void apic_hwint12(void);
void apic_hwint13(void);
void apic_hwint14(void);
void apic_hwint15(void);
void apic_hwint16(void);
void apic_hwint17(void);
void apic_hwint18(void);
void apic_hwint19(void);
void apic_hwint20(void);
void apic_hwint21(void);
void apic_hwint22(void);
void apic_hwint23(void);
void apic_hwint24(void);
void apic_hwint25(void);
void apic_hwint26(void);
void apic_hwint27(void);
void apic_hwint28(void);
void apic_hwint29(void);
void apic_hwint30(void);
void apic_hwint31(void);
void apic_hwint32(void);
void apic_hwint33(void);
void apic_hwint34(void);
void apic_hwint35(void);
void apic_hwint36(void);
void apic_hwint37(void);
void apic_hwint38(void);
void apic_hwint39(void);
void apic_hwint40(void);
void apic_hwint41(void);
void apic_hwint42(void);
void apic_hwint43(void);
void apic_hwint44(void);
void apic_hwint45(void);
void apic_hwint46(void);
void apic_hwint47(void);
void apic_hwint48(void);
void apic_hwint49(void);
void apic_hwint50(void);
void apic_hwint51(void);
void apic_hwint52(void);
void apic_hwint53(void);
void apic_hwint54(void);
void apic_hwint55(void);
void apic_hwint56(void);
void apic_hwint57(void);
void apic_hwint58(void);
void apic_hwint59(void);
void apic_hwint60(void);
void apic_hwint61(void);
void apic_hwint62(void);
void apic_hwint63(void);
void apic_spurios_intr(void);
void apic_error_intr(void);
void lapic_timer_int_handler(void);
void apic_ipi_sched_intr(void);
void apic_ipi_halt_intr(void);

/* SMP IPI handlers (C functions) */
void apic_spurios_intr_handler(void);
void apic_error_intr_handler(void);
void smp_ipi_sched_handler(void);
void smp_ipi_halt_handler(void);

/* APIC interrupt state dump (used by arch_system.c watchdog) */
void dump_apic_irq_state(void);

/* APIC single-CPU init (used when CONFIG_SMP is not enabled) */
int apic_single_cpu_init(void);

#ifdef CONFIG_SMP
/* AP startup IPIs */
int apic_send_init_ipi(unsigned cpu, phys_bytes trampoline);
int apic_send_startup_ipi(unsigned cpu, phys_bytes trampoline);
#endif

/* Convenience macros */
#define cpu_feature_apic_on_chip() _cpufeature(_CPUF_I386_APIC_ON_CHIP)

/* LAPIC MMIO access macros.
 * All LAPIC/IOAPIC registers are 32-bit, even on x86_64. */
#define lapic_read(what)	(*((volatile u32_t *)((what))))
#define lapic_write(what, data) do {				\
	(*((volatile u32_t *)((what)))) = data;			\
} while(0)

#endif /* __ASSEMBLY__ */

#endif /* __APIC_X86_64_H__ */
