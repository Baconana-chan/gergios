# GergiOS Roadmap

> **Version**: 1.0.0 "Nix" (MINIX 3.4.0)
> **Updated**: 2026-07-06

---

## Overview

GergiOS is a modernized microkernel OS built on the MINIX 3.4.0 foundation.
This roadmap describes the planned releases and their target features.

### Versioning Scheme

```
GergiOS X.Y.Z "Codename" (MINIX 3.4.0)
├── X — Major: architectural changes (new kernel, new libc)
├── Y — Minor: feature releases
├── Z — Patch: bug fixes
└── MINIX X.Y.Z — base microkernel (internal reference)
```

### Architecture Model

```
┌───────────────────────────────────────┐
│        GergiOS Native Apps            │
│  (Rust-компоненты, новый userland)     │
├───────────────────────────────────────┤
│     POSIX (BSD) Userland / NetBSD ABI │
│  ┌─────────┬──────────┬─────────────┐ │
│  │ libc    │  userland│  build sys   │ │
│  │ libm    │  tools   │  (BSD Make)  │ │
│  │ sys/*.h │  (bin/,  │              │ │
│  │         │  usr.bin)│              │ │
│  └─────────┴──────────┴─────────────┘ │
├───────────────────────────────────────┤
│    MINIX Microkernel (kernel,         │
│     servers, drivers, fs, net)        │
└───────────────────────────────────────┘
```

---

## GergiOS 1.0 "Nix" — Q3 2026

**Цель**: Первый стабильный релиз GergiOS. Фундамент заложен:
64-бита ✅, криптография ✅, файловая система ✅, драйверы ✅.
Остаётся: графика, безопасность, сеть, тестирование, ARM64.

### ✅ Уже сделано

#### Build System
- [x] CMake build: kernel, servers, drivers, libs, userland, tests (Phases 1-4)
- [x] CMakePresets.json, cmake-build.sh, dual-build infrastructure
- [x] BSD Make сохранён для NetBSD compat layer

#### Crypto
- [x] OpenSSL 0.9.8 → wolfSSL 5.9.1 (Phases 1-4)
- [x] libhcrypto для heimdal (вместо OpenSSL)
- [x] OpenSSL удалён из дерева сборки
- [x] Все компоненты (syslogd, ftp, httpd, telnet, BIND, netpgp, libevent, …) на wolfSSL

#### C Language & Rust
- [x] C89 → C17 (gnu17, register keyword removed, _Noreturn, _Static_assert)
- [x] Rust workspace: **132+ утилит** (весь usr.bin/ портирован)
- [x] grep в Rust (Quick Search + regex + gzip + mmap)
- [x] CI/CD + ASan/MSan/TSan + fuzzing + benchmarks + code coverage

#### Architecture
- [x] **x86_64 migration**: boot, memory, syscalls, signals, drivers (6 phases)
- [x] **i386 removal**: arch code deleted, build system cleaned
- [x] **aarch64 kernel**: все 28 .o файлов компилируются, 0 ошибок
- [x] **aarch64 sysroot + IPC ABI + libs**: libsys, libminc, libc

#### Branding & pkgsrc
- [x] GergiOS branding: OS_NAME 1.0.0, boot menu, kernel announce, MOTD, shutdown
- [x] **18 MK* флагов**, ~255MB удалено (LLVM, BIND, DHCP, blacklistd)
- [x] Rust utilities: **132+ утилиты** портированы
- [x] Boot library cleanup: cd9660, dosfs, ext2fs, ffs, lfs, nfs, ufs — удалены

#### Файловая система
- [x] **ext4 Rust core** (~7,600 LOC): superblock, extent tree r/w/trunc/merge/split,
      htree directory, flex_bg alloc, jbd2 journal, metadata_csum, xattr, ACL, quota
- [x] **ext4 C bridge**: 29/35 fsdriver callbacks, CMake integration
- [x] **58 unit tests**, 19 benchmarks — все PASS
- [x] VFS cleanup: lfs/, chfs/, ufs/, v7fs/ удалены (~11K строк)
- [ ] **MINIX toolchain integration** — линковка ext4-core staticlib под MINIX
      (ждёт cross-toolchain / DESTDIR / QEMU)

#### Драйверы — полный стек (Phases 1-7)
- [x] **Driver Core** — gergios_driver, gergios_device, dispatch, match, compat
- [x] **PCI scanning** — централизованное pci_scan.c, hot-plug фреймворк
- [x] **DMA API + IOMMU** — 3 backend (direct/bounce/IOMMU), AMD-Vi + Intel VT-d
      с page tables, interrupt remap, per-device domain
- [x] **PM Framework** — ACPI S3, runtime PM, PCI D-state, S4 hibernate (suspend-to-disk
      с vm_map_phys memory image save/restore + PCI state + wake event config)
- [x] **AHCI + e1000 DMA migration** — alloc_contig → gergios_dma_alloc_coherent
- [x] **AHCI + e1000 runtime PM** — idle → D3hot, wake on I/O
- [x] **Rust driver pilots** — minix-pci, minix-ahci, virtio-blk, e1000
- [x] **Multi-queue + Performance** — NCQ (AHCI), MSI-X per-port/per-queue,
      threaded IRQ handlers, SCHED_FIFO RT scheduler, per-CPU drvqueue
- [x] **NVMe driver** (Rust) — admin/I/O queues, PRP list, MSI-X per-queue, APST, D-state
- [x] **xHCI (USB 3.0)** (Rust) — MSC, HID (kbd/mouse/gamepad), Hub, device framework
- [x] **Intel HDA Audio** (Rust) — PCM playback/capture, mixer, NetBSD audioio API
- [x] **ACPI Modernization** — SCI, _PS0/_PS3, GPE routing, PCI hot-plug notify
- [x] **Bluetooth HCI** (Rust) — USB transport, chardev, 45+ device IDs

#### Driver Manager + Linux LKM Compat
- [x] **ELF .ko loader** — 32/64-bit, RELA relocations, symbol resolution
- [x] **Kernel API shim** — ~105 функций (PCI, MMIO, IRQ, DMA, timers, workqueues, sk_buff, firmware)
- [x] **modprobe + drvmanager** — device→module binding, deps, .so loader, SMP pool
- [x] **KLKM daemon** — modprobe/insmod/rmmod/lsmod CLI, IPC protocol
- [x] **depmod** — dependency/alias/symbol generation
- [x] **Driver build system** — ko.mk, ko_install.mk, /lib/modules/ hierarchy
- [x] **Rust .ko loader** — memory-safe ELF parser (14 test cases)

#### Hot-plug
- [x] PCI hot-plug registration + ACPI Notify handler
- [x] gergios_device creation + driver autoloading on ACPI enumeration

#### Bootloader
- [x] **Limine** — UEFI+BIOS bootloader migrated (make_limine_test_image.sh)
- [x] GRUB fallback сохранён
- [ ] Bootloader cleanup: удалить GRUB UEFI код, финализировать Limine

---

### 🟡 Планируется для 1.0

#### x86_64: Финальная очистка
- [ ] **Ramdisk boot drivers** — восстановить для x86_64 (T7)

#### Графический стек / GUI
- [ ] **Framebuffer driver** — современные видеорежимы
- [ ] **Display server** — Wayland compositor для микроядерной архитектуры
- [ ] **Input devices** — клавиатура, мышь
- [ ] **Font rendering** — базовый 2D вывод
- [ ] **NetSurf WebView** — интеграция NetSurf как Wayland-нативного браузера
- [ ] **Window manager** — композитинг, decoration, theming
- [ ] **Multi-touch** — поддержка тачскринов
- [ ] **Clipboard** — copy/paste между приложениями

#### Безопасность
- [ ] **Capability-based security** — design and implementation
- [ ] **MAC framework** — SELinux/AppArmor equivalent
- [ ] **Memory-safe IPC** — Rust-based validation layer
- [ ] **Full security audit** — аудит всей кодовой базы перед 1.0 release

#### Сеть
- [ ] **IPv6 support** — базовая адресация, SLAAC, DHCPv6
- [ ] **Network stack evaluation** — lwIP vs FreeBSD TCP/IP stack
- [ ] **Modern TCP/IP stack** — интеграция выбранного стека

#### ARM64 Platform
- [ ] **ARM64 Platform + Drivers** — RPi 4 специфика (T10)
- [ ] **x86_64 + ARM64** — обе архитектуры в CI/CD

#### Package Manager
- [ ] **apk integration** — Alpine's package manager
- [ ] **GergiOS package repository**
- [ ] **pkgsrc → apk migration** (optional)

#### BlueZ Userspace Port (Phase 8)
- [ ] **BlueZ adapter shim** — D-Bus, HCI socket, GLib для userspace Bluetooth стека

#### Тестирование
- [ ] **QEMU test infrastructure** — automated boot tests for x86_64
- [ ] **Testing framework migration** — Google Test / Catch2
- [ ] **ext4 test suite** — fsck, stress, производительность
- [ ] **Fuzzing** — расширение на C-FFI слой
- [ ] **Performance benchmarks** — сравнение с legacy
- [ ] **Property-based testing** — для критических компонентов (ядро, IPC, FS)

#### Остальное
- [ ] **`lib/libwrap/`** — MK* флаг (tcp_wrappers deprecated)
- [ ] **Minix FS → read-only legacy** — подготовка к удалению

---

## GergiOS 1.1 — Q1 2027

**Цель**: Углубление функциональности после стабильного 1.0.

#### Real-time
- [ ] **Real-time extensions** — детерминированные IRQ latency, RT scheduling классы для userspace

#### Прочее
- [ ] Дополнительные улучшения по мере необходимости

---

## GergiOS 1.2+ — Future

**Цель**: Доведение системы до production-качества с продвинутыми фичами.

#### Продвинутые функции
- [ ] **Container runtime** — namespace-изоляция, cgroups, overlayfs
- [ ] **ext4 enhancements** — надстройки над ext4: snapshot layer (COW на уровне
      блоков), inline compression (zstd/lz4), RAID-подобная агрегация устройств
      (всё реализуется как userspace-сервисы поверх VFS + существующего ext4)

#### Избранное (по мере интереса)
- [ ] **VMware/Guest additions** — паравиртуальные драйверы для виртуализации
- [ ] **WiFi** — загрузка ath9k/iwlwifi через LKM compat
- [ ] **GPU** — загрузка i915/amdgpu через LKM compat (

---

## Legend

| Mark | Meaning |
|------|---------|
| ✅ | Completed |
| 🟡 | In progress / planned |

---

## Dependencies Graph

```
1.0 Foundation (CMake, Crypto, C17+Rust, x86_64, Branding) ✅
    │
    ├─> 1.0 Файловая система (ext4 ✅) — остаётся линковка под MINIX
    ├─> 1.0 Драйверы (всё ✅) — Phases 1-7
    │       ├─> Driver Manager + LKM compat (✅)
    │       ├─> NVMe, xHCI, HDA, BT (✅)
    │       └─> PM: S3, Runtime, S4, Wake config (✅)
    ├─> 1.0 Hot-plug (✅)
    ├─> 1.0 Bootloader (Limine 🟡) — остаётся GRUB cleanup
    │
    ├─> 1.0 GUI (Wayland 🟡) ──> Window Manager + Multi-touch + Clipboard
    ├─> 1.0 Security (Cap + MAC + Audit 🟡)
    ├─> 1.0 Network (IPv6 + Stack eval 🟡)
    ├─> 1.0 ARM64 Platform 🟡
    ├─> 1.0 Package Manager (apk 🟡)
    ├─> 1.0 BlueZ Phase 8 🟡
    ├─> 1.0 Testing (QEMU, framework, fuzzing, benchmarks 🟡)
    └─> 1.0 Ramdisk + MinixFS legacy 🟡
                                  │
                                  └─> 1.1 Real-time extensions
                                  └─> 1.2+ Container runtime + ext4 enhancements
```

---

## Related Documents

- `planning/03_migration_roadmap.md` — component-by-component migration roadmaps
- `planning/23_driver_model_modernization.md` — полная документация драйверов
- `planning/10_netbsd_dependency_audit.md` — NetBSD compatibility strategy
- `planning/15_crypto_migration.md` — OpenSSL → wolfSSL migration
- `planning/09_c_language_modernization.md` — C17 + Rust migration
- `planning/07_x86_64_migration_plan.md` — x86_64 migration
- `planning/08_arm64_migration_plan.md` — ARM64 migration
- `planning/19_ext4_driver_architecture.md` — ext4 driver design
- `planning/17_remaining_tasks.md` — remaining task list
