# ARM64 Platform Guide

Supported ARM64 platforms for MINIX.

## QEMU virt (Primary Development Target)

The QEMU `-M virt` platform is the primary development and testing target.

### Hardware Configuration

| Component | Address / Value | Description |
|-----------|----------------|-------------|
| CPU | Cortex-A72 | ARMv8.0-A, 1–4 cores |
| RAM | 128MB–1GB | Configurable via `-m` |
| UART | 0x09000000 | PL011, IRQ 33, 24MHz UARTCLK |
| GIC | 0x08000000 (GICD) | GICv3, 0x080A0000 (GICR) |
| Timer | CNTPCT_EL0 | ARM Generic Timer, 62.5MHz (QEMU default) |
| RTC | 0x09010000 | PL031 |
| PCIe | 0x3F000000 | ECAM, IRQs 64+ |

### Memory Map (QEMU virt)

```
Start           End             Size    Description
0x00000000      0x00000800      2KB     Boot ROM (secure)
0x08000000      0x08010000      64KB    GICv3 Distributor (GICD)
0x080A0000      0x080C0000      128KB   GICv3 Redistributor (GICR)
0x09000000      0x09001000      4KB     PL011 UART
0x09010000      0x09011000      4KB     PL031 RTC
0x09040000      0x09041000      4KB     ACPI generator
0x40000000      varies          RAM     First DIMM
```

### QEMU Command Line

```bash
qemu-system-aarch64 \
    -M virt -cpu cortex-a72 \
    -m 256M \
    -kernel kernel.bin \
    -nographic \
    -serial mon:stdio
```

### GIC Details

- **GIC version**: v3 (emulated)
- **GICD base**: 0x08000000
- **GICR base**: 0x080A0000
- **IPI**: SGI #0 (IRQ 0)
- **Timer interrupt**: PPI #14 (IRQ 30, EDGE)
- **UART interrupt**: SPI #1 (IRQ 33, EDGE)
- **RTC interrupt**: SPI #2 (IRQ 34, EDGE)

### Device Tree

QEMU virt provides a Device Tree Blob at the entry point (x0 register).
MINIX parses it via the FDT parser (`fdt.c`) for:
- Memory size and layout
- CPU count
- Boot arguments (`/chosen/bootargs`)
- UART base address (`/chosen/stdout-path`)

## Raspberry Pi 4 (Secondary Target)

Not yet fully supported. Memory map for reference:

### Hardware Configuration

| Component | Address / Value | Description |
|-----------|----------------|-------------|
| CPU | Cortex-A72 | BCM2711, 4 cores @ 1.5GHz |
| UART0 | 0xFE215000 | PL011, main serial console |
| UART1 | 0xFE201000 | Mini UART (auxiliary) |
| GIC | 0xFF840000 (GICD) | GIC-400 (GICv2) |
| ARM Local | 0xFF800000 | Per-CPU mailboxes, timer IRQs |
| Mailbox | 0xFE00B880 | VC mailbox for firmware calls |
| GPIO | 0xFE200000 | BCM2711 GPIO controller |
| PCIe | 0xFD500000 | BCM2711 PCIe root complex |

### Known Differences from QEMU virt

| Aspect | QEMU virt | RPi 4 |
|--------|-----------|-------|
| GIC version | v3 | v2 (GIC-400) |
| UART base | 0x09000000 | 0xFE215000 |
| GICD base | 0x08000000 | 0xFF840000 |
| GICC base | N/A (sysregs) | 0xFF842000 |
| UART clock | 24 MHz | 48 MHz |
| Boot | EL1 entry | EL2→EL1 drop |
| DTB | Passed in x0 | Loaded by firmware |

## Generic ARMv8-A (Tertiary Target)

For servers and SBSA-compliant platforms:

- UEFI boot via Limine AAC64
- ACPI tables (fallback: FDT)
- GIC v3 or v4
- PL011 or SBSA UART
- ARM Generic Timer

## Platform Detection

At boot, MINIX detects the platform via:
1. FDT `/model` property ("linux,dummy-virt" for QEMU, "Raspberry Pi 4" for RPi)
2. GIC version probe (ICC_SRE_EL1.SRE for v3, GICD_TYPER for v2)
3. MIDR_EL1 register for CPU identification

## Debugging

### UART Output

The kernel uses the PL011 UART for early debug output:
- QEMU virt: 0x09000000 (IRQ 33)
- Automatically detected from FDT `/chosen/stdout-path`

### GDB

```bash
qemu-system-aarch64 -M virt -cpu cortex-a72 -m 256M \
    -kernel kernel.bin -nographic \
    -s -S    # Wait for GDB connection

# In another terminal:
aarch64-linux-gnu-gdb kernel.elf
(gdb) target remote :1234
(gdb) break arm64_boot
(gdb) continue
```

## References

- [ARM64 Build Guide](arm64-build-guide.md)
- [QEMU virt Platform](https://www.qemu.org/docs/master/system/arm/virt.html)
- [BCM2711 ARM Peripherals](https://www.raspberrypi.com/documentation/computers/processors.html)
- [ARM GICv3 Specification (IHI 0069)](https://developer.arm.com/documentation/ihi0069/)
