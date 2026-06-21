/* Interrupt numbers and hardware vectors. */

#ifndef _AARCH64_INTERRUPT_H
#define _AARCH64_INTERRUPT_H

/* ARM64 GICv3 supports up to 256 SPIs, but for MINIX we use
 * a generous default matching the earm value. */
#define NR_IRQ_VECTORS    256

#endif /* _AARCH64_INTERRUPT_H */
