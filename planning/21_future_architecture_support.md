# Future Architecture Support — Beyond x86_64 and ARM64

> **Статус**: 🟡 Планирование (July 2026)
> **Связанные**: `planning/04_target_architecture_support.md` (x86_64 + ARM64), `planning/05_i386_deprecation_timeline.md` (i386 удалена)
> **Приоритет**: Низкий — только после стабилизации x86_64 и ARM64

---

## 1. Обоснование

### 1.1 Почему

GergiOS изначально построен на микроядре MINIX 3, которое уже имеет историческую поддержку нескольких архитектур (i386, ARM, MIPS, SPARC). С переходом на x86_64 и ARM64 как основные цели, и с GUI, не требующим GPU (software rendering), архитектурная портируемость становится достижимой:

- **QEMU поддерживает ~20 архитектур** — тестирование без реального железа
- **Rust + LLVM** — кодогенерация для любой LLVM-цели, включая экзотику
- **Микроядро** — минимум архитектурно-зависимого кода (в отличие от монолитного ядра)
- **Software rendering GUI** — не зависит от GPU драйверов целевой архитектуры
- **Потенциальные пользователи** — энтузиасты со старым/альтернативным железом

### 1.2 Когда

```
Фаза: ПОСЛЕ стабилизации 1.2
Условия:
  - x86_64: production-ready ✅
  - ARM64: kernel boots + basic drivers ✅
  - Core infrastructure (VFS, IPC, drivers) — architecture-independent
  - Rust toolchain for ext4, userspace utilities — работает на целевой архитектуре
```

---

## 2. Матрица архитектур

### 2.1 QEMU + Rust + MINIX feasibility

| Архитектура | QEMU (TCG) | Rust target | Rust Tier | MINIX history | Сложность |
|------------|-----------|-------------|-----------|---------------|-----------|
| **x86_64** | ✅ | `x86_64-unknown-linux-gnu` (и custom) | Tier 1 | ✅ Родная | — |
| **ARM64** | ✅ | `aarch64-unknown-linux-gnu` | Tier 1 | 🟡 Частично | Medium |
| **RISC-V 64** | ✅ | `riscv64gc-unknown-linux-gnu` | Tier 2 | ❌ Нет | Medium-High |
| **RISC-V 32** | ✅ | `riscv32i-unknown-none-elf` | Tier 3 | ❌ Нет | High |
| **PowerPC64** | ✅ | `powerpc64-unknown-linux-gnu` | Tier 2 | ✅ Была (powerpc) | Medium |
| **SPARC64** | ✅ | `sparc64-unknown-linux-gnu` | Tier 3 | ✅ Была (sparc64) | High |
| **m68k** | ✅ | `m68k-unknown-linux-gnu` | Tier 3 | ✅ Была (m68k) | High |
| **MIPS64** | ✅ | `mips64-unknown-linux-gnuabi64` | Tier 3 | ✅ Была (mips64) | Medium-High |
| **MIPS32** | ✅ | `mipsel-unknown-linux-gnu` | Tier 3 | ✅ Была (mips) | Medium |
| **s390x** | ✅ | `s390x-unknown-linux-gnu` | Tier 2 | ❌ Нет | High |
| **Alpha** | ✅ | Нет стабильного | — | ✅ Была (alpha) | Very High |
| **PA-RISC** | ✅ | Нет стабильного | — | ❌ Нет | Very High |
| **LoongArch** | ✅ | `loongarch64-unknown-linux-gnu` | Tier 2 | ❌ Нет | High |
| **SPARC (32-bit)** | ✅ | Нет стабильного | — | ✅ Была (sparc) | Very High |

### 2.2 Приоритеты

```
P0 (CORE):    x86_64, ARM64                    — основные цели           [1.0]
P1 (HIGH):    RISC-V 64, PowerPC64             — современные, QEMU, T2   [1.2+]
P2 (MEDIUM):  MIPS64, MIPS32, s390x            — нишевые, но живые       [1.3+]
P3 (LOW):     SPARC64, m68k                    — ретро/энтузиасты        [2.0+]
P4 (STRETCH): Alpha, PA-RISC, LoongArch, etc.  — экспериментальные        [2.0+]
```

---

## 3. Архитектурные соображения

### 3.1 Что облегчает портирование

Микроядро MINIX 3 изначально спроектировано для портируемости:

```
Arch-dependent:          Arch-independent:
┌─────────────────┐     ┌──────────────────────┐
│ Kernel: startup  │     │ VFS (file systems)    │
│ Kernel: traps    │     │ PM (process mgmt)    │
│ Kernel: context  │     │ VM (virtual mem)     │ ← уже x86_64/ARM64
│ Kernel: IPI/SMP  │     │ RS (reincarnation)   │
│ libc: setjmp     │     │ DS (data store)      │
│ libc: signal     │     │ Все FS drivers       │ ← ext4 на Rust!
│ Drivers: pic/timer│     │ GUI (software render)│ ← не требует GPU
└─────────────────┘     │ Rust userspace utils  │ ← LLVM codegen!
                        └──────────────────────┘
```

**Ключевые факторы**:
1. **Rust** — LLVM backend, не нужно писать ассемблер для каждой архитектуры
2. **Software rendering GUI** — одинаково работает на любой архитектуре с framebuffer
3. **ext4 на Rust** — кросс-компиляция через LLVM, zero platform-specific code
4. **Микроядро** — ~5-10K LOC архитектурно-зависимого кода (vs Linux: ~500K+)

### 3.2 Что нужно портировать для каждой архитектуры

#### Kernel (architecture-dependent)
```
sys/arch/${ARCH}/
├── include/
│   ├── asm.h          ← 50-100 LOC
│   ├── cpu.h          ← 100-300 LOC
│   ├── frame.h        ← trap frame layout
│   ├── intr.h         ← interrupt handling
│   ├── pmap.h         ← page table management
│   └── stack.h        ← stack layout
├── kernel/
│   ├── machdep.c      ← 200-500 LOC (startup, CPU init)
│   ├── mpx.S          ← 100-300 LOC (trap entry/exit, context switch)
│   ├── protect.c      ← MMU setup (page tables)
│   └── trampoline.S   ← multi-CPU startup (if SMP)
└── conf/
    └── files.${ARCH}  ← build configuration
```

#### Libraries (partially)
```
minix/lib/libc/arch/${ARCH}/
├── setjmp.S          ← 20-50 LOC
├── sigpending.S      ← signal handling
└── syscall.S         ← syscall wrapper
```

#### Drivers (board-specific)
```
minix/drivers/
└── ...               ← timer, interrupt controller, serial console
```

### 3.3 Оценка объёма работ

| Компонент | Оценка LOC | Зависит от |
|-----------|-----------|-----------|
| Kernel (arch) | ~1,000-2,000 LOC | Архитектура (MMU, traps) |
| libc (arch) | ~200-500 LOC | ABI, syscall convention |
| Drivers (min) | ~500-1,000 LOC | Платформа (timer, UART, PIC) |
| Build system | ~100-200 LOC | CMake toolchain |
| QEMU config | ~50 LOC | Device tree / command line |
| **Итого (мин)** | **~2,000-4,000 LOC** | Зависит от сложности MMU |

---

## 4. P1: RISC-V 64

### 4.1 Почему RISC-V

- **Открытая ISA** — никаких лицензионных отчислений
- **Растущая экосистема** — Linux, Rust, QEMU поддержка на высоте
- **Rust Tier 2** — `riscv64gc-unknown-linux-gnu` стабилен
- **QEMU** — отличная эмуляция, включая SMP
- **Актуальность** — StarFive VisionFive 2, Milk-V, SiFive HiFive

### 4.2 Особенности

| Аспект | Детали |
|--------|--------|
| **MMU** | Sv39 (39-bit), Sv48 (48-bit), страницы 4KB |
| **Privilege levels** | M-mode (machine), S-mode (supervisor), U-mode (user) |
| **Trap handling** | CSRs: `mtvec`, `stvec`, `sepc`, `scause` |
| **Таймер** | `RDTIME` CSR + CLINT/ACLINT |
| **Interrupt controller** | PLIC (Platform-Level Interrupt Controller) |
| **SMP** | 1-8 ядер через PLIC + IPI |
| **QEMU** | `qemu-system-riscv64 -machine virt` |
| **Rust target** | `riscv64gc-unknown-linux-gnu` (Tier 2) |
| **OpenSBI** | M-mode firmware → S-mode kernel |

### 4.3 План реализации

```
Phase 1: Kernel bootstrap (~4-6 weeks)
  [ ] QEMU virt machine → OpenSBI → S-mode
  [ ] Trap handling (stvec, exception handling)
  [ ] MMU init (Sv39 page tables)
  [ ] Console output (UART 16550)
  [ ] Timer (CLINT/ACLINT)

Phase 2: Process management (~2-3 weeks)
  [ ] Context switch (CSR save/restore)
  [ ] syscall interface (ecall)
  [ ] PM + VM server port
  [ ] IPC (message passing)

Phase 3: Drivers & boot (~2-4 weeks)
  [ ] PLIC interrupt controller
  [ ] Block device (virtio-blk)
  [ ] Network (virtio-net)
  [ ] Boot from initrd/block

Phase 4: Userspace (~2-3 weeks)
  [ ] Rust cross-compilation for riscv64
  [ ] ext4 on RISC-V
  [ ] GUI (software render) on RISC-V
```

---

## 5. P1: PowerPC64

### 5.1 Почему PowerPC64

- **История** — MINIX уже имела поддержку PowerPC (powerpc, powerpc64 планировалась)
- **QEMU** — отличная эмуляция `qemu-system-ppc64` (pseries, mac99)
- **Rust Tier 2** — `powerpc64-unknown-linux-gnu` и `powerpc64le-unknown-linux-gnu`
- **Реальное железо** — POWER9 Blackbird/Talos II (Raptor CS), старые Mac

### 5.2 Особенности

| Аспект | Детали |
|--------|--------|
| **MMU** | Hashed Page Table (HPT) или Radix Tree |
| **Endianness** | Big-endian (ppc64) и Little-endian (ppc64le) |
| **Trap** | `sc` syscall, `mtmsr` для переключения режимов |
| **Таймер** | Decrementer (TB — Time Base) |
| **Interrupt** | XICS / XIVE (для pseries) |
| **QEMU** | `qemu-system-ppc64 -machine pseries` |

---

## 6. P2: MIPS64 / MIPS32

### 6.1 Почему MIPS

- **История** — MINIX активно поддерживала MIPS (R3000, R4000)
- **QEMU** — `qemu-system-mips64` (malta, fulong)
- **Rust Tier 3** — `mips64-unknown-linux-gnuabi64`, `mipsel-unknown-linux-gnu`
- **Реальное железо** — старые роутеры, встраиваемые системы (MIPS32)

### 6.2 Особенности

| Аспект | MIPS64 | MIPS32 |
|--------|--------|--------|
| **Размер регистров** | 64-bit | 32-bit |
| **MMU** | TLB-based | TLB-based |
| **Endianness** | BE (mips) / LE (mipsel) | BE / LE |
| **QEMU** | `qemu-system-mips64 -machine malta` | `qemu-system-mips -machine malta` |
| **Сложность** | Средняя | Средняя |

---

## 7. P2: s390x

### 7.1 Почему s390x

- **Мейнфреймы** — IBM Z, используется в банках/авиации
- **QEMU** — отличная эмуляция
- **Rust Tier 2** — `s390x-unknown-linux-gnu`
- **Архитектурный интерес** — уникальная гарвардская архитектура

### 7.2 Особенности

| Аспект | Детали |
|--------|--------|
| **MMU** |DAT (Dynamic Address Translation) — 3-level page tables |
| **Endianness** | Big-endian |
| **SVC** | Syscall через `SVC` инструкцию |
| **Таймер** | CPU timer (STPT, STCK) |
| **I/O** | Channel subsystem (не memory-mapped I/O) |
| **Сложность** | Высокая — другой подход к I/O |

---

## 8. P3: SPARC64 и m68k

### 8.1 SPARC64

- **История** — MINIX 2 имела SPARC поддержку
- **QEMU** — `qemu-system-sparc64` (Sun4u, Sun4v)
- **Rust** — `sparc64-unknown-linux-gnu` (Tier 3), `sparcv9-sun-solaris` (Tier 2)
- **MMU** — SFMMU (Sparc Reference MMU)
- **Сложность**: Высокая — register windows, сложный MMU

### 8.2 m68k (Motorola 68000)

- **История** — MINIX 1 была написана для m68k!
- **QEMU** — `qemu-system-m68k` (q800 — Macintosh, an5206 — generic)
- **Rust** — `m68k-unknown-linux-gnu` (Tier 3)
- **MMU** — 68851 PMMU или внутренний MMU (CPU32+)
- **Сложность**: Высокая — 32-bit, но нет SSE/NEON, медленная
- **Актуальность**: Низкая — только ретро-энтузиасты

---

## 9. Практические соображения

### 9.1 Что НЕ нужно портировать

Благодаря архитектуре GergiOS, эти компоненты архитектурно-независимы:

```
✅ VFS (filesystem server) — все драйверы ФС (ext4, MFS) на Rust/LLVM
✅ PM (process manager) — независим от архитектуры
✅ RS (reincarnation server) — чистый C
✅ DS (data store) — чистый C
✅ GUI (graphical server) — software rendering, framebuffer
✅ ext4 — Rust + LLVM codegen
✅ Большинство C библиотек — POSIX API
```

### 9.2 Что НУЖНО портировать

```
❌ Kernel: startup (arch-dependent assembly)
❌ Kernel: trap handling (MMU, exceptions)
❌ Kernel: context switch (register save/restore)
❌ libc: setjmp/longjmp
❌ libc: signal handling
❌ Drivers: timer, interrupt controller, UART
❌ Build system: CMake toolchain + cross-compiler
```

### 9.3 Стратегия тестирования

Все архитектуры тестируются **исключительно через QEMU**:

```bash
# RISC-V 64 тест
qemu-system-riscv64 -machine virt -cpu rv64 \
    -kernel gergios.elf -nographic

# PowerPC64 тест
qemu-system-ppc64 -machine pseries \
    -kernel gergios.elf -nographic

# MIPS64 тест
qemu-system-mips64 -machine malta \
    -kernel gergios.elf -nographic

# s390x тест
qemu-system-s390x -machine s390-ccw-virtio \
    -kernel gergios.elf -nographic
```

Для CI/CD: GitHub Actions может запускать QEMU для любой архитектуры (через `apt install qemu-system-*`).

### 9.4 MINIX toolchain

Для каждой архитектуры нужен cross-toolchain:

| Архитектура | Префикс тулчейна | Где взять |
|------------|-----------------|-----------|
| riscv64 | `riscv64-elf64-minix` | Из `sys/arch/riscv64` или crosstool-NG |
| powerpc64 | `powerpc64-elf64-minix` | Из NetBSD/powerpc toolchain |
| mips64 | `mips64-elf64-minix` | Из NetBSD/mips64 toolchain |
| s390x | `s390x-elf64-minix` | Из NetBSD/s390x toolchain |

---

## 10. Дорожная карта

```
v1.0 (2026-2027)  ─── x86_64 ✅ + ARM64 🟡
                         │
v1.2 (2027-2028)  ─── RISC-V 64 + PowerPC64
                         │
v1.3+              ─── MIPS64 + MIPS32 + s390x
                         │
v2.0+              ─── SPARC64 + m68k
                         │
v2.0+ (stretch)    ─── Alpha, PA-RISC, LoongArch
```

### Критерии перехода к P1

- [ ] x86_64: production-ready (boot, drivers, servers, GUI)
- [ ] ARM64: kernel boot, basic drivers, ext4
- [ ] CI/CD: multi-arch QEMU тесты
- [ ] Rust ext4-core: cross-compilation infrastructure готова
- [ ] Хотя бы один энтузиаст с реальным железом (RISC-V board, POWER machine)

---

## 11. Заключение

**Основная идея пользователя верна**: GergiOS с микроядром + Rust + software rendering + QEMU — идеальный кандидат для multi-arch. Это не требует GPU, не требует специфических драйверов, и даёт доступ к растущей аудитории энтузиастов альтернативных архитектур.

**Но**: Это строго post-1.2 задача. Пока x86_64 и ARM64 не стабильны, распыление ресурсов на другие архитектуры контрпродуктивно.

**Рекомендация**: После стабилизации 1.2, начать с RISC-V 64 (самый современный, открытый, Rust Tier 2, QEMU) и PowerPC64 (историческая MINIX поддержка, Rust Tier 2).
