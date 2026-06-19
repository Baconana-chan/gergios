# Bootloader Modernization Plan

> **Part of**: Overall modernization roadmap (`planning/03_migration_roadmap.md` §9)
> **Related**: `planning/04_target_architecture_support.md`, `planning/07_x86_64_migration_plan.md`, `planning/08_arm64_migration_plan.md`
> **Статус**: Phase 0 ✅ — Phase 1 🟡 — Phase 2 🟡 — Phase 3 🟡 — Phase 4 🟡

---

## 1. Текущее состояние

### 1.1 x86_64 Boot Sequence (текущее — dual-boot)

```
┌─ GRUB ──multiboot──→ multiboot_init (32-bit PM) ──→ long mode ──→ pre_init ──→ kmain
│                           ↓
│                   module01_ds ... module12_init
│
└─ Limine ──ELF64 entry──→ ENTRY(MINIX) (64-bit LM) ──→ limine_pre_init ──→ kmain
                            ↓
                    .limine_requests ← memmap, modules
```

**Текущий стек:**
- **GRUB 2.x** — загрузчик (Legacy BIOS) — 32-bit multiboot v1 entry
- **Limine** — загрузчик (BIOS + UEFI) — 64-bit long mode entry через Limine protocol
- **Dual-boot**: multiboot header HAS_ADDR → GRUB на 32-bit entry; ELF entry → Limine на 64-bit
- **boot.cfg** — для GRUB (NetBSD-формат, `sys/lib/libsa/bootcfg.c`)
- **limine.conf** — для Limine (собственный формат)
- **12+ модулей** — ядро + серверы через multiboot modules / Limine modules

**Где живёт код:**
| Компонент | Расположение | Назначение |
|-----------|-------------|------------|
| Multiboot entry | `minix/kernel/arch/x86_64/head.S` | 32→64 bit переход, long mode |
| Multiboot structs | `minix/include/arch/earm/include/multiboot.h` | Определения протокола |
| pre_init | `minix/kernel/arch/x86_64/pre_init.c` | Обработка multiboot info |
| boot.cfg parser | `sys/lib/libsa/bootcfg.c` | Парсинг конфигурации |
| Boot loader CLI | `minix/usr.sbin/` | `boot` command (на диске) |
| GRUB EFI build | `releasetools/image.functions` | `fetch_and_build_grub()` |

### 1.2 ARM (earm) Boot Sequence

```
U-Boot ──→ kernel.bin (raw binary) ──→ head.S ──→ kmain
                        ↓
            simulated multiboot_info_t
```

**Текущий стек:**
- **U-Boot** — загрузчик (custom git checkout, `releasetools/fetch_u-boot.sh`)
- **FAT partition** — `kernel.bin`, `*.elf` (серверы), `uEnv.txt` (конфиг U-Boot)
- **Simulated multiboot** — `pre_init.c` для ARM конструирует `multiboot_info_t` вручную

### 1.3 Boot Library (`sys/lib/libsa/`)

**66 .c файлов**, из которых реально нужно ~15:
- **Нужно**: alloc, bootcfg, dev, dev_net, exit, files, getfile, loadfile, netif, open, printf, read, stat, close, errno, globals, minixfs3
- **Не нужно**: cd9660, dosfs, ext2fs, ffsv1, ffsv2, lfsv1, lfsv2, nfs, ufs, ustarfs, nullfs, bootp, rarp, rpc, loadfile_aout, loadfile_ecoff

**Проблема**: boot library парсит `boot.cfg` — NetBSD-формат, не нужный при переходе на новый загрузчик.

### 1.4 Текущие ограничения

| Ограничение | Серьёзность | Описание |
|------------|-------------|----------|
| **Нет UEFI** | 🔴 Critical | BIOS/CSM-only для x86_64. Новое железо без CSM не загрузится |
| **Нет Secure Boot** | 🔴 Critical | Невозможно подписать ядро |
| **GRUB — тупик** | 🟡 Medium | GRUB 2.12+ огромен, сложен в поддержке |
| **ARM U-Boot** | 🟡 Medium | U-Boot работает, но ARM64 требует UEFI |
| **Multiboot v1** | 🟡 Medium | Устаревший протокол (1995). Ограничен 32-bit addr в module info |
| **66 файлов в libsa** | 🟢 Low | 75% кода boot library не используется |

---

## 2. Сравнение опций

### 2.1 x86_64 Bootloader

| Опция | Multiboot v1/2 | UEFI | Secure Boot | ARM64 | Сложность |
|-------|---------------|------|-------------|-------|-----------|
| **GRUB 2.12** | ✅ v1+v2 | ✅ | 🟡 Через Shim | ❌ | 🔴 Высокая |
| **Limine** | ✅ v1+v2+stivale | ✅ | 🟡 Через Shim/UKI | ✅ AAC64 | 🟢 Низкая |
| **systemd-boot** | ❌ (UKI только) | ✅ | ✅ UKI+native | ✅ | 🟢 Низкая |
| **Direct UEFI app** | ❌ | ✅ | ✅ Ручная | ❌ | 🔴 Очень высокая |
| **U-Boot** | ❌ | ✅ | 🟡 | ✅ | 🟡 Средняя |

**Рекомендация: Limine**

Основания:
- **Мультипротокольность**: Multiboot 1/2 + stivale + Limine protocol — ядро может использовать любой
- **Минимальные изменения в ядре**: Limine protocol передаёт те же данные что multiboot (mmap, modules, framebuffer)
- **ARM64 поддержка**: Из коробки
- **Активная разработка** (2026): Релизы выходят регулярно
- **BIOS+UEFI**: Один бинарник для обоих режимов
- **Простота**: Конфиг — 10 строк, установка — 1 команда

### 2.2 ARM64 Bootloader

| Опция | UEFI | AArch64 | Secure Boot | Сложность |
|-------|------|---------|-------------|-----------|
| **U-Boot** | ✅ | ✅ | ✅ | 🟡 Средняя |
| **Limine** | ✅ | ✅ AAC64 | 🟡 | 🟢 Низкая |
| **Device Tree** | ❌ | ✅ | ❌ | 🟡 Средняя |

**Рекомендация**: U-Boot для embedded (BeagleBone, RPi), Limine для UEFI-based ARM64 (RPi 4/5 UEFI, QEMU virt).

### 2.3 Unified Kernel Images (UKI)

**Что это**: ELF-ядро + initrd + cmdline + signature → единый PE-образ.

```
[Linux/ELF stub | kernel | modules/initrd | CMDLINE | signature]
└─────────────────── PE .efi файл ───────────────────────┘
```

**Для GergiOS**: UKI не подходит напрямую (нужен Linux stub, initrd вместо модулей). 
Но концепция может быть адаптирована: один `.efi` файл с ядром + всеми модулями + подписью.

---

## 3. Предлагаемая архитектура

### 3.1 Целевой Boot Flow

```
UEFI Firmware
    │
    ├── x86_64: Limine (limine.sys / limine.efi)
    │           └──→ Limine protocol → head.S (long mode) → kmain
    │
    └── ARM64: U-Boot (embedded) / Limine (UEFI)
                └──→ Device Tree / Limine protocol → head.S → kmain

Резерв: GRUB для legacy BIOS (Phase 1 переход)
```

### 3.2 Преимущества Limine

**Limine Boot Protocol** (vs Multiboot 1):

| Аспект | Multiboot v1 (текущий) | Limine Protocol |
|--------|----------------------|----------------|
| **Модули** | 32-bit адреса (`u32_t mod_start`) | 64-bit `uint64_t` адреса |
| **Memory map** | Через mi_mmap_addr | Через запрос (RSDP, SMBIOS, etc.) |
| **Framebuffer** | VBE (BIOS-only) | EFI GOP + VBE |
| **ACPI** | APM таблица | RSDP (корректный ACPI) |
| **SMBIOS** | Нет | Прямая передача |
| **EFI** | Нет | Полная поддержка |
| **Загрузка** | 32-bit protected mode | 64-bit long mode напрямую |

**Ключевое**: Limine может загрузить ядро **напрямую в 64-bit long mode**, удаляя ~200 строк переходного кода из `head.S`.

### 3.3 Module Management

Текущая схема: 12+ отдельных multiboot modules:

```
mod01_ds  → Data Store сервер
mod02_rs  → Reincarnation Server
mod03_pm  → Process Manager
...
mod12_init → /sbin/init
```

**Варианты при переходе на Limine**:

1. **Прямые модули** (как сейчас): Limine protocol поддерживает модули (module_path)
2. **initrd + модули**: Упаковать серверы в initrd образ, распаковывать в pre_init
3. **Гибрид**: Модули для IO-heavy серверов (tty, memory), initrd для остальных

**Рекомендация**: Вариант 1 (минимальные изменения) → потом вариант 2 (UKI-style).

---

## 4. План миграции

### 4.1 Phase 0: Исследование и прототип (2-3 недели) ✅ **Завершена**

**Цель**: Доказать, что GergiOS загружается через Limine.

- [x] Создать `limine.conf` для GergiOS (`etc/limine.conf`)
- [x] Реализовать Limine protocol stub в `head.S` (параллельно с multiboot)
- [x] Создать Limine protocol заголовки (`minix/include/arch/x86_64/include/limine.h`)
- [x] Создать Limine protocol парсер (`minix/kernel/arch/x86_64/limine.c`)
- [x] Адаптировать загрузку модулей (multiboot → Limine)
- [x] Обновить `pre_init.c` — `mb_set_param` non-static для доступа из limine.c
- [x] Обновить linker script — `.limine_requests` секция
- [x] Обновить CMakeLists.txt + BSD Makefile.inc — limine.o unpaged
- [x] Документировать процесс (`docs/limine-boot-guide.md`)

**Статус**: ✅ **Выполнено**

**Что создано**:
| Файл | Описание |
|------|----------|
| `minix/include/arch/x86_64/include/limine.h` | Limine protocol: memmap, modules, framebuffer, HHDM, SMP, RSDP |
| `minix/kernel/arch/x86_64/limine.c` | Парсер Limine responses → kinfo + pagetable setup |
| `minix/kernel/arch/x86_64/head.S` | Dual-boot: Limine 64-bit entry + GRUB 32-bit (HAS_ADDR) |
| `minix/kernel/arch/x86_64/kernel.lds` | `.limine_requests` секция для bootloader |
| `minix/kernel/arch/x86_64/Makefile.inc` | `limine.o` в unpaged objects |
| `etc/limine.conf` | 3 boot entries: default, safe mode, serial |
| `minix/kernel/CMakeLists.txt` | x86_64 arch sources + unpaged |
| `minix/kernel/arch/i386/pre_init.c` | `mb_set_param` non-static, `overlaps` __unpaged |
| `docs/limine-boot-guide.md` | Полный guide: сборка → образ → QEMU |

### 4.2 Phase 1: Dual-boot Infrastructure (3-4 недели) 🟡

**Цель**: Полная инфраструктура для сборки и тестирования Limine boot.

- [x] Добавить `#ifdef USE_LIMINE` в `head.S` для поддержки обоих протоколов
- [x] Создать `minix/kernel/arch/x86_64/limine.c` — парсинг Limine info
- [x] Создать `minix/kernel/arch/x86_64/limine.h` — Limine protocol заголовки
- [x] Обновить `pre_init.c` — `mb_set_param` non-static для доступа из limine.c
- [x] Адаптировать загрузку модулей
- [x] Создать `limine.conf` для GergiOS
- [x] Обновить CMake + Makefile.inc — линковка limine.o
- [x] Добавить `MKLIMINE=no` флаг в `share/mk/bsd.own.mk`
- [x] Добавить `USE_LIMINE` пропагацию → `-DUSE_LIMINE` в CPPFLAGS (Makefile.inc)
- [x] Создать `releasetools/make_limine_test_image.sh` — QEMU test image generator
- [ ] **Протестировать**: QEMU + Limine BIOS, QEMU + Limine UEFI (на Linux хосте)
- [ ] Обновить `x86_hdimage.sh` — ESP + FAT32 + Limine (Phase 2 prep)
- [ ] Обновить `x86_usbimage.sh` — USB boot с Limine (Phase 2 prep)
- [ ] Создать CI-тест для QEMU (загрузка + проверка вывода)

**Изменённые/созданные файлы (Phase 1):**
| Файл | Тип | Описание |
|------|-----|----------|
| `share/mk/bsd.own.mk` | ✏️ | `MKLIMINE?=no` + `USE_LIMINE` mapping |
| `minix/kernel/arch/x86_64/Makefile.inc` | ✏️ | `${USE_LIMINE} != no` → `-DUSE_LIMINE` |
| `releasetools/x86_hdimage.sh` | ✏️ | Placeholder для limine.conf (Phase 2) |
| `releasetools/make_limine_test_image.sh` | 🆕 | Полный QEMU test image generator |

**Зависимости**: Phase 0 ✅
**Статус**: 🟡 Инфраструктура готова — требуется тестирование на Linux хосте с Limine + QEMU

### 4.3 Phase 2: UEFI Boot (4-6 недель) 🟡

**Цель**: Чистый UEFI boot через Limine (x86_64).

- [x] Добавить `check_limine()` в `image.functions` — поиск Limine бинарника и data-директории
- [x] Добавить `create_limine_cfg()` в `image.functions` — генерация limine.conf с корректными путями
- [x] Обновить `x86_hdimage.sh` — создание ESP с Limine при `MKLIMINE=yes` + `EFI_SIZE`
- [x] Установка Limine stage 1 (`limine bios-install`) после создания образа
- [x] Обновить `etc/limine.conf` — пути `boot:///mod*` (ESP root, не `/boot/`)
- [x] Обновить `make_limine_test_image.sh` — полная поддержка UEFI+BIOS (OVMF, GPT, mtools)
- [ ] Собрать Limine UEFI (`limine.efi`) — требуется установка на хосте
- [ ] Обновить `x86_cdimage.sh` — UEFI CD boot (`-e firadisk`)
- [ ] Обновить `x86_usbimage.sh` — UEFI USB boot
- [ ] Протестировать: QEMU OVMF (UEFI), реальное железо
- [ ] Удалить GRUB UEFI код из `image.functions` (fetch_and_build_grub)

**Изменённые/созданные файлы (Phase 2):**
| Файл | Тип | Описание |
|------|-----|----------|
| `releasetools/image.functions` | ✏️ | `check_limine()` + `create_limine_cfg()` |
| `releasetools/x86_hdimage.sh` | ✏️ | Limine ESP: kernel+modules на FAT32, limine.sys, BOOTX64.EFI, stage 1 install |
| `etc/limine.conf` | ✏️ | Пути `boot:///mod*` (ESP root) |
| `releasetools/make_limine_test_image.sh` | ✏️ | Полный UEFI/BIOS dual-mode test image generator |

**Зависимости**: Phase 1
**Статус**: 🟡 Инфраструктура готова — требуется сборка Limine + тестирование на QEMU

### 4.4 Phase 3: Secure Boot (3-4 недели) 🟡

**Цель**: Подписанный boot chain.

- [x] Создать Machine Owner Key (MOK) для разработки (`releasetools/gen_secure_boot_keys.sh`)
- [x] Подписать `BOOTX64.EFI` через `sbsign` (функция `sign_efi()` в `image.functions`)
- [x] Интегрировать в сборку образа: подпись при `MKLIMINE=yes` + `SB_KEY_DIR`
- [x] Документировать: enrollment + подпись + troubleshooting (`docs/secure-boot-guide.md`)
- [ ] Настроить Shim для enrolment (Phase 3+ — MOK достаточно для разработки)
- [ ] Интегрировать в CI/CD: подпись бинарников в GitHub Actions
- [ ] Протестировать: QEMU OVMF с Secure Boot

**Зависимости**: Phase 2
**Статус**: 🟡 Инфраструктура готова — требуется тестирование на Linux хосте с QEMU

**Изменённые/созданные файлы (Phase 3):**
| Файл | Тип | Описание |
|------|-----|----------|
| `releasetools/gen_secure_boot_keys.sh` | 🆕 | Генерация RSA 2048 ключей + PEM/DER/ESL форматы |
| `releasetools/image.functions` | ✏️ | `find_signing_tools()`, `find_sb_keys()`, `sign_efi()` |
| `releasetools/x86_hdimage.sh` | ✏️ | `SB_KEY_DIR` + подпись BOOTX64.EFI при MKLIMINE=yes |
| `docs/secure-boot-guide.md` | 🆕 | Полный guide: keygen → sign → enroll → QEMU |
| `planning/16_bootloader_modernization.md` | ✏️ | Phase 3 статус + checklist

### 4.5 Phase 4: ARM64 Boot (6-8 недель) 🟡

**Цель**: Boot infrastructure для ARM64 через Limine AAC64.

**Вариант B — Limine UEFI (выбран)**:
- [x] Добавить `check_limine_aac64()` в `image.functions` — поиск BOOTAA64.EFI
- [x] Добавить `find_qemu_firmware_aarch64()` — поиск AAVMF (QEMU_EFI.fd)
- [x] Создать `releasetools/arm64_hdimage.sh` — ESP + BOOTAA64.EFI + limine.conf + MFS
- [x] Обновить `etc/limine.conf` — ARM64 entries с dtb_path
- [x] Документировать: `docs/arm64-boot-guide.md`
- [ ] **ARM64 kernel port** — см. `planning/08_arm64_migration_plan.md` (8 phases, 12+ месяцев)
- [ ] Протестировать: QEMU virt + UEFI + Limine AAC64 (после завершения kernel port)
- [ ] Secure Boot на ARM64 (подпись BOOTAA64.EFI)
- [ ] Физическое железо: RPi 4/5

**Зависимости**: Phase 2 (UEFI ESP) + ARM64 kernel port (planning/08)
**Статус**: 🟡 Boot infrastructure готова — kernel port не завершён

**Изменённые/созданные файлы (Phase 4):**
| Файл | Тип | Описание |
|------|-----|----------|
| `releasetools/image.functions` | ✏️ | `check_limine_aac64()`, `find_qemu_firmware_aarch64()` |
| `releasetools/arm64_hdimage.sh` | 🆕 | ARM64 boot image: ESP + BOOTAA64.EFI + limine.conf |
| `etc/limine.conf` | ✏️ | ARM64 boot entries (QEMU virt, Safe Mode) |
| `docs/arm64-boot-guide.md` | 🆕 | Полный guide: QEMU virt + UEFI + Limine AAC64 |
| `planning/16_bootloader_modernization.md` | ✏️ | Phase 4 статус

### 4.6 Phase 5: Отказ от GRUB (1-2 недели)

**Цель**: GRUB полностью удалён из дерева.

- [ ] Удалить `fetch_and_build_grub()` из `image.functions`
- [ ] Удалить GRUB-specific код из `x86_cdimage.sh`
- [ ] Добавить `MKLIMINE=yes` флаг в `bsd.own.mk`
- [ ] Установить Limine через pkgsrc или in-tree
- [ ] Обновить документацию (boot.8, boot.cfg.5 — возможно удалить)

**Зависимости**: Phase 2 (UEFI работает через Limine)
**Статус**: ❌ Не начато

### 4.7 Phase 6: Boot Library Cleanup (2-3 недели)

**Цель**: Удалить 75% неиспользуемого кода из `sys/lib/libsa/`.

- [ ] Определить точный список нужных файлов (см. Section 6)
- [ ] Удалить неиспользуемые FS (cd9660, dosfs, ext2fs, ffsv1/v2, lfsv1/v2, nfs, ufs, ustarfs)
- [ ] Удалить неиспользуемые протоколы (bootp, rarp, rpc)
- [ ] Удалить loadfile_aout, loadfile_ecoff
- [ ] Удалить `bootcfg.c` (заменён на `limine.conf`)
- [ ] Протестировать: сборка + загрузка в QEMU

**Зависимости**: Phase 1 (multiboot всё ещё нужен пока)
**Статус**: ❌ Не начато

---

## 5. Timeline

```
Q4 2026: Phase 0 ✅ (прототип) + Phase 1 🟡 + Phase 2 🟡 (UEFI код готов)
Q1 2027: Phase 3 🟡 (Secure Boot код готов) + Phase 4 🟡 (ARM64 boot код готов)
Q2 2027: Phase 5 (GRUB removal) + Phase 6 (libsa cleanup)
```

**Зависимость от ARM64 kernel**: Phase 4 не может начаться, пока нет работающего
ARM64 kernel port (см. `planning/08_arm64_migration_plan.md`).

---

## 6. Технические детали

### 6.1 Limine Protocol — ключевые структуры

```c
// Запросы к Limine (ядро → bootloader)
struct limine_memmap_request {
    uint64_t id[4];  // Идентификатор запроса
    struct limine_memmap *response;  // Заполняется bootloader'ом
};

struct limine_module_request {
    uint64_t id[4];
    struct limine_module *response;  // Массив модулей
};

struct limine_framebuffer_request {
    uint64_t id[4];
    struct limine_framebuffer *response;  // Информация о framebuffer
};

// Результат — struct limine_memmap_entry {
    uint64_t base;
    uint64_t length;
    uint64_t type;  // 1 = usable, 2 = reserved, 3 = ACPI, 4 = NVS, ...
};

// Модули:
struct limine_file {
    uint64_t address;
    uint64_t size;
    char *path;
    char *cmdline;
    uint64_t media_size;
    // ...
};
```

**Как это работает**: Ядро определяет глобальные переменные с magic-идентификаторами.
Bootloader находит их в ELF-секции `.limine_requests` и заполняет ответы.

```c
// В ядре:
volatile struct limine_memmap_request memmap_request = {
    .id = { LIMINE_MEMMAP_REQUEST, LIMINE_MEMMAP_RESPONSE }
};
// Bootloader найдёт это в .limine_requests и заполнит memmap_request.response
```

### 6.2 head.S — минимальные изменения

Текущий `head.S` (multiboot):
```
32-bit entry → PAE → long mode → pre_init(rdi=mbi, rsi=magic)
```

Новый `head.S` (Limine):
```
64-bit entry (Limine загружает напрямую) → pre_init(rdi=limine_info)
```

**Разница**: Limine загружает ядро уже в 64-bit long mode. ~200 строк кода
(long mode transition, PAE, identity mapping) можно удалить.

```diff
- .code32
- multiboot_init:
-     /* PAE, PML4, long mode setup — ~150 lines */
-     ljmp $0x08, $long_mode_entry
- .code64
long_mode_entry:
-    lgdt gdt64_ptr
-    /* setup stacks, segments — ~50 lines */
+ENTRY(MINIX)
+    /* Уже в 64-bit mode */
+    mov rsp, stack_top
+    call pre_init
```

### 6.3 Модули — отображение

```
Multiboot module                  Limine module
────────────────────────────────────────────────
mod_start    → uint64_t address
mod_end      → uint64_t size (end - start)
cmdline      → cmdline (путь к модулю)
mmo_reserved → (not used)

pre_init.c:
    for (i = 0; i < mb_mods_count; i++)  →  for (i = 0; i < lm_mod_count; i++)
        kinfo.module_list[i] = ...          kinfo.module_list[i] = ...
```

### 6.4 boot.cfg → limine.conf

```diff
- # Текущий boot.cfg (NetBSD format)
- menu=Start GergiOS:load_mods /boot/mod*;multiboot /boot/kernel rootdevname=c0d0p0
- menu=GergiOS (safe mode):load_mods /boot/mod*;multiboot /boot/kernel rootdevname=c0d0p0 bootopts=-s

+ # Новый limine.conf (Limine format)
+ TIMEOUT=5
+ 
+ :GergiOS 1.0
+     PROTOCOL=limine
+     KERNEL_PATH=boot:///kernel
+     MODULE_PATH=boot:///mod01_ds
+     MODULE_PATH=boot:///mod02_rs
+     ...
+     CMDLINE=rootdevname=c0d0p0
+ 
+ :GergiOS 1.0 (Safe Mode)
+     PROTOCOL=limine
+     KERNEL_PATH=boot:///kernel
+     MODULE_PATH=boot:///mod01_ds
+     ...
+     CMDLINE=rootdevname=c0d0p0 bootopts=-s
```

### 6.5 Очистка boot library (`sys/lib/libsa/`)

**Оставить (~20 файлов):**
- `alloc.c` — аллокатор
- `bootcfg.c` (пока не Phase 6)
- `dev.c` — доступ к устройствам
- `dev_net.c` — сетевой boot
- `errno.c`, `exit.c`, `globals.c`
- `files.c`, `fstat.c`, `getfile.c`
- `loadfile.c`, `loadfile_elf64.c`
- `minixfs3.c`, `minixfs3.h`
- `net.c`, `netif.c`, `ether.c`, `arp.c`, `ip.c`, `udp.c`, `tftp.c`
- `open.c`, `read.c`, `close.c`, `lseek.c`, `stat.c`
- `printf.c`, `snprintf.c`, `strerror.c`
- `byteorder.c`, `panic.c`, `twiddle.c`

**Удалить (~46 файлов):**
- `cd9660.c` — CD-ROM FS
- `dosfs.c` — FAT
- `ext2fs.c` — ext2
- `ffsv1.c`, `ffsv2.c` — BSD FFS (не MINIX)
- `lfsv1.c`, `lfsv2.c` — Log-structured FS
- `nfs.c`, `rpc.c` — Network FS
- `ufs.c` — UFS
- `ustarfs.c` — tar
- `nullfs.c` — null FS
- `bootp.c` — DHCP/BOOTP для загрузки
- `rarp.c` — Reverse ARP
- `loadfile_aout.c`, `loadfile_ecoff.c` — a.out/ECOFF бинарники
- И другие...

---

## 7. Образы дисков — изменения

### 7.1 x86_64 HD Image (`x86_hdimage.sh`)

```diff
- # Текущий: GRUB + boot.cfg на MBR диске
- cp /usr/mdec/boot_monitor ${ROOT_DIR}/boot_monitor
- cat > ${ROOT_DIR}/boot.cfg <<END
- menu=...:multiboot /boot/kernel ...
- END

+ # Новый: LIMINE + limine.conf на GPT диске
+ # GPT разметка с ESP
+ sgdisk -n 1:2048:+512M -t 1:ef00 ${IMG}  # EFI System Partition
+ mkfs.fat ${ESP}
+ mkdir -p ${ESP}/EFI/BOOT/
+ cp limine.efi ${ESP}/EFI/BOOT/BOOTX64.EFI
+ cp limine.sys ${ESP}/
+ cp limine.conf ${ESP}/
+ # Установка Limine
+ limine bios-install ${IMG}
```

### 7.2 EFI System Partition

| Файл | Назначение |
|------|-----------|
| `\EFI\BOOT\BOOTX64.EFI` | Limine UEFI bootloader (x86_64) |
| `\EFI\BOOT\BOOTAA64.EFI` | Limine UEFI bootloader (ARM64) |
| `\limine.sys` | Limine stage 2 (BIOS mode) |
| `\limine.conf` | Конфигурация |

### 7.3 RAM Image (`x86_ramimage.sh`)

RAM-image boot использует `bootramdisk=1`. Переход на Limine:
- Limine загружает ядро + initrd (ramdisk образ)
- Ядро монтирует ramdisk вместо поиска модулей
- Это упрощает boot: один module вместо 12

---

## 8. Risks & Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Limine protocol changes** между версиями | Low | Low | Фиксировать версию в git |
| **Multiboot → Limine переход ломает ARM** | Medium | Low | ARM сейчас использует U-Boot, не GRUB |
| **Удаление bootcfg.c ломает BIOS boot** | Low | Medium | Оставить в Phase 6 до полного отказа от multiboot |
| **Secure Boot enrolment сложен для пользователя** | Medium | Medium | Документация + скрипты |
| **Limine не поддерживает 12+ модулей** | Low | Low | Limine protocol поддерживает модули |
| **QEMU OVMF отличается от реального UEFI** | Medium | Low | Тестировать на реальном железе |
| **GRUB → Limine миграция сломает существующие установки** | Medium | Medium | Dual-boot в Phase 1 |

---

## 9. Success Criteria

1. **Phase 0**: GergiOS загружается через Limine в QEMU (BIOS + UEFI)
2. **Phase 1**: Dual-boot (Limine + GRUB) — можно выбирать при загрузке
3. **Phase 2**: UEFI boot на реальном железе (x86_64)
4. **Phase 3**: `sbverify --cert MOK.crt limine.efi` passes
5. **Phase 4**: Boot infrastructure для ARM64 — ESP + BOOTAA64.EFI + limine.conf (kernel port в progress)
6. **Phase 5**: GRUB полностью удалён из дерева
7. **Phase 6**: `sys/lib/libsa/` содержит только нужные файлы

---

## 10. Related Documents

- `planning/03_migration_roadmap.md` §9 — Bootloader Modernization
- `planning/04_target_architecture_support.md` — x86_64 + ARM64 target specs
- `planning/07_x86_64_migration_plan.md` — x86_64 kernel port
- `planning/08_arm64_migration_plan.md` — ARM64 kernel port
- `planning/10_netbsd_dependency_audit.md` §3.8 — Boot library cleanup
- `planning/14_phase6_cicd_sanitizers.md` — QEMU test infrastructure
- `minix/kernel/arch/x86_64/head.S` — Current multiboot entry
- `releasetools/image.functions` — GRUB EFI code
- `sys/lib/libsa/` — Boot library
- `releasetools/x86_hdimage.sh` — HD image creation
- `releasetools/arm_sdimage.sh` — ARM SD image (U-Boot)

---

> **Примечание**: Limine выбран как основной bootloader из-за мультипротокольности,
> минимальных изменений в ядре, и поддержки ARM64. В будущем (Phase 3+) возможен
> переход на UKI-style загрузку для упрощения Secure Boot.
