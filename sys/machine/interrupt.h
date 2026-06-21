/*	$NetBSD$	*/
/* Interrupt handling — stub for AArch64 (minimal placeholder). */

#ifndef _MACHINE_INTERRUPT_H_
#define _MACHINE_INTERRUPT_H_

/* AArch64 interrupt constants. */
/* IRQ numbers and interrupt controller setup are defined by the board/SoC. */

/* Number of IRQ vectors for GICv3 (256 SPIs + 32 PPIs + 16 SGIs) */
#define NR_IRQ_VECTORS    256

#endif /* _MACHINE_INTERRUPT_H_ */
