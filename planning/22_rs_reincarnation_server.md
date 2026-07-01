# Reincarnation Server — "Заботливая мамочка"

> **Статус**: 📋 Планирование (July 2026)
> **Целевой релиз**: "Если останется время перед 1.0" (Q1 2027)
> **Приоритет**: Низкий — после стабилизации x86_64 + ARM64 + ext4 + базового GUI
> **Связанные**: `minix/servers/rs/` (существующий RS, ~3000 LOC)

---

## 1. Executive Summary

### 1.1 Что такое RS сейчас

Reincarnation Server (RS) — это сердце отказоустойчивости MINIX 3. Он:
- Запускает системные сервисы при загрузке
- Мониторит их через heartbeat'ы
- Перезапускает упавшие сервисы (reincarnation)
- Поддерживает live update (обновление без остановки)

**Текущий уровень**: "Просто перезапусти" ✅
 
### 1.2 Что мы хотим

Превратить RS в "заботливую мамочку", которая:
1. **Перепробует всё** — десятки стратегий восстановления
2. **Соберёт диагностику** — core dump, логи, состояние зависимостей
3. **Проанализирует причину** — segfault? memory leak? deadlock?
4. **Устранит проблему** — освободит память, перезапустит зависимости, изолирует сбой
5. **Сдастся красиво** — только исчерпав все варианты, с полным отчётом

---

## 2. Текущая архитектура RS

```
RS →
  ├── main.c         — главный цикл, получение сообщений
  ├── request.c      — обработка запросов (RS_UP, RS_DOWN, RS_RESTART, ...)
  ├── manager.c      — управление сервисами (start, stop, restart, crash)
  ├── exec.c         — execve сервисов
  ├── update.c       — live update
  ├── utility.c      — утилиты (init_service, asynsend, reply)
  ├── error.c        — строки ошибок
  ├── table.c        — boot image tables (какие сервисы запускать)
  ├── const.h        — константы (RS_IN_USE, RS_EXITING, RS_REINCARNATE...)
  ├── type.h         — struct rproc (системный процесс), struct rprocupd
  └── proto.h        — прототипы
```

### Ключевые структуры данных

```c
// struct rproc — каждый сервис
struct rproc {
    struct rprocpub *r_pub;       // публичная информация (endpoint, label)
    int r_restarts;                // сколько раз перезапускался
    long r_backoff;                // сколько периодов ждать перед рестартом
    unsigned r_flags;              // RS_IN_USE, RS_EXITING, RS_ACTIVE, ...
    clock_t r_alive_tm;            // когда был последний heartbeat
    clock_t r_check_tm;            // когда проверяли в последний раз
    char r_script[MAX_SCRIPT_LEN]; // скрипт восстановления
    // ... (ещё ~20 полей)
};
```

### Текущая логика восстановления

```
1. Сервис умирает → kernel → SIGCHLD → RS
2. RS проверяет r_flags:
   - RS_EXITING → "ожидаемая смерть" → не рестартим
   - RS_REINCARNATE → "надо рестартить" → restart_service()
3. restart_service():
   a. fork()
   b. execve() сервиса
   c. Ждём RS_INIT (инициализация)
   d. Если OK → сервис снова работает
4. Если сервис падает слишком часто:
   a. r_backoff++ (увеличиваем задержку)
   b. Пропускаем рестарты на r_backoff периодов
```

---

## 3. Уровни зрелости RS

### Уровень 1: "Just restart it" ✅ (текущий)

```
Падение → restart → готово
```

**Что есть**: всё перечисленное в §2
**Чего нет**: диагностики, анализа, healthcheck'ов, графа зависимостей

---

### Уровень 2: "Health monitoring" 🟡 ~2-3 недели

**Цель**: RS не просто ждёт death notification, а активно проверяет здоровье сервисов.

#### Новые структуры

```c
// Healthcheck — кастомная проверка
struct rs_healthcheck {
    endpoint_t ep;                     // какой сервис проверяем
    int (*check_fn)(endpoint_t ep);    // функция проверки (0 = OK, -1 = dead)
    clock_t interval;                  // как часто проверять (в тиках)
    clock_t timeout;                   // максимальное время ответа
    char name[32];                     // имя проверки ("ping", "fs_ready", ...)
};

// Типы healthcheck'ов
enum healthcheck_type {
    HC_PING,            // сервис отвечает на IPC?
    HC_HEARTBEAT,       // heartbeat своевременен?
    HC_RESOURCES,       // нет утечки памяти/дескрипторов?
    HC_RESPONSE_TIME,   // время ответа на запрос?
    HC_CUSTOM,          // зарегистрированная сервисом проверка
};
```

#### Новая логика

```c
// Периодическая проверка (в do_period)
for_each_service(rp) {
    if (rp->r_healthchecks == NULL) continue;
    
    for_each_healthcheck(hc, rp) {
        int status = hc->check_fn(rp->r_pub->endpoint);
        
        if (status != OK) {
            // Сервис не прошёл healthcheck
            rs_log("RS: %s failed healthcheck '%s' (status=%d)",
                   srv_to_string(rp), hc->name, status);
            
            // Level 1: просто перезапуск
            // Level 2+: диагностика
            diagnose_service(rp);
            restart_service(rp);
            break;
        }
    }
}
```

#### Изменения в коде

| Файл | Изменения |
|------|-----------|
| `type.h` | Добавить `struct rs_healthcheck`, `struct rs_diag` |
| `const.h` | Добавить `RS_HEALTHCHECK_FAIL`, `RS_HEALTHCHECK_INTERVAL` |
| `proto.h` | Добавить `do_healthcheck()`, `register_healthcheck()` |
| `manager.c` | Добавить `diagnose_service()`, `check_healthchecks()` |
| `request.c` | Добавить `RS_REGISTER_HEALTHCHECK` IPC |
| `main.c` | Добавить `do_healthcheck` в цикл обработки |

---

### Уровень 3: "Dependency-aware recovery" 🟡 ~2-3 недели

**Цель**: RS знает, какие сервисы от каких зависят, и восстанавливает в правильном порядке.

#### Новые структуры

```c
// Зависимость между сервисами
struct rs_dep {
    endpoint_t service;         // кто зависит
    endpoint_t depends_on;      // от кого зависит
    int critical;               // TRUE = не может работать без зависимости
    int restart_order;          // порядок при рестарте (1 = сначала этот)
    char reason[64];            // почему зависит ("provides block I/O")
};

// Статус зависимости
struct rs_dep_status {
    endpoint_t service;
    endpoint_t depends_on;
    int is_alive;               // зависимость жива?
    int is_healthy;             // зависимости здорова?
    clock_t last_alive;         // когда видела зависимость живой
};
```

#### Новые таблицы

```c
// Таблица зависимостей (в table.c или отдельном файле)
struct rs_dep dep_table[] = {
    // FS серверы → block driver
    { VFS_PROC_NR,  DEV_PROC_NR,      .critical = TRUE,  .restart_order = 1 },
    { MFS_PROC_NR,  DEV_PROC_NR,      .critical = TRUE,  .restart_order = 1 },
    { EXT4_PROC_NR, DEV_PROC_NR,      .critical = TRUE,  .restart_order = 1 },
    
    // VFS → FS drivers (монтированные ФС)
    { VFS_PROC_NR,  MFS_PROC_NR,      .critical = FALSE, .restart_order = 2 },
    { VFS_PROC_NR,  EXT4_PROC_NR,     .critical = FALSE, .restart_order = 2 },
    
    // Все → PM (process management)
    { ALL_ENDPOINTS, PM_PROC_NR,      .critical = TRUE,  .restart_order = 0 },
    
    // Все → VM (memory management)
    { ALL_ENDPOINTS, VM_PROC_NR,      .critical = TRUE,  .restart_order = 0 },
};
```

#### Новая логика рестарта

```
Сервис A падает:
  1. RS проверяет: у A есть зависимости?
  2. Да: B (от которого A зависит)
  3. B тоже мёртв? → перезапустить B сначала
  4. Подождать, пока B инициализируется
  5. Затем перезапустить A
  
  6. A зависит C и D → каскадный рестарт
  7. Все зависимости восстановлены → готово
  
  8. Если критическая зависимость не восстанавливается →
     RS: "Я не могу восстановить A, потому что B мёртв и не хочет жить"
```

---

### Уровень 4: "Diagnostics & analysis" 🟡 ~4-6 недель

**Цель**: RS собирает полную диагностику перед каждым рестартом и анализирует причину падения.

#### Диагностический пакет

```c
struct rs_diag_packet {
    // Кто упал
    endpoint_t ep;
    char label[RS_MAX_LABEL_LEN];
    
    // Как упал
    int signal;                     // SIGSEGV, SIGKILL, SIGBUS, ...
    int exit_status;                // код возврата (если exit())
    clock_t uptime;                 // сколько проработал (в тиках)
    
    // Ресурсы до падения
    uint64_t mem_usage;             // память (в байтах)
    uint64_t mem_mapped;            // mmap'ированная память
    int fd_count;                   // открытые файловые дескрипторы
    int ipc_send_queue;             // IPC очередь отправки
    
    // Системные ресурсы
    struct {
        uint64_t free_mem;          // свободная память
        uint64_t free_blocks;       // свободные блоки ФС
        int cpu_load;               // загрузка CPU (0-100)
        int total_procs;            // всего процессов
    } system;
    
    // Стек (если доступен)
    char stack_trace[4096];
    
    // Зависимости
    struct rs_dep_status deps[32];
    int num_deps;
    
    // Логи (последние N сообщений IPC)
    char recent_log[8192];
    
    // Предполагаемая причина
    enum fail_reason {
        FAIL_UNKNOWN,               // неизвестно
        FAIL_SEGFAULT,              // баг (SIGSEGV)
        FAIL_NOMEM,                 // не хватило памяти
        FAIL_TIMEOUT,               // завис (no heartbeat)
        FAIL_DEADLOCK,              // взаимная блокировка
        FAIL_HW_ERROR,              // ошибка железа
        FAIL_DEP_DIED,              // зависимость умерла
        FAIL_RESOURCE_EXHAUSTION,   // исчерпание ресурсов
        FAIL_SOFTWARE_BUG,          // программная ошибка
    } fail_reason;
    
    // Вердикт
    char recommendation[256];       // что делать пользователю
};
```

#### Анализ причины

```c
fail_reason_t analyze_failure(struct rs_diag_packet *dp) {
    // SIGSEGV → баг
    if (dp->signal == SIGSEGV) return FAIL_SEGFAULT;
    
    // Нет сигнала, просто exit с ненулевым кодом
    if (dp->signal == 0 && dp->exit_status != 0) {
        // Много рестартов за короткое время → возможно нехватка ресурсов
        if (dp->uptime < MIN_UPTIME && dp->system.free_mem < LOW_MEM_THRESHOLD)
            return FAIL_NOMEM;
        return FAIL_SOFTWARE_BUG;
    }
    
    // Таймаут heartbeat'а
    if (dp->signal == SIGKILL && dp->exit_status == 0)
        return FAIL_TIMEOUT;
    
    return FAIL_UNKNOWN;
}
```

---

### Уровень 5: "Proactive recovery" 🟡 ~4-6 недель

**Цель**: RS не просто диагностирует, а активно устраняет причину падения.

#### Стратегии восстановления

```c
enum recovery_strategy {
    STRAT_RESTART,                  // 1. Просто перезапуск
    STRAT_RESTART_DEPS,             // 2. + перезапуск зависимостей
    STRAT_RESTART_CLEAN,            // 3. + очистка состояния
    STRAT_RESTART_ISOLATE,          // 4. + изоляция (новый endpoint)
    STRAT_RESTART_MINIMAL,          // 5. + минимальный режим
    STRAT_FREE_MEMORY,              // 6. освободить память через VM
    STRAT_CLEAR_CACHE,              // 7. очистить кеш ФС через VFS
    STRAT_RECONFIGURE,              // 8. переконфигурировать сервис
    STRAT_RECOVER_JOURNAL,          // 9. восстановить журнал ext4
    STRAT_FALLBACK_DRIVER,          // 10. fallback на другой драйвер
    STRAT_USER_ALERT,               // 11. уведомить пользователя
    STRAT_SURRENDER,                // 12. белый флаг
};

// Приоритет стратегий (пробуем по порядку)
struct recovery_plan {
    enum recovery_strategy strategies[8];  // до 8 попыток
    int num_strategies;
    clock_t timeout_per_attempt;           // таймаут на попытку
    int max_attempts_total;                // всего попыток до surrender
};
```

#### Пример recovery plan для разных причин

```c
// Для FAIL_NOMEM (не хватило памяти)
struct recovery_plan plan_nomem = {
    .strategies = {
        STRAT_FREE_MEMORY,          // "VM, освободи кеш"
        STRAT_CLEAR_CACHE,          // "VFS, очисти буферный кеш"
        STRAT_RESTART_CLEAN,        // "перезапусти с чистой памятью"
        STRAT_RESTART_MINIMAL,      // "запусти в минимальном режиме"
        STRAT_USER_ALERT,           // "пользователь, у тебя кончилась память"
    },
    .num_strategies = 5,
    .timeout_per_attempt = 2 * system_hz,  // 2 секунды на попытку
    .max_attempts_total = 10,
};

// Для FAIL_SEGFAULT (баг)
struct recovery_plan plan_segfault = {
    .strategies = {
        STRAT_RESTART_ISOLATE,      // "перезапусти с новым endpoint"
        STRAT_RESTART_DEPS,         // "перезапусти зависимости"
        STRAT_RESTART_CLEAN,        // "очисти и перезапусти"
        STRAT_USER_ALERT,           // "пользователь, у тебя баг в сервисе X"
    },
    .num_strategies = 4,
    .timeout_per_attempt = 5 * system_hz,
    .max_attempts_total = 5,
};

// Для FAIL_HW_ERROR (ошибка железа)
struct recovery_plan plan_hw = {
    .strategies = {
        STRAT_RESTART,              // "может повезёт?"
        STRAT_FALLBACK_DRIVER,      // "попробуй другой драйвер"
        STRAT_USER_ALERT,           // "пользователь, у тебя проблема с железом"
    },
    .num_strategies = 3,
    .timeout_per_attempt = 3 * system_hz,
    .max_attempts_total = 3,
};
```

---

### Уровень 6: "Заботливая мамочка" ★ ~8-12 недель

**Цель**: Полная реализация концепции — RS проходит воду, огонь и медные трубы, прежде чем поднять белый флаг.

#### Полный цикл восстановления

```
 Сервис VFS упал (SIGSEGV):
 
 Шаг 1 ─── Обнаружение
   RS получает SIGCHLD (сигнал смерти)
   rp = rproc_ptr[VFS_PROC_NR]
   rp->r_flags |= RS_TERMINATED
 
 Шаг 2 ─── Сбор диагностики
   rs_collect_diag(rp, &diag)
     ├── signal = SIGSEGV
     ├── stack_trace = sys_diagctl_stacktrace(VFS_PROC_NR)
     ├── mem_usage = vm_query(VFS_PROC_NR)
     ├── deps[] = { DEV_PROC_NR → alive=true, MFS_PROC_NR → alive=true }
     └── system.free_mem = vm_info(MEM_FREE)
 
 Шаг 3 ─── Анализ причины
   reason = analyze_failure(&diag)
   → FAIL_SEGFAULT (signal=SIGSEGV, no memory pressure)
   → Выбираем recovery_plan: plan_segfault
 
 Шаг 4 ─── Попытки восстановления
   Attempt 1: STRAT_RESTART_ISOLATE
     → fork + execve VFS (новый endpoint)
     → VFS падает снова (SIGSEGV) через 0.5s
     └── Fail
 
   Attempt 2: STRAT_RESTART_DEPS
     → Перезапускаем DEV (block driver) — всё равно надо
     → Ждём готовности DEV
     → Перезапускаем VFS
     → VFS снова падает через 0.3s
     └── Fail
 
   Attempt 3: STRAT_FREE_MEMORY
     → "VM, освободи кеш"
     → Перезапускаем VFS
     → VFS падает
     └── Fail
 
   Attempt 4: STRAT_CLEAR_CACHE
     → "VFS (старая)... а, она мертва"
     → Пропускаем
     └── Skip
     
   Attempt 5: STRAT_USER_ALERT
     → Есть core dump? Да, сохранён.
     → Сохраняем диагностический пакет
  
 Шаг 5 ─── Белый флаг
   RS:
   ╔══════════════════════════════════════════════════════════╗
   ║  ── RS: "Я перепробовала всё, что могла..."          ── ║
   ║                                                        ║
   ║  Service:   VFS (Virtual File System server)            ║
   ║  PID:       1423                                       ║
   ║  Uptime:    2h 34m                                     ║
   ║                                                        ║
   ║  Cause:     SIGSEGV (signal 11) at 0x7F00DEAD          ║
   ║             Stack:                                      ║
   ║               ext4_write+0x142                          ║
   ║               vfs_write+0x78                            ║
   ║               syscall_handler+0x3f                      ║
   ║                                                        ║
   ║  Attempts:  5 (in 8.2s)                                ║
   ║    • Restart with isolation → FAIL                     ║
   ║    • Restart with deps    → FAIL                       ║
   ║    • Free memory          → FAIL                       ║
   ║                                                        ║
   ║  System state:                                         ║
   ║    • Memory: 12.4 GB free / 24 GB total (enough)        ║
   ║    • CPU: idle 94%                                     ║
   ║    • All other services: healthy                       ║
   ║                                                        ║
   ║  Core dump: /var/log/rs/crash/vfs.20260701-143502.dump ║
   ║  Log:       /var/log/rs/crash/vfs.20260701-143502.log  ║
   ║                                                        ║
   ║  "Мне очень жаль. Я сделала всё, что могла,           ║
   ║   но VFS продолжает падать. Пожалуйста,                ║
   ║   проверь лог и core dump, может быть,                 ║
   ║   это связано с последним обновлением ext4."           ║
   ║                                                        ║
   ║  Suggest:                                              ║
   ║    $ gdb /usr/sbin/vfs /var/log/rs/crash/vfs.dump      ║
   ║    $ tail -100 /var/log/rs/crash/vfs.log               ║
   ║                                                        ║
   ║  RS будет ждать твоих указаний.                        ║
   ╚══════════════════════════════════════════════════════════╝
```

---

## 4. Изменения в IPC и протоколах

### Новые системные вызовы (IPC → RS)

```c
// Регистрация healthcheck'а
#define RS_REGISTER_HEALTHCHECK   (RS_RQ_BASE + 20)

// Отправка диагностики после восстановления
#define RS_DIAG_REPORT            (RS_RQ_BASE + 21)

// Запрос на освобождение ресурсов
#define RS_FREE_RESOURCES         (RS_RQ_BASE + 22)

// Уведомление о зависимости
#define RS_REGISTER_DEP           (RS_RQ_BASE + 23)
```

### Изменения в IPC между RS и другими серверами

```c
// RS → VM: освободить память
#define VM_RS_FREE_MEM     (VM_RQ_BASE + 50)

// RS → VFS: очистить буферный кеш
#define VFS_RS_CLEAR_CACHE (VFS_RQ_BASE + 50)

// RS → scheduler: увеличить квант для сервиса
#define SCHED_RS_BOOST     (SCHED_RQ_BASE + 10)
```

---

## 5. План реализации

### Phase 0: Foundation (Level 1) ✅ (существующий RS)

Ничего не делаем — текущий RS работает.

### Phase 1: Level 2 — Healthchecks (~2-3 недели)

**Новые файлы**:
```
minix/servers/rs/
  ├── health.c          ← healthcheck framework
  └── health.h          ← healthcheck structures
```

**Изменения**:
- `type.h`: добавить `struct rs_healthcheck`, `enum healthcheck_type`
- `manager.c`: `check_service_health()`, `handle_healthcheck_failure()`
- `request.c`: `do_register_healthcheck()`, `do_unregister_healthcheck()`
- `main.c`: вызов `check_service_health()` в `do_period()`
- `proto.h`: новые прототипы

### Phase 2: Level 3 — Dependency graph (~2-3 недели)

**Новые файлы**:
```
minix/servers/rs/
  └── deps.c           ← dependency management
```

**Изменения**:
- `type.h`: добавить `struct rs_dep`, `struct rs_dep_status`
- `table.c`: таблица зависимостей
- `manager.c`: `cascade_restart()`, `check_dependencies()`
- `request.c`: `do_register_dep()`

### Phase 3: Level 4 — Diagnostics (~4-6 недель)

**Новые файлы**:
```
minix/servers/rs/
  ├── diag.c           ← diagnostic collection
  ├── diag.h           ← diagnostic structures
  └── analyze.c        ← failure analysis
```

**Изменения**:
- `type.h`: добавить `struct rs_diag_packet`, `enum fail_reason`
- `manager.c`: `collect_diagnostics()`, `analyze_and_recover()`
- `utility.c`: `save_core_dump()`, `save_diag_report()`
- IPC с kernel: `sys_diagctl_stacktrace()` для снятия стека

### Phase 4: Level 5 — Proactive recovery (~4-6 недель)

**Новый файл**:
```
minix/servers/rs/
  ├── strategy.c       ← recovery strategies
  └── strategy.h       ← strategy definitions
```

**Изменения**:
- `type.h`: добавить `struct recovery_plan`, `enum recovery_strategy`
- `manager.c`: `execute_recovery_plan()`
- IPC с VM: `VM_RS_FREE_MEM`
- IPC с VFS: `VFS_RS_CLEAR_CACHE`
- IPC с scheduler: `SCHED_RS_BOOST`

### Phase 5: Level 6 — "Заботливая мамочка" (~8-12 недель)

**Новый файл**:
```
minix/servers/rs/
  ├── surrender.c      ← white flag / surrender logic
  └── surrender.h      ← surrender structures
```

**Изменения**:
- `manager.c`: полный recovery loop (Level 5 → surrender)
- `error.c`: человеческие сообщения об ошибках
- `surrender.c`: красивый вывод с диагностикой
- `diag.c`: интеграция всех diagnostic пакетов

---

## 6. Оценка объёма работ

| Компонент | LOC | Фаза | Сложность |
|-----------|-----|------|-----------|
| Healthcheck framework | ~400 | P1 | 🟡 Средняя |
| Dependency graph | ~300 | P2 | 🟡 Средняя |
| Diagnostic collection | ~600 | P3 | 🔴 Высокая |
| Failure analysis | ~400 | P3 | 🔴 Высокая |
| Recovery strategies | ~500 | P4 | 🔴 Высокая |
| Surrender + UI | ~300 | P5 | 🟡 Средняя |
| IPC изменения (VM, VFS, sched) | ~200 | P4 | 🟡 Средняя |
| **Итого** | **~2,700 LOC** | | |

---

## 7. Открытые вопросы

1. **Диагностика после падения** — как собрать stack trace мёртвого процесса? Сейчас `sys_diagctl_stacktrace()` работает только для живых процессов. Нужен `SIGSEGV` → kernel сохраняет стек перед убийством.

2. **Core dump** — MINIX 3 не имеет традиционного core dump механизма. Нужен новый сервис или расширение RS для дампа памяти сервиса при падении.

3. **Взаимодействие с VFS** — если VFS мёртв, как RS может скинуть отчёт на диск? Нужен fallback: писать в кольцевой буфер в памяти, который сохраняется при следующей загрузке.

4. **Graceful degradation** — что значит "минимальный режим" для сервиса? Нужно определить для каждого сервиса fallback-режим, который потребляет меньше ресурсов.

5. **Тестирование** — как тестировать RS? Нужны скрипты, которые убивают сервисы разными способами (SIGSEGV, SIGKILL, OOM, зависание) и проверяют корректность восстановления.

---

## 8. Критерии готовности

### Level 2 ✅
- [ ] Healthcheck'и регистрируются и выполняются
- [ ] Сервис, не прошедший healthcheck, перезапускается
- [ ] Результаты healthcheck'ов логируются

### Level 3 ✅
- [ ] Таблица зависимостей определена
- [ ] Каскадный рестарт работает (B→A, а не A→B→A)
- [ ] Критические vs некритические зависимости обрабатываются по-разному

### Level 4 ✅
- [ ] diagnostic packet собирается для каждого падения
- [ ] Причина падения анализируется (хотя бы 4 типа)
- [ ] Stack trace сохраняется (если доступен)
- [ ] Core dump записывается

### Level 5 ✅
- [ ] Recovery plan выбирается на основе причины
- [ ] VM_FREE_MEM работает (освобождение кеша)
- [ ] VFS_CLEAR_CACHE работает
- [ ] Не более N попыток до surrender

### Level 6 ✅
- [ ] Полный цикл: падение → диагностика → анализ → рестарт ✕ N → surrender
- [ ] Человеческий отчёт с рекомендациями
- [ ] "Заботливая мамочка" чувствуется пользователем

---

## 9. Связанные документы

- `minix/servers/rs/` — существующий RS код
- `minix/servers/pm/` — process manager (fork, exec, signal delivery)
- `minix/servers/vm/` — memory management (free memory, low memory detection)
- `minix/servers/vfs/` — file system (cache clearing)
