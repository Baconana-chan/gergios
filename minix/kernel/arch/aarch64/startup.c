/* ============================================================
 * startup.c — ARM64 kernel C startup
 *
 * Called by head.S after MMU is enabled.
 * Initializes the PL011 UART, parses the Device Tree Blob (DTB)
 * for memory and system information, then calls pre_init() to
 * set up the kinfo structure and boot the kernel.
 *
 * Phase 3: FDT parser integration — Device Tree is parsed for
 *          memory layout, CPU count, and boot arguments.
 * ============================================================ */

#include <stdint.h>

/* FDT parser header */
#include "fdt.h"

/* Forward declaration of pre_init (defined in pre_init.c)
 * pre_init sets up the kinfo structure from the DTB and returns
 * it for use by kmain().
 */
struct kinfo *pre_init(unsigned long dtb_address);

/* =========================================================================
 * PL011 UART (QEMU virt) — Early boot console
 * =========================================================================
 *
 * The PL011 is hardcoded at 0x09000000 for QEMU virt platform.
 * In Phase 3+, this address can be determined from the DTB's
 * stdout-path and UART node reg property.
 */

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

/* =========================================================================
 * PL011 UART driver
 * ========================================================================= */

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

/* =========================================================================
 * Hex output helpers (for DTB dump)
 * ========================================================================= */

static void pl011_put_hex64(uint64_t val)
{
    const char *hex = "0123456789ABCDEF";
    int i;

    for (i = 60; i >= 0; i -= 4) {
        pl011_putc(hex[(val >> i) & 0xF]);
    }
}

static void pl011_put_hex32(uint32_t val)
{
    const char *hex = "0123456789ABCDEF";
    int i;

    for (i = 28; i >= 0; i -= 4) {
        pl011_putc(hex[(val >> i) & 0xF]);
    }
}

static void pl011_put_dec(uint64_t val)
{
    char buf[20];
    int i = 0;

    if (val == 0) {
        pl011_putc('0');
        return;
    }

    while (val > 0 && i < 20) {
        buf[i++] = '0' + (val % 10);
        val /= 10;
    }

    while (i > 0)
        pl011_putc(buf[--i]);
}

/*
 * ARM64 kernel boot entry (called from head.S).
 *
 * @param dtb_address  Physical address of Device Tree Blob (or 0 if unknown)
 */
void arm64_boot(unsigned long dtb_address)
{
    uint64_t mem_addr = 0, mem_size = 0;
    int cpu_count = 0;
    const char *bootargs = NULL;
    const char *stdout_path = NULL;

    /* Initialize UART */
    pl011_init();

    /* Print boot banner */
    pl011_puts("\r\n");
    pl011_puts("========================================\r\n");
    pl011_puts("  GergiOS ARM64 Kernel Bootstrap\r\n");
    pl011_puts("  Phase 3: FDT Parser\r\n");
    pl011_puts("========================================\r\n");
    pl011_puts("\r\n");
    pl011_puts("Hello from ARM64!\r\n");
    pl011_puts("\r\n");
    pl011_puts("[BOOT] CPU: ARMv8-A (AArch64)\r\n");
    pl011_puts("[BOOT] EL1: exception level set up\r\n");
    pl011_puts("[BOOT] MMU: enabled (identity map)\r\n");
    pl011_puts("[BOOT] UART: PL011 at 0x09000000, 115200 baud\r\n");

    /* =================================================================
     * Parse Device Tree Blob
     *
     * The bootloader passes the DTB address in x0 (saved by head.S).
     * We validate it and extract boot-critical information.
     * ================================================================= */
    if (dtb_address) {
        pl011_puts("[BOOT] DTB: 0x");
        pl011_put_hex64(dtb_address);
        pl011_puts("\r\n");

        /* Validate DTB */
        if (fdt_validate((const void *)dtb_address, 0) == 0) {
            uint32_t dtb_size = fdt_total_size((const void *)dtb_address);

            pl011_puts("[FDT]  Valid DTB detected\r\n");
            pl011_puts("[FDT]  Size: ");
            pl011_put_dec(dtb_size);
            pl011_puts(" bytes\r\n");

            /* Parse memory */
            if (fdt_get_memory((const void *)dtb_address,
                               &mem_addr, &mem_size) == 1) {
                pl011_puts("[FDT]  Memory: 0x");
                pl011_put_hex64(mem_addr);
                pl011_puts(" - 0x");
                pl011_put_hex64(mem_addr + mem_size);
                pl011_puts(" (");
                pl011_put_dec(mem_size / (1024 * 1024));
                pl011_puts(" MB)\r\n");
            } else {
                pl011_puts("[FDT]  Memory: NOT FOUND, using defaults\r\n");
                /* Default: 512 MB at 0x40000000 (QEMU virt) */
                mem_addr = 0x40000000ULL;
                mem_size = 512ULL * 1024 * 1024;
            }

            /* Parse CPU count */
            cpu_count = fdt_get_cpu_count((const void *)dtb_address);
            if (cpu_count > 0) {
                pl011_puts("[FDT]  CPUs: ");
                pl011_put_dec(cpu_count);
                pl011_puts("\r\n");
            }

            /* Parse bootargs */
            bootargs = fdt_get_chosen_bootargs((const void *)dtb_address);
            if (bootargs) {
                pl011_puts("[FDT]  Bootargs: ");
                pl011_puts(bootargs);
                pl011_puts("\r\n");
            }

            /* Parse stdout-path */
            stdout_path = fdt_get_chosen_stdout((const void *)dtb_address);
            if (stdout_path) {
                pl011_puts("[FDT]  Stdout:   ");
                pl011_puts(stdout_path);
                pl011_puts("\r\n");
            }
        } else {
            pl011_puts("[FDT]  INVALID DTB at given address\r\n");
            /* Default memory for fallback */
            mem_addr = 0x40000000ULL;
            mem_size = 512ULL * 1024 * 1024;
        }
    } else {
        pl011_puts("[BOOT] DTB: not provided by bootloader\r\n");
        /* Default memory for fallback */
        mem_addr = 0x40000000ULL;
        mem_size = 512ULL * 1024 * 1024;
    }

    /* =================================================================
     * Call pre_init with parsed boot info
     *
     * pre_init() will fill the kinfo structure using the DTB-parsed
     * memory map, CPU count, and boot arguments.
     * ================================================================= */
    pl011_puts("\r\n[BOOT] Calling pre_init()...\r\n");
    pl011_puts("\r\n");

    /* Phase 3: call pre_init with DTB address and parsed boot info.
     * pre_init() returns the kinfo struct that kmain() expects.
     *
     * For now, we pass the DTB address. pre_init() will parse
     * what it needs and set up the kinfo structure.
     *
     * Phase 3+: pre_init() will set up page tables, enable paging,
     *           and return the kinfo for kmain().
     */
    {
        struct kinfo *cbi;

        /* Call pre_init with DTB address.
         * pre_init now uses FDT parser internally for memory info. */
        cbi = pre_init(dtb_address);

        (void)cbi;  /* Phase 3: will pass to kmain() in Phase 4 */
    }

    /* Phase 3: halt here until kmain() integration is complete.
     * Phase 4+: call kmain(cbi) with the boot info. */
    pl011_puts("[BOOT] Bootstrap complete. Halting.\r\n");
    pl011_puts("\r\n");

    while (1) {
        __asm__ __volatile__("wfi");
    }
}
