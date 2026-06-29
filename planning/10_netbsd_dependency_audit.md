# NetBSD Dependency Audit and Compatibility Strategy

> **Part of**: Overall modernization roadmap (`planning/03_migration_roadmap.md`)
> **Related**: `planning/02_legacy_dependencies.md`, `planning/09_c_language_modernization.md`
> **Статус**: Аудит завершён — стратегия определена: NetBSD → POSIX (BSD) userland, как в macOS

---

## 1. Введение

### 1.1 Предыстория и новая стратегия

MINIX 3 был ответвлён от **NetBSD** около 2005-2006 годов. Команда MINIX взяла userland, библиотеки, систему сборки и инфраструктуру ядра NetBSD и построила на их основе новый **микроядро**. Результат — гибрид:

- **MINIX-родное**: `minix/kernel/`, `minix/servers/`, `minix/drivers/`, `minix/fs/`, `minix/net/`
- **Заимствовано из NetBSD**: библиотеки (`lib/`), утилиты (`bin/`, `sbin/`, `usr.bin/`, `usr.sbin/`), система сборки (`share/mk/`), заголовки ядра (`sys/sys/`, `sys/ufs/`), внешние пакеты (`external/`), общий код (`common/`)

Проект переименовывается в **GergiOS** (см. Section 5). MINIX остаётся микроядром, но система в целом движется вперёд под новой идентичностью.

**Ключевое изменение стратегии**:

Вместо *удаления* NetBSD-кода, цель — **сделать NetBSD userland POSIX-совместимым слоем**,
по аналогии с тем, как **macOS** имеет POSIX (BSD) userland поверх XNU:

```
macOS:         [XNU kernel] → [POSIX (BSD) userland] → [Cocoa/Carbon apps]
                                              ↓
                                   BSD syscall ABI

GergiOS:       [MINIX µkernel] → [NetBSD ABI/userland] → [GergiOS-native apps]
                                              ↓
                                   NetBSD syscall ABI
```

**Как это работает в macOS**:
- XNU — гибридное ядро (Mach + BSD)
- POSIX (BSD) слой обеспечивает: fork, exec, signals, pthreads, BSD sockets
- Нативные приложения (Cocoa) поверх этого слоя
- Любая POSIX-программа компилируется и работает "как на BSD"

**Как это будет работать в GergiOS**:
- MINIX — микроядро (серверы PM, VFS, VM, RS, DS)
- NetBSD ABI/syscall слой обеспечивает совместимость на уровне системных вызовов
- GergiOS-native приложения используют микроядро напрямую (или через облегчённую libgergios)
- Любая NetBSD-программа работает без изменений через тот же ABI

**Что это значит на практике**:
- Сложные компоненты вроде **libc, libm, sys-заголовков** — остаются от NetBSD навсегда
- Заменяются только те компоненты, которые *выигрывают* от замены (криптография, тулы, язык)
- Чёткой границы "убрать всё NetBSD" нет — есть чёткая граница "что GergiOS-native, что NetBSD compat"
- NetBSD — не враг, а фундамент; GergiOS надстраивает новое поверх

---

## 2. NetBSD Dependency Map

### 2.1 Layer Diagram (целевое состояние: macOS-модель)

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
                               ↑ NetBSD syscall ABI
                          (fork, exec, signals, IPC, ...)

                      ┌─────────────────────────────┐
                      │  Future: Linux Compat Layer │
                      │  (через LACC или аналог)    │
                      └─────────────────────────────┘
```

**Модель**: Как в macOS — XNU ядро + POSIX (BSD) userland layer.
Здесь: MINIX микроядро + NetBSD ABI/userland.
libc/libm/sys-заголовки — **перманентная часть системы**, не заменяются.
NetBSD — не внешняя зависимость, а фундаментальный слой ОС.

### 2.2 Full Dependency Inventory

| # | Component | Location | NetBSD Origin | Size (est.) | Critical? | Replaceable? |
|---|-----------|----------|---------------|-------------|-----------|-------------|
| 1 | **libc** | `lib/libc/` + `common/lib/libc/` | ✅ 100% | ~500 files | 🔴 **Critical** | 🟡 Complex |
| 2 | **libm** | `lib/libm/` | ✅ 100% | ~150 files | 🔴 **Critical** | 🟡 Complex |
| 3 | **sys headers** | `sys/sys/*.h` | ✅ 100% | ~350 files | 🔴 **Critical** | 🔴 Very Hard |
| 4 | **BSD Make** | `share/mk/*.mk` | ✅ 100% | 37 files | 🟡 Important | 🟢 **Easy** ✅ |
| 5 | **Userland utils** | `bin/`, `sbin/`, `usr.bin/`, `usr.sbin/` | ✅ 90% | ~250 tools | 🟡 Important | 🟢 **Easy** |
| 6 | **Libraries** | `lib/{edit,curses,form,menu,pci,prop,puffs,...}` | ✅ 100% | ~40 libs | 🟢 Low | 🟢 **Easy** |
| 7 | **Kernel FS** | `sys/ufs/`, `sys/fs/` | ✅ 100% | ~80 files | 🟡 Important | 🟡 Complex |
| 8 | **UVM/VMM** | `sys/uvm/` | ✅ 100% | ~40 files | 🟡 Important | 🟡 Complex |
| 9 | **boot lib** | `sys/lib/libsa/` | ✅ 100% | ~45 files kept (~37 deleted) | 🔴 **Critical** | 🟡 Complex ✅ |
| 10 | **External pkg** | `external/{bsd,gpl2,gpl3,mit,public-domain}/` | ✅ 90% | ~50 packages | 🟢 Low | 🟢 **Easy** |
| 10b | **External/cleanup** | ✅ **Аудит завершён** | ~100MB удалено | 🟢 Low | 🟢 **Done** ✅ |
| 11 | **Crypto** | `crypto/external/{bsd,gpl2}/` | ✅ 100% | ~5 packages | 🟡 Important | 🟢 **Easy** ✅ |
| 12 | **Common code** | `common/lib/libc/` (atomic, md, string, stdlib, sys) | ✅ 100% | ~100 files | 🔴 **Critical** | 🟡 Complex |
| 13 | **games** | `games/` | ✅ 100% | ~30 games | 🟢 Low | 🟢 **Easy** |
| 14 | **Documentation** | `share/man/`, `share/doc/` | ✅ 100% | ~50 files | 🟢 Low | 🟢 **Easy** |
| 15 | **i18n/locale** | `share/i18n/`, `share/locale/` | ✅ 100% | ~500 files | 🟢 Low | 🟢 **Easy** |
| 16 | **termcap/terminfo** | `share/terminfo/` | ✅ 100% | ~1 file (db) | 🟢 Low | 🟢 **Easy** |

### 2.3 Dependency Graph (Critical Path)

```
MINIX Microkernel
    ├── Needs: libc (syscall ABI) ← 🔴 Critical — **ОСТАЁТСЯ NetBSD**
    │       └── Needs: sys/sys/ headers ← 🔴 Critical — **ОСТАЁТСЯ NetBSD**
    ├── Needs: common/lib/libc/ (shared kernel/userland code) ← 🔴 Critical
    ├── Needs: boot library (sys/lib/libsa/) ← 🔴 Critical
    └── Needs: libm (math) ← 🟡 Important — **ОСТАЁТСЯ NetBSD**
    │
    └── МОГУТ БЫТЬ ЗАМЕНЕНЫ (без потери ABI-совместимости):
            ├── Userland utils → pkgsrc 🟢
            ├── BSD Make → CMake 🟢 ✅
            ├── External packages → pkgsrc 🟢
            ├── Libraries (curses, edit, etc.) → pkgsrc 🟢
            ├── games → pkgsrc 🟢
            ├── crypto/openssl → wolfSSL 🟢 ✅
            ├── locale/i18n → pkgsrc 🟢
            └── terminfo → pkgsrc 🟢
```

---

## 3. Migration Phases

### 3.1 Phase 0: Quick Wins ✅ **(Завершена)**

| Task | Status | Effort |
|------|--------|--------|
| **BSD Make → CMake** | ✅ Phase 1-4 complete | 3 months |
| **OpenSSL 0.9.8 → wolfSSL + hcrypto** | ✅ All 4 phases complete | 3 months |
| **GergiOS branding** (boot, uname, motd) | ✅ **Done** — config.h, main.c, boot.cfg, motd | 1 week |

**Что сделано**:
- `OS_NAME` → `"GergiOS"`, `OS_RELEASE` → `"1.0.0"` (config.h)
- Kernel announce — "GergiOS 1.0.0, Copyright 2026 GergiOS Project, Based on MINIX 3.4.0 microkernel"
- Boot menu — "Start GergiOS" / "Start GergiOS (single user mode)"
- MOTD — "Welcome to GergiOS 1.0!" + gergios.dev
- Shutdown messages — "GergiOS has halted", "GergiOS will now reset"
- Internal `__minix` defines и `minix/` директория не тронуты (как в macOS-модели)

### 3.2 Phase 1: Консолидация NetBSD-слоя (pkgsrc)

**Цель**: Выделить NetBSD-компоненты в чётко определённый слой совместимости,
с возможностью установки через pkgsrc. ~60% NetBSD-кода остаётся доступным,
но не дублируется в GergiOS-native сборке.

**Стратегия**: 
Вместо удаления NetBSD-кода — **упаковка**:
- GergiOS-native core: минимальная система на микроядре MINIX
- NetBSD compat layer: устанавливается опционально через pkgsrc
- Dual-build: CMake для GergiOS-native, BSD Make для NetBSD-совместимости
- Со временем: NetBSD compat → pkgsrc package `gergios-netbsd-compat`

#### Компоненты для консолидации:

| Компонент | Статус | Действие |
|-----------|--------|----------|
| `bin/` (cat, cp, ls, mv, rm, sh, test...) | 🟢 **Rust: cat, echo, hostname, kill, ln, mkdir, mv, pwd, rm, rmdir, sleep, sync, true, false, yes** ✅ | GergiOS-native (Rust), NetBSD C-версии остаются для совместимости |
| `usr.bin/` (find, grep, sed, awk, diff...) | 🟡 В дереве | Оставить совместимость; grep ✅ уже в Rust |
| `sbin/` (mount, fsck, newfs, ifconfig...) | 🟡 В дереве | Нужны MINIX-специфичные обёртки |
| `usr.sbin/` (syslogd, inetd, sysctl...) | 🟢 Мигрированы | syslogd на wolfSSL ✅ |
| `lib/{curses,edit,form,menu,pci,prop,puffs}` | 🟡 В дереве | Оставить в compat layer |
| `external/` (LLVM, GCC, GDB, tmux, less...) | ✅ **Аудит завершён** | ~255MB удалено, 18 MK* флагов |
| `games/` | ✅ **MKGAMES=no** | pkgsrc/bsd-games |
| `share/{man,locale,i18n,terminfo,misc}` | ✅ **MKMAN=no, MKNLS=no** | Дата-файлы, pkgsrc |
| `crypto/external/bsd/openssl/` | ✅ **Удалён** | Заменён на wolfSSL + libhcrypto |
| `crypto/external/bsd/heimdal/` | ✅ **HCrypto** | Собственная libhcrypto |
| `lib/libtelnet/` | ✅ **MKLIBTELNET=no** | telnet deprecated, через pkgsrc |
| `lib/libkvm/` | ✅ **MKLIBKVM=no** | не используется MINIX, через pkgsrc |

#### Процесс:
1. Определить границы NetBSD compat layer (что входит, что исключено)
2. Создать pkgsrc-метапакет `gergios-netbsd-compat`
3. Оставить NetBSD-код в дереве как опциональную сборку (через BSD Make)
4. Постепенно заменять GergiOS-native аналогами
5. NetBSD compat → внешняя зависимость (не в основном дереве)

**Статус**: ✅ **external/ аудит и очистка завершены.**

**Существовали ранее** (5, все default = no):
- `MKGAMES=no` — игры (pkgsrc/bsd-games)
- `MKLIBTELNET=no` — telnet (deprecated)
- `MKLIBKVM=no` — kvm (не используется MINIX)
- `MKMAN=no` — man pages
- `MKNLS=no` — locale/i18n/nls

**Добавлены в этом PR** (18, все default = no) в `share/mk/bsd.own.mk`:
- `MKLESS=no` — less (pkgsrc/misc/less)
- `MKTMUX=no` — tmux (pkgsrc/misc/tmux)
- `MKTOP=no` — top (pkgsrc/sysutils/top)
- `MKNVI=no` — nvi (pkgsrc/editors/nvi)
- `MKBZIP2=no` — bzip2 (pkgsrc/archivers/bzip2)
- `MKFILE=no` — file/libmagic (pkgsrc/sysutils/file)
- `MKFLEX=no` — flex (pkgsrc/devel/flex)
- `MKBYACC=no` — byacc (pkgsrc/devel/byacc)
- `MKBPF=no` — libpcap/tcpdump (pkgsrc/net/libpcap, pkgsrc/net/tcpdump)
- `MKTCPDUMP=no` — tcpdump
- `MKFETCH=no` — libfetch (pkgsrc/net/libfetch)
- `MKBIND=no` — BIND/named (pkgsrc/net/bind)
- `MKDHCP=no` — ISC DHCP (pkgsrc/net/isc-dhcp)
- `MKBLACKLIST=no` — blacklistd (pkgsrc/security/blacklist)
- `MKMDOCML=no` — mandoc (pkgsrc/textproc/mandoc)
- `MKOPENRESOLV=no` — openresolv (pkgsrc/net/openresolv)
- `MKCTWM=no` — ctwm WM (pkgsrc/wm/ctwm)
- `MKLLVM=no` — LLVM 3.x (удалён из дерева, 204MB)

**🗑 Удалено из дерева (~255MB):**
| Пакет | Размер | Причина |
|-------|--------|---------|
| **LLVM 3.x** (`external/bsd/llvm/` + `tools/llvm-*/`) | ⬇️ 204MB | Устаревший LLVM (сейчас LLVM 18+), был без clang/tools |
| **BIND/named** (`external/bsd/bind/`) | ⬇️ 45MB | DNS-сервер, через pkgsrc/net/bind |
| **ISC DHCP** (`external/bsd/dhcp/`) | ⬇️ 3MB | DHCP-сервер, через pkgsrc/net/isc-dhcp |
| **blacklistd** (`external/bsd/blacklist/`) | ~1MB | Через pkgsrc/security/blacklist |
| **dhcpcd** | 🟢 **Оставлен** | Критичен для сети, не удалён |
| **libpcap/tcpdump** | 🟢 **Оставлены** (под MKBPF=no) | Могут понадобиться |

**🔧 Починены ссылки на удалённые пакеты:**
- `lib/Makefile` — bind/blacklist SUBDIR → под MK* guards
- `usr.sbin/postinstall/postinstall` — blacklistd check → guard `[ -d ]`
- `minix/llvm/generate_gold_plugin.sh` — комментарий о статусе
- **Критический баг:** `MKLIBCXX?=no` случайно поставлен → **починено** (сломал бы C++)

**share/Makefile**: Для MINIX строится только `mk/` (BSD Make инфраструктура);
`man`, `misc`, `terminfo`, `i18n`, `locale`, `nls` — скипаются (data files из pkgsrc).

**Effort**: 4-8 weeks
**Risk**: Low (pkgsrc уже поддерживается MINIX)

### 3.3 Phase 2: Crypto Consolidation ✅ **(Завершена)**

| Компонент | Статус | Действие |
|-----------|--------|----------|
| `crypto/external/bsd/openssl/` (OpenSSL 0.9.8) | ✅ Удалён | wolfSSL + libhcrypto |
| `crypto/external/bsd/heimdal/` (Kerberos) | ✅ Мигрирован | libhcrypto (собственная библиотека) |
| `crypto/external/bsd/libsaslc/` (SASL) | ✅ Мигрирован | wolfSSL |
| `crypto/external/bsd/netpgp/` (PGP) | ✅ Мигрирован | wolfSSL |
| `crypto/external/gpl2/wolfssl/` | ✅ Основной | Первичный крипто-провайдер |

**Детали**: `planning/15_crypto_migration.md` — все 4 фазы завершены.

### 3.4 Phase 3: BSD Make → CMake (Dual-Build) ✅ **(Завершена)**

| Задача | Статус |
|--------|--------|
| CMake build for kernel | ✅ Complete |
| CMake build for servers | ✅ Complete |
| CMake build for drivers | ✅ Complete |
| CMake build for libraries | ✅ Complete |
| CMake build for userland | ✅ Complete |
| CMake build for tests | ✅ Complete |
| CMakePresets.json | ✅ Complete |
| cmake-build.sh | ✅ Complete |
| build.sh deprecation notice | ✅ Complete |
| BSD Make сохранён для NetBSD compat | ✅ Совместимость |

**Статус**: CMake — основной для GergiOS-native. BSD Make сохранён для сборки NetBSD compat layer.

### 3.5 Phase 4: libc/libm — **Не заменяются**

**Стратегия**: NetBSD libc и libm остаются перманентно.

По аналогии с macOS (где POSIX (BSD) userland — неотъемлемая часть системы),
NetBSD libc/libm — фундаментальный слой GergiOS, который не заменяется:

- **libc** — syscall ABI микроядра (PM, VFS, VM) завязан на NetBSD libc обёртки
- **libm** — математическая библиотека, полностью стандартизирована, нет причин заменять
- **sys/sys/ заголовки** — описания типов и структур ядра, неотделимы от MINIX
- **common/lib/libc/** — общий код ядра/userland, портабельный C

**Что можно сделать (опционально, низкий приоритет)**:
- Добавить musl как *экспериментальную* сборку для изолированных newlib-компонентов
- Использовать отдельные библиотеки (OpenLibm) для специфических задач
- Но **NetBSD libc остаётся libc по умолчанию** навсегда

### 3.6 Phase 5: ~~Math Library~~ — **Не нужна**

libm — часть NetBSD POSIX (BSD) userland, не заменяется.
См. Phase 4 (libc/libm — не заменяются).

### 3.7 Phase 6: Boot Library — Очистка ✅

**Статус**: ✅ **Завершено**

**Стратегия**: Boot library (`sys/lib/libsa/`) — общий код, не зависит от
NetBSD. Удалены заведомо неиспользуемые части:
- Файловые системы: cd9660, dosfs, ext2fs, ffsv1, ffsv2, lfsv1, lfsv2, nfs, ufs, ustarfs, nullfs
- Протоколы: bootp, rarp, rpc, bootparam, tftp
- Бинарные форматы: loadfile_aout, loadfile_ecoff
- Другое: bootcfg, cread, dev_net, ls

**Оставлено**: core-инфраструктура + minixfs3 + loadfile ELF (32/64) + ethernet ARP/IP/UDP стек

**Проверка**: x86_64 kernel (1.9MB) + aarch64 kernel (1.8MB) — 0 ошибок сборки ✅

**Effort**: 1-2 weeks ✅
**Risk**: Low ✅

### 3.8 Phase 7: VFS/Filesystem — ✅ **Очистка завершена**

**Стратегия**: NetBSD VFS (`sys/ufs/`, `sys/fs/`) — удалены неиспользуемые части.
Оставлены только те FS, которые реально нужны MINIX.

| Компонент | Статус | Действие |
|-----------|--------|----------|
| `sys/ufs/ffs/` | 🟢 **Оставлен** | Для совместимости (не используется MINIX) |
| `sys/ufs/lfs/` | ✅ **Удалён** | Log-structured FS (~3000 строк) |
| `sys/ufs/chfs/` | ✅ **Удалён** | Cached HFS (~1500 строк) |
| `sys/ufs/ufs/` | ✅ **Удалён** | Core UFS (~5000 строк) |
| `sys/ufs/mfs/` | 🔴 **Оставлен** | MINIX MFS |
| `sys/ufs/ext2fs/` | 🔴 **Оставлен** | MINIX fs/ext2 использует заголовки |
| `sys/fs/v7fs/` | ✅ **Удалён** | V7 FS (~1500 строк) |
| `sys/fs/{cd9660,msdosfs,udf,puffs}` | 🟢 **Оставлены** | Для совместимости |

**Обновлены**: sys/ufs/Makefile, sys/fs/Makefile, sbin/Makefile, tests/fs/*, lib/libc/sys/makelintstub
**Проверка**: cmake build — 0 ошибок ✅
**Effort**: ✅ Завершено (1 день)

### 3.9 Summary Timeline

```
Q2 2026 ✅: Phase 2 (crypto) + Phase 3 (CMake) — завершены
Q3 2026 ✅: Phase 0 (branding) + Phase 1 (external/ cleanup, MK* flags, Rust build) — завершены
Q3-Q4 2026: Phase 6 (boot library cleanup) ✅ завершено + Phase 7 (VFS audit)
```

---

## 4. Detailed Component Analysis

### 4.1 NetBSD libc — Фундаментальный слой POSIX (BSD) userland

**Почему это критично**: Каждый процесс линкуется с libc. MINIX syscalls проходят через libc-обёртки.
Это **неотъемлемая часть системы**, как POSIX (BSD) userland в macOS.

**Что MINIX нужно от libc:**

```
libc needed by MINIX servers (PM, VFS, VM, RS, DS, etc.):
  stdio:    printf, fprintf, sprintf, snprintf, vprintf
  stdlib:   malloc, free, realloc, calloc, atoi, strtol, exit, getenv
  string:   memcpy, memmove, memset, strlen, strcpy, strcmp, strcat,
            strncpy, strncmp, strchr, strrchr, strstr, strsep, strlcpy
  signal:   sigaction, sigprocmask, sigemptyset, sigfillset
  time:     time, clock_gettime, nanosleep, gettimeofday
  errno:    errno, __errno, strerror
  syscall:  _syscall, __syscall (MINIX custom)
  pthread:  mutex_lock, mutex_unlock, thread_create (via libc pthread stubs)
  math:     (often not needed by servers, only by userland)
```

**Стратегия**: NetBSD libc — **перманентна**. Не заменяется.

- MINIX syscall ABI завязан на NetBSD libc обёртки (`__syscall`, `_syscall`)
- Микроядро использует `common/lib/libc/` (rb.c, sha2.c, atomics) — портабельный C
- Сигналы, TLS, pthreads — всё через NetBSD libc
- Замена libc = переписывание syscall ABI = переписывание MINIX IPC

**musl**, **OpenLibm** и другие альтернативы — опционально, не для 1.0.
В будущем (1.2+) musl может быть добавлен как альтернативная libc для
изолированных GergiOS-native компонентов, но НЕ как замена NetBSD libc.

### 4.2 `common/lib/libc/` — Общий код ядра/userland

**Почему критично**: Этот код выполняется и в контексте ядра (через `libminc`),
и в userland (через `libc`). Не зависит от NetBSD — это переносимый C-код.

| Файл | Используется | Примечание |
|------|-------------|------------|
| `atomic/*.c` | kernel, servers | C11 atomics via CAS |
| `gen/rb.c` | kernel (VM), servers | Red-black tree |
| `gen/radixtree.c` | kernel | Radix tree |
| `gen/ptree.c` | kernel | Priority tree |
| `gen/rpst.c` | kernel | Range-partitioning tree |
| `inet/*.c` | network | htonl, htons |
| `md/*.c` | kernel, crypto | MD4, MD5 |
| `string/*.c` | kernel, libc | memcpy, memset, strlen |
| `stdlib/*.c` | kernel, libc | strtol, random, heapsort |
| `quad/*.c` | kernel | 64-bit ops on 32-bit |

**Действие**: Оставить как GergiOS-native утилитарную библиотеку.
Никаких NetBSD-специфичных зависимостей.

### 4.3 `sys/lib/libsa/` — Boot Library

Загрузчик MINIX использует ~40 файлов, реально нужно ~15.
Остальные — поддержка других FS/протоколов, не используемых MINIX.

**Оставить (нужно MINIX):**
- `alloc.c`, `printf.c`, `snprintf.c`, `strerror.c`, `errno.c`
- `dev.c`, `dev_net.c`, `files.c`, `fstat.c`, `getfile.c`, `open.c`, `read.c`, `close.c`, `lseek.c`, `stat.c`
- `loadfile.c`, `loadfile_elf32.c`, `loadfile_elf64.c`
- `minixfs3.c`, `minixfs3.h`
- `net.c`, `netif.c`, `ether.c`, `arp.c`, `ip.c`, `udp.c`, `tftp.c`
- `bootcfg.c`, `exit.c`, `panic.c`, `byteorder.c`, `globals.c`, `twiddle.c`

**Удалить (не используется):**
- `cd9660.c`, `dosfs.c`, `ext2fs.c`, `ffsv1.c`, `ffsv2.c`, `lfsv1.c`, `lfsv2.c`, `nfs.c`, `ufs.c`, `nullfs.c`, `ustarfs.c`
- `bootp.c`, `rarp.c`, `rpc.c`
- `loadfile_aout.c`, `loadfile_ecoff.c`

---

## 5. GergiOS Rebranding Concept

### 5.1 Philosophy

> **"MINIX"** — The microkernel heritage (internal, technical). Like "Linux" in "Android" — the kernel base.
> **"GergiOS"** — The product name (external, user-facing). The operating system the user interacts with.

This mirrors:
- **Android** (product) built on **Linux** (kernel)
- **macOS** (product) built on **XNU/Darwin** (kernel)
- **GergiOS** (product) built on **MINIX** (microkernel)

### 5.2 User-Facing Touchpoints

| Location | Current | Target | Priority |
|----------|---------|--------|----------|
| **Boot menu** (`etc/boot.cfg.default`) | `Start MINIX 3` | `Start GergiOS` | 🔴 High |
| **Kernel announce** (`minix/kernel/main.c`) | `MINIX 3.4.0` | `GergiOS 1.0 (MINIX 3.4.0)` | 🔴 High |
| **OS_NAME** (`minix/include/minix/config.h`) | `"Minix"` | `"GergiOS"` | 🔴 High |
| **OS_VERSION** | `"Minix 3.4.0 (GENERIC)"` | `"GergiOS 1.0 (GENERIC, MINIX 3.4.0)"` | 🔴 High |
| **motd** (`etc/motd`) | `MINIX 3 wiki...` | `GergiOS docs...` | 🟡 Medium |
| **uname -o** (via MIB) | `Minix` | `GergiOS` | 🟡 Medium |
| **uname -r** (via OS_RELEASE) | `3.4.0` | `1.0` (GergiOS version) | 🟡 Medium |
| **libc identification** | minix3 | gergios | 🟢 Low |
| **sysctl kern.ostype** | `Minix` | `GergiOS` | 🟡 Medium |
| **Website/Community** | `minix3.org` | `gergios.dev` (future) | 🟢 Low |
| **Man pages** (`minix/man/`) | `MINIX` references | `GergiOS` references | 🟢 Low |
| **Source file headers** | `Minix` in comments | `GergiOS` in comments | 🟢 Low |
| **Version file** (`etc/version`) | — | Add GergiOS version info | 🟢 Low |
| **makewhatis** database | MINIX | GergiOS | 🟢 Low |
| **Boot splash** (future) | MINIX logo | GergiOS logo | 🟢 Low |

### 5.3 Implementation Approach

**Internal reference** (keep "MINIX"):
- `minix/` directory name — stays
- `minix/include/minix/` headers — stay
- `__minix` preprocessor defines — stay
- Internal comments referencing MINIX heritage — keep

**User-facing** (change to "GergiOS"):
- `OS_NAME` in `minix/include/minix/config.h`
- Kernel `announce()` message
- Bootloader menu
- motd, issue, rc prompt
- uname output
- Package metadata
- Documentation and man pages

### 5.4 Versioning Scheme

```
GergiOS 1.0.0 "Aurora" (MINIX 3.4.0)
├── GergiOS major.minor.patch
│   ├── Major: architectural changes (new kernel, new libc)
│   ├── Minor: feature releases
│   └── Patch: bug fixes
├── Codename: marketing name per release
└── MINIX X.Y.Z: base microkernel version (internal reference)
```

### 5.5 Quick Branding Change (Phase 0)

The minimal change to establish GergiOS identity:

```c
// minix/include/minix/config.h
#define OS_NAME "GergiOS"
#define OS_RELEASE "1.0.0"     // GergiOS version
#define OS_VERSION OS_NAME " " OS_RELEASE " (MINIX 3.4.0, GENERIC)"
#define OS_CONFIG "GENERIC"
```

```c
// minix/kernel/main.c — announce() function
printf("\nGergiOS %s "
    "(MINIX microkernel 3.4.0)\n"
    "Copyright 2026, GergiOS Project\n",
    OS_RELEASE);
```

```makefile
# etc/boot.cfg.default
menu=Start GergiOS:load_mods /boot/default/mod*;multiboot /boot/default/kernel rootdevname=$rootdevname $args
menu=Start GergiOS (safe mode):load_mods /boot/default/mod*;multiboot /boot/default/kernel rootdevname=$rootdevname bootopts=-s $args
```

---

## 6. Detailed Userland Audit: `bin/`, `sbin/`, `usr.bin/`

### 6.1 Общая картина

Проаудировано **~116 утилит** в трёх каталогах. Все они — NetBSD-код (C, BSD Make).
На MINIX собирается ~80% от общего количества.

**Ключевые наблюдения по зависимостям:**

| Зависимость | Используют |
|-------------|-----------|
| `-lutil` (libutil) | ~35 утилит (самая популярная) |
| `-lm` (libm) | ping, ping6, ps, sleep, seq, jot |
| `-lcrypt` (libcrypt) | login, passwd, su, pwhash, lock, bdes, newgrp, ed, init |
| `-lterminfo` | csh, sh, telnet, cal, ul, tput, tic, infocmp, ftp |
| `-ledit` (libedit) | csh, sh, ftp |
| `-lkvm` (libkvm) | ps, w, netstat |
| `-lwolfssl` | telnet, ftp, passwd (после миграции с OpenSSL) |
| `-lpam` (libpam) | login, su, passwd, lock |
| `-lprop` | newfs_ext2fs, newfs_msdos, fsck |
| только libc | cat, chmod, cp, echo, expr, hostname, kill, ln, mkdir, mv, pwd, rm, rmdir, stty, sync, test, domainname, basename, dirname, env, false, head, id, printenv, printf, true, tty, uname, wc, yes и ~30 других |

**Критический вывод**: ~90% утилит зависят только от **libc + libutil**.
Ни одна утилита не зависит от OpenSSL (после миграции на wolfSSL — только telnet, ftp, passwd).
Ни одна утилита не требует NetBSD-специфичных ABI-фич; все используют POSIX API.

### 6.2 `bin/` — Core (/bin)

Собираются всегда, линкуются статически при `MKDYNAMICROOT=no`.
Исполняемые файлы, критически важные для загрузки и однопользовательского режима.

| Утилита | Зависимости | Категория | Приоритет | Примечание |
|---------|------------|-----------|-----------|------------|
| **sh** | -ll -ledit -lterminfo | 🔴 **NetBSD compat** | 1.0 | Bourne shell, критичен. Замена на Rust-shell = огромная работа (POSIX shell spec ~2000 строк) |
| **csh** | -ledit -lterminfo -lutil | 🟡 **NetBSD compat** | 1.1 | C shell. Можно заменить на GergiOS-shell, но не приоритет |
| **ksh** | libc only | 🟡 **NetBSD compat** | 1.1 | Korn shell. Альтернатива sh |
| **pax** | -lutil -lrmt | 🟡 **NetBSD compat** | 1.1 | Архиватор cpio/tar/pax. Можно pkgsrc |
| **ps** | -lm -lkvm | 🟡 **NetBSD compat** | 1.1 | Требует MINIX-специфичного kvm |
| **cat** | libc only | ✅ **Rust** | 1.0 | `rust/cat/` |
| **chmod** | libc only | ✅ **Rust** | 1.0 | `rust/chmod/` |
| **cp** | libc only | ✅ **Rust** | 1.0 | `rust/cp/` |
| **date** | -lutil | 🟡 **GergiOS-native** | 1.1 | Требует strftime, timezone |
| **dd** | -lutil | 🟡 **GergiOS-native** | 1.1 | Конвертация, сложная обработка сигналов |
| **df** | -lutil | 🟡 **GergiOS-native** | 1.1 | Статистика FS, getmntinfo |
| **echo** | libc only | ✅ **Rust** | 1.0 | `rust/echo/` (был до этого PR) |
| **ed** | -lcrypt | 🟡 **NetBSD compat** | 1.1 | Редактор, устаревший; pkgsrc или оставить |
| **expr** | libc only | 🟡 **GergiOS-native** | 1.1 | Парсер выражений |
| **hostname** | libc only | ✅ **Rust** | 1.0 | `rust/hostname/` |
| **kill** | libc only | ✅ **Rust** | 1.0 | `rust/kill/` |
| **ln** | libc only | ✅ **Rust** | 1.0 | `rust/ln/` |
| **ls** | -lutil | ✅ **Rust** | 1.0 | `rust/ls/` |
| **mkdir** | libc only | ✅ **Rust** | 1.0 | `rust/mkdir/` |
| **mv** | libc only | ✅ **Rust** | 1.0 | `rust/mv/` |
| **pwd** | libc only | ✅ **Rust** | 1.0 | `rust/pwd/` |
| **rm** | libc only | ✅ **Rust** | 1.0 | `rust/rm/` |
| **rmdir** | libc only | ✅ **Rust** | 1.0 | `rust/rmdir/` |
| **sleep** | -lm | ✅ **Rust** | 1.0 | `rust/sleep/` (был до этого PR) |
| **stty** | libc only | 🟡 **GergiOS-native** | 1.1 | tcsetattr, termios |
| **sync** | libc only | ✅ **Rust** | 1.0 | `rust/sync/` |
| **test** | libc only | 🟡 **GergiOS-native** | 1.1 | Ещё не портирован |
| **domainname** | libc only | 🟡 **GergiOS-native** | 1.1 | Ещё не портирован |

**Итого bin/**: 29 утилит. 15 сразу в Rust (GergiOS-native), 4 сложных (1.1), 5 NetBSD compat.

### 6.3 `sbin/` — System (/sbin)

Системные утилиты для администрирования. Многие требуют прав root.

| Утилита | Зависимости | Категория | Приоритет | Примечание |
|---------|------------|-----------|-----------|------------|
| **init** | -lutil -lcrypt | 🔴 **NetBSD compat** | 1.0 | process 1. Критичен. Замена — переписывание системы инициализации |
| **ifconfig** | сложный: RUMP, pf, inet6 | 🔴 **NetBSD compat** | 1.0 | Настройка сети. Огромная зависимость от ядра |
| **mount** | сложный: много FS | 🔴 **NetBSD compat** | 1.0 | Монтирование ФС. Завязан на VFS |
| **reboot** | -lutil | 🟡 **NetBSD compat** | 1.0 | reboot(2), halt |
| **shutdown** | libc only | 🟡 **NetBSD compat** | 1.0 | Сигналит init |
| **route** | сложный: routing | 🔴 **NetBSD compat** | 1.0 | Управление маршрутизацией |
| **sysctl** | сложный: sysctl MIB | 🔴 **NetBSD compat** | 1.0 | Доступ к параметрам ядра |
| **fsck** | -lutil -lprop | 🟡 **NetBSD compat** | 1.1 | Проверка ФС. Часть init |
| **chown** | libc only | 🟢 **GergiOS-native** | 1.1 | chown(2) |
| **mknod** | libc only | 🟢 **GergiOS-native** | 1.1 | mknod(2) |
| **nologin** | shell script | 🟢 **pkgsrc** | 1.1 | Простой скрипт |
| **ping** | -lm | 🟡 **NetBSD compat** | 1.1 | ICMP, raw socket, сложный |
| **ping6** | -lm -lipsec | 🟡 **NetBSD compat** | 1.1 | IPv6 ICMP |
| **rcorder** | -lutil | 🟡 **NetBSD compat** | 1.1 | Порядок rc скриптов |
| **fsck_ext2fs** | -lutil | 🟡 **NetBSD compat** | 1.1 | ext2fs fsck |
| **newfs_ext2fs** | -lutil -lprop | 🟡 **NetBSD compat** | 1.1 | mkfs.ext2 |
| **newfs_msdos** | -lutil -lprop | 🟡 **NetBSD compat** | 1.1 | FAT форматирование |
| **newfs_udf** | -lutil | 🟡 **NetBSD compat** | 1.1 | UDF форматирование |
| **newfs_v7fs** | -lutil | 🟡 **NetBSD compat** | 1.1 | V7 форматирование |

**Итого sbin/**: 18 утилит на MINIX. ~8 критических (NetBSD compat).
Большинство системных утилит жёстко завязаны на NetBSD ABI ядра.
GergiOS-native замена sbin/ — задача для 1.1+.

### 6.4 `usr.bin/` — User (/usr/bin)

Самый большой набор — ~80+ утилит. По категориям:

#### 6.4.1 Критическая инфраструктура → NetBSD compat (1.0)

| Утилита | Зависимости | Примечание |
|---------|------------|------------|
| **make** | -lutil | Система сборки. Критична для BSD Make |
| **sh** (не дублируется, см. bin/sh) | — | — |
| **ftp** | -ledit -lterminfo -lwolfssl | Сетевой клиент, сложный |
| **telnet** | -lterminfo -lwolfssl -lkrb5 -lpam | Очень сложный. Оставить в compat |
| **gzip** | -lz -lbz2 -llzma | Компрессия, внешние библиотеки |
| **login** | -lutil -lcrypt -lpam -lkrb5 | login(1). Завязан на PAM, auth |
| **passwd** | -lcrypt -lutil -lkrb5 -lwolfssl | Смена пароля. Kerberos, PAM |
| **su** | -lpam -lcrypt -lutil -lkrb5 -lhcrypto | Смена пользователя. PAM |
| **man** | -lutil | Чтение man страниц |
| **find** | -lutil | Поиск файлов |
| **xargs** | libc only | Аргументы команд |
| **sed** | libc only | Потоковый редактор |
| **patch** | libc only | Наложение патчей |
| **sort** | -lutil | Сортировка |
| **mail** | (TODO) | Email клиент |

#### 6.4.2 GergiOS-native кандидаты (1.0) — простые, POSIX-only

Эти утилиты зависят только от libc и имеют простую логику.

**✅ Уже в Rust:**
`basename`, `cat`, `chmod`, `cksum`, `cmp`, `comm`, `cp`, `cut`, `date`, `dd`, `df`, `dirname`, `domainname`, `du`, `echo`, `env`, `expand`, `false`, `fold`, `head`, `hostname`, `id`, `kill`, `ln`, `ls`, `mkdir`, `mv`, `nl`, `nohup`, `paste`, `pathchk`, `printenv`, `printf`, `pwd`, `rm`, `rmdir`, `seq`, `sleep`, `sort`, `split`, `stat`, `sync`, `tail`, `tee`, `test`, `time`, `touch`, `tr`, `true`, `tty`, `uname`, `unexpand`, `uniq`, `wc`, `yes`

**🟡 Ещё не портированы:**
_(все POSIX-утилиты 1.0 завершены!)_

#### 6.4.3 GergiOS-native (1.1) — средней сложности ✅

`colrm`, `join`, `jot`, `pr`, `rev`, `tabs`, `tsort`, `ul`, `unifdef`, `unvis`, `vis` — ✅ **все 11 реализованы**

#### 6.4.4 GergiOS-native (1.1) — простые (pure std)

Эти утилиты не требуют libc и реализуются на Rust как pure-std:

`cal`, `col`, `colcrt`, `column`, `csplit`, `fmt`, `hexdump`, `lam`, `uudecode`, `uuencode`, `uuidgen`, `what`, `whereis`, `xstr` — ✅ **все 14 реализованы**

#### 6.4.5 pkgsrc (опционально, 1.0+)

Остальные утилиты требуют libc или внешних библиотек — остаются в pkgsrc:

`m4`, `man`, `netstat`, `tput`, `unzip`

**✅ В Rust (июнь 2026):** `lock`, `logger`, `logname`, `lorder`, `machine`, `menuc`, `mkstr`, `msgc`, `newgrp`, `nice`, `pwhash`, `renice`, `w` — **13 утилит**

**✅ В Rust (июнь 2026):** `mcookie`, `mesg`, `mkfifo`, `mktemp`, `nbperf`, `pagesize`, `sdiff`, `shar`, `shlock`, `shuffle`, `soelim`, `units`, `users`, `wall`, `who`, `whois`, `write` — **17 из 18 (rsync пропущен — слишком сложный протокол, через pkgsrc)**

#### 6.4.6 Build-time инструменты (только для кросс-компиляции)

Эти утилиты используются только во время сборки системы, не нужны на target:

**✅ Все 6 в Rust (июнь 2026):**
- `genassym` — парсинг ASSYM/STRUCT/MEMBER макросов из C-заголовков, генерация .equ (ассемблерные символы)
- `mkcsmapper` — генерация таблиц кодировок из codepoint-пар
- `mkdep` — парсинг #include директив, генерация Makefile dependency правил
- `mkesdb` — парсинг конфигов [section]/key=value, генерация ESDB таблиц
- `mklocale` — парсинг LC_CTYPE/LC_COLLATE, генерация C-таблиц локалей
- `xinstall` — установка файлов с chmod/chown/strip поддержкой

### 6.5 Стратегия замены по приоритетам — ИТОГОВАЯ

#### ✅ Все реализуемые утилиты портированы на Rust **(132 утилиты)**

Весь `usr.bin/` раздел очищен. В Rust портированы все утилиты, которые можно
реализовать как single-file утилиты без внешних библиотек.

**Полный список Rust-утилит:**

```
# 6.4.2 Core POSIX (55):
basename    cat         chmod       cksum       cmp         comm
cp          cut         date        dd          df          dirname
domainname  du          echo        env         expand      false
fold        head        hostname    id          kill        ln
ls          mkdir       mv          nl          nohup       paste
pathchk     printenv    printf      pwd         rm          rmdir
seq         sleep       sort        split       stat        sync
tail        tee         test        time        touch       tr
true        tty         uname       unexpand    uniq        wc

# 6.4.3 Средней сложности (11):
colrm       join        jot         pr          rev         tabs
tsort       ul          unifdef     unvis       vis

# 6.4.4 Простые pure-std (14):
cal         col         colcrt      column      csplit      fmt
hexdump     lam         uudecode    uuencode    uuidgen     what
whereis     xstr

# 6.4.5a pkgsrc candidates (17):
mcookie     mesg        mkfifo      mktemp      nbperf      pagesize
sdiff       shar        shlock      shuffle     soelim      units
users       wall        who         whois       write

# 6.4.5b pkgsrc candidates (15):
banner      calendar    ctags       finger      flock       fpr
from        fsplit      gencat      getopt      ipcrm       ipcs
last        leave       locale

# 6.4.5c pkgsrc candidates (13):
lock        logger      logname     lorder      machine     menuc
mkstr       msgc        newgrp      nice        pwhash      renice
w

# 6.4.6 Build-time tools (6):
genassym    mkcsmapper  mkdep       mkesdb      mklocale    xinstall
```

#### Остались в pkgsrc (сложные, не портированы):

| Утилита | Причина | Источник |
|---------|---------|----------|
| **rsync** | Сложный протокол (rolling checksum, delta encoding) | pkgsrc/net/rsync |
| **m4** | Полноценный макропроцессор (~3000 LOC) | pkgsrc/devel/m4 |
| **bzip2** | Библиотека компрессии (libbz2) | pkgsrc/archivers/bzip2 |
| **unzip** | ZIP extraction (zlib) | pkgsrc/archivers/unzip |
| **tput** | Terminfo DB query | pkgsrc/misc/ncurses |
| **infocmp** | Terminfo comparison | pkgsrc/misc/ncurses |
| **indent** | C formatter (~5000 LOC) | pkgsrc/devel/indent |
| **man** | Man page reader (mandoc/groff) | pkgsrc/textproc/mandoc |
| **netstat** | Требует kernel API (kvm, sysctl) | pkgsrc/net/netstat |

#### Приоритет 1.1+: Shell и критическая инфраструктура (NetBSD compat)

Эти компоненты остаются от NetBSD — слишком объёмные для single-file Rust:

```
bin/sh, csh, ksh
usr.bin/ftp, telnet, gzip, login, passwd, su, make, find, sed, patch, sort, mail, xargs
```

**Причина**: shell (~50K LOC), ftp/telnet (сетевые протоколы),
login/passwd/su (PAM, Kerberos), make (BSD Make infrastructure),
find/sed (сложные парсеры).

#### Остаются NetBSD compat навсегда (sbin/):

```
sbin/init, ifconfig, mount, reboot, shutdown, route, sysctl,
fsck, newfs_*, ping, ping6, rcorder, chown, mknod
```

**Причина**: Системное администрирование, завязано на kernel API/ioctl.

### 6.6 Итоговая статистика — Июнь 2026

| Категория | Кол-во | % от ~250 tools |
|-----------|--------|-----------------|
| ✅ **Rust — реализовано** | **132** | **53%** |
| 🟢 **Pure std (Windows-совместимые)** | ~90 | 36% |
| 🔴 **POSIX-only (требуют unix/libc)** | ~42 | 17% |
| 🟡 **NetBSD compat (остались в C)** | ~50 | 20% |
| 🟢 **pkgsrc / deferred** | ~20 | 8% |

#### Build-статус Rust workspace (Windows 2026-06):

**✅ Pure std (собираются на Windows):**
`basename`, `cal`, `cat`, `cksum`, `cmp`, `col`, `colcrt`, `colrm`, `column`, `comm`,
`csplit`, `cut`, `date`, `dd`, `dirname`, `du`, `echo`, `env`, `expand`, `false`,
`fmt`, `fold`, `gencat`, `getopt`, `grep`, `head`, `hexdump`, `join`, `jot`, `lam`,
`leave`, `logname`, `lorder`, `machine`, `mcookie`, `menuc`, `mkdep`, `mkdir`,
`mkesdb`, `mklocale`, `msgc`, `mv`, `nbperf`, `nl`, `paste`, `pathchk`, `pr`,
`printenv`, `printf`, `pwhash`, `rev`, `rm`, `rmdir`, `sdiff`, `seq`, `shar`,
`shuffle`, `sleep`, `soelim`, `sort`, `split`, `tabs`, `tail`, `tee`, `test`,
`time`, `touch`, `tr`, `true`, `tsort`, `tty`, `ul`, `unexpand`, `unifdef`,
`uniq`, `unvis`, `uudecode`, `uuencode`, `uuidgen`, `vis`, `wc`, `what`,
`whereis`, `xstr`, `yes`

**🔴 POSIX-only (libc/unix, не на Windows):**
`banner`, `calendar`, `chmod`, `cp`, `ctags`, `df`, `domainname`, `finger`,
`flock`, `fpr`, `from`, `fsplit`, `genassym`, `hostname`, `id`, `ipcrm`, `ipcs`,
`kill`, `last`, `ln`, `locale`, `lock`, `logger`, `ls`, `mkcsmapper`, `mkfifo`,
`mkstr`, `mktemp`, `newgrp`, `nice`, `nohup`, `pagesize`, `pwd`, `renice`,
`shlock`, `stat`, `sync`, `uname`, `units`, `users`, `w`, `wall`, `who`,
`whois`, `write`, `xinstall`

**Build-time инфраструктура (не userland):**
`audio-buf`, `fuzz`, `minix-alloc`, `minix-driver`, `minix-rs`, `net-parse`, `procfs-path`

---

## 7. Risk Assessment

### 7.1 Migration Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| pkgsrc compatibility issues | Medium | Low | Test on QEMU before removing in-tree tools |
| Rebranding breaks scripts | Low | Low | `uname -s` still returns something consistent |
| Boot library cleanup breaks boot | Critical | Low | ✅ **Done** — ~37 unused files deleted, both arches build |
| VFS cleanup breaks FS | Critical | Low | Keep existing VFS, only remove unused filesystems |

### 7.2 Rebranding Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| MINIX name recognition loss | Low | High | Keep "MINIX" in technical references |
| Config migration confusion | Low | Low | Version file documents the change |
| Package compatibility | Low | Low | OS_NAME change propagates to pkgin |

### 7.3 Dependencies Between Phases

```
Phase 0 (Branding) ──→ Phase 1 (pkgsrc)
                              │
                              ↓
Phase 6 (boot lib) ←─── Phase 7 (VFS)
```

Все фазы могут выполняться параллельно. libc/libm не затрагиваются.
Phase 2 (crypto) и Phase 3 (CMake) завершены ✅

---

## 8. Success Criteria

1. **GergiOS boots** with new branding (boot menu, kernel announce, uname)
2. **NetBSD POSIX (BSD) userland** чётко определён как фундаментальный слой (libc, libm, sys-заголовки)
3. **GergiOS-native компоненты** собираются с CMake; NetBSD compat — с BSD Make (dual-build)
4. **wolfSSL** — sole crypto provider ✅ **(done)**
5. **Boot library** очищена от неиспользуемых FS/протоколов ✅
6. **100% of existing tests pass** after each phase
7. **Documentation** updated for GergiOS identity

**Ключевое отличие от предыдущей стратегии**: libc/libm НЕ заменяются.
NetBSD — не внешняя зависимость, а POSIX (BSD) userland, как в macOS.

---

## 9. Effort Summary

| Phase | Description | Effort | Risk | Priority | Status |
|-------|-------------|--------|------|----------|--------|
| **0** | GergiOS branding (boot, uname, motd) + Rust migration | 1 week | 🟢 Low | 🔴 High | ✅ **Done** |
| **1** | NetBSD ABI/userland консолидация | 4-8 weeks | 🟢 Low | 🟡 Medium | ✅ **Phase 1a: external/ + Rust build — Done** |
| **2** | Crypto consolidation (wolfSSL + hcrypto) | 3 months | 🟢 Low | 🟡 Medium | ✅ **Done** |
| **3** | BSD Make → CMake (dual-build) | 3 months | 🟢 Low | 🔴 High | ✅ **Done** |
| **4** | ~~libc → musl~~ — **Не нужно** | — | — | — | ❌ Отменён |
| **5** | ~~libm альтернатива~~ — **Не нужно** | — | — | — | ❌ Отменён |
| **6** | Boot library cleanup | 1-2 weeks | 🟢 Low | 🟢 Low | ✅ **Done** |
| **7** | VFS/filesystem cleanup | 4-8 weeks | 🟡 Medium | 🟢 Low | 🟡 План |

**Total estimated effort**: 4-8 weeks remaining (Q4 2026)
**Completed**: Phase 0 (branding + Rust migration) + Phase 1a (external/ cleanup + MK* flags) + Phase 2 (crypto) + Phase 3 (CMake) + Phase 6 (boot library)
**Remaining**: Phase 1b (pkgsrc meta-package), Phase 7 (VFS audit)
**NetBSD код**: не удаляется. NetBSD = POSIX (BSD) userland, как в macOS.
libc/libm/sys-заголовки остаются перманентно. Заменяются только криптография (✅), external пакеты (✅) и тулы (🟡).

---

## 10. Related Documents

- `planning/03_migration_roadmap.md` — overall roadmap (see Section 2: Architecture Migration)
- `planning/02_legacy_dependencies.md` — legacy dependency analysis
- `planning/05_i386_deprecation_timeline.md` — architecture deprecation
- `planning/06_openssl_to_wolfssl_migration.md` — crypto migration
- `planning/09_c_language_modernization.md` — C standard modernization
- `minix/include/minix/config.h` — OS_NAME, OS_RELEASE, OS_VERSION definitions
- `minix/kernel/main.c` — kernel announce() function
- `minix/servers/pm/misc.c` — uname service
- `minix/servers/mib/kern.c` — sysctl service
