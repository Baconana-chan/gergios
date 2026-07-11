# Consolidated Remaining Tasks — Сводка оставшихся задач

> **Цель**: Единый список всех оставшихся задач из planning/01–28.
> **Дата**: 2026-07-11
> **Статус**: 🟡 Актуален — отражает все незавершённые задачи из всех 28 planning-документов.

---

## А. ARCHITECTURE & PLATFORM

### A1. x86_64: Ramdisk boot drivers 🟡
**Источник**: T7 (бывший), planning/03
**Статус**: 🟡 Код драйвера портирован (minix/drivers/storage/memory/ через IPC). Полный boot chain требует QEMU тестирования.
**Блокируется**: QEMU test infra (недоступен на Windows CI)

### A2. ARM64: Platform + Drivers (RPi 4) 🟡
**Источник**: T10 (бывший), planning/08
**Статус**: 🟡 FDT parser ✅ + PL011 MINIX driver ✅ + console/keyboard stubs ✅
**Остаётся**: BCM283x GPIO, Mailbox, USB (dwc2), GIC-400, PCIe, SDHCI (emmc2)
**Зависит**: A3

### A3. ARM64 SMP 🔮
**Источник**: Новая
**Статус**: 🔮 После стабилизации UP ARM64:
- PSCI CPU_ON для secondary cores
- GICv3 SGI (IPI)
- ACPI/DTB CPU discovery
- Per-CPU structures + cacheline padding
**LOC**: ~1500 | **Приоритет**: P3

---

## B. TOOLCHAIN & CROSS-COMPILATION

### B1. MINIX cross-toolchain — nightly Rust + -Zbuild-std 🟡
**Источник**: Новая
**Статус**: 🟡 **Rust cross-compilation работает через nightly + -Zbuild-std=core,alloc**:
- Target specs `x86_64-unknown-minix.json` + `aarch64-unknown-minix.json` ✅
- `releasetools/setup_minix_sysroot.sh` (check/headers/libs/install-target/test) ✅
- Cross-compilation no_std crates: `cargo +nightly build -Zbuild-std=core,alloc --target x86_64-unknown-minix --lib` ✅
- ext4-core кросс-компилируется (no_std + alloc) ✅
- Слинковать Rust staticlib в MINIX образ (остаётся): ext4-core → ext4 driver, AHCI, NVMe, e1000, virtio
- QEMU boot test с ext4 rootfs
**Остаётся**: DESTDIR sysroot + линковка в MINIX образ
**Блокирует**: C1 (частично), D1, E1

---

## C. FILESYSTEM

### C1. ext4 — MINIX boot & stress testing 🟡
**Источник**: Новая
**Статус**: 🟡 ext4-core (7,600 LOC) ✅:
- Pure Rust ext4 parser (superblock, extent tree, htree, directory, journal) ✅
- **no_std + alloc conversion completed** (cross-compiles with nightly -Zbuild-std=core,alloc) ✅
- C bridge (29/35 callbacks) ✅
- Release scripts ✅
**Остаётся**: линковка под MINIX (ждёт B1), QEMU boot, fsck/stress, MFS read-only finalization
**Зависит**: B1

### C2. MINIX FS (MFS) — read-only deprecation 🟡
**Источник**: Новая
**Статус**: 🟡 MFS_READONLY define есть. Нужно: verify ro mode, EROFS на write, удалить MFS write код
**LOC**: ~200 | **Зависит**: C1

---

## D. DRIVERS

### D1. Rust driver — MINIX QEMU integration test 🟡
**Источник**: Новая
**Статус**: 🟡 Rust драйверы реализованы, не тестированы под MINIX:
- AHCI: SATA r/w в QEMU
- NVMe: namespace I/O
- e1000/virtio-net: сетевой I/O
- virtio-blk: block I/O
- xHCI: USB device enumeration
- PCI: device scan + BAR + MSI-X
**Зависит**: B1

### D2. Bluetooth — Rust-native stack завершён ✅ (BlueZ НЕ НУЖЕН)
**Источник**: planning/27
**Статус**: ✅ **Rust Bluetooth stack полностью заменяет BlueZ**:
- C API (libbluetooth, 13 функций) ✅
- CLI (bt-tool, 12+ команд) ✅
- Daemon (IPC, L2CAP, SDP, RFCOMM, GATT, pairing) ✅
- **BlueZ/D-Bus/GLib НЕ ТРЕБУЮТСЯ** — нативная реализация их заменяет
- 🔮 HCI socket эмуляция для совместимости с Linux BlueZ-инструментами (post-1.0)
**LOC**: ~8000 | **Приоритет**: ✅

### D3. Intel HDA — audio server integration 🟡
**Источник**: Новая
**Статус**: 🟡 Rust HDA driver ✅ (PCM, mixer, audioio API)
**Остаётся**: audio server, ALSA-совместимый ioctl, libaudio, amixer/aplay
**LOC**: ~2000 | **Приоритет**: P3

---

## E. NETWORK — Phase 6 Integration 🟡

### E1. Network Phase 6: QEMU stability & regression 🟡
**Источник**: T24 (бывший), planning/25
**Статус**: 🟡 Phases 1–5 ✅:
- lwIP 2.2.1, INET6, TSO/GRO, SYN cookies, WireGuard, IPsec, DTLS, monitoring ✅
**Остаётся Phase 6**:
- QEMU stability: e1000 + virtio-net под MINIX
- TCP throughput regression suite
- WireGuard key exchange test
- IPsec tunnel up/down test
- DTLS handshake test
- IPv6 SLAAC + DHCPv6 test
**LOC**: ~1500 | **Зависит**: B1, D1

---

## F. BOOTLOADER — Limine Modernization 🟡

### F1. Dual-boot QEMU test 🟡
**Источник**: T12
**Статус**: 🟡 BIOS + UEFI boot paths — требует QEMU

### F2. UEFI Boot (x86_64) 🟡
**Источник**: T13
**Статус**: 🟡 OVMF + Limine UEFI — ждёт QEMU

### F3. Secure Boot evaluation 🟡
**Источник**: T14
**Статус**: 🟡 Инфраструктура готова

### F4. ARM64 Boot (Limine AAC64) 🟡
**Источник**: T15
**Статус**: 🟡 Request structures ✅, kernel port в процессе (A2)

### F5. GRUB Removal ❌
**Источник**: T16
**Статус**: ❌ Ждёт F2

---

## G. DEBUGGING — GDB Remote Stub 🟡

### G1. GDB Stub Phase 1: Serial + базовые команды 🟡
**Источник**: planning/24_gdb_stub_debugger.md
**Описание**: GDB-совместимый remote serial stub как userspace сервис.
Вместо KGDB (как в Linux) — отдельный `drivers/debug/gdb_stub/` сервис через serial.
**Phase 1** (~1,000 LOC): serial init, GDB protocol parser, memory read/write (sys_safecopy), register get/set (DIAGCTL), main loop
- [ ] serial.c — UART 16550 init + send/recv
- [ ] protocol.c — GDB packet encode/decode
- [ ] memory.c — sys_safecopy read/write
- [ ] registers.c — DIAGCTL get/set regs
- [ ] main.c — entry point + dispatch
**LOC**: ~1,000 | **Приоритет**: P4

### G2. GDB Stub Phase 2: Breakpoints + single-step 🟡
**Источник**: planning/24
**Phase 2** (~400 LOC): software INT3 breakpoints, hardware DR0-DR3, continue, single-step (TF flag)
- [ ] breakpoint.c — sw + hw BP management
- [ ] continue.c — continue + single-step + wait
**Зависит**: G1 | **LOC**: ~400 | **Приоритет**: P4

### G3. GDB Stub Phase 3: Multi-thread + QoL 🟡
**Источник**: planning/24
**Phase 3** (~700 LOC): qSupported, qXfer, thread list, watchpoints, kernel memory, panic hook, RS integration
- [ ] query.c — qSupported, qXfer, thread list
- [ ] watchpoint.c — DR1-DR3
- [ ] phys_mem.c — kernel memory access
- [ ] panic_hook.c — kgdb_panic вместо panic
- [ ] rs_integration.c — RS notify при падении
**Зависит**: G2 | **LOC**: ~700 | **Приоритет**: P4

### G4. Kernel changes for GDB 🟡
**Источник**: planning/24
- [ ] DIAGCTL коды: GET_REGS, SET_REGS, SINGLE_STEP, HWBP_SET/CLEAR, BP_NOTIFY
- [ ] proc.h: p_debug_dr[4], dr6, dr7
- [ ] exception.c: #DB handler (TF + HW breakpoints)
- [ ] switch.S: DR save/restore в context switch
**LOC**: ~275 | **Зависит**: G1

---

## H. REINCARNATION SERVER — Extended RS 🔮

### H1. RS Level 2: Healthchecks 🔮
**Источник**: planning/22_rs_reincarnation_server.md
**Описание**: Активный мониторинг здоровья сервисов (ping, heartbeat, ресурсы, время ответа)
- [ ] struct rs_healthcheck, healthcheck framework
- [ ] Периодическая проверка в do_period()
- [ ] RS_REGISTER_HEALTHCHECK IPC
**LOC**: ~400 | **Приоритет**: P4

### H2. RS Level 3: Dependency graph 🔮
**Источник**: planning/22
- [ ] struct rs_dep, таблица зависимостей
- [ ] cascade_restart() — рестарт в правильном порядке
- [ ] Критические vs некритические зависимости
**LOC**: ~300 | **Приоритет**: P4

### H3. RS Level 4: Diagnostics & analysis 🔮
**Источник**: planning/22
- [ ] struct rs_diag_packet — диагностика при падении
- [ ] analyze_failure() — SIGSEGV, OOM, deadlock, timeout
- [ ] Core dump, stack trace, IPC лог
**LOC**: ~1,000 | **Приоритет**: P5

### H4. RS Level 5: Proactive recovery 🔮
**Источник**: planning/22
- [ ] recovery_plan → 12+ стратегий
- [ ] VM_FREE_MEM, VFS_CLEAR_CACHE, fallback driver
- [ ] IPC с VM, VFS, scheduler
**LOC**: ~700 | **Приоритет**: P5

### H5. RS Level 6: "Заботливая мамочка" 🔮
**Источник**: planning/22
- [ ] Полный recovery loop: падение → диагностика → анализ → рестарт × N → surrender
- [ ] Человеческий отчёт с рекомендациями, core dump
**LOC**: ~300 | **Приоритет**: P5

---

## I. FOREIGN FILESYSTEM ACCESS 🔮

### I1. Partition parser + CLI (Phase 1) 🔮
**Источник**: planning/20_foreign_filesystem_access.md
**Описание**: GergiOS как live rescue OS — сканирует диски, определяет ext4/NTFS/FAT32/APFS, копирует файлы.
**Phase 1** (6-8 weeks):
- [ ] gergios-partition crate — MBR + GPT parser
- [ ] gergios-fs-foreign — ForeignFilesystem trait
- [ ] ext4 adapter → ForeignFilesystem
- [ ] gergios-import CLI — scan, ls-foreign, import
- [ ] Block I/O — raw disk + partition-relative reader
**LOC**: ~1,500 | **Приоритет**: P5

### I2. NTFS + FAT32 drivers (Phase 2) 🔮
**Источник**: planning/20
- [ ] NTFS adapter (ntfs crate) - read
- [ ] FAT32 adapter (fatfs crate)
- [ ] gergios scan/ls-foreign/import для NTFS/FAT32
**LOC**: ~1,000 | **Приоритет**: P5

### I3. OS detection + Smart Import (Phase 3) 🔮
**Источник**: planning/20
- [ ] OS detection: Linux (/etc/os-release), Windows (Users/), macOS (/Users/)
- [ ] Smart import: --from linux/windows/macos
- [ ] Deduplication, conflict resolution, progress bar
**LOC**: ~1,000 | **Приоритет**: P5

### I4. VFS Integration (Phase 4) 🔮
**Источник**: planning/20
- [ ] VFS proxy → real mount point (/foreign/)
- [ ] Automatic mount on boot
- [ ] Symlink farm: /foreign/linux/, /foreign/windows/
**LOC**: ~1,500 | **Приоритет**: P5

### I5. APFS + Advanced FS (Phase 5) 🔮
**Источник**: planning/20
- [ ] APFS adapter (experimental crate)
- [ ] exFAT, HFS+, Btrfs, ZFS read (experimental)
**LOC**: ~2,000 | **Приоритет**: P5

---

## J. GUI & GRAPHICS 🔮

### J1. Phase 1: Foundation (DRM/KMS) 🔮
**Источник**: planning/11_gui_architecture.md
- [ ] Port libdrm userspace to MINIX
- [ ] Rust-safe DRM bindings (minix-drm)
- [ ] KMS на framebuffer driver
- [ ] Rust-safe Input bindings (minix-input)
- [ ] MVP: Rust рисует пиксели на framebuffer
**LOC**: ~2,000 | **Приоритет**: P5

### J2. Phase 2: Software Renderer + Fonts 🔮
**Источник**: planning/11
- [ ] Software 2D rasterizer (pixmap ops, alpha-blend)
- [ ] Font stack: ttf-parser + rustybuzz + swash
- [ ] Text rendering (UTF-8, BiDi)
- [ ] Software cursor
**LOC**: ~2,500 | **Приоритет**: P5

### J3. Phase 3: Wayland Compositor MVP 🔮
**Источник**: planning/11
- [ ] Event loop (calloop + MINIX IPC backend)
- [ ] wayland-server-rs на MINIX
- [ ] wl_compositor + wl_surface + xdg_shell
- [ ] wl_seat + input (keyboard, pointer, touch)
- [ ] wl_data_device (copy-paste)
**LOC**: ~4,000 | **Приоритет**: P5

### J4. Phase 4: Window Manager 🔮
**Источник**: planning/11
- [ ] Tiling WM (как i3/sway)
- [ ] Floating windows (drag, resize, snap)
- [ ] Window decorations (software render)
- [ ] Workspaces, keyboard shortcuts
- [ ] Lua-конфигурируемая панель/статус-бар
**LOC**: ~3,000 | **Приоритет**: P5

### J5. Phase 5: GUI Toolkit 🔮
**Источник**: planning/11
- [ ] Slint или iced адаптация для Wayland на MINIX
- [ ] Demo apps: terminal, file manager, calculator
- [ ] Lua GUI bindings, themes, fonts
**LOC**: ~3,000 | **Приоритет**: P5

### J6. Phase 6: Hardware Acceleration 🔮
**Источник**: planning/11
- [ ] Vulkan software fallback (Mesa Lavapipe)
- [ ] VirGL для виртуальных GPU (QEMU)
- [ ] wgpu hardware backend → 60fps vsync
**LOC**: ~2,000 | **Приоритет**: P5

---

## K. SECURITY — Ongoing Hardening 🟡

### K1. Service-by-service capability audit 🟡
**Источник**: Практическая задача
**Статус**: 🟡 MAC framework ✅, capability model ✅, audit subsystem ✅ (все 6 фаз завершены).
**Остаётся**: аудит каждого MINIX-сервера (PM, VFS, VM, RS, DS, MIB...), миграция system.conf на capability-ориентированные политики, MAC STRICT mode, интеграционные тесты. **Можно делать прямо сейчас.**
**LOC**: ~2,000

### K2. CFI/SafeStack production-ready 🟡
**Источник**: Практическая задача
**Статус**: 🟡 CFI и SafeStack есть в cmake (OFF по умолчанию).
**Остаётся**: тестирование с CFI в CI, SafeStack для всех серверов, ASan для C кода. **Можно делать прямо сейчас.**
**LOC**: ~500

---

---

> **🔮 = post-1.0, не активно планируется.** Большинство 🔮 задач (GUI, Extended RS, Foreign FS, Future arch) — это концептуальные направления на будущее, а не взятые обязательства. Они включены для полноты картины, но не блокируют 1.0.

## M. COMPLEX PACKAGES (Deferred) 🔮

### M1. Deferred Rust ports (Categories A-B) 🔮
**Источник**: planning/18_complex_packages.md
**Категория A** (сложные, но реализуемые на Rust):
- [ ] indent — C formatter (~3-5K LOC)
- [ ] m4 — macro processor (~2-4K LOC)
- [ ] unzip — ZIP extraction (~1.5K LOC через zip crate)
**Категория B** (требуют внешних библиотек):
- [ ] bzip2 — через bzip2 crate (~500 LOC)
- [ ] tput / infocmp — terminfo DB (~2K LOC)
- [ ] man — через mandoc или w3m (~3K LOC)
**Приоритет**: P5

### M2. Kernel API utilities (Categories C-D) 🔮
**Источник**: planning/18
**Категория C** (требуют kernel API):
- [ ] netstat — sysctl, kvm, routing sockets
- [ ] ifconfig — SIOCGIF* ioctl, PF_KEY
- [ ] route — PF_ROUTE socket
- [ ] sysctl — MIB tree
- [ ] ps — kvm (kern.proc)
**Категория D** (критическая инфраструктура):
- [ ] init — замена на GergiOS-native system manager
- [ ] sh/csh — Rust shell
- [ ] make → CMake (уже идёт)
**Приоритет**: P6

---

## N. FUTURE ARCHITECTURE SUPPORT 🔮

### N1. RISC-V 64 port 🔮
**Источник**: planning/21_future_architecture_support.md
**Статус**: 🔮 QEMU virt machine, Sv39 MMU, PLIC, OpenSBI firmware.
Rust Tier 2 (`riscv64gc-unknown-linux-gnu`)
**Phases**: Kernel bootstrap → Process mgmt → Drivers → Userspace
**LOC**: ~3,000 | **Приоритет**: P6

### N2. PowerPC64 port 🔮
**Источник**: planning/21
**Статус**: 🔮 QEMU pseries machine, HPT/Radix MMU, XICS/XIVE
Rust Tier 2 (`powerpc64-unknown-linux-gnu`)
**LOC**: ~3,000 | **Приоритет**: P6

### N3. MIPS64/32 + s390x ports 🔮
**Источник**: planning/21
**Статус**: 🔮 Post-RISC-V и PowerPC
**LOC**: ~5,000 | **Приоритет**: P6

---

## O. CI/CD — Expansion 🔮

### O1. ARM64 QEMU testing 🔮
**Статус**: 🔮 Сейчас smoke тесты только для x86_64
**Зависит**: F4

### O2. Flaky test retry + notifications 🔮
**Статус**: 🔮 Retry logic + Slack/Discord webhook
**LOC**: ~200

---

## P. DOCUMENTATION 📝

### P1. GergiOS man pages — rebranding completion 🟡
**Статус**: 🟡 Основное rebranding ✅. Остаётся: man pages (intro, boot, system.conf, drivers), gergios.dev links, logo
**LOC**: ~500

### P2. Developer documentation 🟡
**Статус**: 🟡 CI docs ✅, testing guide ✅. Остаётся: onboarding guide, API docs, cargo doc, ext4 integration guide
**LOC**: ~2,000

---

## Q. FUTURE IDEAS 🔮

### Q1. pkgsync — BitTorrent-транспорт для pkgsrc 🔮
**Статус**: 🔮 Userspace демон для peer-to-peer распространения пакетов в кластере. Не актуально без реального кластера.

### Q2. Container runtime 🔮
**Статус**: 🔮 Post-1.2. LOC: ~5,000+

### Q3. pkgsrc → apk migration 🔮
**Статус**: 🔮 LOC: ~3,000

---

## Сводка по приоритетам

| Приоритет | Задачи | Статус |
|-----------|--------|--------|
| 🔴 Hard Blocker | — (снят) | B1 больше не Hard Blocker |
| 🟡 Toolchain | B1 (nightly Rust + -Zbuild-std) | **Работает, остаётся линковка** |
| 🟡 Architecture | A1 (ramdisk), A2 (ARM64 RPi 4), P1-P2 (docs) | Can proceed in parallel |
| 🟡 Security | K1 (cap audit), K2 (CFI/SafeStack) | **Можно делать прямо сейчас** |
| 🟡 Toolchain | B1 (nightly Rust), L1 (wolfSSL submodule) | **B1 работает** |
| 🟡 Filesystem | C1 (ext4 boot), C2 (MFS ro) | Ждут B1 |
| 🟡 Drivers | D1 (Rust QEMU), D3 (HDA audio) | Ждут B1 / P3 |
| 🟡 Debugging | G1-G4 (GDB stub) | Можно делать (serial + userspace) |
| 🟡 Network | E1 (Phase 6) | Ждёт B1 + D1 |
| 🟡 Bootloader | F1-F5 | Ждут QEMU |
| 🔮 Future | A3 (ARM64 SMP), H1-H5 (Extended RS), I1-I5 (Foreign FS), J1-J6 (GUI), L2 (VFS), M1-M2 (Complex pkgs), N1-N3 (Future arch), O1-O2 (CI), Q1-Q3 | Post-1.0 |
| **Итого** | **~68 задач** | **0 🔴, 20 🟡, 48 🔮** |

---

## Critical Path

```
B1 (Rust cross-toolchain 🟡 — работает через nightly + -Zbuild-std)
    ├──→ C1 (ext4 boot 🟡 — кросс-компилируется ✅, ждёт линковку) ──→ C2 (MFS ro 🟡)
    ├──→ D1 (Rust driver QEMU 🟡) ──→ E1 (Network Phase 6 🟡)
    └──→ D2 (Bluetooth ✅ — native Rust заменяет BlueZ)

K1 (cap audit 🟡) + K2 (CFI 🟡) ── можно делать прямо сейчас, не ждут ничего
G1-G4 (GDB stub 🟡) ── можно идти параллельно

A1 (ramdisk 🟡) ── ждёт QEMU
A2 (ARM64 RPi 4 🟡) ──→ A3 (ARM64 SMP 🔮) → F4 (ARM64 AAC64 🟡)

F1-F3 (boot tests 🟡) ── ждут QEMU → F5 (GRUB removal ❌)

H1-H5 (Extended RS 🔮) ── post-1.0
I1-I5 (Foreign FS 🔮) ── post-1.0
J1-J6 (GUI 🔮) ── post-1.0
M1-M2 (Complex pkgs 🔮) ── post-1.0
N1-N3 (Future arch 🔮) ── post-1.0
P1-P2 (Docs 🟡) ── может идти параллельно
```

---

## Что можно делать прямо сейчас (без QEMU, без B1)

| Задача | Описание | Зависимости | Оценка |
|--------|----------|------------|--------|
| **K1** | Service-by-service capability audit | Нет | 2-3 недели |
| **K2** | CFI/SafeStack CI testing | Нет | 1 неделя |
| **P1** | Man pages rebranding | Нет | 1 неделя |
| **P2** | Developer documentation | Нет | 2-3 недели |
| **L1** | wolfSSL submodule setup | Нет | 1 день |
| **M1** | unzip на Rust (через zip crate) | Нет | 1-2 недели |
| **G1** | GDB stub Phase 1 | kernel DIAGCTL (G4) | 2-3 недели |

---

> **pkgsync — BitTorrent-транспорт для pkgsrc**: Userspace демон для peer-to-peer распространения пакетов. Не актуально без кластера MINIX машин. 🔮
