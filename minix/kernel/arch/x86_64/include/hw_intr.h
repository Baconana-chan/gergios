/* ============================================================
 * hw_intr.h — x86_64 hardware interrupt interface
 *
 * Declares the hardware-dependent interrupt functions used by
 * the kernel's generic interrupt.c layer.
 *
 * When USE_APIC is defined, all interrupt routing goes through
 * the IOAPIC (hw_intr_mask/unmask = ioapic_mask/unmask,
 * hw_intr_ack = ioapic_eoi). Otherwise, the legacy 8259 PIC
 * is used.
 * ============================================================ */

#ifndef _X86_64_HW_INTR_H_
#define _X86_64_HW_INTR_H_

#include "kernel/kernel.h"

/* 8259 PIC functions (defined in i8259.c and klib.S) */
void irq_8259_unmask(int irq);
void irq_8259_mask(int irq);
void irq_8259_eoi(int irq);
void irq_handle(int irq);
void i8259_disable(void);
void eoi_8259_master(void);
void eoi_8259_slave(void);

/* -----------------------------------------------------------------
 * Hardware interrupt abstraction
 * -----------------------------------------------------------------
 *
 * When USE_APIC is defined, APIC/IOAPIC functions are used.
 * Otherwise, the legacy 8259 PIC is used.
 */

#if defined(USE_APIC)

#include "kernel/arch/x86_64/apic.h"

#define hw_intr_mask(irq)		ioapic_mask_irq(irq)
#define hw_intr_unmask(irq)		ioapic_unmask_irq(irq)
#define hw_intr_ack(irq)		ioapic_eoi(irq)
#define hw_intr_used(irq)	do {					\
					if (ioapic_enabled)		\
						ioapic_set_irq(irq);	\
				} while (0)
#define hw_intr_not_used(irq)	do {					\
					if (ioapic_enabled)		\
						ioapic_unset_irq(irq);	\
				} while (0)
#define hw_intr_disable_all() do {					\
					ioapic_disable_all();		\
					ioapic_reset_pic();		\
					lapic_disable();		\
				} while (0)

#else
/* Legacy 8259 PIC mode */

void hw_intr_mask(int irq);
void hw_intr_unmask(int irq);
void hw_intr_ack(int irq);
void hw_intr_used(int irq);
void hw_intr_not_used(int irq);
void hw_intr_disable_all(void);

#endif /* USE_APIC */

#endif /* _X86_64_HW_INTR_H_ */
