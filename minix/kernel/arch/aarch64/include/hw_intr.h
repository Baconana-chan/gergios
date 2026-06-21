/* ============================================================
 * hw_intr.h — ARM64 hardware interrupt interface
 *
 * Declares the hardware-dependent interrupt functions used by
 * the kernel's generic interrupt.c layer, plus the BSP-level
 * interrupt handler called from mpx.S assembly.
 *
 * The GICv3 implementation lives in hw_intr.c.
 *
 * Phase 2: GICv3 for QEMU virt platform.
 * ============================================================ */

#ifndef _AARCH64_HW_INTR_H_
#define _AARCH64_HW_INTR_H_

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
 * BSP interrupt handler (called from mpx.S assembly)
 *
 * bsp_irq_handle() reads the GIC IAR to determine the interrupt
 * number, calls irq_handle() from generic interrupt.c, then writes
 * the EOI to signal completion.
 *
 * Called from:
 *   - arm64_irq_entry_from_user (mpx.S)
 *   - arm64_irq_entry_from_kernel (mpx.S)
 * ----------------------------------------------------------------- */

void bsp_irq_handle(void);

#endif /* _AARCH64_HW_INTR_H_ */
