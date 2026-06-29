/* ============================================================
 * hw_intr.h — x86_64 hardware interrupt interface
 *
 * Declares the hardware-dependent interrupt functions used by
 * the kernel's generic interrupt.c layer, plus the BSP-level
 * interrupt handler.
 *
 * On x86_64, interrupts are routed through the 8259 PIC or
 * APIC (I/O APIC + local APIC), with interrupt gate entries
 * in the IDT that dispatch to hwint00-hwint15 handlers.
 * ============================================================ */

#ifndef _X86_64_HW_INTR_H_
#define _X86_64_HW_INTR_H_

#include "kernel/kernel.h"

/* -----------------------------------------------------------------
 * Kernel interrupt interface (called from interrupt.c)
 * ----------------------------------------------------------------- */

void hw_intr_mask(int irq);
void hw_intr_unmask(int irq);
void hw_intr_ack(int irq);
void hw_intr_used(int irq);
void hw_intr_not_used(int irq);
void hw_intr_disable_all(void);

/* -----------------------------------------------------------------
 * Generic interrupt dispatcher (declared in kernel/interrupt.c)
 * ----------------------------------------------------------------- */

void irq_handle(int irq);

/* -----------------------------------------------------------------
 * BSP interrupt handler
 *
 * On x86_64, bsp_irq_handle() is called from the assembly-level
 * interrupt stubs (hwint00-hwint15 in mpx.S) to dispatch the
 * interrupt to the generic interrupt.c handler.
 *
 * For PIC mode, it acknowledges the 8259 master/slave.
 * For APIC mode, it writes the EOI to the local APIC.
 * ----------------------------------------------------------------- */

void bsp_irq_handle(void);

/* -----------------------------------------------------------------
 * 8259 PIC End-Of-Interrupt (defined in klib.S)
 * ----------------------------------------------------------------- */

void eoi_8259_master(void);
void eoi_8259_slave(void);

#endif /* _X86_64_HW_INTR_H_ */
