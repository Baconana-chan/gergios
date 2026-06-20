/* ============================================================
 * hw_intr.c — ARM64 GICv3 interrupt controller driver
 *
 * Implements the hardware-dependent interrupt functions for the
 * ARM Generic Interrupt Controller v3 (GICv3) as used on QEMU
 * virt platform.
 *
 * GICv3 memory map (QEMU virt, default):
 *   GICD (Distributor):  0x08000000
 *   GICR (Redistributor): 0x080A0000  (CPU0)
 *
 * GICv3 CPU interface is accessed via system registers:
 *   ICC_IAR1_EL1    — Interrupt Acknowledge Register
 *   ICC_EOIR1_EL1   — End of Interrupt Register
 *   ICC_PMR_EL1     — Priority Mask Register
 *   ICC_IGRPEN1_EL1 — Group 1 Enable Register
 *   ICC_SRE_EL1     — System Register Enable
 *
 * Phase 2: Basic GICv3 operation — mask, unmask, ack, dispatch.
 * Phase 4+: SMP, MSI, interrupt affinity, proper priority management.
 * ============================================================ */

#include "hw_intr.h"
#include "kernel/kernel.h"
#include "kernel/proc.h"
#include "kernel/vm.h"

#include <minix/board.h>
#include <minix/type.h>
#include <minix/com.h>

/* =========================================================================
 * GICv3 register definitions (QEMU virt)
 * ========================================================================= */

/* Physical base addresses (QEMU virt, default GICv3) */
#define GICD_BASE               0x08000000UL   /* Distributor */
#define GICR_BASE               0x080A0000UL   /* Redistributor (CPU0) */

/* ---- Distributor registers (offset from GICD_BASE) ---- */
#define GICD_CTLR               0x0000          /* Distributor Control */
#define  GICD_CTLR_ENABLE       (1U << 0)       /* Enable distributor */

#define GICD_TYPER              0x0004          /* Distributor Type */
#define  GICD_TYPER_ITLINES(x)  (((x) & 0x1F) + 1) /* Number of IRQ lines / 32 */

#define GICD_ISENABLER(n)       (0x0100 + (n) * 4)  /* Interrupt Set-Enable, n = irq/32 */
#define GICD_ICENABLER(n)       (0x0180 + (n) * 4)  /* Interrupt Clear-Enable */

#define GICD_ICPEND(n)          (0x0280 + (n) * 4)  /* Interrupt Clear-Pending */

/* ---- Redistributor registers (offset from GICR_BASE) ---- */
/*
 * GICR frame layout per CPU (QEMU virt):
 *   +0x00000: GICR_SGI_BASE  — SGI (0-15) and PPI (16-31) control
 *   +0x10000: GICR_VLPI_BASE — VLPI (virtual LPI) control
 *
 * Each frame is 64KB. We use only the SGI/PPI frame here.
 */
#define GICR_ISENABLER(n)       (0x10000 + 0x0100 + (n) * 4)  /* SGI+PPI Set-Enable */
#define GICR_ICENABLER(n)       (0x10000 + 0x0180 + (n) * 4)  /* SGI+PPI Clear-Enable */
#define GICR_ICPEND(n)          (0x10000 + 0x0280 + (n) * 4)  /* SGI+PPI Clear-Pending */

/* ---- CPU interface system registers ---- */
#define ICC_PMR_EL1             "ICC_PMR_EL1"           /* S3_0_C4_C6_0 */
#define ICC_SRE_EL1             "ICC_SRE_EL1"           /* S3_0_C12_C12_5 */
#define ICC_IGRPEN1_EL1         "ICC_IGRPEN1_EL1"       /* S3_0_C12_C12_7 */
#define ICC_IAR1_EL1            "ICC_IAR1_EL1"          /* S3_0_C12_C12_0 */
#define ICC_EOIR1_EL1           "ICC_EOIR1_EL1"         /* S3_0_C12_C12_1 */

/* ICC_SRE_EL1 fields */
#define ICC_SRE_SRE             (1U << 0)       /* Enable system register interface */
#define ICC_SRE_DIB             (1U << 1)       /* Disable IRQ bypass */
#define ICC_SRE_DFB             (1U << 2)       /* Disable FIQ bypass */

/* Interrupt ID ranges */
#define INTID_SGI               0               /* Software Generated Interrupt (0-15) */
#define INTID_PPI               16              /* Private Peripheral Interrupt (16-31) */
#define INTID_SPI               32              /* Shared Peripheral Interrupt (32-1019) */
#define INTID_IRQ_BASE          32              /* First SPI */

/* Special interrupt IDs */
#define INTID_SPURIOUS          1023            /* Spurious interrupt */

/* =========================================================================
 * Barrier macros (ARM64 inline asm)
 *
 * These are used by MMIO accessors for proper ordering.
 * Prefixed with __gic_ to avoid conflicts with potential system-wide
 * barrier definitions added in later phases.
 * ========================================================================= */

#define __gic_dsb()  __asm__ __volatile__("dsb sy" : : : "memory")
#define __gic_isb()  __asm__ __volatile__("isb"   : : : "memory")

/* =========================================================================
 * MMIO access helpers
 *
 * Used to read/write GIC distributor and redistributor registers.
 * The GIC registers must be accessed as 32-bit wide, byte-aligned
 * memory-mapped I/O.
 *
 * FIXME (Phase 3): The GICD_BASE and GICR_BASE below are physical
 * addresses. They work during early boot (identity mapping is active),
 * but will fault after the kernel switches to high VMA page tables.
 * Once VM is enabled, these need to be accessed through a kernel virtual
 * mapping established via kern_req_phys_map() or a fixed high VA
 * reservation. See earm/memory.c arch_phys_map() for reference.
 * ========================================================================= */

static inline u32_t gicd_read32(u32_t offset)
{
	return *(volatile u32_t *)(GICD_BASE + offset);
}

static inline void gicd_write32(u32_t offset, u32_t val)
{
	*(volatile u32_t *)(GICD_BASE + offset) = val;
	__gic_dsb();
}

static inline u32_t gicr_read32(u32_t offset)
{
	return *(volatile u32_t *)(GICR_BASE + offset);
}

static inline void gicr_write32(u32_t offset, u32_t val)
{
	*(volatile u32_t *)(GICR_BASE + offset) = val;
	__gic_dsb();
}

/* =========================================================================
 * System register access helpers
 * ========================================================================= */

static inline u32_t read_icc_iar1(void)
{
	u32_t irq;
	__asm__ volatile("mrs %0, " ICC_IAR1_EL1 : "=r"(irq));
	return irq;
}

static inline void write_icc_eoir1(u32_t irq)
{
	__asm__ volatile("msr " ICC_EOIR1_EL1 ", %0" : : "r"(irq));
	__gic_isb();
}

static inline void write_icc_pmr(u32_t priority)
{
	__asm__ volatile("msr " ICC_PMR_EL1 ", %0" : : "r"(priority));
}

static inline u32_t read_icc_sre(void)
{
	u32_t val;
	__asm__ volatile("mrs %0, " ICC_SRE_EL1 : "=r"(val));
	return val;
}

static inline void write_icc_sre(u32_t val)
{
	__asm__ volatile("msr " ICC_SRE_EL1 ", %0" : : "r"(val));
	__gic_isb();
}

static inline void write_icc_igrpen1(u32_t val)
{
	__asm__ volatile("msr " ICC_IGRPEN1_EL1 ", %0" : : "r"(val));
	__gic_isb();
}

/* =========================================================================
 * GICv3 initialization
 *
 * Called once during boot on the primary CPU.
 * Secondary CPUs will need their own GICR + sysreg init (Phase 4+ SMP).
 * ========================================================================= */

static int gic_initialized = 0;

static void gic_init_cpu(void)
{
	u32_t sre;

	/*
	 * Step 1: Enable system register interface.
	 * GICv3 must be accessed via ICC_* system registers, not via
	 * the legacy GICC memory-mapped interface, for the CPU interface.
	 */
	sre = read_icc_sre();
	sre |= ICC_SRE_SRE | ICC_SRE_DIB | ICC_SRE_DFB;
	write_icc_sre(sre);

	/*
	 * Step 2: Set priority mask to lowest (0xFF).
	 * This allows all interrupts with priority >= 0xFF to be signaled.
	 * 0xFF = lowest priority, 0x00 = highest.
	 */
	write_icc_pmr(0xFF);

	/*
	 * Step 3: Enable Group 1 interrupts.
	 * GICv3 supports two groups: Group 0 (Secure) and Group 1 (Non-Secure).
	 * MINIX runs in Non-Secure EL1, so we use Group 1.
	 */
	write_icc_igrpen1(1);
}

static void gic_init_dist(void)
{
	u32_t typer;
	u32_t num_lines;
	int i;

	/*
	 * Enable the distributor.
	 * GICD_CTLR = 1 enables the distributor for Group 1 (Non-Secure)
	 * and forwards Group 1 interrupts to the CPU interfaces.
	 */
	gicd_write32(GICD_CTLR, GICD_CTLR_ENABLE);
	__gic_dsb();

	/*
	 * Read the number of interrupt lines supported.
	 * GICD_TYPER[4:0] encodes the number of IRQ lines as
	 * (ITLinesNumber + 1) * 32. The minimum is 32.
	 */
	typer = gicd_read32(GICD_TYPER);
	num_lines = GICD_TYPER_ITLINES(typer) * 32;

	printf("GICv3: Distributor enabled, %u interrupt lines\n", num_lines);

	/*
	 * Clear all pending interrupts and disable all SPIs.
	 * This ensures a clean initial state.
	 */
	for (i = INTID_SPI / 32; i < (int)(num_lines / 32); i++) {
		gicd_write32(GICD_ICENABLER(i), 0xFFFFFFFF);
		gicd_write32(GICD_ICPEND(i), 0xFFFFFFFF);
	}
	__gic_dsb();

	/*
	 * Disable SGI/PPI pending bits via the redistributor.
	 * Note: We clear pending but do NOT disable SGI (IRQ 0-15) because
	 * they are used for IPI inter-processor interrupts (SMP, Phase 4+).
	 * We clear PPI (IRQ 16-31) pending and disable them since they are
	 * not used until specifically requested by drivers.
	 *
	 * FIXME (Phase 4+): For SMP, GICR_BASE must be per-CPU, computed
	 * from the MPIDR_EL1 affinity rather than hardcoded to CPU0.
	 */
	/* Clear pending for SGI+PPI */
	gicr_write32(GICR_ICPEND(0), 0xFFFFFFFF);
	/* Disable only PPIs (IRQ 16-31), leave SGIs (IRQ 0-15) enabled */
	gicr_write32(GICR_ICENABLER(0), 0xFFFF0000);
	__gic_dsb();
}

/* =========================================================================
 * intr_init — Initialize the interrupt controller
 *
 * Called from main.c during kernel initialization.
 * Performs one-time GICv3 setup for the distributor and CPU interface.
 *
 * Parameters:
 *   type  Currently ignored; reserved for future use (GIC type).
 *
 * Returns:
 *   OK (0) on success.
 * ========================================================================= */

int intr_init(int type)
{
	/* Suppress unused parameter warning */
	(void)type;

	if (!gic_initialized) {
		gic_init_dist();
		gic_initialized = 1;
	}

	gic_init_cpu();

	return OK;
}

/* =========================================================================
 * hw_intr_mask — Disable (mask) a specific IRQ line
 *
 * Clears the enable bit for the given interrupt in the GIC.
 * After this call, the interrupt will not be forwarded to the CPU.
 *
 * Parameters:
 *   irq  Interrupt request number.
 *
 * Notes:
 *   - For SGIs (0-15): masking is not supported (SGI is always enabled).
 *   - For PPIs (16-31): uses the redistributor (GICR_ICENABLER).
 *   - For SPIs (32+):   uses the distributor (GICD_ICENABLER).
 * ========================================================================= */

void hw_intr_mask(int irq)
{
	int reg = irq / 32;
	u32_t bit = 1U << (irq % 32);

	if (irq < 0)
		return;

	if (irq < INTID_SPI) {
		/* PPI (16-31): use redistributor.
		 * SGI (0-15): cannot be masked via enable bits. */
		if (irq >= INTID_PPI) {
			gicr_write32(GICR_ICENABLER(reg), bit);
		}
	} else {
		/* SPI: use distributor */
		gicd_write32(GICD_ICENABLER(reg), bit);
	}
}

/* =========================================================================
 * hw_intr_unmask — Enable (unmask) a specific IRQ line
 *
 * Sets the enable bit for the given interrupt in the GIC.
 * After this call, the interrupt can be signalled to the CPU.
 *
 * Parameters:
 *   irq  Interrupt request number.
 *
 * Notes:
 *   - For SGIs (0-15): unmasking is not supported (SGI is always enabled).
 *   - For PPIs (16-31): uses the redistributor (GICR_ISENABLER).
 *   - For SPIs (32+):   uses the distributor (GICD_ISENABLER).
 * ========================================================================= */

void hw_intr_unmask(int irq)
{
	int reg = irq / 32;
	u32_t bit = 1U << (irq % 32);

	if (irq < 0)
		return;

	if (irq < INTID_SPI) {
		if (irq >= INTID_PPI) {
			gicr_write32(GICR_ISENABLER(reg), bit);
		}
	} else {
		gicd_write32(GICD_ISENABLER(reg), bit);
	}
}

/* =========================================================================
 * hw_intr_ack — Acknowledge an interrupt
 *
 * For GICv3 with system register interface, this is handled inside
 * bsp_irq_handle() via ICC_EOIR1_EL1. The per-IRQ ack is a no-op
 * since GICv3 uses a single EOIR write per interrupt.
 *
 * Parameters:
 *   irq  Interrupt request number (unused).
 * ========================================================================= */

void hw_intr_ack(int irq)
{
	(void)irq;
	/* GICv3 EOIR is written in bsp_irq_handle() after irq_handle().
	 * Per-IRQ ack is not needed here. */
}

/* =========================================================================
 * hw_intr_used — Called when the first handler is registered for an IRQ
 *
 * GICv3 does not need special handling for first registration beyond
 * the enable that interrupt.c already performs via hw_intr_unmask().
 *
 * Parameters:
 *   irq  Interrupt request number (unused).
 * ========================================================================= */

void hw_intr_used(int irq)
{
	(void)irq;
	/* No special action needed — hw_intr_unmask handles enable. */
}

/* =========================================================================
 * hw_intr_not_used — Called when the last handler is deregistered for an IRQ
 *
 * Cleanup is already done by interrupt.c via hw_intr_mask().
 *
 * Parameters:
 *   irq  Interrupt request number (unused).
 * ========================================================================= */

void hw_intr_not_used(int irq)
{
	(void)irq;
	/* No special action needed — hw_intr_mask handles disable. */
}

/* =========================================================================
 * hw_intr_disable_all — No-op (interrupt critical sections use DAIF)
 *
 * On ARM64, critical sections are managed by the DAIF register
 * (interrupt disable bits), not by the GIC priority mask.
 *
 * The MINIX kernel expects this function to be a lightweight hint;
 * the real interrupt masking happens via intr_disable()/intr_enable()
 * in <minix/portio.h>, which use DAIFSet/DAIFClr.
 *
 * This follows the same pattern as the earm architecture where
 * hw_intr_disable_all is also an empty stub.
 * ========================================================================= */

void hw_intr_disable_all(void)
{
	/* No-op. Use intr_disable()/intr_enable() for critical sections. */
}

/* =========================================================================
 * bsp_irq_handle — Main interrupt handler called from mpx.S
 *
 * This is the assembly entry point for ALL external interrupts
 * (both from EL0 user mode and EL1 kernel mode).
 *
 * Flow:
 *   1. Read ICC_IAR1_EL1 to get the interrupt ID (also performs ack)
 *   2. If spurious (1023), skip handling
 *   3. Call irq_handle(irq) from kernel/interrupt.c
 *   4. Write ICC_EOIR1_EL1 to signal End of Interrupt
 *   5. Data synchronization barrier for MMIO ordering
 *
 * Called from:
 *   - arm64_irq_entry_from_user (mpx.S) — after saving user context
 *   - arm64_irq_entry_from_kernel (mpx.S) — after saving kernel context
 *
 * Note: This function follows the ARM64 calling convention.
 * It may clobber x0-x18 (caller-saved registers).
 * ========================================================================= */

void bsp_irq_handle(void)
{
	u32_t irq;

	/* Read ICC_IAR1_EL1 to get the interrupt ID.
	 * This also performs a priority drop and signals to the GIC
	 * that the interrupt is being handled. The read is ordered
	 * by the GIC hardware. */
	irq = read_icc_iar1();

	/* Check for spurious interrupt (ID 1023). */
	if (irq == INTID_SPURIOUS) {
		return;
	}

	/* Dispatch to the kernel's generic interrupt handler.
	 * irq_handle() calls registered handlers and manages the
	 * enable/mask state via hw_intr_*() functions. */
	irq_handle((int)irq);

	/* Signal End of Interrupt to the GIC CPU interface.
	 * This must happen after all handlers have completed.
	 * A DSB is required before the write if the handler
	 * performed any memory-mapped accesses. */
	__gic_dsb();
	write_icc_eoir1(irq);
}
