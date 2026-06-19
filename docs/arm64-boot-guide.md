# GergiOS ARM64 Boot Guide

> **Phase 4**: ARM64 boot infrastructure with Limine AAC64 on QEMU virt
> **Related**: `planning/16_bootloader_modernization.md` §4.5, `planning/08_arm64_migration_plan.md`
> **Requires**: ARM64 kernel port (Phase 1-2 of 08_arm64_migration_plan.md)

---

## 1. Overview

GergiOS uses **Limine AAC64** as the bootloader for ARM64 (AArch64) platforms.
The boot flow is:

```
QEMU virt / UEFI firmware (AAVMF)
    │
    └── EFI/BOOT/BOOTAA64.EFI (Limine AAC64)
            │
            ├── limine.conf
            ├── kernel (ARM64 ELF)
            ├── mod* (boot modules)
            └── qemu-virt.dtb (Device Tree, optional)
```

**Current status**: Boot infrastructure is ready. ARM64 kernel port is in
progress — see `planning/08_arm64_migration_plan.md`.

---

## 2. Prerequisites

### Tools (on Linux build host)

```bash
# Limine with AAC64 support
git clone https://github.com/limine-bootloader/limine.git
cd limine && make && sudo make install
# Verify AAC64 support
ls /usr/local/share/limine/BOOTAA64.EFI

# AArch64 cross-toolchain
sudo apt install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu

# QEMU AArch64
sudo apt install qemu-system-arm qemu-efi-aarch64

# Image creation tools
sudo apt install gdisk dosfstools mtools
```

### Files

- Limine AAC64: `/usr/share/limine/BOOTAA64.EFI` or `/usr/local/share/limine/BOOTAA64.EFI`
- QEMU AAVMF firmware: `/usr/share/qemu-efi-aarch64/QEMU_EFI.fd`

---

## 3. Build the ARM64 Boot Image

```bash
# From project root
./releasetools/arm64_hdimage.sh
```

This creates `minix_arm64.img` with:
| Partition | Type | Content |
|-----------|------|---------|
| 1 (sector 2048) | ESP FAT32 | `EFI/BOOT/BOOTAA64.EFI`, `limine.conf` |
| 2 | MFS ROOT | GergiOS root filesystem |
| 3 | MFS USR | /usr partition |
| 4 | MFS HOME | /home partition |

---

## 4. Boot in QEMU

### Basic UEFI boot:

```bash
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a72 \
    -m 512M \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
    -drive format=raw,file=minix_arm64.img \
    -serial stdio
```

### With Device Tree generation:

```bash
# Generate DTB (for reference)
qemu-system-aarch64 -M virt -cpu cortex-a72 -machine dumpdtb=qemu-virt.dtb

# Boot with custom DTB
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a72 \
    -m 512M \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
    -drive format=raw,file=minix_arm64.img \
    -dtb qemu-virt.dtb \
    -serial stdio
```

### With graphical output:

```bash
qemu-system-aarch64 \
    -M virt \
    -cpu max \
    -m 1G \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
    -drive format=raw,file=minix_arm64.img \
    -device virtio-gpu-pci \
    -vga none \
    -serial stdio
```

---

## 5. Boot Flow Details

### 5.1 UEFI Firmware

1. QEMU loads AAVMF (ARM Architecture Virtual Machine Firmware) — EDK II port
2. AAVMF scans boot options, finds `EFI/BOOT/BOOTAA64.EFI` on the ESP
3. UEFI Secure Boot checks the signature (if enabled)
4. Control passes to Limine AAC64

### 5.2 Limine AAC64

1. Limine reads `limine.conf` from the partition root
2. Parses boot entries — finds `GergiOS ARM64`
3. Loads kernel from `boot:///kernel`
4. Loads boot modules from `boot:///mod*`
5. (Optional) Loads Device Tree from `DTB_PATH`
6. Sets up Limine protocol structures
7. Jumps to kernel entry point (64-bit EL2 or EL1)

### 5.3 Kernel Entry

```
Limine AAC64
    └── ELF entry (ENTRY(MINIX)) — 64-bit EL1
            └── head.S
                    ├── Exception vectors (VBAR_EL1)
                    ├── Stack setup (SP_EL1)
                    ├── Page tables (TTBR0_EL1, TTBR1_EL1)
                    ├── MMU enable (SCTLR_EL1.M=1)
                    └── C startup code
                            └── kmain()
```

---

## 6. Device Tree

QEMU virt provides a Device Tree Blob (DTB) describing the virtual hardware:

| Node | Address | Description |
|------|---------|-------------|
| `/memory` | 0x40000000+ | RAM layout |
| `/cpus` | — | Cortex-A72 cores |
| `pl011@9000000` | 0x09000000 | UART serial |
| `pl031@9010000` | 0x09010000 | RTC |
| `gic@8000000` | 0x08000000 | GIC v2 distributor |
| `virtio_mmio` | 0x0A000000+ | VirtIO devices |

```bash
# Dump QEMU virt DTB for reference:
qemu-system-aarch64 -M virt -cpu cortex-a72 -machine dumpdtb=qemu-virt.dtb

# Decompile to DTS:
dtc -I dtb -O dts qemu-virt.dtb > qemu-virt.dts
```

---

## 7. Current Limitations

| Limitation | Status | Workaround |
|------------|--------|------------|
| ARM64 kernel code | ❌ Not implemented | See `planning/08_arm64_migration_plan.md` |
| Limine AAC64 DM/DT parsing | 🟡 Not tested | Manual DTB path in config |
| UEFI runtime services | ❌ Not implemented | QEMU virt only for now |
| Secure Boot on ARM64 | ❌ Not tested | Same as Phase 3 but with `BOOTAA64.EFI` |
| Physical hardware (RPi 4) | ❌ Not tested | QEMU virt is primary target |
| 32-bit ARM (earm) compatibility | ✅ Unchanged | U-Boot still used for BeagleBone |

---

## 8. U-Boot Alternative (Legacy ARM)

For existing 32-bit ARM platforms (BeagleBone, BeagleBoard-XM),
the existing U-Boot boot flow in `releasetools/arm_sdimage.sh` continues to work:

```
U-Boot → kernel.bin (raw binary) → head.S (earm/32-bit) → kmain
```

No changes are needed to the U-Boot flow — ARM64 boot via Limine AAC64
is a new, parallel boot path.

---

## 9. References

- [Limine Boot Protocol](https://github.com/limine-bootloader/limine)
- [ARMv8-A Architecture Reference Manual](https://developer.arm.com/architectures/cpu-architecture/a-profile)
- [QEMU virt platform documentation](https://www.qemu.org/docs/master/system/arm/virt.html)
- [EDK II / AAVMF](https://github.com/tianocore/tianocore.github.io/wiki/ArmPlatformPkg-AArch64)
- [ARM64 Kernel Bootstrap (OSDev)](https://wiki.osdev.org/ARM64)
- `planning/08_arm64_migration_plan.md` — Detailed ARM64 kernel port plan
- `planning/16_bootloader_modernization.md` §4.5 — Phase 4 status
- `releasetools/arm64_hdimage.sh` — Boot image creation script
