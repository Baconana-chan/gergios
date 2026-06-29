# Consolidated Remaining Tasks — Сводка оставшихся задач

> **Цель**: Единый список всех оставшихся задач из planning/01–16.
> **Дата**: June 2026

---

## 0. Planning Audit — Исправленные противоречия

| # | Противоречие | Было | Стало |
|---|-------------|------|-------|
| A | **03 §Phase 5 (i386 Removal): "PARTIALLY COMPLETE" vs 05 §Phase 4: "✅ Complete"** | 03: 7 remaining items; 05: всё ✅ | ✅ **Синхронизировано**: 03 now links to 17 §T1–T7 for remaining x86_64 cleanup |
| B | **04 "Full x86_64 implementation complete" vs 03 remaining cleanup** | 04: ✅ Complete; 03: 7 items | ✅ **Синхронизировано**: 04 header updated to "Full x86_64 kernel implementation complete" |
| C | **08 Phase 2 status "🟢 Complete" при ⬜ задачах 15–16** | 🟢 Complete | 🟡 **Исправлено**: "🟡 Source files complete — сборка и sysroot не завершены" |
| D | **08 Phase 8 header 🟢 при 0 checkboxes** | 🟢 Easiest | 🟡 **Исправлено**: "🟡 Planned (not started)" |
| E | **14 header "Status: Planning" при ✅ sub-phases** | Planning | ✅ **Исправлено**: "Status: Completed (6a–6d)" |
| F | **03 не упоминает aarch64 CMake work** | Пропущено | ✅ **Добавлена** ссылка на 08 §Phase 1 |
| G | **Нет файла planning/12_** | Пропуск в нумерации | Добавлен комментарий: reserved for future use |

---

## 1. HARD BLOCKERS — Без них ARM64 не компилируется

### T1. MINIX sysroot для кросс-компиляции ARM64 ✅
**Статус**: ✅ Завершено

### T2. Сборка kernel для aarch64 ✅
**Статус**: ✅ Завершено — все .o файлы созданы (0 ошибок компиляции)

### T2a. Настройка линкера aarch64-elf (lld) 🔴
**Статус**: 🔴 Hard Blocker — финальная линковка невозможна

---

## 2. ARCHITECTURE — x86_64 / ARM64

### x86_64: Очистка shared i386 arch кода

### T3. Создать `minix/kernel/arch/x86_64/` с 64-bit native assembly ✅
**Описание**: Полная изоляция x86_64 arch кода от arch/i386/. Создано 13 файлов:
- **arch/х86_64/**: arch_clock.c, arch_do_vmctl.c, arch_system.c, exception.c, i8259.c, arch_reset.c, direct_tty_utils.c, memory.c, pg_utils.c, protect.c, pre_init.c, procoffsets.cf
- **arch/x86_64/include/**: arch_proto.h (новый)
**Статус**: ✅ Завершено
**Багфиксы**: vm_memset sign-extension fix (memory.c), LSTAR 64-bit addr fix (protect.c)
**Примечание**: CMake configure для x86_64 проходил с ошибками в usr.bin/commands — **ИСПРАВЛЕНО** (см. ниже).

**⚠️ Все configure ошибки исправлены — 0 остаётся.**

### T4. Создать `minix/lib/libsys/arch/x86_64/` с 64-bit I/O wrappers ✅
**Файл**: `planning/03_migration_roadmap.md` §Architecture Phase 5
**Описание**: Часть кода в `arch/i386/` используется libsys.
**Статус**: ✅ **Завершено** — 18 файлов уже существовали в arch/x86_64/, идентичных i386. Используют message passing (_kernel_call), 64-bit адаптация не требуется. Добавлен `MACHINE_ARCH STREQUAL "x86_64"` в CMakeLists.txt.
**Примечание**: cmake configure 0 errors. Сборка через MSVC падает из-за предсуществующей проблемы `#include_next` (не относится к T4).

### T5. Обновить `minix/kernel/CMakeLists.txt` — добавить `x86_64` case ✅
**Файл**: `planning/03_migration_roadmap.md` §Architecture Phase 5
**Описание**: Обновлён CMakeLists.txt: добавлены `head.S`, `pre_init.c`, `arch_smp.c`/`trampoline.S` (CONFIG_SMP), x86_64 compile options (`-mcmodel=kernel -mno-red-zone`).
**Статус**: ✅ **Завершено**
**Зависит от**: T3–T4

### T5.5. Починить pre-existing build errors для x86_64 kernel ✅
**Описание**: После T5 (переключения на чистый `arch/x86_64/`) в shared kernel code проявились 4 pre-existing ошибки, которые раньше были скрыты использованием `arch/i386/`:

1. **`watchdog.c`** — `#include "arch/i386/glo.h"` не работает; `arch_watchdog_lockup()`/`nmi_watchdog_handler()` объявлены только под `#ifdef __i386__`
2. **`main.c`** — `direct_utils.h` не существует для x86_64
3. **`proc.c`** — `IPC_STATUS_REG` определён как `gpr[1]` (aarch64), но x86_64 `stackframe_s` не имеет `gpr[]` (именованные поля)
4. **`do_trace.c`** — `SETPSW` не виден (отсутствовал include `archconst.h` в include chain)

**Статус**: ✅ **Завершено**

**Изменённые/созданные файлы**:
| Файл | Изменение |
|------|-----------|
| `sys/machine/ipcconst.h` | ✏️ `IPC_STATUS_REG` conditional: `retreg` для x86_64, `gpr[1]` для ARM |
| `sys/machine/memory.h` | 🆕 Memory constants (PAGE_SIZE, KERNEL_VBASE, etc.) |
| `sys/machine/ports.h` | 🆕 `#include_next <ports.h>` delegation |
| `sys/machine/cmos.h` | 🆕 `#include_next <cmos.h>` delegation |
| `sys/machine/partition.h` | 🆕 `#include_next <partition.h>` delegation |
| `minix/kernel/watchdog.h` | ✏️ `#ifdef __i386__` → `#if defined(__i386__) || defined(__x86_64__)` |
| `arch/x86_64/include/arch_watchdog.h` | ✏️ `struct nmi_frame` definition for x86_64 |
| `arch/x86_64/include/direct_utils.h` | 🆕 Declares `direct_print()`, `direct_cls()` |
| `arch/x86_64/include/archconst.h` | ✏️ Added `X86_FLAGS_USER = 0x240CD5` |
| `arch/x86_64/sconst.h` | ✏️ `"kernel/procoffsets.h"` → `"procoffsets.h"` |
| `arch/x86_64/include/arch_proto.h` | ✏️ Added `#include "archconst.h"` |
| `minix/kernel/kernel.h` | ✏️ Added `#include "arch_proto.h"` |

**Остаётся (не относится к T5.5)**: 6+ pre-existing ошибок в x86_64 arch source файлах (missing headers: `machine/bios.h`, `apic.h`, `serial.h`; assembly errors)

### T6. Исправить `cmake/options.cmake` — ACPI/APIC/PCI/Watchdog для x86_64 ✅
**Статус**: ✅ **Завершено**

**Изменения**:
1. `cmake/options.cmake` — compile definitions перенесены из options.cmake в CMakeLists.txt
2. `CMakeLists.txt` — USE_* compile definitions теперь добавляются ПОСЛЕ `include(arch_${MACHINE_ARCH})`
3. `sys/machine/bios.h` — 🆕 delegation header
4. `sys/machine/interrupt.h` — ✏️ self-contained x86 + AArch64 constants
5. `arch/x86_64/apic.h` — 🆕 APIC constants + declarations
6. `arch/x86_64/serial.h` — 🆕 UART 8250/16550 definitions
7. `arch/x86_64/oxpcie.h` — 🆕 OXPCIe952 serial port definitions
8. `sys/arch/x86_64/include/vm.h` — ✏️ X86_64_CR0_TS added
9. `arch/x86_64/include/hw_intr.h` — ✏️ eoi_8259_master/slave declarations
10. `arch/x86_64/klib.S` — ✏️ retfq → lretq (Clang compat)
11. `kernel/CMakeLists.txt` — ✏️ CMAKE_CURRENT_SOURCE_DIR в include paths

**Остаётся (pre-existing, не T6)**:
- **head.S**: assembly-time/relocatable expression errors
- **mpx.S**: `expected ')'` syntax error
- **glo.h**: EXTERN type (возможно include order)
- **acpi.h**: missing header для x86_64

### T7. Восстановить ramdisk boot драйверы для x86_64 🟡
**Статус**: ⬜ Не начато

### ARM64: Kernel bootstrap

### T8. ARM64: IPC ABI для LP64 ✅
**Статус**: ✅ Завершено

### T9. ARM64: Libraries (libsys, libminc, libc) ✅
**Статус**: ✅ Завершено

### T10. ARM64: Platform + Drivers 🟡
**Статус**: 🟡 FDT parser ✅ + PL011 MINIX driver ✅ + console/keyboard stubs ✅ — остаётся RPi 4 специфика

### T11. ARM64: Testing + Polish ✅
**Статус**: ✅ Завершено

---

## 3. BOOTLOADER — Limine Modernization

### T12. Phase 1: Dual-boot Infrastructure (QEMU test) 🟡
**Статус**: 🟡 Инфраструктура готова — требуется тестирование

### T13. Phase 2: UEFI Boot (x86_64) 🟡
**Статус**: 🟡 Инфраструктура готова — требуется тестирование

### T14. Phase 3: Secure Boot 🟡
**Статус**: 🟡 Инфраструктура готова — требуется тестирование

### T15. Phase 4: ARM64 Boot (Limine AAC64) 🟡
**Статус**: 🟡 Инфраструктура готова — kernel port в процессе

### T16. Phase 5: GRUB Removal ❌
**Статус**: ❌ Не начато (ждёт T13)

### T17. Phase 6: Boot Library Cleanup ❌
**Статус**: ❌ Не начато

---

## 4. C LANGUAGE + RUST

### T18. Phase 7: Future Directions 🔮
**Статус**: 🔮 Отложено

---

## 5. NETBSD DEPENDENCY

### T19. Boot Library Cleanup 🟡
**Статус**: ❌ Не начато (см. T17)

### T20. Phase 7: VFS/Filesystem Cleanup 🟡
**Статус**: ❌ Не начато

---

## 6. MIGRATION ROADMAP — Долгосрочные планы

### T21. Filesystem Migration ❌
**Статус**: ❌ Не начато

### T22. Driver Model Modernization ❌
**Статус**: ❌ Не начато

### T23. Security Model Modernization ❌
**Статус**: ❌ Не начато

### T24. Network Stack Modernization ❌
**Статус**: ❌ Не начато

### T25. Testing Framework Migration ❌
**Статус**: ❌ Не начато

---

## 7. GUI ARCHITECTURE

### T26–T29. GUI Phases 1–6 🔮
**Статус**: 🔮 Отложено

---

## 8. CRYPTO ✅
Вся миграция OpenSSL → wolfSSL + hcrypto завершена.

---

## 9. CI/CD + SANITIZERS ✅
Всё завершено.

---

## 10. CRITICAL PATH

```
T1 (sysroot ✅) ──→ T2 (kernel build ✅) ──→ T2a (linker setup 🔴)
                          │
                          ↓
                       T8 (IPC ABI ✅) ──→ T9 (libs ✅) ──→ T10 (✅) ──→ T11 (✅)
                          │
                          ↓
                       T15 (Limine AAC64: request structures ✅)
```

---

## 11. Статистика

| Приоритет | Всего | Выполнено | Осталось |
|-----------|-------|-----------|----------|
| 🔴 Hard Blocker | 3 | 2 | **1** (T2a) |
| 🟡 Architecture | 9 | **6** ✅✅✅✅ | **3** (T4–T7: T3 done) |
| 🟡 ARM64 Libraries | 3 | 3 | **0** ✅ |
| 🟡 Bootloader | 6 | 2 | **4** (T12–T14, T16) |
| 🔮 Future | 10 | 0 | **10** |
| ❌ Not Started | 5 | 0 | **5** |
| ✅ Completed | 8 | 8 | — |
| **Итого** | **32** | **8** | **24** |

### Недавно завершено:
- ✅ **T3** — Создан `minix/kernel/arch/x86_64/` с 64-bit native assembly и C файлами (13 файлов)

### Сводка T3:
| Файл | Тип | Описание |
|------|-----|----------|
| `minix/kernel/arch/x86_64/arch_clock.c` | 🆕 | x86-common clock code (i8253, APIC, TSC) |
| `minix/kernel/arch/x86_64/arch_do_vmctl.c` | 🆕 | CR3/TLB VM control |
| `minix/kernel/arch/x86_64/arch_system.c` | 🆕 | FPU, CPUID, context, syscall dispatch |
| `minix/kernel/arch/x86_64/exception.c` | 🆕 | Exception handler (64-bit regs) |
| `minix/kernel/arch/x86_64/i8259.c` | 🆕 | 8259 PIC driver |
| `minix/kernel/arch/x86_64/arch_reset.c` | 🆕 | Reset/shutdown |
| `minix/kernel/arch/x86_64/direct_tty_utils.c` | 🆕 | Emergency VGA text I/O |
| `minix/kernel/arch/x86_64/memory.c` | 🆕 | VM memory mgmt (4-level X86_64_VM constants) |
| `minix/kernel/arch/x86_64/pg_utils.c` | 🆕 | Page table utils (512 entries, 2MB pages, 64-bit PTEs) |
| `minix/kernel/arch/x86_64/protect.c` | 🆕 | Protected mode init (64-bit GDT/IDT) |
| `minix/kernel/arch/x86_64/pre_init.c` | 🆕 | Boot-time kinfo + pagetable setup |
| `minix/kernel/arch/x86_64/procoffsets.cf` | 🆕 | Assembly offsets (R8-R15) |
| `minix/kernel/arch/x86_64/include/arch_proto.h` | 🆕 | Function declarations + structs |
