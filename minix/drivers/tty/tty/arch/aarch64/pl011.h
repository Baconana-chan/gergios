/* ============================================================
 * pl011.h — PL011 UART Register Definitions (ARM PrimeCell)
 *
 * ARM PrimeCell PL011 UART register map and bit definitions.
 * Compatible with QEMU virt, Raspberry Pi 4, and most ARM
 * SBSA-compliant platforms.
 *
 * Reference: ARM DDI 0183G (PL011 Technical Reference Manual)
 *
 * Created for T10: AArch64 device driver support
 * ============================================================ */

#ifndef _AARCH64_PL011_H
#define _AARCH64_PL011_H

/* =========================================================================
 * PL011 register map (offsets from UART base address)
 * =========================================================================
 *
 * All registers are 32-bit wide and accessible via 32-bit loads/stores.
 * The PL011 uses a memory-mapped interface with byte, halfword, and word
 * accesses (word accesses recommended for performance).
 */

#define PL011_DR        0x000   /* Data Register (R/W) — TX if written, RX if read */
#define PL011_RSR       0x004   /* Receive Status Register (R/W) */
#define PL011_ECR       0x004   /* Error Clear Register (W) — alias for RSR */
#define PL011_FR        0x018   /* Flag Register (RO) */
#define PL011_ILPR      0x020   /* IrDA Low-Power Counter Register (R/W) — optional */
#define PL011_IBRD      0x024   /* Integer Baud Rate Divisor (R/W) */
#define PL011_FBRD      0x028   /* Fractional Baud Rate Divisor (R/W) */
#define PL011_LCRH      0x02C   /* Line Control Register (R/W) */
#define PL011_CR        0x030   /* Control Register (R/W) */
#define PL011_IFLS      0x034   /* Interrupt FIFO Level Select (R/W) */
#define PL011_IMSC      0x038   /* Interrupt Mask Set/Clear (R/W) */
#define PL011_RIS       0x03C   /* Raw Interrupt Status (RO) */
#define PL011_MIS       0x040   /* Masked Interrupt Status (RO) */
#define PL011_ICR       0x044   /* Interrupt Clear Register (W) */
#define PL011_DMACR     0x048   /* DMA Control Register (R/W) */
#define PL011_PERIPHID0 0xFE0   /* Peripheral ID0 (RO) */
#define PL011_PERIPHID1 0xFE4   /* Peripheral ID1 (RO) */
#define PL011_PERIPHID2 0xFE8   /* Peripheral ID2 (RO) */
#define PL011_PERIPHID3 0xFEC   /* Peripheral ID3 (RO) */
#define PL011_PCELLID0  0xFF0   /* PrimeCell ID0 (RO) */
#define PL011_PCELLID1  0xFF4   /* PrimeCell ID1 (RO) */
#define PL011_PCELLID2  0xFF8   /* PrimeCell ID2 (RO) */
#define PL011_PCELLID3  0xFFC   /* PrimeCell ID3 (RO) */

/* =========================================================================
 * Flag Register (FR) bits — offset 0x018
 * =========================================================================
 *
 * Most flags are read-only. The FR gives status information about the
 * UART's FIFOs and line state.
 */

#define PL011_FR_RI     (1U << 8)  /* Ring Indicator */
#define PL011_FR_TXFE   (1U << 7)  /* Transmit FIFO Empty */
#define PL011_FR_RXFF   (1U << 6)  /* Receive FIFO Full */
#define PL011_FR_TXFF   (1U << 5)  /* Transmit FIFO Full */
#define PL011_FR_RXFE   (1U << 4)  /* Receive FIFO Empty */
#define PL011_FR_BUSY   (1U << 3)  /* Transmit Busy (TX state machine active) */
#define PL011_FR_DCD    (1U << 2)  /* Data Carrier Detect */
#define PL011_FR_DSR    (1U << 1)  /* Data Set Ready */
#define PL011_FR_CTS    (1U << 0)  /* Clear to Send */

/* =========================================================================
 * Line Control Register (LCRH) bits — offset 0x02C
 * =========================================================================
 *
 * Controls framing parameters: word length, parity, stop bits, and FIFO
 * enable. The SPS and EPS fields control stick/even parity selection.
 *
 * NOTE: LCRH must NOT be written while the UART is enabled (CR_UARTEN = 1).
 * Disable the UART before changing LCRH and re-enable afterward.
 */

#define PL011_LCRH_SPS  (1U << 7)  /* Stick Parity Select */
#define PL011_LCRH_WLEN (3U << 5)  /* Word Length mask */
#define PL011_LCRH_WLEN_5  (0U << 5)  /* 5-bit word */
#define PL011_LCRH_WLEN_6  (1U << 5)  /* 6-bit word */
#define PL011_LCRH_WLEN_7  (2U << 5)  /* 7-bit word */
#define PL011_LCRH_WLEN_8  (3U << 5)  /* 8-bit word */
#define PL011_LCRH_FEN  (1U << 4)  /* FIFO Enable */
#define PL011_LCRH_STP2 (1U << 3)  /* 2 Stop Bits (0 = 1 stop bit) */
#define PL011_LCRH_EPS  (1U << 2)  /* Even Parity Select (0 = odd) */
#define PL011_LCRH_PEN  (1U << 1)  /* Parity Enable */
#define PL011_LCRH_BRK  (1U << 0)  /* Send Break (force TX low) */

/* =========================================================================
 * Control Register (CR) bits — offset 0x030
 * =========================================================================
 *
 * Enables/disables the UART and its sub-blocks. CR must be programmed
 * after all other configuration is complete (IBRD, FBRD, LCRH).
 */

#define PL011_CR_CTSEN  (1U << 15) /* CTS Hardware Flow Control Enable */
#define PL011_CR_RTSEN  (1U << 14) /* RTS Hardware Flow Control Enable */
#define PL011_CR_OUT2   (1U << 13) /* Complement Output 2 */
#define PL011_CR_OUT1   (1U << 12) /* Complement Output 1 */
#define PL011_CR_RTS    (1U << 11) /* Request to Send */
#define PL011_CR_DTR    (1U << 10) /* Data Terminal Ready */
#define PL011_CR_RXE    (1U << 9)  /* Receive Enable */
#define PL011_CR_TXE    (1U << 8)  /* Transmit Enable */
#define PL011_CR_LBE    (1U << 7)  /* Loopback Enable */
#define PL011_CR_SIRLP  (1U << 2)  /* SIR Low-Power IrDA (0 = SIR) */
#define PL011_CR_SIREN  (1U << 1)  /* SIR IrDA Enable */
#define PL011_CR_UARTEN (1U << 0)  /* UART Enable */

/* =========================================================================
 * Interrupt FIFO Level Select (IFLS) bits — offset 0x034
 * =========================================================================
 *
 * Selects the FIFO level at which the RX/TX interrupts are triggered.
 * The granularity depends on the FIFO size (16 entries for PL011).
 */

#define PL011_IFLS_RX_MASK   (7U << 3)  /* RX FIFO level mask */
#define PL011_IFLS_RX_1_8    (0U << 3)  /* RX 1/8 full (2 chars) */
#define PL011_IFLS_RX_1_4    (1U << 3)  /* RX 1/4 full (4 chars) */
#define PL011_IFLS_RX_1_2    (2U << 3)  /* RX 1/2 full (8 chars) */
#define PL011_IFLS_RX_3_4    (3U << 3)  /* RX 3/4 full (12 chars) */
#define PL011_IFLS_RX_7_8    (4U << 3)  /* RX 7/8 full (14 chars) */

#define PL011_IFLS_TX_MASK   (7U << 0)  /* TX FIFO level mask */
#define PL011_IFLS_TX_1_8    (0U << 0)  /* TX 1/8 full (2 chars) */
#define PL011_IFLS_TX_1_4    (1U << 0)  /* TX 1/4 full (4 chars) */
#define PL011_IFLS_TX_1_2    (2U << 0)  /* TX 1/2 full (8 chars) */
#define PL011_IFLS_TX_3_4    (3U << 0)  /* TX 3/4 full (12 chars) */
#define PL011_IFLS_TX_7_8    (4U << 0)  /* TX 7/8 full (14 chars) */

/* =========================================================================
 * Interrupt registers — IMSC, RIS, MIS, ICR
 * =========================================================================
 *
 * PL011 supports 9 interrupt sources. Interrupts are enabled by setting
 * the corresponding bit in IMSC. The active interrupt source can be
 * determined by reading MIS (Masked Interrupt Status) — if the
 * corresponding IMSC bit is clear, the interrupt is masked.
 *
 * All interrupts are cleared by writing the corresponding bit to ICR.
 */

/* Interrupt Mask Set/Clear (IMSC) at 0x038 */
/* Raw Interrupt Status (RIS) at 0x03C */
/* Masked Interrupt Status (MIS) at 0x040 */
/* Interrupt Clear Register (ICR) at 0x044 (write 1 to clear) */

#define PL011_INT_OEIM (1U << 10) /* Overrun Error Interrupt */
#define PL011_INT_BEIM (1U << 9)  /* Break Error Interrupt */
#define PL011_INT_PEIM (1U << 8)  /* Parity Error Interrupt */
#define PL011_INT_FEIM (1U << 7)  /* Framing Error Interrupt */
#define PL011_INT_RTIM (1U << 6)  /* Receive Timeout Interrupt */
#define PL011_INT_TXIM (1U << 5)  /* Transmit Interrupt */
#define PL011_INT_RXIM (1U << 4)  /* Receive Interrupt */
#define PL011_INT_DSRM (1U << 3)  /* DSR Interrupt */
#define PL011_INT_DCDM (1U << 2)  /* DCD Interrupt */
#define PL011_INT_CTSM (1U << 1)  /* CTS Interrupt */
#define PL011_INT_RI   (1U << 0)  /* RI Interrupt (read-only in RIS/MIS) */

/* Combined mask for data interrupts (RX, TX, errors) */
#define PL011_INT_DATA \
    (PL011_INT_RXIM | PL011_INT_TXIM | PL011_INT_RTIM | \
     PL011_INT_OEIM | PL011_INT_BEIM | PL011_INT_PEIM | PL011_INT_FEIM)

/* Combined mask for modem status interrupts */
#define PL011_INT_MODEM \
    (PL011_INT_DSRM | PL011_INT_DCDM | PL011_INT_CTSM)

/* =========================================================================
 * Data Register (DR) bits — offset 0x000
 * =========================================================================
 *
 * Reading DR returns the received character (if data available).
 * Writing DR transmits a character.
 */

#define PL011_DR_DATA_MASK  0xFF   /* Data bits (bits 7:0) */
#define PL011_DR_FE         (1U << 8)  /* Framing Error */
#define PL011_DR_PE         (1U << 9)  /* Parity Error */
#define PL011_DR_BE         (1U << 10) /* Break Error */
#define PL011_DR_OE         (1U << 11) /* Overrun Error */

/* =========================================================================
 * Receive Status Register (RSR) / Error Clear Register (ECR) bits
 * =========================================================================
 *
 * RSR reflects the error status of the last character read from DR.
 * ECR is a write-only alias — writing any value clears all error bits.
 */

#define PL011_RSR_OE       (1U << 3)  /* Overrun Error */
#define PL011_RSR_BE       (1U << 2)  /* Break Error */
#define PL011_RSR_PE       (1U << 1)  /* Parity Error */
#define PL011_RSR_FE       (1U << 0)  /* Framing Error */

/* =========================================================================
 * DMA Control Register (DMACR) bits — offset 0x048
 * =========================================================================
 */

#define PL011_DMACR_DMAONERR (1U << 2)  /* Disable DMA on error */
#define PL011_DMACR_TXDMAE   (1U << 1)  /* Transmit DMA Enable */
#define PL011_DMACR_RXDMAE   (1U << 0)  /* Receive DMA Enable */

/* =========================================================================
 * Peripheral ID registers — used to detect PL011
 * =========================================================================
 *
 * PL011 PrimeCell ID (reads as 0xB105F00D across PCELLID0-3):
 *   PCELLID0 = 0x0D
 *   PCELLID1 = 0xF0
 *   PCELLID2 = 0x05
 *   PCELLID3 = 0xB1
 *
 * Peripheral ID (reads as 0x00341011 across PERIPHID0-3):
 *   PERIPHID0 = 0x11 (Part number low)
 *   PERIPHID1 = 0x10 (Part number high + designer low)
 *   PERIPHID2 = 0x34 (Designer high + revision)
 *   PERIPHID3 = 0x00 (Configuration)
 */

/* =========================================================================
 * Baud rate calculation helpers
 * =========================================================================
 *
 * The PL011 baud rate divider is calculated as:
 *   Divisor = UARTCLK / (16 * baud_rate)
 *   IBRD = integer(Divisor)
 *   FBRD = round(fraction(Divisor) * 64)
 *
 * Standard rates with UARTCLK = 24 MHz (QEMU virt):
 *   Baud    Divisor     IBRD  FBRD
 *   9600    156.25      156   16
 *   19200   78.125      78    8
 *   38400   39.0625     39    4
 *   57600   26.0417     26    3
 *   115200  13.0208     13    1
 *   230400  6.5104      6     33
 *   460800  3.2552      3     16
 *   921600  1.6276      1     40
 */

#define PL011_UARTCLK_DEFAULT   24000000UL  /* QEMU virt default UART clock */
#define PL011_BAUD_DEFAULT      B115200     /* Default baud rate */

/* Calculate IBRD from UART clock and baud rate */
#define PL011_IBRD_CALC(clk, baud) \
    ((unsigned int)((clk) / (16UL * (unsigned int)(baud))))

/* Calculate FBRD from UART clock and baud rate */
#define PL011_FBRD_CALC(clk, baud) \
    ((unsigned int)((((clk) / (16UL * (unsigned int)(baud))) - \
                     PL011_IBRD_CALC(clk, baud)) * 64.0 + 0.5))

/* =========================================================================
 * PL011 private data structure (per UART instance)
 * =========================================================================
 */

struct pl011_device {
    /* MMIO mapping */
    vir_bytes base;                 /* Virtual address of UART registers */
    phys_bytes phys_base;           /* Physical base address */
    vir_bytes map_size;             /* Size of mapped region */

    /* TTY association */
    tty_t *tty;                     /* Associated TTY structure */

    /* Interrupt handling */
    int irq;                        /* IRQ number */
    int irq_hook_id;                /* Hook ID for sys_irqsetpolicy */
    int irq_hook_kernel_id;         /* Kernel IRQ hook ID */

    /* UART configuration */
    unsigned int uartclk;           /* Reference clock rate (Hz) */
    unsigned int baud_rate;         /* Current baud rate (termios value) */
    unsigned int ier_shadow;        /* Shadow of IMSC register */

    /* RX buffer (circular) */
#define PL011_IBUFSIZE  4096        /* Input buffer size */
    int icount;                     /* Bytes in input buffer */
    char *ihead;                    /* Next free spot */
    char *itail;                    /* First byte to give to TTY */
    char idevready;                 /* Nonzero if hardware ready (RTS) */
    int rx_overrun_events;          /* Count of RX overruns */
    char ibuf[PL011_IBUFSIZE];      /* Input buffer */

    /* TX buffer (circular) */
#define PL011_OBUFSIZE  4096        /* Output buffer size */
    int ocount;                     /* Bytes in output buffer */
    char *ohead;                    /* Next byte to write */
    char *otail;                    /* Next byte to send */
    unsigned char ostate;           /* Output state flags */
#define PL011_ODONE        0x01    /* Output completed */
#define PL011_ORAW         0x02    /* Raw mode for xoff disable */
#define PL011_OWAKEUP      0x04    /* tty_wakeup() pending */
#define PL011_OQUEUED      0x08    /* Output buffer not empty */
#define PL011_OSWREADY     0x10    /* Software ready */
    char obuf[PL011_OBUFSIZE];      /* Output buffer */

    /* Flow control */
    char inhibited;                 /* Output inhibited? */
    char drain;                     /* Drain output then reconfigure? */
    unsigned char oxoff;            /* XOFF character */
};

#endif /* _AARCH64_PL011_H */
