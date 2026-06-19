/* ============================================================
 * startup.c — ARM64 kernel C startup
 *
 * Called by head.S after MMU is enabled.
 * Initializes the PL011 UART and performs initial kernel setup.
 *
 * Phase 2: Minimal boot — prints "Hello from ARM64" and halts.
 * Phase 2+ will add proper MINIX kernel initialization:
 *   - Parse Device Tree for memory, CPUs, peripherals
 *   - Set up kinfo structure
 *   - Call kmain()
 * ============================================================ */

#include <stdint.h>

/* QEMU virt platform definitions */
#define VIRT_UART_BASE      0x09000000UL

/* PL011 UART registers */
#define PL011_DR            (*(volatile uint32_t *)(VIRT_UART_BASE + 0x000))
#define PL011_FR            (*(volatile uint32_t *)(VIRT_UART_BASE + 0x018))
#define PL011_IBRD          (*(volatile uint32_t *)(VIRT_UART_BASE + 0x024))
#define PL011_FBRD          (*(volatile uint32_t *)(VIRT_UART_BASE + 0x028))
#define PL011_LCRH          (*(volatile uint32_t *)(VIRT_UART_BASE + 0x02C))
#define PL011_CR            (*(volatile uint32_t *)(VIRT_UART_BASE + 0x030))

/* PL011 bit definitions */
#define PL011_FR_TXFF       (1 << 5)    /* Transmit FIFO Full */
#define PL011_FR_BUSY       (1 << 3)    /* Transmit Busy */
#define PL011_LCRH_FEN      (1 << 4)    /* Enable FIFOs */
#define PL011_LCRH_WLEN_8   (3 << 5)    /* 8-bit word */
#define PL011_CR_UARTEN     (1 << 0)    /* UART Enable */
#define PL011_CR_TXE        (1 << 8)    /* Transmit Enable */
#define PL011_CR_RXE        (1 << 9)    /* Receive Enable */

/* UART clock for QEMU virt: 24 MHz */
#define UART_CLOCK_HZ       24000000
#define UART_BAUD_RATE      115200

/*
 * Initialize PL011 UART at 115200 baud.
 * QEMU virt uses a 24 MHz reference clock.
 */
static void pl011_init(void)
{
    /* Disable UART while configuring */
    PL011_CR = 0;

    /* Wait for TX to finish */
    while (PL011_FR & PL011_FR_BUSY)
        ;

    /* Set baud rate: IBRD = UART_CLK / (16 * 115200)
     *   24000000 / (16 * 115200) = 13.0208
     *   IBRD = 13, FBRD = 0.0208 * 64 = 1.33 → 1
     */
    PL011_IBRD = 13;
    PL011_FBRD = 1;

    /* Line control: 8 bits, FIFO enabled */
    PL011_LCRH = PL011_LCRH_FEN | PL011_LCRH_WLEN_8;

    /* Enable UART, TX, RX */
    PL011_CR = PL011_CR_UARTEN | PL011_CR_TXE | PL011_CR_RXE;
}

/*
 * Send a single character via PL011 UART.
 */
static void pl011_putc(char c)
{
    /* Wait for TX FIFO to have space */
    while (PL011_FR & PL011_FR_TXFF)
        ;

    PL011_DR = (uint32_t)(unsigned char)c;

    /* Handle newline: need to send carriage return too */
    if (c == '\n')
        pl011_putc('\r');
}

/*
 * Send a string via PL011 UART.
 */
static void pl011_puts(const char *str)
{
    while (*str)
        pl011_putc(*str++);
}

/*
 * ARM64 kernel boot entry (called from head.S).
 *
 * @param dtb_address  Address of Device Tree Blob (or 0 if unknown)
 */
void arm64_boot(unsigned long dtb_address)
{
    /* Initialize UART */
    pl011_init();

    /* Print boot banner */
    pl011_puts("\r\n");
    pl011_puts("========================================\r\n");
    pl011_puts("  GergiOS ARM64 Kernel Bootstrap\r\n");
    pl011_puts("  Phase 2: Minimal Boot\r\n");
    pl011_puts("========================================\r\n");
    pl011_puts("\r\n");
    pl011_puts("Hello from ARM64!\r\n");
    pl011_puts("\r\n");
    pl011_puts("[BOOT] CPU: ARMv8-A (AArch64)\r\n");
    pl011_puts("[BOOT] EL1: exception level set up\r\n");
    pl011_puts("[BOOT] MMU: enabled (identity map)\r\n");
    pl011_puts("[BOOT] UART: PL011 at 0x09000000, 115200 baud\r\n");

    if (dtb_address) {
        /* Print DTB address if available */
        pl011_puts("[BOOT] DTB: 0x");
        /* Simple hex print for dtb_address */
        char hex[17];
        const char *hex_chars = "0123456789ABCDEF";
        for (int i = 15; i >= 0; i--) {
            hex[i] = hex_chars[dtb_address & 0xF];
            dtb_address >>= 4;
        }
        hex[16] = '\0';
        pl011_puts(hex);
        pl011_puts("\r\n");
    }

    pl011_puts("[BOOT] Bootstrap complete. Halting.\r\n");
    pl011_puts("\r\n");

    /* Phase 2: halt here.
     * Phase 2+ will call kmain() with boot info. */
    while (1) {
        /* Wait for interrupt (though timer not set up yet) */
        __asm__ __volatile__("wfi");
    }
}
