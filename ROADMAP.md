# GergiOS Roadmap

> **Version**: 1.0.0 "Nix" (MINIX 3.4.0)
> **Updated**: 2026-06-29

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

**Цель**: Первый стабильный релиз GergiOS. Заложить фундамент для всех
ключевых направлений модернизации: 64-бита, криптография, графический стек,
файловая система, драйверы, безопасность.

### ✅ Уже сделано

#### Build System (planning/03 §1)
- [x] CMake build: kernel, servers, drivers, libs, userland, tests (Phases 1-4)
- [x] CMakePresets.json, cmake-build.sh, dual-build infrastructure
- [x] BSD Make сохранён для NetBSD compat layer

#### Crypto (planning/03 §10)
- [x] OpenSSL 0.9.8 → wolfSSL 5.9.1 (Phases 1-4)
- [x] libhcrypto для heimdal (вместо OpenSSL)
- [x] OpenSSL удалён из дерева сборки
- [x] Все компоненты (syslogd, ftp, httpd, telnet, BIND, netpgp, libevent, …) на wolfSSL

#### C Language & Rust (planning/03 §3)
- [x] C89 → C17 (gnu17, register keyword removed, _Noreturn, _Static_assert)
- [x] Rust workspace: **132+ утилит** (весь usr.bin/ портирован)
- [x] grep в Rust (Quick Search + regex + gzip + mmap)
- [x] CI/CD + ASan/MSan/TSan + fuzzing + benchmarks + code coverage
- [x] 6.4.3–6.4.6: 59 утилит (colrm, cal, mcookie, banner, lock, genassym и др.)

#### Architecture (planning/03 §2)
- [x] x86_64 migration: boot, memory, syscalls, signals, drivers (6 phases)
- [x] i386 removal: arch code deleted, build system cleaned (Phase 4)

#### Branding
- [x] GergiOS branding: OS_NAME 1.0.0, boot menu, kernel announce, MOTD, shutdown
- [x] Internal `__minix` defines и `minix/` directory preserved (macOS-модель)

#### pkgsrc Compatibility
- [x] MKGAMES=no (игры через pkgsrc)
- [x] MKLIBTELNET=no (telnet deprecated)
- [x] MKLIBKVM=no (не используется MINIX)
- [x] **18 MK* флагов** (less, tmux, top, nvi, bzip2, file, flex, byacc, LLVM и др.)
- [x] **~255MB удалено** из дерева (LLVM, BIND, ISC DHCP, blacklistd)

---

### 🟡 Планируется для 1.0

#### x86_64: Финальная очистка
- [x] **x86_64 shared code separation** — создан `arch/x86_64/` с 13 файлами (T3 ✅)
- [x] **cmake/options.cmake** — ACPI/APIC/PCI/Watchdog для x86_64 (T6 ✅)
- [x] **x86_64 kernel** — собран 0 ошибок (T5.6 ✅)
- [x] **aarch64 kernel** — собран 0 ошибок (T2a ✅)
- [x] **pre-existing errors** — 4 ошибки исправлены (T5.5 ✅)
- [ ] **Ramdisk boot drivers** — восстановить для x86_64 (T7 🟡)

#### Файловая система (planning/03 §4)
- [x] **VFS/filesystem audit** — очистка `sys/ufs/`, `sys/fs/` от неиспользуемого кода (T20 ✅)
- [x] **Удалены:** `lfs/`, `chfs/`, `ufs/`, `v7fs/` (~11K строк)
- [x] **Research & design** — ext4 архитектура спроектирована (`planning/19`)
- [ ] **ext4 driver — Phase 1 (read-only)** — Rust ext4-core + C FFI bridge
- [ ] **ext4 driver — Phase 2 (write)** — block alloc, extent split/merge
- [ ] **ext4 driver — Phase 3 (journal)** — jbd2 recovery

#### Графический стек / GUI (planning/03 §9 + planning/11)
- [ ] **Framebuffer driver** — современные видеорежимы
- [ ] **Display server** — Wayland compositor для микроядерной архитектуры
- [ ] **Input devices** — клавиатура, мышь
- [ ] **Font rendering** — базовый 2D вывод
- [ ] **NetSurf WebView** — интеграция [NetSurf](https://www.netsurf-browser.org/) (GPLv2, собственный layout engine на C) как Wayland-нативного WebView/browser
      для конфигурационных интерфейсов, справки и базового веб-доступа
      (через [visurf](https://drewdevault.com/blog/visurf-announcement/) или собственный Wayland frontend)
- [ ] **Bootloader modernization** — UEFI support, GRUB2 или systemd-boot

#### Драйверы (planning/03 §5)
- [ ] **Driver framework — design** — современная модель драйверов
- [ ] **USB stack — evaluation** — портирование Linux USB stack
- [ ] **Hot-plug support** — основа (device insertion/removal)

#### Безопасность (planning/03 §6)
- [ ] **Capability-based security** — design and prototype
- [ ] **MAC framework** — design (SELinux/AppArmor equivalent)
- [ ] **Memory-safe IPC** — Rust-based validation layer

#### Сеть (planning/03 §7)
- [ ] **IPv6 support** — базовая реализация
- [ ] **Network stack evaluation** — lwIP vs FreeBSD stack

#### Boot library
- [x] **Cleanup** — удалены неиспользуемые FS/протоколы из `sys/lib/libsa/` (T17/T19 ✅)
      (cd9660, dosfs, ext2fs, ffsv1/2, lfsv1/2, nfs, ufs, ustarfs, nullfs)

#### pkgsrc & Userland
- [x] **Аудит `external/bsd/`** — 18 MK* флагов, ~255MB удалено (LLVM, BIND, DHCP)
- [x] **Rust utilities** — **132+ утилиты** портированы (весь usr.bin/ + build-time)
- [ ] **`lib/libwrap/`** — MK* флаг (tcp_wrappers deprecated)

#### Тестирование
- [ ] **QEMU test infrastructure** — automated boot tests for x86_64
- [ ] **Testing framework migration** (planning/03 §8) — Google Test / Catch2

---

## GergiOS 1.1 — Q1 2027

**Цель**: Расширение заложенного в 1.0 фундамента. ARM64, полный ext4,
зрелый графический стек, Linux совместимость.

#### Architecture
- [x] **ARM64 kernel источники** — 28 .o файлов, 0 ошибок компиляции (T2 ✅)
- [x] **ARM64 sysroot** — кросс-компиляция (T1 ✅)
- [x] **ARM64 IPC ABI** — LP64 message format (T8 ✅)
- [x] **ARM64 libs** — libsys, libminc, libc (T9 ✅)
- [ ] **ARM64 Platform + Drivers** — RPi 4 специфика (T10 🟡)
- [ ] **x86_64 + ARM64** — обе архитектуры в CI/CD

#### Filesystem
- [x] **VFS cleanup** — LFS, CHFS, v7fs, UFS core удалены (T20 ✅)
- [ ] **ext4 — полная поддержка** — journaling, extents, delayed allocation
- [ ] **ext4 FS server** — полноценный сервер для MINIX VFS
- [ ] **Minix FS → read-only legacy** — подготовка к удалению

#### Graphics
- [ ] **Window manager** — композитинг, decoration, theming
- [ ] **Multi-touch** — поддержка тачскринов
- [ ] **Clipboard** — copy/paste между приложениями

#### Drivers
- [ ] **USB stack — port** — EHCI, xHCI, mass storage
- [ ] **Driver migration** — block, char, network drivers на новом framework

#### Security
- [ ] **Capability system — implementation** — IPC-level capabilities
- [ ] **MAC — implementation** — mandatory access control
- [ ] **Rust components** — memory-safe device drivers

#### Network
- [ ] **Modern TCP/IP stack** — интеграция lwIP или FreeBSD stack
- [ ] **IPv6 — full** — адресация, SLAAC, DHCPv6

#### Linux Compatibility
- [ ] **Linux ABI layer** — LACC или аналог для запуска Linux бинарников
- [ ] **Linux driver compat** — портирование драйверов через слой совместимости

#### Package Manager
- [ ] **apk integration** — Alpine's package manager
- [ ] **GergiOS package repository**
- [ ] **pkgsrc → apk migration** (optional)

#### Testing
- [ ] **ext4 test suite** — fsck, stress, производительность
- [ ] **Fuzzing** — расширение на C-FFI слой
- [ ] **Performance benchmarks** — сравнение с legacy

---

## GergiOS 1.2+ — Future

**Цель**: Доведение системы до production-качества.

- [ ] **musl libc как альтернатива NetBSD libc** — не замена, а опция
      для изолированных GergiOS-native компонентов (см. planning/10 §3.5)
- [ ] **Собственная файловая система** (btrfs / ZFS)
- [ ] **Distributed systems support**
- [ ] **Real-time extensions**
- [ ] **Cloud-native features** (container runtime)
- [ ] **Full security audit**
- [ ] **Property-based testing**

---

## Legend

| Mark | Meaning |
|------|---------|
| ✅ | Completed |
| 🟡 | In progress / planned |
| 🔮 | Future / aspirational |

---

## Dependencies Graph

```
1.0 Build System (CMake) ✅
1.0 Crypto (wolfSSL) ✅
1.0 C17 + Rust (132 utils) ✅
1.0 x86_64 (kernel ✅) ──> T7 Ramdisk 🟡
1.0 aarch64 (kernel ✅) ──> T10 Platform 🟡
1.0 i386 Removal ✅
1.0 Branding ✅
1.0 pkgsrc flags (18 MK*) ✅
1.0 VFS Cleanup (T20) ✅
1.0 Boot Library Cleanup (T17/T19) ✅
    │
    ├─> 1.0 ext4 design (planning/19) ──> ext4 Phase 1 (read) ──> ext4 Phase 2-3 (write+journal) ──> 1.1 ext4 full
    ├─> 1.0 GUI (Wayland) ──> 1.1 Window Manager
    ├─> 1.0 Driver framework ──> 1.1 USB + Driver migration
    ├─> 1.0 Security design ──> 1.1 Cap/MAC implementation
    ├─> 1.0 IPv6 + Network eval ──> 1.1 Modern TCP/IP
    ├─> 1.0 Bootloader (UEFI) ──> 1.1 Linux ABI
    └─> 1.0 Testing framework ──> 1.1 Full test suite
                                  │
                                  └─> 1.2+ musl, Production quality
```

---

## Related Documents

- `planning/03_migration_roadmap.md` — component-by-component migration roadmaps
- `planning/10_netbsd_dependency_audit.md` — NetBSD compatibility strategy
- `planning/15_crypto_migration.md` — OpenSSL → wolfSSL migration
- `planning/09_c_language_modernization.md` — C17 + Rust migration
- `planning/07_x86_64_migration_plan.md` — x86_64 migration
- `planning/08_arm64_migration_plan.md` — ARM64 migration (planned)
- `TODO.md` — detailed task list
- `planning/19_ext4_driver_architecture.md` — ext4 driver design
