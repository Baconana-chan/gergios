/* MSI-X support for MINIX kernel.
 *
 * MSI-X (PCI Express Message Signaled Interrupts eXtended) allows PCIe
 * devices to send interrupts directly to the Local APIC without going
 * through the I/O APIC. Each MSI-X entry in the device's PCI BAR holds
 * a message address (LAPIC write address) and message data (vector
 * number + delivery mode).
 *
 * This module provides:
 *   - MSI-X IRQ allocation (from a dedicated pool above legacy IRQs)
 *   - MSI-X message address/data computation
 *   - Integration with the existing irq_hook mechanism
 *
 * IRQ namespace:
 *   0 .. NR_IRQ_VECTORS-1  — legacy IOAPIC IRQs (routed via IOAPIC)
 *   MSIX_IRQ_BASE .. MSIX_IRQ_BASE+NR_MSIX_IRQS-1 — MSI-X IRQs (direct LAPIC)
 */

#ifndef _MSIX_H
#define _MSIX_H

#include <minix/com.h>  /* for NR_MSIX_IRQS, MSIX_IRQ_BASE */

/* MSI-X message address (must be written to device's MSI-X table entry).
 *
 * Standard format for x86 LAPIC:
 *   Bits 31:20 = 0xFEE (LAPIC memory-mapped base >> 12)
 *   Bits 19:12 = destination APIC ID
 *   Bit  3     = destination mode (0 = physical)
 *   Bit  2     = redirection hint (0 = use destination)
 *   Rest       = 0
 */
#define MSIX_MESSAGE_ADDR(apic_id) \
    (0xFEE00000u | ((apic_id & 0xFF) << 12))

/* MSI-X message data (must be written to device's MSI-X table entry).
 *
 * Standard format:
 *   Bits 7:0  = vector (IRQ0_VECTOR + irq)
 *   Bits 10:8 = delivery mode (000 = fixed)
 *   Bit  11   = redirection hint (0 = logical APIC ID)
 *   Bit  14   = trigger mode (0 = edge)
 *   Bit  15   = level (0 = deassert)
 */
#define MSIX_MESSAGE_DATA(irq) \
    ((unsigned)(IRQ0_VECTOR + (irq)))

/* Test if an IRQ number is in the MSI-X range. */
#define IS_MSIX_IRQ(irq) \
    ((irq) >= MSIX_IRQ_BASE && (irq) < MSIX_IRQ_BASE + NR_MSIX_IRQS)

/* MSI-X IRQ number from index (0 .. NR_MSIX_IRQS-1) */
#define MSIX_IRQ(idx)  (MSIX_IRQ_BASE + (idx))

#ifndef __ASSEMBLY__

/* MSI-X IRQ allocation bitmap — one bit per MSI-X slot. */
#define MSIX_BITMAP_WORDS  ((NR_MSIX_IRQS + 31) / 32)

/* Allocate a free MSI-X IRQ. Returns the IRQ number, or -1 if none free. */
int msix_alloc_irq(void);

/* Free a previously allocated MSI-X IRQ. */
void msix_free_irq(int irq);

/* Check if an IRQ is MSI-X allocated. */
int msix_is_allocated(int irq);

/* Register an IRQ handler for an MSI-X vector (skips IOAPIC).
 * Defined in kernel/interrupt.c. Declared here only when irq_hook_t is
 * available (included from kernel/kernel.h via interrupt.h).
 */
#ifdef TYPE_H
void put_msix_handler(irq_hook_t *hook, int irq,
    const irq_handler_t handler);
#endif

#endif /* !__ASSEMBLY__ */

#endif /* _MSIX_H */
