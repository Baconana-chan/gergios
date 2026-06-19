/* ============================================================
 * QEMU virt platform memory map for ARM64
 *
 * Memory-mapped peripherals on QEMU's "virt" machine type
 * (qemu-system-aarch64 -M virt)
 *
 * Reference: QEMU source hw/arm/virt.c
 * ============================================================ */

#ifndef _AARCH64_QEMU_VIRT_H
#define _AARCH64_QEMU_VIRT_H

/* PL011 UART (serial console) */
#define VIRT_UART_BASE          0x09000000
#define VIRT_UART_IRQ           33

/* UART (ARM PrimeCell PL011) register offsets */
#define PL011_DR                0x000   /* Data Register */
#define PL011_FR                0x018   /* Flag Register */
#define PL011_IBRD              0x024   /* Integer Baud Rate */
#define PL011_FBRD              0x028   /* Fractional Baud Rate */
#define PL011_LCRH              0x02C   /* Line Control */
#define PL011_CR                0x030   /* Control Register */
#define PL011_IMSC              0x038   /* Interrupt Mask */
#define PL011_ICR               0x044   /* Interrupt Clear */

/* PL011 Flag Register bits */
#define PL011_FR_TXFE           (1 << 7)  /* Transmit FIFO Empty */
#define PL011_FR_TXFF           (1 << 5)  /* Transmit FIFO Full */
#define PL011_FR_RXFE           (1 << 4)  /* Receive FIFO Empty */

/* PL011 Line Control bits */
#define PL011_LCRH_FEN          (1 << 4)  /* Enable FIFOs */
#define PL011_LCRH_WLEN_8       (3 << 5)  /* 8-bit word length */

/* PL011 Control Register bits */
#define PL011_CR_UARTEN         (1 << 0)  /* UART Enable */
#define PL011_CR_TXE            (1 << 8)  /* Transmit Enable */
#define PL011_CR_RXE            (1 << 9)  /* Receive Enable */

/* GIC v2 (Generic Interrupt Controller) */
#define VIRT_GIC_DIST_BASE      0x08000000
#define VIRT_GIC_CPU_BASE       0x08010000
#define VIRT_GIC_IRQ_START      32        /* First SPI interrupt */

/* System registers */
#define VIRT_RTC_BASE           0x09010000  /* PL031 RTC */

/* RAM layout */
#define VIRT_RAM_BASE           0x40000000
#define VIRT_RAM_SIZE           0x20000000  /* 512MB default */

/* ARM Generic Timer interrupts */
#define VIRT_TIMER_IRQ_PHYS     30
#define VIRT_TIMER_IRQ_VIRT     27
#define VIRT_TIMER_IRQ_HYP      26
#define VIRT_TIMER_IRQ_SEC      29

/* PSCI (Power State Coordination Interface) */
#define VIRT_PSCI_METHOD        0  /* 0 = HVC, 1 = SMC */

#endif /* _AARCH64_QEMU_VIRT_H */
