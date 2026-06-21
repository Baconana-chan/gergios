/* ============================================================
 * pl011.c — PL011 UART Driver for ARM PrimeCell (AArch64)
 *
 * Implements the TTY device-dependent functions for the ARM
 * PrimeCell PL011 UART on AArch64 platforms. Provides the
 * low-level serial I/O, interrupt handling, and termios
 * configuration for the MINIX terminal driver.
 *
 * Key features:
 *   - Interrupt-driven RX with circular buffer
 *   - Interrupt-driven TX with circular buffer
 *   - Termios-compatible configuration (baud, parity, etc.)
 *   - MMIO access via vm_map_phys through the kernel VM
 *   - GIC-compatible interrupt binding
 *
 * This driver follows the pattern established by the ARM (earm)
 * rs232.c driver but adapted for the PL011 UART architecture.
 *
 * Public symbols (called by tty.c):
 *   rs_init()    — Initialize a serial line
 *   rs_interrupt() — Interrupt dispatcher
 *
 * Reference:
 *   ARM DDI 0183G — PL011 Technical Reference Manual
 *   QEMU virt — PL011 at 0x09000000, IRQ 33, UARTCLK = 24 MHz
 *
 * Created for T10: AArch64 device driver support
 * ============================================================ */

#include <minix/config.h>
#include <minix/drivers.h>
#include <minix/vm.h>
#include <minix/type.h>
#include <minix/board.h>
#include <sys/mman.h>
#include <assert.h>
#include <signal.h>
#include <termios.h>

#include "tty.h"
#include "pl011.h"

/* =========================================================================
 * Configuration
 * =========================================================================
 *
 * NR_RS_LINES controls how many serial lines are available.
 * This must be set in the kernel config (typically 1 for QEMU virt).
 */

#if NR_RS_LINES > 0

/* =========================================================================
 * Default UART parameters (QEMU virt)
 * =========================================================================
 *
 * The QEMU virt platform maps PL011 at 0x09000000 with IRQ 33 (SPI #1).
 * UARTCLK is 24 MHz by default.
 *
 * In future phases, the UART base address can be determined from the
 * FDT parser (fdt_get_uart_info) for platform portability.
 */

#define PL011_DEFAULT_PHYS_BASE     0x09000000UL
#define PL011_DEFAULT_IRQ           33
#define PL011_DEFAULT_UARTCLK       24000000UL  /* 24 MHz */

/* =========================================================================
 * Input/output buffer watermarks
 * =========================================================================
 *
 * Flow control watermarks for the circular buffers.
 * The external device is asked to stop when the buffer reaches high water,
 * and restarts when it drops below low water.
 */

#define PL011_ILOWWATER    (1 * PL011_IBUFSIZE / 4)
#define PL011_IHIGHWATER   (3 * PL011_IBUFSIZE / 4)
#define PL011_OLOWWATER    (1 * PL011_OBUFSIZE / 4)

/* =========================================================================
 * PL011 instances (one per serial line)
 * =========================================================================
 */

static struct pl011_device pl011_lines[NR_RS_LINES];

/* =========================================================================
 * Forward declarations
 * =========================================================================
 */

static int pl011_write(tty_t *tp, int try);
static void pl011_echo(tty_t *tp, int c);
static int pl011_ioctl(tty_t *tp, int try);
static void pl011_config(struct pl011_device *pl011);
static int pl011_read(tty_t *tp, int try);
static int pl011_icancel(tty_t *tp, int try);
static int pl011_ocancel(tty_t *tp, int try);
static void pl011_ostart(struct pl011_device *pl011);
static int pl011_break_on(tty_t *tp, int try);
static int pl011_break_off(tty_t *tp, int try);
static int pl011_close(tty_t *tp, int try);
static int pl011_open(tty_t *tp, int try);
static void pl011_interrupt_handler(struct pl011_device *pl011);
static void pl011_reset(struct pl011_device *pl011);
static unsigned int pl011_check_uartclk(struct pl011_device *pl011);
static int termios_baud_rate(struct termios *term);

/* =========================================================================
 * MMIO accessor macros
 * =========================================================================
 *
 * All PL011 registers are 32-bit. Use volatile accessors for MMIO.
 */

static inline unsigned int
pl011_read(const struct pl011_device *pl011, unsigned int reg)
{
    return *(volatile unsigned int *)(pl011->base + reg);
}

static inline void
pl011_write(const struct pl011_device *pl011, unsigned int reg, unsigned int val)
{
    *(volatile unsigned int *)(pl011->base + reg) = val;
}

/* =========================================================================
 * pl011_reset — Perform UART soft reset
 * =========================================================================
 *
 * The PL011 does not have a dedicated reset register. The standard
 * reset sequence is to disable the UART, flush the FIFOs by toggling
 * the FIFO enable bit, and reinitialize.
 */

static void
pl011_reset(struct pl011_device *pl011)
{
    unsigned int cr;

    /* Disable UART */
    cr = pl011_read(pl011, PL011_CR);
    pl011_write(pl011, PL011_CR, cr & ~PL011_CR_UARTEN);

    /* Wait for BUSY to clear */
    while (pl011_read(pl011, PL011_FR) & PL011_FR_BUSY)
        ;

    /* Clear all pending interrupts */
    pl011_write(pl011, PL011_ICR, 0x7FF);

    /* Flush FIFOs by toggling FEN */
    pl011_write(pl011, PL011_LCRH, 0);
    pl011_write(pl011, PL011_LCRH, PL011_LCRH_FEN | PL011_LCRH_WLEN_8);

    /* Mask all interrupts */
    pl011_write(pl011, PL011_IMSC, 0);
}

/* =========================================================================
 * pl011_check_uartclk — Detect UART clock rate
 * =========================================================================
 *
 * On QEMU virt, the PL011 reference clock is fixed at 24 MHz.
 * On other platforms, the clock may differ. For now we use the
 * default. In future phases, the clock rate could be derived from
 * the DTB's clock-frequency property.
 */

static unsigned int
pl011_check_uartclk(struct pl011_device *pl011)
{
    /* QEMU virt: 24 MHz fixed */
    (void)pl011;
    return PL011_DEFAULT_UARTCLK;
}

/* =========================================================================
 * pl011_config — Configure UART parameters
 * =========================================================================
 *
 * Sets baud rate, data format, and flow control according to the
 * associated TTY's termios settings. Called on init and whenever
 * termios parameters change (via pl011_ioctl -> drain -> config).
 *
 * The PL011 requires:
 *   1. Disable UART (CR_UARTEN = 0)
 *   2. Wait for BUSY clear
 *   3. Set IBRD and FBRD for desired baud rate
 *   4. Set LCRH for word length, parity, stop bits, FIFOs
 *   5. Set IFLS for FIFO trigger levels
 *   6. Set IMSC for interrupt enables
 *   7. Enable UART (CR_UARTEN = 1)
 */

static void
pl011_config(struct pl011_device *pl011)
{
    tty_t *tp = pl011->tty;
    unsigned int baud_rate, divisor_int, divisor_frac;
    unsigned int lcrh, cr, ifls, imsc;
    int baud;

    /* Read current register state */
    cr = pl011_read(pl011, PL011_CR);

    /* Step 1: Disable UART */
    pl011_write(pl011, PL011_CR, cr & ~PL011_CR_UARTEN);

    /* Step 2: Flush */
    while (pl011_read(pl011, PL011_FR) & PL011_FR_BUSY)
        ;

    /* Clear errors */
    pl011_write(pl011, PL011_ECR, 0xFF);

    /* Step 3: Calculate and set baud rate */
    baud = termios_baud_rate(&tp->tty_termios);
    baud_rate = (unsigned int)baud;

    if (baud_rate == 0) {
        /* B0 = hang up */
        divisor_int = 0;
        divisor_frac = 0;
    } else {
        /* Divisor = UARTCLK / (16 * baud) */
        unsigned int div = pl011->uartclk / (16 * baud_rate);
        divisor_int = div;

        /* FBRD = round(((UARTCLK/(16*baud)) - IBRD) * 64) =
         * = round((UARTCLK % (16*baud)) * 64 / (16*baud)) */
        unsigned int remainder = pl011->uartclk % (16 * baud_rate);
        divisor_frac = (remainder * 64 + (16 * baud_rate / 2)) / (16 * baud_rate);
    }

    pl011_write(pl011, PL011_IBRD, divisor_int);
    pl011_write(pl011, PL011_FBRD, divisor_frac);

    /* Step 4: Set line control */
    lcrh = PL011_LCRH_FEN;  /* Enable FIFOs by default */

    switch (tp->tty_termios.c_cflag & CSIZE) {
    case CS5: lcrh |= PL011_LCRH_WLEN_5; break;
    case CS6: lcrh |= PL011_LCRH_WLEN_6; break;
    case CS7: lcrh |= PL011_LCRH_WLEN_7; break;
    default:
    case CS8: lcrh |= PL011_LCRH_WLEN_8; break;
    }

    if (tp->tty_termios.c_cflag & PARENB) {
        lcrh |= PL011_LCRH_PEN;
        if (!(tp->tty_termios.c_cflag & PARODD))
            lcrh |= PL011_LCRH_EPS;  /* Even parity */
    }

    if (tp->tty_termios.c_cflag & CSTOPB)
        lcrh |= PL011_LCRH_STP2;  /* 2 stop bits */

    pl011_write(pl011, PL011_LCRH, lcrh);

    /* Step 5: Set FIFO interrupt trigger levels */
    ifls = PL011_IFLS_RX_1_2 | PL011_IFLS_TX_1_8;
    pl011_write(pl011, PL011_IFLS, ifls);

    /* Step 6: Enable interrupts */
    imsc = PL011_INT_RXIM | PL011_INT_RTIM | PL011_INT_OEIM |
           PL011_INT_BEIM | PL011_INT_PEIM | PL011_INT_FEIM;
    pl011_write(pl011, PL011_IMSC, imsc);
    pl011->ier_shadow = imsc;

    /* Step 7: Enable UART */
    cr = PL011_CR_UARTEN | PL011_CR_TXE | PL011_CR_RXE;

    if (tp->tty_termios.c_cflag & CRTSCTS) {
        cr |= PL011_CR_CTSEN | PL011_CR_RTSEN | PL011_CR_RTS;
    }

    pl011_write(pl011, PL011_CR, cr);

    /* Update baud rate tracking */
    pl011->baud_rate = baud_rate;
}

/* =========================================================================
 * termios_baud_rate — Convert termios baud constant to Hz
 * =========================================================================
 */

static int
termios_baud_rate(struct termios *term)
{
    int baud;

    switch (term->c_ospeed) {
    case B300:     baud = 300;     break;
    case B600:     baud = 600;     break;
    case B1200:    baud = 1200;    break;
    case B1800:    baud = 1800;    break;
    case B2400:    baud = 2400;    break;
    case B4800:    baud = 4800;    break;
    case B9600:    baud = 9600;    break;
    case B19200:   baud = 19200;   break;
    case B38400:   baud = 38400;   break;
    case B57600:   baud = 57600;   break;
    case B115200:  baud = 115200;  break;
    case B230400:  baud = 230400;  break;
    case B460800:  baud = 460800;  break;
    case B921600:  baud = 921600;  break;
    case B0:
    default:
        term->c_ospeed = PL011_BAUD_DEFAULT;
        baud = termios_baud_rate(term);
        break;
    }

    return baud;
}

/* =========================================================================
 * pl011_write — TTY devwrite: initiate/output more data
 * =========================================================================
 */

static int
pl011_write(tty_t *tp, int try)
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;
    int r, ocount;

    if (pl011->inhibited != tp->tty_inhibited) {
        pl011->ostate |= PL011_OSWREADY;
        if (tp->tty_inhibited)
            pl011->ostate &= ~PL011_OSWREADY;
        pl011->inhibited = tp->tty_inhibited;
    }

    if (pl011->drain) {
        if (pl011->ocount > 0)
            return 1;
        pl011->drain = 0;
        pl011_config(pl011);
    }

    for (;;) {
        ocount = (int)buflen(pl011->obuf) - pl011->ocount;
        int count = (int)(bufend(pl011->obuf) - pl011->ohead);
        if (count > ocount)
            count = ocount;
        if (count > (int)tp->tty_outleft)
            count = (int)tp->tty_outleft;

        if (count == 0 || tp->tty_inhibited) {
            if (try)
                return (pl011->ocount > 0) ? 1 : 0;
            break;
        }

        if (try)
            return 1;

        if (tp->tty_outcaller == KERNEL) {
            memcpy(pl011->ohead,
                   (char *)tp->tty_outgrant + tp->tty_outcum,
                   (size_t)count);
        } else {
            if ((r = sys_safecopyfrom(tp->tty_outcaller,
                     tp->tty_outgrant, tp->tty_outcum,
                     (vir_bytes)pl011->ohead, (size_t)count)) != OK) {
                return 0;
            }
        }

        out_process(tp, pl011->obuf, pl011->ohead, bufend(pl011->obuf),
                    &count, &ocount);
        if (count == 0)
            break;

        tp->tty_reprint = 1;

        pl011->ocount += ocount;
        pl011_ostart(pl011);

        if ((pl011->ohead += ocount) >= bufend(pl011->obuf))
            pl011->ohead -= (int)buflen(pl011->obuf);

        tp->tty_outcum += count;
        if ((tp->tty_outleft -= count) == 0) {
            if (tp->tty_outcaller != KERNEL)
                chardriver_reply_task(tp->tty_outcaller,
                                     tp->tty_outid, tp->tty_outcum);
            tp->tty_outcum = 0;
            tp->tty_outcaller = NONE;
        }
    }

    if (tp->tty_outleft > 0 && tp->tty_termios.c_ospeed == B0) {
        if (tp->tty_outcaller != KERNEL)
            chardriver_reply_task(tp->tty_outcaller, tp->tty_outid, EIO);
        tp->tty_outleft = tp->tty_outcum = 0;
        tp->tty_outcaller = NONE;
    }

    return 1;
}

/* =========================================================================
 * pl011_echo — Echo a single character
 * =========================================================================
 */

static void
pl011_echo(tty_t *tp, int character)
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;
    int ocount;

    ocount = (int)buflen(pl011->obuf) - pl011->ocount;
    if (ocount == 0)
        return;

    int count = 1;
    *pl011->ohead = (char)character;

    out_process(tp, pl011->obuf, pl011->ohead, bufend(pl011->obuf),
                &count, &ocount);
    if (count == 0)
        return;

    pl011->ocount += ocount;
    pl011_ostart(pl011);

    if ((pl011->ohead += ocount) >= bufend(pl011->obuf))
        pl011->ohead -= (int)buflen(pl011->obuf);
}

/* =========================================================================
 * pl011_ioctl — TTY ioctl: reconfigure line
 * =========================================================================
 */

static int
pl011_ioctl(tty_t *tp, int UNUSED(dummy))
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;

    pl011->drain = 1;
    return 0;
}

/* =========================================================================
 * pl011_interrupt_handler — Handle UART interrupts
 * =========================================================================
 *
 * Reads MIS (Masked Interrupt Status) to determine the interrupt source:
 *   RX:  Data in RX FIFO
 *   RT:  Receive timeout
 *   TX:  TX FIFO below trigger level
 *   OE/BE/PE/FE: Error conditions
 */

static void
pl011_interrupt_handler(struct pl011_device *pl011)
{
    unsigned int mis;

    mis = pl011_read(pl011, PL011_MIS);
    if (mis == 0)
        return;

    /* RX data available or RX timeout */
    if (mis & (PL011_INT_RXIM | PL011_INT_RTIM)) {
        while (!(pl011_read(pl011, PL011_FR) & PL011_FR_RXFE)) {
            unsigned int dr = pl011_read(pl011, PL011_DR);
            char c = (char)(dr & PL011_DR_DATA_MASK);

            if (dr & (PL011_DR_OE | PL011_DR_BE | PL011_DR_PE | PL011_DR_FE)) {
                if (dr & PL011_DR_OE)
                    pl011->rx_overrun_events++;
                pl011_write(pl011, PL011_ECR, 0xFF);
            }

            if (pl011->icount < (int)buflen(pl011->ibuf)) {
                *pl011->ihead = c;
                if (++pl011->ihead == bufend(pl011->ibuf))
                    pl011->ihead = pl011->ibuf;
                pl011->icount++;

                if (pl011->icount >= PL011_IHIGHWATER && pl011->idevready) {
                    unsigned int cr = pl011_read(pl011, PL011_CR);
                    pl011_write(pl011, PL011_CR, cr & ~PL011_CR_RTS);
                    pl011->idevready = 0;
                }

                if (pl011->icount == 1)
                    pl011->tty->tty_events = 1;
            }
        }
        pl011_write(pl011, PL011_ICR, mis & (PL011_INT_RXIM | PL011_INT_RTIM));
    }

    /* TX FIFO below trigger level */
    if (mis & PL011_INT_TXIM) {
        pl011_write(pl011, PL011_ICR, PL011_INT_TXIM);

        if (pl011->ostate & PL011_OQUEUED) {
            pl011->ier_shadow &= ~PL011_INT_TXIM;
            pl011_write(pl011, PL011_IMSC, pl011->ier_shadow);
        }

        if (pl011->ocount > 0) {
            int i;
            for (i = 0; i < 16 && pl011->ocount > 0; i++) {
                pl011_write(pl011, PL011_DR,
                            (unsigned int)(unsigned char)*pl011->otail);
                if (++pl011->otail == bufend(pl011->obuf))
                    pl011->otail = pl011->obuf;
                pl011->ocount--;
            }

            if (pl011->ocount == 0) {
                pl011->ostate &= ~PL011_OQUEUED;
                pl011->tty->tty_events = 1;
            } else {
                pl011->ier_shadow |= PL011_INT_TXIM;
                pl011_write(pl011, PL011_IMSC, pl011->ier_shadow);

                if (pl011->ocount <= PL011_OLOWWATER)
                    pl011->tty->tty_events = 1;
            }
        }
    }

    /* Error interrupts */
    if (mis & (PL011_INT_OEIM | PL011_INT_BEIM |
               PL011_INT_PEIM | PL011_INT_FEIM)) {
        pl011_write(pl011, PL011_ICR, mis & (PL011_INT_OEIM | PL011_INT_BEIM |
                                              PL011_INT_PEIM | PL011_INT_FEIM));
        pl011_write(pl011, PL011_ECR, 0xFF);
    }

    /* Modem interrupts */
    if (mis & (PL011_INT_DSRM | PL011_INT_DCDM | PL011_INT_CTSM)) {
        pl011_write(pl011, PL011_ICR, mis & (PL011_INT_DSRM |
                                              PL011_INT_DCDM | PL011_INT_CTSM));
    }
}

/* =========================================================================
 * pl011_read — TTY devread: process received characters
 * =========================================================================
 */

static int
pl011_read(tty_t *tp, int try)
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;
    int count;

    if (!(tp->tty_termios.c_cflag & CLOCAL)) {
        if (try)
            return 1;

        if (!(pl011_read(pl011, PL011_FR) & PL011_FR_DCD)) {
            sigchar(tp, SIGHUP, 1);
            tp->tty_termios.c_ospeed = B0;
            return 0;
        }
    }

    if (try)
        return (pl011->icount > 0);

    while ((count = pl011->icount) > 0) {
        int icount = (int)(bufend(pl011->ibuf) - pl011->itail);
        if (count > icount)
            count = icount;

        if ((count = in_process(tp, pl011->itail, count)) == 0)
            break;

        pl011->icount -= count;

        if (!pl011->idevready && pl011->icount < PL011_ILOWWATER) {
            unsigned int cr = pl011_read(pl011, PL011_CR);
            pl011_write(pl011, PL011_CR, cr | PL011_CR_RTS);
            pl011->idevready = 1;
        }

        if ((pl011->itail += count) == bufend(pl011->ibuf))
            pl011->itail = pl011->ibuf;
    }

    return 0;
}

/* =========================================================================
 * pl011_ostart — Start TX engine
 * =========================================================================
 */

static void
pl011_ostart(struct pl011_device *pl011)
{
    pl011->ostate |= PL011_OQUEUED;

    if (!(pl011->ier_shadow & PL011_INT_TXIM)) {
        pl011->ier_shadow |= PL011_INT_TXIM;
        pl011_write(pl011, PL011_IMSC, pl011->ier_shadow);
    }

    /* Write directly if TX FIFO is empty */
    if ((pl011_read(pl011, PL011_FR) & PL011_FR_TXFE) && pl011->ocount > 0) {
        int i;
        for (i = 0; i < 16 && pl011->ocount > 0; i++) {
            pl011_write(pl011, PL011_DR,
                        (unsigned int)(unsigned char)*pl011->otail);
            if (++pl011->otail == bufend(pl011->obuf))
                pl011->otail = pl011->obuf;
            pl011->ocount--;
        }

        if (pl011->ocount == 0) {
            pl011->ostate &= ~PL011_OQUEUED;
            pl011->tty->tty_events = 1;

            pl011->ier_shadow &= ~PL011_INT_TXIM;
            pl011_write(pl011, PL011_IMSC, pl011->ier_shadow);
        }
    }
}

/* =========================================================================
 * pl011_icancel — Cancel pending input
 * =========================================================================
 */

static int
pl011_icancel(tty_t *tp, int UNUSED(dummy))
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;

    pl011->icount = 0;
    pl011->itail = pl011->ihead;

    if (!pl011->idevready) {
        unsigned int cr = pl011_read(pl011, PL011_CR);
        pl011_write(pl011, PL011_CR, cr | PL011_CR_RTS);
        pl011->idevready = 1;
    }

    return 0;
}

/* =========================================================================
 * pl011_ocancel — Cancel pending output
 * =========================================================================
 */

static int
pl011_ocancel(tty_t *tp, int UNUSED(dummy))
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;

    pl011->ostate &= ~(PL011_ODONE | PL011_OQUEUED);
    pl011->ocount = 0;
    pl011->otail = pl011->ohead;

    pl011->ier_shadow &= ~PL011_INT_TXIM;
    pl011_write(pl011, PL011_IMSC, pl011->ier_shadow);

    return 0;
}

/* =========================================================================
 * pl011_break_on — Assert break condition
 * =========================================================================
 */

static int
pl011_break_on(tty_t *tp, int UNUSED(dummy))
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;
    unsigned int lcrh;

    lcrh = pl011_read(pl011, PL011_LCRH);
    pl011_write(pl011, PL011_LCRH, lcrh | PL011_LCRH_BRK);

    return 0;
}

/* =========================================================================
 * pl011_break_off — De-assert break condition
 * =========================================================================
 */

static int
pl011_break_off(tty_t *tp, int UNUSED(dummy))
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;
    unsigned int lcrh;

    lcrh = pl011_read(pl011, PL011_LCRH);
    pl011_write(pl011, PL011_LCRH, lcrh & ~PL011_LCRH_BRK);

    return 0;
}

/* =========================================================================
 * pl011_open — TTY open: prepare the line
 * =========================================================================
 */

static int
pl011_open(tty_t *tp, int UNUSED(dummy))
{
    tp->tty_termios.c_ospeed = PL011_BAUD_DEFAULT;
    return 0;
}

/* =========================================================================
 * pl011_close — TTY close: optionally hang up the line
 * =========================================================================
 */

static int
pl011_close(tty_t *tp, int UNUSED(dummy))
{
    struct pl011_device *pl011 = (struct pl011_device *)tp->tty_priv;

    if (tp->tty_termios.c_cflag & HUPCL) {
        unsigned int cr = pl011_read(pl011, PL011_CR);
        pl011_write(pl011, PL011_CR, cr & ~(PL011_CR_DTR | PL011_CR_RTS));

        if (pl011->ier_shadow & PL011_INT_TXIM) {
            pl011->ier_shadow &= ~PL011_INT_TXIM;
            pl011_write(pl011, PL011_IMSC, pl011->ier_shadow);
        }
    }

    return 0;
}

/* =========================================================================
 * rs_interrupt — Interrupt dispatcher (public symbol, called by tty.c)
 * =========================================================================
 *
 * tty.c calls this function by name from its main loop when a hardware
 * interrupt notification has matching bits in rs_irq_set.
 */

void
rs_interrupt(message *m)
{
    unsigned long irq_set;
    int line;
    struct pl011_device *pl011;

    irq_set = m->m_notify.interrupts;

    for (line = 0, pl011 = pl011_lines; line < NR_RS_LINES; line++, pl011++) {
        if (irq_set & (1 << pl011->irq_hook_id)) {
            pl011_interrupt_handler(pl011);
            if (sys_irqenable(&pl011->irq_hook_kernel_id) != OK)
                panic("PL011: unable to re-enable interrupts");
        }
    }
}

/* =========================================================================
 * rs_init — Initialize a serial line (public symbol, called by tty.c)
 * =========================================================================
 *
 * Called by tty_init() for each RS232 serial line during TTY driver
 * initialization. Sets up MMIO mapping, reset, interrupt binding, and
 * TTY function pointer hooks.
 *
 * The UART base address defaults to the QEMU virt PL011 at 0x09000000.
 */

void
rs_init(tty_t *tp)
{
    struct pl011_device *pl011;
    int line;
    struct minix_mem_range mr;
    char l[10];

    line = tp - &tty_table[NR_CONS];

    if (env_get_param(SERVARNAME, l, sizeof(l) - 1) == OK && atoi(l) == line) {
        printf("TTY: PL011 line %d not initialized (used by kernel)\n", line);
        return;
    }

    pl011 = tp->tty_priv = &pl011_lines[line];
    pl011->tty = tp;
    pl011->phys_base = PL011_DEFAULT_PHYS_BASE;

    /* Input queue */
    pl011->ihead = pl011->itail = pl011->ibuf;
    pl011->icount = 0;
    pl011->idevready = 1;
    pl011->rx_overrun_events = 0;

    /* Output queue */
    pl011->ohead = pl011->otail = pl011->obuf;
    pl011->ocount = 0;
    pl011->ostate = PL011_ORAW | PL011_OSWREADY;
    pl011->inhibited = 0;
    pl011->drain = 0;
    pl011->oxoff = 0;

    /* Map UART registers */
    mr.mr_base = pl011->phys_base;
    mr.mr_limit = pl011->phys_base + 0x1000;

    if (sys_privctl(SELF, SYS_PRIV_ADD_MEM, &mr) != OK)
        panic("PL011: unable to request UART memory access");

    pl011->base = (vir_bytes)vm_map_phys(SELF, (void *)pl011->phys_base, 0x1000);
    if (pl011->base == (vir_bytes)MAP_FAILED)
        panic("PL011: unable to map UART registers");

    pl011->map_size = 0x1000;
    pl011->uartclk = pl011_check_uartclk(pl011);
    tp->tty_termios.c_ospeed = PL011_BAUD_DEFAULT;

    /* IRQ setup */
    pl011->irq = PL011_DEFAULT_IRQ;
    pl011->irq_hook_kernel_id = pl011->irq_hook_id = line + 1;

    if (sys_irqsetpolicy(pl011->irq, 0, &pl011->irq_hook_kernel_id) != OK) {
        printf("PL011: Couldn't obtain hook for IRQ %d\n", pl011->irq);
    } else {
        if (sys_irqenable(&pl011->irq_hook_kernel_id) != OK)
            printf("PL011: Couldn't enable IRQ %d\n", pl011->irq);
    }

    rs_irq_set |= (1 << (pl011->irq_hook_id));

    /* Reset and configure */
    pl011_reset(pl011);
    pl011_config(pl011);

    /* TTY function hooks */
    tp->tty_devread = pl011_read;
    tp->tty_devwrite = pl011_write;
    tp->tty_echo = pl011_echo;
    tp->tty_icancel = pl011_icancel;
    tp->tty_ocancel = pl011_ocancel;
    tp->tty_ioctl = pl011_ioctl;
    tp->tty_break_on = pl011_break_on;
    tp->tty_break_off = pl011_break_off;
    tp->tty_open = pl011_open;
    tp->tty_close = pl011_close;

    printf("PL011: UART at 0x%lx, IRQ %d, %u baud\n",
           (unsigned long)pl011->phys_base, pl011->irq, pl011->baud_rate);
}

#endif /* NR_RS_LINES > 0 */
