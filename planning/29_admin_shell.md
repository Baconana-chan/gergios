# Admin + Shell — System Administration & Interactive Console

> **Status**: 🟢 Project A — M1+M2 реализованы (M3-M7 предстоят)
> **Связанные**: `planning/26_security_model_modernization.md` (audit, MAC, capabilities),
>   `planning/25_phase2_performance_detailed.md` (monitoring),
>   `planning/01_microkernel_architecture.md`
> **Репозитории**: `minix/servers/`, `minix/usr.sbin/`, `usr.bin/`

## 1. Текущее состояние

### 1.1 Что уже есть (разрозненные инструменты)

| Инструмент | Назначение | Тип | Язык |
|-----------|-----------|-----|------|
| **svrctl** | set/get параметров VFS/PM | CLI | C (~140 LOC) |
| **mtop** | Мониторинг процессов (top) | TUI | C |
| **netstat** | Сетевая статистика | CLI | C |
| **auditd** | Демон аудита безопасности | daemon | C |
| **macd** | Демон MAC политик | daemon | C |
| **btrace** | Трассировка системных вызовов | CLI | C |
| **diskctl** | Управление дисками | CLI | C |
| **fbdctl** | Управление framebuffer | CLI | C |
| **capsh** | Управление capabilities | CLI | C (~170 LOC) |
| **macctl** | Управление MAC политиками | CLI | Rust |
| **auditctl** | Управление аудитом | CLI | Rust |
| **audit2txt** | Конвертер аудита | CLI | Rust |
| **mac-compile** | Компилятор MAC политик | CLI | Rust |
| **/etc/rc.d/** | Скрипты управления сервисами | shell | sh |
| **/bin/sh** | Shell (ASH / MINIX sh) | shell | C |
| **/bin/ksh** | Korn shell | shell | C |

### 1.2 Проблемы текущей архитектуры

1. **Разрозненность** — каждый демон/сервис имеет свой CLI (`svrctl`, `btrace`, `mtop`, ...), нет единой точки входа
2. **Нет интерактивного admin shell** — нельзя войти в режим «администрирования» с автодополнением, историей, help
3. **Нет web/monitoring UI** — всё только через SSH/терминал
4. **Скрипты rc.d** — ручное управление, нет `systemctl status/restart/logs`
5. **Нет единого лога** — auditd пишет в свой лог, syslogd в свой, macd в stdout
6. **Нет dashboard** — нельзя увидеть сразу: CPU, память, диски, сеть, аудит, сервисы

---

## 2. Концепция: `gergios-admin` — Admin Shell

### 2.1 Что это?

Единый, интерактивный **admin shell** для GergiOS, объединяющий все аспекты администрирования в одном TUI/CLI инструменте.

```text
$ gergios-admin
╔══════════════════════════════════════════════════════════╗
║              GergiOS Admin Shell v0.1                    ║
╠══════════════════════════════════════════════════════════╣
║  Services │ System │ Network │ Security │ Audit │ Debug   ║
║━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━║
║  STATUS:  ✓ auditd running    ✓ macd running             ║
║           ✓ vfs running       ✓ pm running               ║
║           ✗ bluetoothd stopped                           ║
║━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━║
║  CPU: 12%  Mem: 245M/1024M  Disk: 2.1G/8G  Net: eth0 ↑ ║
║━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━║
║ admin@hostname > restart bluetoothd                      ║
║ ✓ bluetoothd restarted (pid 1423)                        ║
║ admin@hostname >                                        ║
╚══════════════════════════════════════════════════════════╝
```

### 2.2 Архитектура

```text
┌────────────────────────────────────────────────────────────────┐
│                       gergios-admin (Rust TUI)                  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                  Command Layer                             │  │
│  │  services | system | network | security | audit | debug    │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │                  Service Layer                             │  │
│  │  • ServiceManager — запуск/остановка/status/logs          │  │
│  │  • SystemMonitor — CPU, память, диски, uptime             │  │
│  │  • NetworkManager — интерфейсы, маршруты, статистика      │  │
│  │  • SecurityManager — MAC, capabilities, audit             │  │
│  │  • AuditViewer — поиск/фильтрация событий аудита          │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │                  Backend Layer                             │  │
│  │  • syscall → svrctl, PMGETPARAM, VFSGETPARAM             │  │
│  │  • IPC → sendrec(auditd), sendrec(macd)                  │  │
│  │  • procfs → /proc/*/psinfo (mtop-совместимый)             │  │
│  │  • sysctl → /etc/rc.d/sysctl                              │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

### 2.3 Команды (режим CLI и интерактивный)

```text
services list                          — список всех сервисов
services status [name]                 — статус сервиса(ов)
services start <name>                  — запустить
services stop <name>                   — остановить
services restart <name>                — перезапустить
services logs <name> [lines]           — последние N строк лога

system info                            — общая информация о системе
system cpu                             — CPU загрузка/температура
system memory                          — память (total/used/free)
system disk                            — диски (монтирование, место)
system uptime                          — аптайм, load average

network interfaces                     — список интерфейсов
network stats                          — счётчики пакетов/ошибок
network route                          — таблица маршрутизации
network arp                            — ARP таблица

security mac status                    — статус MAC enforcement
security mac show                      — правила MAC политики
security caps list <pid>               — capabilities процесса
security caps set <pid> <cap>          — установить capability
security audit search <filter>         — поиск событий аудита
security audit stats                   — статистика аудита

debug trace <pid>                      — трассировка процесса (btrace)
debug kcall                            — kernel call статистика
debug ipc                              — IPC статистика
```

---

## 3. Проект A: `minix-admin` (Rust crate + CLI/TUI)

### 3.1 План реализации

| Компонент | LOC | Сложность | Зависимости |
|-----------|-----|-----------|-------------|
| **CLI парсер** | 300 | 🟢 Легко | `minix-term` |
| **Service Manager** | 500 | 🟡 Средне | IPC с RS, procfs |
| **System Monitor** | 400 | 🟡 Средне | procfs, `sysctl`, `svrctl` |
| **Network Manager** | 400 | 🟡 Средне | `ioctl(socket)`, procfs/net |
| **Security Manager** | 300 | 🟡 Средне | IPC с macd, auditd, capctl |
| **Audit Viewer** | 400 | 🟡 Средне | IPC с auditd |
| **TUI режим (interactive)** | 500 | 🟡 Средне | `minix-term` TUI |
| **Справка / help** | 200 | 🟢 Легко | — |
| **Итого core** | ~3,000 | | |

### 3.2 Структура

```
rust/minix-admin/
├── Cargo.toml
├── Makefile
└── src/
    ├── main.rs              — Точка входа, CLI/TUI выбор
    ├── cli.rs               — Парсер команд
    ├── services.rs          — ServiceManager (RS IPC)     ✅
    ├── system.rs            — SystemMonitor (procfs, sysctl) ✅
    ├── shell.rs             — Интерактивный shell (readline-like) 🔄
    ├── network.rs           — NetworkManager (ioctl, sysctl) ✅
    ├── security.rs          — SecurityManager (macd, auditd, capctl) ✅
    ├── audit.rs             — AuditViewer (auditd IPC) 🔄
    ├── debug.rs             — Debug commands (btrace wrapper) 🔄
    └── tui.rs               — TUI dashboard (minix-term) 🔄
```

### 3.3 Milestones

| Milestone | Что готово | Время | Статус |
|-----------|-----------|-------|--------|
| **M1** | CLI парсер, `services list/status/start/stop/restart` | 1 неделя | ✅ Готово |
| **M2** | `system info/cpu/memory/disk/uptime` | 1 неделя | ✅ Готово |
| **M3** | `network interfaces/stats/route/arp` | 1 неделя | ✅ Готово |
| **M4** | `security mac/caps/audit` | 1 неделя | ✅ Готово |
| **M5** | Интерактивный TUI shell (readline, автодополнение, история) | 2 недели | ✅ Готово |
| **M6** | TUI dashboard (реал-тайм мониторинг, обновление каждые 2s) | 2 недели | ✅ Готово |
| **M7** | Документация, man pages, тесты | 1 неделя | ✅ Готово |

---

## 4. Проект B: `minish` — Minimal Rust Shell

### 4.1 Что это?

Замена `/bin/sh` на лёгкую Rust-shell с фокусом на:
- Безопасность (memory-safe, sandbox)
- Встроенные админ-команды (без внешних утилит)
- Tab-completion
- История команд
- Цветной prompt с информацией о системе

### 4.2 Архитектура

```
rust/minish/
├── Cargo.toml
├── Makefile
└── src/
    ├── main.rs              — REPL (read-eval-print loop)
    ├── parser.rs            — Парсер команд (трубы, перенаправления)
    ├── builtins.rs          — Встроенные команды (cd, ls, echo, etc.)
    ├── exec.rs              — Запуск внешних программ (fork/exec)
    ├── prompt.rs            — Цветной prompt (user@host, git status)
    ├── complete.rs          — Tab-completion
    ├── history.rs           — История команд
    └── jobs.rs              — Job control (bg, fg, jobs)
```

### 4.3 Встроенные команды (builtins)

| Команда | LOC | Описание |
|---------|-----|----------|
| `cd` | 30 | Сменить директорию |
| `ls` | 100 | Список файлов (цветной) |
| `echo` | 20 | Вывод текста |
| `pwd` | 10 | Текущая директория |
| `cat` | 40 | Вывод файла |
| `rm` | 30 | Удаление файлов |
| `mv` | 30 | Перемещение |
| `cp` | 40 | Копирование |
| `mkdir` | 20 | Создание директории |
| `ps` | 50 | Список процессов |
| `kill` | 20 | Отправка сигнала |
| `exit` | 10 | Выход |
| `help` | 30 | Справка |
| `export` | 20 | Переменные окружения |
| `source` | 20 | Выполнить скрипт |

### 4.4 Статус реализации

#### Реализовано ✅ (87 тестов)

| Компонент | LOC | Статус | Детали |
|-----------|-----|--------|--------|
| REPL цикл | 140 | ✅ | Интерактивный и скриптовый режимы, Ctrl+D exit |
| Парсер (трубы, перенаправления) | 310 | ✅ | `\|`, `>`, `>>`, `<`, `2>`, `&&`, `\|\|`, `&`, `;`, кавычки, escape |
| Builtins (15) | 550 | ✅ | cd, ls, echo, pwd, cat, rm, mv, cp, mkdir, ps, kill, help, export, source, true/false |
| exec (single + внешние) | 100 | ✅ | `std::process::Command`, перенаправления |
| **exec (pipe chain)** | **130** | **✅ NEW** | **Настоящие OS pipes через `Stdio::piped()`, spawn всех команд, wait, exit code последней** |
| Prompt | 80 | ✅ | Цветной user@host:cwd$ с цветом по exit code |
| Tab-completion | 130 | ✅ | Builtins, PATH, файлы, env vars |
| История | 120 | ✅ | In-memory (500 записей), навигация ↑↓, поиск, dedup |
| Raw-mode input | 220 | ✅ | termios raw mode, ↑↓←→ Home/End, Tab, Ctrl+D/U/L, Backspace/Delete |
| Job control | 320 | ✅ | jobs, fg, bg, background (&), process groups, SigCtl |
| **Итого core** | **~1,680** | | **87 тестов, 0 ошибок** |

#### Ключевые особенности exec

- **Single command** (`echo hello`): builtin → external `run_external()`
- **Conditionals** (`cmd1 && cmd2`, `cmd1 || cmd2`): последовательно с short-circuit (первая команда всегда выполняется)
- **Pipe chain** (`cmd1 | cmd2 | cmd3`): настоящие OS pipes — `Stdio::piped()` между всеми командами, конкурентный spawn, поддержка stdin/stdout/stderr redirects на любом этапе пайпа
- **Background** (`cmd &`): spawn в отдельном pgrp, `[1] 1234`, JobManager (jobs/fg/bg), SIGCONT, waitpid с WUNTRACED
- **Process groups**: `setpgid()` через `pre_exec` + родительская safetynet, сигналы SIGINT/SIGTSTP/SIGQUIT → SIG_DFL в дочерних процессах
- **Job Manager**: `jobs` (список), `fg %N` (foreground + wait), `bg %N` (SIGCONT + Running)
- **Cleanup**: `kill_children()` убивает частично запущенные процессы при ошибке

---

## 5. Проект C: Web Admin Dashboard

### 5.1 Что это?

Легковесный HTTP-сервер на Rust, предоставляющий:
- Web dashboard с реальным временем
- REST API для администрирования
- Серверная часть: `minix-httpd` (на базе lwIP + http-parser)

### 5.2 Архитектура

```text
┌─────────────────────────┐     ┌──────────────────────────┐
│      Web Browser         │     │   HTTP Client (curl)     │
│    localhost:8080        │     │   localhost:8080/api/... │
└─────────┬───────────────┘     └──────────┬───────────────┘
          │ HTTP/websocket                  │ HTTP/REST
┌─────────▼────────────────────────────────▼───────────────┐
│               minix-httpd (Rust HTTP server)               │
│                                                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐ │
│  │ Static   │ │ REST API │ │ WS Push  │ │ Auth         │ │
│  │ (HTML/JS)│ │ (/api/*) │ │ (events) │ │ (token)      │ │
│  └──────────┘ └────┬─────┘ └────┬─────┘ └──────────────┘ │
│                    │            │                          │
│  ┌─────────────────▼────────────▼───────────────────────┐ │
│  │           Backend collectors                          │ │
│  │  services │ system │ network │ audit │ security       │ │
│  └──────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────┘
```

### 5.3 API endpoints

```text
GET  /api/v1/system          — CPU, memory, disk, uptime
GET  /api/v1/services        — список сервисов и статус
POST /api/v1/services/<name>/start
POST /api/v1/services/<name>/stop
POST /api/v1/services/<name>/restart
GET  /api/v1/network         — интерфейсы, статистика
GET  /api/v1/audit?limit=50  — последние события аудита
GET  /api/v1/security/mac    — статус и правила MAC
```

### 5.4 Оценка

| Компонент | LOC | Сложность |
|-----------|-----|-----------|
| HTTP server (lwIP + http) | 800 | 🔴 Сложно |
| REST API handlers | 500 | 🟡 Средне |
| WebSocket push | 300 | 🟡 Средне |
| HTML/JS dashboard | 500 | 🟡 Средне |
| Auth (Token) | 200 | 🟡 Средне |
| **Итого** | **~2,300** | |

---

## 6. Приоритеты и рекомендации

### 6.1 Выбор проекта

| Проект | Значимость | Сложность | Время | Рекомендация |
|--------|-----------|-----------|-------|-------------|
| **A: `minix-admin`** | 🟢 Высокая | 🟡 Средняя | ~9 недель | **⭐ MVP** |
| **B: `minish`** | 🟡 Средняя | 🟡 Средняя | ~6 недель | 🟡 Опционально |
| **C: Web Dashboard** | 🟡 Средняя | 🔴 Высокая | ~8 недель | 🔵 После 1.0 |

**Рекомендация**: Начать с **Проекта A** (`minix-admin`), так как:
1. Покрывает основную потребность — единая точка входа для администрирования
2. Использует готовые компоненты (`minix-term`, `minix-net`, `minix-bt-stack`)
3. Даёт немедленную пользу — можно смотреть статус всех сервисов, управлять ими
4. TUI режим можно сделать на базе `minix-term`

### 6.2 Roadmap

```text
Неделя 1-2:   ▓▓▓▓░░░░░░  M1 + M2: CLI, services, system
Неделя 3-4:   ░░▓▓▓▓░░░░  M3 + M4: network, security, audit
Неделя 5-6:   ░░░░░░▓▓▓░  M5: TUI interactive shell
Неделя 7-8:   ░░░░░░░░▓▓  M6: Dashboard (real-time)
Неделя 9:     ░░░░░░░░░░  M7: docs, tests, polish
```

### 6.3 Технические решения

| Решение | Почему |
|---------|--------|
| **Rust** | Безопасность, serde, cargo check за секунды |
| **`minix-term`** | TUI уже готов (termios, ANSI, мышь, gamepad) |
| **IPC с серверами** | Стандартный MINIX `sendrec()` — быстрее чем procfs |
| **procfs** | Для чтения /proc/*/psinfo — уже есть |
| **Нет зависимостей** | Никаких внешних crate — только libc + minix-term |

---

## 7. Существующие инструменты для референса

### 7.1 GergiOS/MINIX утилиты

| Утилита | Что делает | Как вызывать |
|---------|-----------|-------------|
| `svrctl vfs get <param>` | Читает параметр VFS | `svrctl()` syscall |
| `svrctl pm get <param>` | Читает параметр PM | `svrctl()` syscall |
| `auditctl status` | Статус auditd | IPC `sendrec(auditd)` |
| `auditctl enable/disable` | Вкл/выкл аудит | IPC |
| `macctl status` | Статус MAC | IPC `sendrec(macd)` |
| `macctl enable/disable` | Вкл/выкл MAC | IPC |
| `btrace <pid>` | Трассировка процесса | `ptrace()` + `/proc/<pid>/psinfo` |
| `capsh --print` | Capabilities процесса | `SYS_CAPCTL` |
| `mtop` | Мониторинг | `/proc` |
| `netstat` | Сеть | `ioctl(socket, SIOCGIF*)` |

### 7.2 Linux референсы

| Утилита | Для чего |
|---------|----------|
| `systemctl` | Управление systemd services |
| `journalctl` | Просмотр логов |
| `top`/`htop` | Мониторинг |
| `ip` | Network (замена ifconfig + route) |
| `nmtui` | Network Manager TUI |
| `cockpit` | Web admin dashboard |

---

## 8. Сценарии использования

### 8.1 Диагностика проблемы

```text
$ gergios-admin
admin> services status
  ✓ auditd       running  pid=123  10s ago
  ✓ macd         running  pid=124  10s ago
  ✓ vfs          running  pid=100  2m ago
  ✗ bluetoothd   crashed  pid=0    5s ago  ← PROBLEM

admin> services logs bluetoothd
  [ERROR] HCI device /dev/hci0 not found
  [INFO]  Retrying in 5s...

admin> debug trace 1423
  SYS_READ  → 0 bytes
  SYS_IOCTL → ENODEV
  ...
```

### 8.2 Проверка безопасности

```text
admin> security mac status
  MAC enforcement: ENABLED
  Rules loaded: 47

admin> security audit search "IPC_DENIED last 1h"
  12:34:56  AUDIT_IPC_DENIED  pm → vfs  EPERM
  12:35:01  AUDIT_IPC_DENIED  init → mfs  EACCES ← investigation needed

admin> security caps list 1423
  CAP_NET_ADMIN
  CAP_SYS_RAWIO
```

### 8.3 Мониторинг (TUI dashboard)

```text
╔══════════════════════════════════════════════════════════════════╗
║              GergiOS Dashboard  (refreshing every 2s)           ║
╠══════════════════════════════════════════════════════════════════╣
║  Services              │ System          │ Network              ║
║  ─────────             │ ──────          │ ───────              ║
║  auditd   ▲ active     │ CPU: 23% ████   │ eth0: ↑1.2Mbps       ║
║  macd     ▲ active     │ Mem: 512MB/2GB  │ eth0: ↓3.4Mbps       ║
║  vfs      ▲ active     │ Disk: 45% ████  │ lo:    ↑0bps         ║
║  pm       ▲ active     │ Uptime: 3d 12h  │                      ║
║  rs       ▲ active     │ Procs: 47       │ Security              ║
║  bluetooth ▼ stopped   │ Load: 0.5       │ ───────              ║
║  dhcpcd   ▲ active     │                 │ MAC: ENABLED         ║
║  sshd     ▲ active     │ Audit Events    │ Caps: 1423 active    ║
║                         │ ──────────      │                      ║
║  Last Alert             │ Today: 142      │                      ║
║  12:34 AUDIT_IPC_DENIED │ This hr: 3      │                      ║
╚══════════════════════════════════════════════════════════════════╝
```

---

## 9. Технические риски

### 9.1 Зависимость от `minix-term`
- TUI режим требует raw-терминал (termios)
- На SSH-сессиях работает, на serial-консолях — тоже
- Но framebuffer-режим (без TTY) не поддерживается

### 9.2 IPC с MINIX серверами
- `sendrec(auditd)` и `sendrec(macd)` работают через MINIX IPC
- Требуются endpoint'ы сервисов (получаются через `ds_retrieve_label_endpt`)
- Некоторые сервисы могут не отвечать на неизвестные запросы

### 9.3 Производительность
- TUI обновление каждые 2s — нормально для администрирования
- WebSocket push для dashboard — требует lwIP + http-parser

---

## Приложение A: Сравнение вариантов shell

| Аспект | `minix-admin` (TUI) | `minish` (REPL) | Web Dashboard |
|--------|--------------------|----------------|---------------|
| **Интерактивность** | ✅ Высокая (TUI) | ✅ Высокая (REPL) | ✅ Высокая (Web) |
| **Scriptability** | ⚠️ Частично | ✅ Полная | ❌ Нет |
| **Удалённый доступ** | ⚠️ Через SSH | ✅ Через SSH | ✅ Любой браузер |
| **Сложность** | 🟡 3,000 LOC | 🟡 1,800 LOC | 🔴 2,300 LOC |
| **Зависимости** | `minix-term` + IPC | `libc` + fork/exec | lwIP + http |
| **Время** | ~9 недель | ~6 недель | ~8 недель |

## Приложение B: Глоссарий

| Термин | Описание |
|--------|----------|
| **Admin Shell** | Интерактивная консоль для администрирования системы |
| **TUI** | Text-based User Interface — текстовый интерфейс на базе терминала |
| **REPL** | Read-Eval-Print Loop — интерактивный цикл команд |
| **IPC** | Inter-Process Communication — MINIX sendrec() |
| **procfs** | /proc — process filesystem |
| **RS** | Reincarnation Server — управление сервисами в MINIX |
