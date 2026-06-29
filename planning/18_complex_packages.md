# Complex Packages — Deferred to GergiOS 1.1+

> **Part of**: Overall modernization roadmap
> **Related**: `planning/10_netbsd_dependency_audit.md` (Section 6.4.5 pkgsrc)
> **Статус**: Список deferred-пакетов, требующих значительных усилий

---

## 1. Зачем этот документ

Некоторые утилиты из `usr.bin/` (и смежных каталогов) слишком сложны для
реализации в виде single-file Rust-утилиты. Они требуют:

- Внешних библиотек (zlib, terminfo, libarchive)
- Доступа к системным интерфейсам ядра (kvm, sysctl, routing)
- Полноценных парсеров/компиляторов (m4, indent)
- Существенного объёма кода (тысячи строк)

Вместо того чтобы пытаться портировать их срезами, они откладываются до
**GergiOS 1.1+** (post-release), когда будет больше контекста для
серьёзных реализаций.

---

## 2. Категории сложности

### 🟢 Категория A: Сложные, но реализуемые (1.1)

Эти утилиты можно реализовать на Rust с минимальными зависимостями,
но требуется нетривиальный объём кода (>500 строк).

| Утилита | Описание | Приблизительный LOC | Сложность |
|---------|----------|---------------------|-----------|
| **gencat** | Message catalog compiler | ~200 | 🟢 Medium |
| **indent** | C code formatter | ~3000-5000 | 🔴 High |
| **m4** | Macro processor | ~2000-4000 | 🔴 High |
| **unzip** | ZIP archive extractor | ~1500 | 🟡 Medium-High |
| **banner** | ASCII art banner | ~500 (font data) | 🟢 Medium |

> **Примечание**: `gencat` и `banner` уже реализованы как Rust-утилиты.
> Остальные требуют больше работы.

### 🟡 Категория B: Требуют внешних библиотек (1.1+)

Эти утилиты зависят от внешних библиотек, которые нужно либо
портировать, либо подключать через pkgsrc.

| Утилита | Зависимость | Альтернатива |
|---------|------------|-------------|
| **bzip2** | libbz2 (собственная) | Через pkgsrc (`archivers/bzip2`) |
| **tput** | libterminfo | Встроенная terminfo DB или pkgsrc |
| **infocmp** | libterminfo | pkgsrc |
| **unzip** | zlib | Чистый Rust zlib (miniz-oxide) или pkgsrc |
| **man** | groff/mandoc | pkgsrc `textproc/mandoc` |

### 🔴 Категория C: Требуют kernel API (1.2+)

Эти утилиты требуют доступа к внутренним структурам ядра (kvm, sysctl,
routing table). Они останутся NetBSD compat layer до появления
GergiOS-native syscall API.

| Утилита | Kernel API | Причина |
|---------|-----------|---------|
| **netstat** | sysctl, kvm, routing sockets | Статистика сети, требует доступа к ядру |
| **ifconfig** | SIOCGIF* ioctl, PF_KEY | Настройка интерфейсов |
| **route** | PF_ROUTE socket | Управление маршрутизацией |
| **sysctl** | sysctl MIB tree | Параметры ядра |
| **ps** | kvm (kern.proc) | Список процессов |
| **w** | kvm, utmp | Кто залогинен (utmp-only уже в Rust) |
| **arp** | PF_ROUTE, ioctl | ARP таблица |
| **ndp** | PF_ROUTE | IPv6 Neighbor Discovery |

### 🔴 Категория D: Критическая инфраструктура (1.2+)

Эти компоненты являются фундаментальными для системы и требуют
осторожного подхода к замене.

| Утилита | Роль | Причина |
|---------|------|---------|
| **init** | Process 1 | Замена = переписывание системы инициализации |
| **sh** | Bourne shell | ~50K LOC C, POSIX shell spec ~2000 строк |
| **csh** | C shell | ~30K LOC C |
| **make** | BSD Make | Система сборки, завязана на BSD Make инфраструктуру |
| **find** | Поиск файлов | Сложный парсер выражений |
| **sed** | Stream editor | Сложный парсер |
| **patch** | Apply patches | Сложный алгоритм |
| **ftp** | FTP client | Сетевые протоколы, аутентификация |
| **telnet** | Telnet client | Сетевые протоколы, аутентификация |
| **login** | User login | PAM, Kerberos, shadow |
| **passwd** | Password change | PAM, Kerberos, wolfSSL |
| **su** | Switch user | PAM, Kerberos |

---

## 3. Что осталось в pkgsrc (окончательный список)

После полной очистки Section 6.4.5, в pkgsrc остаются только
действительно сложные пакеты:

```
Оставшиеся в pkgsrc (не портированы на Rust):
─────────────────────────────────────────────
m4          # макропроцессор (~3000 LOC)
netstat     # статистика сети (kernel API)
tput        # terminfo query (terminfo DB)
unzip       # ZIP extraction (zlib)
man         # man page reader (groff/mandoc)

Дополнительно (не в 6.4.5, но deferred):
──────────────────────────────────────────
indent      # C formatter (~5000 LOC)
bzip2       # компрессия (libbz2)
infocmp     # terminfo comparison
```

---

## 4. План реализации (пост-релиз)

### GergiOS 1.1 (первый пострелиз)

**Категория A** (на что есть шанс):
- `unzip` — через крейт `zip` на Rust (crates.io)
- `indent` — минимальная версия (только базовое форматирование)

**Улучшения:**
- `pwhash` — настоящий SHA-256/SHA-512 вместо DefaultHasher
- `units` — поддержка температурных конверсий (C/F/K)
- `mcookie` — чтение `/dev/urandom` вместо LCG

### GergiOS 1.2 (второй пострелиз)

**Категория B-C**:
- `tput`/`infocmp` — встроенная terminfo DB (или через pkgsrc)
- `bzip2` — через Rust crate `bzip2` или `bzlib`
- `netstat` — после стабилизации kernel syscall API

**Инфраструктура:**
- `man` — через mandoc или w3m
- `find`/`sed` — Rust-версии с полной совместимостью

### GergiOS 2.0 (дальняя перспектива)

**Категория D**:
- Замена shell (sh/csh) на Rust shell
- init → GergiOS-native system manager
- BSD Make → полностью CMake

---

## 5. Связанные документы

- `planning/10_netbsd_dependency_audit.md` — полный аудит userland
- `planning/17_remaining_tasks.md` — текущие задачи
