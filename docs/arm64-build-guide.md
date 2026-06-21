# Building MINIX for ARM64 (AArch64)

This document describes how to build MINIX for ARM64 (AArch64) platforms.

## Prerequisites

- AArch64 cross-compiler (GCC 10+ or Clang 12+)
- CMake 3.20+
- Ninja build system
- QEMU with AArch64 system emulation (`qemu-system-aarch64`)

### Installing the Toolchain

**Debian/Ubuntu:**
```bash
sudo apt install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu
```

**Arch Linux:**
```bash
sudo pacman -S aarch64-linux-gnu-gcc aarch64-linux-gnu-binutils
```

**macOS (Homebrew):**
```bash
brew install aarch64-elf-gcc aarch64-elf-binutils
```

## Quick Start

```bash
# Configure for AArch64 debug build
cmake --preset aarch64-debug

# Build the kernel
cmake --build build-aarch64 --target kernel

# Build libraries
cmake --build build-aarch64 --target minc
cmake --build build-aarch64 --target sys

# Full build
cmake --build build-aarch64
```

## CMake Presets

| Preset | Build Type | Description |
|--------|-----------|-------------|
| `aarch64-debug` | Debug | Full debug symbols, no optimizations |
| `aarch64-release` | Release | Optimized, stripped |

### Manual Configuration

```bash
cmake -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
    -DMACHINE_ARCH=aarch64 \
    -DCMAKE_BUILD_TYPE=Debug \
    -S . -B build-aarch64
```

## Build Targets

| Target | Description |
|--------|-------------|
| `kernel` | ARM64 kernel (head.S, vectors.S, mpx.S, klib.S, etc.) |
| `minc` | Minimal C library for servers/drivers |
| `sys` | libsys (spin, timer, serial) |
| `tty` | Terminal driver with PL011 UART support |

## Architecture Support

### Implemented

- **Boot**: EL1 entry, MMU enable (4KB pages, 48-bit VA), identity page tables
- **Exceptions**: Full VBAR_EL1 vector table (16 entries × 128 bytes)
- **Interrupts**: GICv3 (QEMU virt), GICD + GICR, ICC system registers
- **Timer**: ARM Generic Timer (CNTPCT_EL0, CNTP_CVAL_EL0)
- **Memory**: 4-level VMSAv8-64 translation tables (L0–L3)
- **IPC**: MINIX message-passing via SVC #0 (KERVEC_INTR / IPCVEC_INTR)
- **Context**: Full AAPCS64 context save/restore (x0–x30, SP, SPSR, ELR)
- **UART**: PL011 (QEMU virt at 0x09000000, IRQ 33)
- **Libraries**: libsys (spin, timer), libminc (setjmp/longjmp), libc (IPC, ucontext, brk)
- **FDT**: Device Tree parser (memory, CPUs, chosen, stdout-path UART lookup)
- **Limine**: AAC64 boot protocol support (request structures, pre_init)

### In Progress

- **Reset/Poweroff**: PSCI via HVC/SMC
- **Signal handling**: Signal frame layout, sigreturn
- **User-space**: Process creation, EL0 switching
- **Drivers**: Console/keyboard stubs, additional storage

## QEMU Testing

See `scripts/qemu-aarch64.sh` for the QEMU test script.

```bash
# Boot kernel in QEMU (direct kernel boot)
./scripts/qemu-aarch64.sh --kernel build-aarch64/minix/kernel/kernel

# Boot with UEFI (Limine AAC64)
./scripts/qemu-aarch64.sh --uefi --image minix_aarch64.img
```

## References

- [ARM64 Platform Guide](arm64-platform-guide.md)
- [ARM64 Migration Plan](../planning/08_arm64_migration_plan.md)
- [ARM Architecture Reference Manual (ARMv8)](https://developer.arm.com/documentation/ddi0487/)
- [QEMU virt platform documentation](https://www.qemu.org/docs/master/system/arm/virt.html)
