/* ============================================================
 * direct_tty_utils.c — ARM64 emergency serial I/O
 *
 * Provides direct_cls(), direct_print(), direct_print_char()
 * for use by main.c during shutdown/panic, when normal printf()
 * and console drivers may not be available.
 *
 * Uses PL011 UART at physical address 0x09000000 (QEMU virt
 * default). During early boot and shutdown, the identity map
 * is active, so physical addresses work directly.
 *
 * References:
 *   ARM DDI 0183G — PL011 Technical Reference Manual
 * ============================================================ */

#include "kernel/kernel.h"
#include "direct_utils.h"

/* =========================================================================
 * PL011 UART constants (QEMU virt)
 * ========================================================================= */

#define PL011_BASE              0x09000000UL
#define PL011_DR                (PL011_BASE + 0x000)   /* Data Register (R/W) */
#define PL011_FR                (PL011_BASE + 0x018)   /* Flag Register (RO) */

/* PL011 flag register bits */
#define PL011_FR_TXFF           (1U << 5)              /* TX FIFO Full */

/* =========================================================================
 * PL011 MMIO accessors
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
 * ========================================================================= */

static void pl011_putc(char c)
{
	/* Handle newline: LF -> CR+LF translation */
	if (c == '\n') {
		while (pl011_read(PL011_FR) & PL011_FR_TXFF) { /* Spin */ }
		pl011_write(PL011_DR, '\r');
	}

	/* Write the character */
	while (pl011_read(PL011_FR) & PL011_FR_TXFF) { /* Spin */ }
	pl011_write(PL011_DR, (uint32_t)(unsigned char)c);
}

/* =========================================================================
 * pl011_puts — Write a null-terminated string to PL011 UART
 * ========================================================================= */

static void pl011_puts(const char *str)
{
	while (*str != '\0') {
		pl011_putc(*str++);
	}
}

/* =========================================================================
 * direct_cls — Clear screen (no-op on serial console)
 * ========================================================================= */

void direct_cls(void)
{
	/* No-op: serial console doesn't support screen clearing. */
}

/* =========================================================================
 * direct_print_char — Print a single character via UART
 * ========================================================================= */

void direct_print_char(char c)
{
	pl011_putc(c);
}

/* =========================================================================
 * direct_print — Print a string directly to UART (emergency output)
 * ========================================================================= */

void direct_print(const char *s)
{
	pl011_puts(s);
}
