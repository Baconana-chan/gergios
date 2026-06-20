/* ============================================================
 * arch_reset.c — ARM64 system reset, shutdown, and serial I/O
 *
 * Implements architecture-specific shutdown and serial output
 * for the ARM64 port. System reset uses PSCI (Power State
 * Coordination Interface) via HVC #0, which is the standard
 * mechanism on ARM64 platforms including QEMU virt.
 *
 * Serial output uses PL011 UART directly at 0x09000000
 * (QEMU virt default address).
 *
 * References:
 *   ARM DEN 0022D — PSCI Specification
 *   ARM DDI 0487 — ARM Architecture Reference Manual
 * ============================================================ */

#include "kernel/kernel.h"

#include <unistd.h>
#include <ctype.h>
#include <string.h>
#include <assert.h>
#include <signal.h>
#include <machine/vm.h>
#include <sys/reboot.h>

#include <minix/board.h>
#include <minix/type.h>
#include <minix/u64.h>

#include "arch_proto.h"
#include "hw_intr.h"
#include "kernel/proc.h"
#include "kernel/debug.h"

/* =========================================================================
 * PL011 UART constants (QEMU virt)
 *
 * The PL011 UART is at physical address 0x09000000 on QEMU virt.
 * During early boot (identity mapping active), physical addresses
 * work directly. After VM enable (Phase 3), this will need a kernel
 * virtual mapping.
 *
 * FIXME (Phase 3): This uses physical addresses directly. Once VM
 * is enabled, the PL011 registers must be accessed through a kernel
 * virtual mapping established via kern_req_phys_map().
 * ========================================================================= */

#define PL011_BASE              0x09000000UL
#define PL011_DR                (PL011_BASE + 0x000)   /* Data Register (R/W) */
#define PL011_FR                (PL011_BASE + 0x018)   /* Flag Register (RO) */
#define PL011_IBRD              (PL011_BASE + 0x024)   /* Integer Baud Rate */
#define PL011_FBRD              (PL011_BASE + 0x028)   /* Fractional Baud Rate */
#define PL011_LCRH              (PL011_BASE + 0x02C)   /* Line Control */
#define PL011_CR                (PL011_BASE + 0x030)   /* Control Register */
#define PL011_IMSC              (PL011_BASE + 0x038)   /* Interrupt Mask */
#define PL011_ICR               (PL011_BASE + 0x044)   /* Interrupt Clear */

/* PL011 flag register bits */
#define PL011_FR_TXFF           (1U << 5)              /* TX FIFO Full */
#define PL011_FR_BUSY           (1U << 3)              /* Transmit Busy */
#define PL011_FR_RXFE           (1U << 4)              /* RX FIFO Empty */

/* PL011 control register bits */
#define PL011_CR_UARTEN         (1U << 0)              /* UART Enable */
#define PL011_CR_TXE            (1U << 8)              /* Transmit Enable */
#define PL011_CR_RXE            (1U << 9)              /* Receive Enable */

/* PL011 line control bits */
#define PL011_LCRH_FEN          (1U << 4)              /* FIFO Enable */
#define PL011_LCRH_WLEN_8B      (3U << 5)              /* 8-bit word length */

/* =========================================================================
 * PL011 MMIO accessors
 *
 * These use volatile pointers for MMIO access. During early boot
 * (identity mapping), the physical addresses are directly accessible.
 * ========================================================================= */

static inline uint32_t pl011_read(uint32_t reg)
{
	return *(volatile uint32_t *)(reg);
}

static inline void pl011_write(uint32_t reg, uint32_t val)
{
	*(volatile uint32_t *)(reg) = val;
}

/* =========================================================================
 * pl011_putc — Write a single character to PL011 UART
 *
 * Waits for the TX FIFO to have space, then writes the character.
 * CR+LF translation: if writing LF, also sends preceding CR.
 *
 * Parameters:
 *   c  Character to write.
 * ========================================================================= */

static void pl011_putc(char c)
{
	/* Handle newline: LF -> CR+LF translation */
	if (c == '\n') {
		/* Write CR first with its own TXFF wait */
		while (pl011_read(PL011_FR) & PL011_FR_TXFF) {
			/* Spin */
		}
		pl011_write(PL011_DR, '\r');
	}

	/* Write the character */
	while (pl011_read(PL011_FR) & PL011_FR_TXFF) {
		/* Spin */
	}
	pl011_write(PL011_DR, (uint32_t)(unsigned char)c);
}

/* =========================================================================
 * pl011_puts — Write a null-terminated string to PL011 UART
 *
 * Parameters:
 *   str  Null-terminated string to write.
 * ========================================================================= */

static void pl011_puts(const char *str)
{
	while (*str != '\0') {
		pl011_putc(*str++);
	}
}

/* =========================================================================
 * pl011_init — Initialize PL011 UART
 *
 * Sets up the PL011 UART for 115200 baud, 8N1, with FIFOs enabled.
 * This is called during boot before the full console driver is active.
 *
 * For QEMU virt, the UART is typically pre-configured by QEMU firmware
 * at 115200 baud, so this is a safety initialization to ensure known state.
 *
 * Reference: ARM DDI 0183G (PL011 Technical Reference Manual)
 * ========================================================================= */

void pl011_init(void)
{
	uint32_t cr;

	/* Disable UART before changing configuration */
	cr = pl011_read(PL011_CR);
	pl011_write(PL011_CR, cr & ~(PL011_CR_UARTEN));

	/* Flush TX FIFO by waiting for BUSY to clear */
	while (pl011_read(PL011_FR) & PL011_FR_BUSY) {
		/* Spin */
	}

	/* Clear any pending interrupts */
	pl011_write(PL011_ICR, 0x7FF);

	/* Set baud rate for 115200 with UARTCLK = 24 MHz (QEMU virt default).
	 * Divisor = UARTCLK / (16 * baud) = 24000000 / (16 * 115200) = 13.0208
	 * IBRD = 13, FBRD = 0.0208 * 64 = 1.33 ~= 1 */
	pl011_write(PL011_IBRD, 13);
	pl011_write(PL011_FBRD, 1);

	/* Set line control: 8-bit, FIFO enabled, no parity, 1 stop bit */
	pl011_write(PL011_LCRH, PL011_LCRH_FEN | PL011_LCRH_WLEN_8B);

	/* Re-enable UART with TX and RX */
	pl011_write(PL011_CR, PL011_CR_UARTEN | PL011_CR_TXE | PL011_CR_RXE);

	/* Mask all interrupts (we use polling for emergency output) */
	pl011_write(PL011_IMSC, 0);
}

/* =========================================================================
 * direct_print — Print a string directly to UART (emergency output)
 *
 * Used during shutdown and panic situations when normal printf()
 * may not be available (e.g., interrupts disabled, no VM, etc.).
 *
 * Static: for now only used internally by arch_shutdown() and poweroff().
 * If needed externally (e.g., by exception.c panic handler), make
 * non-static and add a declaration in a shared header.
 * ========================================================================= */

static void direct_print(const char *s)
{
	pl011_puts(s);
}

/* =========================================================================
 * halt_cpu() and reset() are implemented in klib.S:
 *
 *   halt_cpu:  MSR DAIFSet, #0xF + WFI loop
 *   reset:     PSCI SYSTEM_RESET (0x84000009) via HVC #0, SMC #0 fallback
 *
 * Declarations in arch_proto.h provide C access.
 * ========================================================================= */

/* =========================================================================
 * PSCI HVC/SMC helpers
 *
 * PSCI requires the function ID in x0 when issuing HVC #0 or SMC #0.
 * We use `register ... asm("x0")` to guarantee x0 assignment.
 * The clobber list includes caller-saved registers that HVC/SMC
 * may modify (x0-x17), as well as memory for ordering.
 * ========================================================================= */

/* Invoke PSCI function via HVC. Returns x0 (return value). */
static uint64_t psci_hvc_call(uint64_t function_id)
{
	register uint64_t fn asm("x0") = function_id;

	__asm__ volatile(
		"hvc #0"
		: "+r"(fn)
		:
		: "x1", "x2", "x3", "x4", "x5", "x6", "x7",
		  "x8", "x9", "x10", "x11", "x12", "x13", "x14", "x15",
		  "x16", "x17", "memory"
	);

	return fn;
}

/* Invoke PSCI function via SMC. Returns x0 (return value). */
static uint64_t psci_smc_call(uint64_t function_id)
{
	register uint64_t fn asm("x0") = function_id;

	__asm__ volatile(
		"smc #0"
		: "+r"(fn)
		:
		: "x1", "x2", "x3", "x4", "x5", "x6", "x7",
		  "x8", "x9", "x10", "x11", "x12", "x13", "x14", "x15",
		  "x16", "x17", "memory"
	);

	return fn;
}

/* =========================================================================
 * poweroff — Power off the system via PSCI SYSTEM_OFF
 *
 * Calls PSCI SYSTEM_OFF function (0x84000008) via HVC #0.
 * Falls back to SMC #0 if HVC doesn't work.
 * If both fail, halts the CPU in a WFI loop.
 * ========================================================================= */

static void poweroff(void)
{
	/* PSCI SYSTEM_OFF function ID (Arm Architecture Call, 64-bit) */
	uint64_t psci_function_id = 0x84000008UL;

	/* Attempt HVC call first (standard for PSCI on ARM64).
	 * If successful, this call does not return. */
	psci_hvc_call(psci_function_id);

	/* Fallback: SMC call (for EL3 firmware).
	 * x0 is explicitly reloaded by psci_smc_call(). */
	psci_smc_call(psci_function_id);

	/* If both fail, print message and halt */
	direct_print("poweroff: PSCI SYSTEM_OFF failed, halting.\r\n");

	/* Fall through to infinite halt */
	for (;;) {
		halt_cpu();
	}
}

/* =========================================================================
 * arch_shutdown — Architecture-specific shutdown
 *
 * Called by the kernel during system shutdown (from minix_shutdown()
 * in main.c after prepare_shutdown() has completed).
 *
 * Parameters:
 *   how  Bitmask of RB_* flags from <sys/reboot.h>:
 *        RB_POWERDOWN — Power off
 *        RB_HALT      — Halt (wait for interrupt)
 *        RB_RESET     — Reset (default)
 *
 * This function never returns (marked _Noreturn in proto.h).
 * ========================================================================= */

void arch_shutdown(int how)
{
	/* Print shutdown message via direct UART access.
	 * Normal printf() may not be safe at this point. */
	direct_print("\r\nMINIX ARM64: shutdown requested.\r\n");

	if ((how & RB_POWERDOWN) == RB_POWERDOWN) {
		direct_print("Powering off...\r\n");
		poweroff();
		/* NOTREACHED */
	}

	if (how & RB_HALT) {
		direct_print("Halted.\r\n");
		for (;;) {
			halt_cpu();
		}
		/* NOTREACHED */
	}

	/* Default: Reset the system via PSCI SYSTEM_RESET */
	direct_print("Resetting...\r\n");
	reset();

	/* If reset returns (failed), halt */
	for (;;) {
		halt_cpu();
	}
}

/* =========================================================================
 * ser_putc — Serial output character (debug)
 *
 * Called by the kernel's debug serial subsystem when
 * DEBUG_SERIAL is enabled. Outputs a single character
 * directly to the PL011 UART.
 *
 * Parameters:
 *   c  Character to output.
 * ========================================================================= */

#ifdef DEBUG_SERIAL
void ser_putc(char c)
{
	pl011_putc(c);
}
#endif /* DEBUG_SERIAL */
