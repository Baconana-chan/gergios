# Reincarnation Server — "Anis"

> **Статус**: 🟡 В разработке (July 2026)
> **Level 1**: ✅ "Just restart it" — существующий RS
> **Level 2**: ✅ "Health monitoring" — реализован
> **Level 3**: ✅ "Dependency-aware recovery" — реализован
> **Level 4**: ✅ "Diagnostics & Analysis" — реализован
> **Level 5**: ✅ "Proactive recovery" — реализован
> **Level 6 P1**: ✅ "Anis — Console surrender" — реализован
> **Level 6 P2**: 🟡 "Anis — GUI surrender" — запланирован
> **Связанные**: `minix/servers/rs/` (~4000 LOC)

---

## 1. Executive Summary

### 1.1 Что такое RS сейчас

Reincarnation Server (RS) — это сердце отказоустойчивости MINIX 3. Он:
- Запускает системные сервисы при загрузке
- Мониторит их через heartbeat'ы
- Перезапускает упавшие сервисы (reincarnation)
- Поддерживает live update (обновление без остановки)

**Текущий уровень**: "Health monitoring" ✅ — 6 файлов (health.c/health.h, +4 модифицированных), ~410 LOC добавлено
 
### 1.2 Что мы хотим

Превратить RS в "Anis", заботливую систему восстановления, которая:
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
  ├── request.c      — обработка запросов (RS_UP, RS_DOWN, RS_RESTART, ...),
  │                    do_period (сердцебиение + healthcheck loop)
  ├── manager.c      — управление сервисами (start, stop, restart, crash),
  │                    RS_HEALTHCHECK_FAIL обработка, free_slot/clone_slot
  ├── exec.c         — execve сервисов
  ├── update.c       — live update
  ├── utility.c      — утилиты (init_service, asynsend, reply)
  ├── error.c        — строки ошибок
  ├── table.c        — boot image tables (какие сервисы запускать)
  ├── const.h        — константы (RS_IN_USE, RS_EXITING, RS_REINCARNATE,
  │                    RS_HEALTHCHECK_FAIL...)
  ├── type.h         — struct rproc (+r_healthchecks), struct rprocupd
  ├── proto.h        — прототипы
  ├── health.c       — healthcheck framework (Level 2 NEW!)
  └── health.h       — healthcheck structures (Level 2 NEW!)
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
  struct rs_healthcheck *r_healthchecks; // массив healthcheck'ов (Level 2)
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

### Уровень 2: "Health monitoring" ✅ РЕАЛИЗОВАН

**Цель**: RS не просто ждёт death notification, а активно проверяет здоровье сервисов.

**Реализовано**: July 2026, ~410 LOC добавлено (health.h + health.c + модификации в 6 файлах)

#### Структуры (health.h)

```c
// Healthcheck — кастомная проверка
struct rs_healthcheck {
    int hc_type;                    // enum healthcheck_type
    endpoint_t hc_endpoint;         // какой сервис проверяем
    clock_t hc_interval;            // как часто проверять (в тиках)
    clock_t hc_timeout;             // максимальное время ответа
    clock_t hc_last_check;          // timestamp последней проверки
    clock_t hc_last_response;       // timestamp последнего ответа
    int hc_consecutive_failures;    // сколько раз подряд упал
    char hc_name[RS_HC_NAME_LEN];   // имя проверки ("ping", "fs_ready", ...)
};

// Типы healthcheck'ов
enum healthcheck_type {
    HC_PING = 1,            // сервис отвечает на IPC?
    HC_HEARTBEAT,           // heartbeat своевременен?
    HC_RESOURCES,           // нет утечки памяти/дескрипторов?
    HC_RESPONSE_TIME,       // время ответа на запрос?
    HC_CUSTOM,              // зарегистрированная сервисом проверка
};

// IPC-структура для регистрации (safecopy через m_rs_req)
struct rs_hc_req {
    endpoint_t rsh_ep;
    int rsh_type;
    clock_t rsh_interval;
    clock_t rsh_timeout;
    char rsh_name[RS_HC_NAME_LEN];
};
```

#### Флаг `RS_HEALTHCHECK_FAIL`

Новый флаг `0x20000` в `r_flags`, который:
- **Устанавливается** в `handle_healthcheck_failure()` перед вызовом `crash_service()`
- **Проверяется** в `do_period()` — heartbeat loop пропускает сервисы с этим флагом
- **Обрабатывается** в `terminate_service()` — отдельная ветка для healthcheck-индуцированных падений
- **Очищается** в `clone_slot()` — новый инстанс не наследует флаг

#### Новая логика в `do_period()`

```c
do_period(m_ptr) {
    clock_t now = m_ptr->m_notify.timestamp;

    // 1. Update period (existing)
    if(RUPDATE_IS_UPDATING() && !RUPDATE_IS_INITIALIZING())
        update_period(m_ptr);

    // === NEW: Healthcheck loop ===
    for (rp = BEG_RPROC_ADDR; rp < END_RPROC_ADDR; rp++) {
        if ((rp->r_flags & RS_ACTIVE) && rp->r_healthchecks) {
            check_service_health(rp, now);
        }
    }

    // 2. Heartbeat loop (existing, skips RS_HEALTHCHECK_FAIL)
    for (rp = BEG_RPROC_ADDR; rp < END_RPROC_ADDR; rp++) {
        if (rp->r_flags & RS_HEALTHCHECK_FAIL) continue;  // NEW guard
        // ... existing heartbeat/ping logic
    }
}
```

#### Healthcheck failure handling

```c
handle_healthcheck_failure(rp, hc, result) {
    printf("RS: %s FAILED healthcheck '%s' (...), %d consecutive\n", ...);

    if (hc->hc_consecutive_failures >= 3) {
        rp->r_flags |= RS_HEALTHCHECK_FAIL;

        if (rp->r_flags & RS_TERMINATED || rp->r_flags & RS_EXITING) {
            // Service already dead — restart directly
            rp->r_flags &= ~RS_HEALTHCHECK_FAIL;
            restart_service(rp);
        } else {
            // Service alive but sick — force crash
            crash_service(rp);  // → SIGKILL → terminate_service()
        }
    }
}
```

#### IPC сообщения

| Сообщение | Номер | Описание |
|-----------|-------|----------|
| `RS_REGISTER_HEALTHCHECK` | `RS_RQ_BASE + 25` (0x725) | Зарегистрировать healthcheck |
| `RS_UNREGISTER_HEALTHCHECK` | `RS_RQ_BASE + 26` (0x726) | Отменить healthcheck |

Оба используют safecopy `struct rs_hc_req` через `m_rs_req.addr`/`m_rs_req.len` (паттерн RS_UP).

#### Изменения в коде (фактические)

| Файл | Изменения |
|------|-----------|
| `health.h` (NEW) | `struct rs_healthcheck`, `enum healthcheck_type`, `enum healthcheck_result`, `struct rs_hc_req`, прототипы |
| `health.c` (NEW) | `do_register_healthcheck()`, `do_unregister_healthcheck()`, `check_service_health()`, `handle_healthcheck_failure()`, helpers |
| `type.h` | +`struct rs_healthcheck *r_healthchecks` в `struct rproc` |
| `const.h` | +`RS_HEALTHCHECK_FAIL 0x20000` |
| `proto.h` | +5 прототипов для health.c |
| `com.h` | +`RS_REGISTER_HEALTHCHECK` (0x725), `RS_UNREGISTER_HEALTHCHECK` (0x726) |
| `request.c` | healthcheck loop в `do_period()` + `RS_HEALTHCHECK_FAIL` guard в heartbeat |
| `main.c` | 2 новых case в dispatch cycle |
| `inc.h` | `#include "health.h"` |
| `Makefile` | `health.c` в `SRCS` |
| `manager.c` | `free_slot()` (no free — shared pointer), `clone_slot()` (clear flag), `terminate_service()` (healthcheck branch) |

---

### Уровень 3: "Dependency-aware recovery" ✅ РЕАЛИЗОВАН

**Цель**: RS знает, какие сервисы от каких зависят, и восстанавливает в правильном порядке.

**Реализовано**: July 2026, ~350 LOC добавлено (deps.h + deps.c + модификации в 8 файлах)

#### Структуры (deps.h)

```c
// Зависимость между сервисами
struct rs_dep {
    endpoint_t service;           // кто зависит
    endpoint_t depends_on;        // от кого зависит
    int critical;                 // TRUE = не может работать без зависимости
    int restart_order;            // порядок при рестарте (1 = сначала этот)
    char reason[64];              // почему зависит ("provides block I/O")
};

// Статус зависимости (для диагностики Level 4)
struct rs_dep_status {
    endpoint_t service;
    endpoint_t depends_on;
    int is_alive;                 // зависимость жива?
    int is_healthy;               // зависимости здорова?
    clock_t last_alive;           // когда видела зависимость живой
};

// IPC-структура для регистрации (safecopy через m_rs_req)
struct rs_dep_req {
    endpoint_t rsr_service;       // кто зависит (NONE = отправитель)
    endpoint_t rsr_depends_on;    // от кого зависит
    int rsr_critical;             // критическая?
    char rsr_reason[64];          // причина
};
```

#### Built-in таблица зависимостей (deps.c, загружается при boot)

```c
static const struct rs_dep_entry builtin_deps[] = {
    { VFS_PROC_NR,  MEM_PROC_NR,  .critical = TRUE,  .reason = "block I/O via RAM disk" },
    { MFS_PROC_NR,  MEM_PROC_NR,  .critical = TRUE,  .reason = "block I/O via RAM disk" },
    { ALL_ENDPOINTS, PM_PROC_NR,  .critical = TRUE,  .reason = "process management" },
    { ALL_ENDPOINTS, VM_PROC_NR,  .critical = TRUE,  .reason = "memory management" },
    { ALL_ENDPOINTS, RS_PROC_NR,  .critical = TRUE,  .reason = "service management" },
    { VFS_PROC_NR,  MFS_PROC_NR,  .critical = FALSE, .reason = "root filesystem" },
};
```

Загрузка: `deps_init_table()` обходит все активные слоты и для каждого сервиса, подходящего под `service` (поддержка `ANY` wildcard), добавляет `rs_dep` в его `r_deps`.

#### Каскадный рестарт

```c
void cascade_restart(struct rproc *rp) {
    int num_dead_deps;
    rp->r_flags |= RS_DEP_FAIL;

    num_dead_deps = check_dependencies(rp);

    if (num_dead_deps > 0) {
        // Phase 1: restart dead critical deps
        for (int i = 0; i < rp->r_num_deps; i++) {
            struct rs_dep *dep = &rp->r_deps[i];
            if (!dep->critical) continue;
            struct rproc *dep_rp = rproc_ptr[_ENDPOINT_P(dep->depends_on)];
            if (dep_rp->r_flags & RS_ACTIVE && !(dep_rp->r_flags & RS_TERMINATED)) continue;
            restart_service(dep_rp);  // reboot dep first
        }
    }

    // Phase 2: restart the service itself
    if (rp->r_flags & RS_TERMINATED || rp->r_flags & RS_EXITING) {
        rp->r_flags &= ~RS_DEP_FAIL;
        restart_service(rp);
    } else {
        crash_service(rp);  // → SIGKILL → terminate_service(rp)
    }
}
```

#### Интеграция в terminate_service()

```c
terminate_service(rp) {
    // ... существующие проверки RS_INITIALIZING, RS_EXITING, RS_REFRESHING ...

    else if (rp->r_flags & RS_HEALTHCHECK_FAIL) {
        // Проверяем зависимости перед рестартом
        if (check_dependencies(rp) > 0) {
            cascade_restart(rp);
            return;
        }
        // Нет мёртвых зависимостей → обычный рестарт с backoff
        rp->r_flags &= ~(RS_HEALTHCHECK_FAIL | RS_DEP_FAIL);
        if (rp->r_restarts > 0) {
            rp->r_backoff = MIN(rp->r_backoff * 2, 30);
            return;
        }
        restart_service(rp);
    }
}
```

#### IPC сообщения

| Сообщение | Номер | Описание |
|-----------|-------|----------|
| `RS_REGISTER_DEP` | `RS_RQ_BASE + 27` (0x727) | Зарегистрировать dependency |
| `RS_UNREGISTER_DEP` | `RS_RQ_BASE + 28` (0x728) | Отменить dependency |

Оба используют safecopy `struct rs_dep_req` через `m_rs_req.addr`/`m_rs_req.len`.

#### Изменения в коде (фактические)

| Файл | Изменения |
|------|-----------|
| `deps.h` (NEW) | `struct rs_dep`, `struct rs_dep_status`, `struct rs_dep_req`, прототипы |
| `deps.c` (NEW) | Built-in dep table, `deps_init_table()`, `cascade_restart()`, `check_dependencies()`, `do_register_dep()`, `do_unregister_dep()`, helpers |
| `type.h` | +`struct rs_dep *r_deps`, `int r_num_deps` в `struct rproc` |
| `const.h` | +`RS_DEP_FAIL 0x40000` |
| `proto.h` | +8 прототипов для deps.c |
| `com.h` | +`RS_REGISTER_DEP` (0x727), `RS_UNREGISTER_DEP` (0x728) |
| `main.c` | `deps_init_table()` в boot cycle, 2 новых IPC case |
| `request.c` | `RS_DEP_FAIL` cleared в `do_init_ready` |
| `manager.c` | `cascade_restart()` в `terminate_service` healthcheck branch |
| `inc.h` | `#include "deps.h"` |
| `Makefile` | `deps.c` в `SRCS` |

#### Исправленные баги

1. **`RS_DEP_FAIL` flag leak** — флаг устанавливался в `cascade_restart()` но никогда не очищался при успешном рестарте. Исправлено: очищается в `do_init_ready()`.
2. **Dead code в `do_register_dep()`** — первый `rp` lookup (caller) нигде не использовался, сразу перезаписывался. Удалён.
3. **`MEM_PROC_NR` как placeholder** — MINIX block drivers (AHCI, virtio_blk, at_wini) запускаются динамически и не имеют фикс. endpoint'ов. Добавлен комментарий.

---

### Уровень 4: "Diagnostics & analysis" ✅ РЕАЛИЗОВАН

**Цель**: RS собирает полную диагностику перед каждым рестартом и анализирует причину падения.

**Реализовано**: July 2026, ~580 LOC добавлено (diag.h + diag.c + analyze.c + модификации в 7 файлах)

#### Структуры (diag.h)

```c
enum fail_reason {
    FAIL_UNKNOWN,               // неизвестно
    FAIL_SEGFAULT,              // SIGSEGV — баг
    FAIL_NOMEM,                 // не хватило памяти
    FAIL_TIMEOUT,               // завис (no heartbeat)
    FAIL_DEADLOCK,              // взаимная блокировка
    FAIL_HW_ERROR,              // ошибка железа (SIGBUS и т.д.)
    FAIL_DEP_DIED,              // зависимость умерла
    FAIL_RESOURCE_EXHAUSTION,   // fd, IPC очереди
    FAIL_SOFTWARE_BUG,          // assert, abort
    FAIL_KILLED,                // явно убит RS
    FAIL_INIT_FAILURE,          // ошибка инициализации
};

// Diagnostic packet — собранный при падении/healthcheck-failure
struct rs_diag_packet {
    endpoint_t d_ep;                         // кто упал
    char       d_label[RS_MAX_LABEL_LEN];    // label
    clock_t    d_crash_time;                 // когда (ticks)
    int        d_signal;                     // SIGSEGV/SIGKILL/etc
    int        d_exit_status;                // код возврата
    struct rs_diag_service_resources d_svc_res;  // ресурсы сервиса
    struct rs_diag_system_resources  d_sys_res;  // ресурсы системы
    enum fail_reason d_reason;               // классифицированная причина
    char d_recommendation[256];              // человеческая рекомендация
};

// Ring buffer entry (64 entries, ~2560 bytes всего)
struct rs_diag_log_entry {
    clock_t    dle_timestamp;
    endpoint_t dle_endpoint;
    int        dle_signal;
    enum fail_reason dle_reason;
    int        dle_restarts;
    int        dle_used;
};
```

#### Анализ причины (analyze.c)

```c
enum fail_reason analyze_failure(const struct rs_diag_packet *dp) {
    // Init failure (короткий uptime, нет рестартов)
    if (uptime < 5s && restarts == 0) return FAIL_INIT_FAILURE;

    // Memory corruption signals → software bug
    switch (dp->d_signal) {
    case SIGSEGV: case SIGBUS: case SIGILL:
    case SIGFPE:  case SIGTRAP: return FAIL_SEGFAULT;
    case SIGABRT: return FAIL_SOFTWARE_BUG;
    case SIGKILL:  // Killed by RS
        if (free_mem > 0 && free_mem < 4MB) return FAIL_NOMEM;
        if (uptime < 5s) return FAIL_INIT_FAILURE;
        return FAIL_TIMEOUT;
    case SIGTERM:  return FAIL_KILLED;
    case SIGXCPU: case SIGXFSZ: return FAIL_RESOURCE_EXHAUSTION;
    }

    // No signal, non-zero exit → check resources
    if (dp->d_signal == 0 && dp->d_exit_status != 0) {
        if (free_mem < 4MB) return FAIL_NOMEM;
        if (restarts >= 5 && uptime < 5s) return FAIL_RESOURCE_EXHAUSTION;
        return FAIL_SOFTWARE_BUG;
    }

    return FAIL_UNKNOWN;
}
```

#### Интеграция в terminate_service()

```c
terminate_service(rp) {
    struct rs_diag_packet diag_packet;

    // Сначала собираем диагностику (до принятия решения о рестарте)
    if (collect_diagnostics(rp, &diag_packet) == OK) {
        save_diag_report(&diag_packet);          // → ring buffer + console
        save_diag_report_to_disk(&diag_packet);  // → /var/log/rs/crash/
    }

    // ... существующая логика terminate_service ...
}
```

#### IPC сообщения

| Сообщение | Номер | Описание |
|-----------|-------|----------|
| `RS_DIAG_REPORT` | `RS_RQ_BASE + 29` (0x72D) | Получить diagnostic log (safecopy) |
| `RS_DIAG_CLEAR` | `RS_RQ_BASE + 30` (0x72E) | Очистить diagnostic log |

#### Изменения в коде (фактические)

| Файл | Изменения |
|------|-----------|
| `diag.h` (NEW) | `struct rs_diag_packet`, `enum fail_reason` (11 типов), `struct rs_diag_log_entry`, `DIAG_COUNT`, прототипы |
| `diag.c` (NEW) | `collect_diagnostics()`, `save_diag_report()`, `save_diag_report_to_disk()`, `do_diag_report()`, `clear_diag_log()`, ring buffer |
| `analyze.c` (NEW) | `analyze_failure()`, `fail_reason_to_string()`, `signal_num_to_string()` |
| `const.h` | `RS_DIAG_COLLECTED` (закомментирован, задел на будущее) |
| `proto.h` | +9 прототипов для diag.c и analyze.c |
| `com.h` | +`RS_DIAG_REPORT` (0x72D), `RS_DIAG_CLEAR` (0x72E) |
| `main.c` | `diag_init()` в boot cycle, 2 новых IPC case |
| `manager.c` | `collect_diagnostics()` в начале `terminate_service()` |
| `inc.h` | `#include "diag.h"` |
| `Makefile` | `diag.c analyze.c` в `SRCS` |

---

### Уровень 5: "Proactive recovery" ✅ РЕАЛИЗОВАН

**Цель**: RS использует диагностику Level 4 для выбора стратегии восстановления и пробует progressively более invasive mitigation'ы перед surrender.

**Реализовано**: July 2026, ~450 LOC добавлено (strategy.h + strategy.c + модификации в 5 файлах)

#### Дизайн

Каждый crash cycle пробует ОДНУ стратегию (следующую непробованную). Если сервис снова падает, пробуется следующая стратегия в плане. Это позволяет системе тестировать progressively более invasive подходы без busy-looping на одном и том же.

#### Структуры (strategy.h)

```c
enum recovery_strategy {
    STRAT_RESTART = 1,              // простой перезапуск
    STRAT_RESTART_DEPS,             // + перезапуск зависимостей
    STRAT_RESTART_CLEAN,            // + сброс backoff/restarts
    STRAT_RESTART_ISOLATE,          // + новый endpoint (RS_REINCARNATE)
    STRAT_RESTART_MINIMAL,          // + минимальный режим
    STRAT_FREE_MEMORY,              // hint VM на освобождение памяти
    STRAT_CLEAR_CACHE,              // hint VM на очистку кеша
    STRAT_USER_ALERT,               // вывод surrender notice в консоль
    STRAT_SURRENDER,                // RS_EXITING — остановка рестартов
};

struct recovery_plan {
    enum recovery_strategy strategies[RS_MAX_STRATEGIES];  // до 12
    int  num_strategies;
    int  max_attempts;
};

// Per-service tracking (хранится в struct rproc)
struct rs_recovery_data {
    int rrd_attempts;                // всего попыток
    int rrd_current_strategy;        // индекс следующей стратегии
    int rrd_surrendered;             // 1 = surrender
};
```

#### Recovery plans (по fail_reason)

| fail_reason | Стратегии | Max attempts |
|-------------|-----------|--------------|
| `FAIL_SEGFAULT` | ISOLATE → DEPS → CLEAN → ALERT → SURRENDER | 5 |
| `FAIL_NOMEM` | FREE_MEM → CLEAR_CACHE → CLEAN → MINIMAL → ALERT → SURRENDER | 10 |
| `FAIL_TIMEOUT` | RESTART → DEPS → CLEAN → ALERT → SURRENDER | 5 |
| `FAIL_HW_ERROR` | RESTART → MINIMAL → ALERT → SURRENDER | 3 |
| `FAIL_DEP_DIED` | DEPS → RESTART → ALERT → SURRENDER | 5 |
| `FAIL_KILLED` | RESTART → CLEAN → ALERT → SURRENDER | 3 |
| `FAIL_INIT_FAILURE` | CLEAN → MINIMAL → ALERT → SURRENDER | 5 |
| `FAIL_UNKNOWN` | RESTART → DEPS → CLEAN → ALERT → SURRENDER | 5 |

#### Интеграция в terminate_service()

```c
terminate_service(rp) {
    collect_diagnostics(rp, &diag_packet);           // Level 4

    if (!(rp->r_flags & RS_INITIALIZING) &&
        !(rp->r_flags & RS_EXITING)) {
        reason = analyze_failure(&diag_packet);
        recovery_rv = execute_recovery_plan(rp, reason, &diag_packet);

        if (recovery_rv == RS_RECOVERY_SURRENDER) {
            rp->r_flags |= RS_EXITING;  // stop all restarts
        }
    }

    // Fall through to existing logic (handles RS_EXITING properly)
    if (rp->r_flags & RS_EXITING) { ... cleanup ... }
    else if (rp->r_flags & RS_HEALTHCHECK_FAIL) { ... cascade_restart ... }
    else { ... backoff + restart ... }
}

end_srv_init(rp) {
    // ... cleanup old replica ...
    recovery_reset(rp);  // clear recovery tracking on success
}
```

#### Return codes

| Код | Значение |
|-----|----------|
| `RS_RECOVERY_OK` | Стратегия выполнена, continue с normal restart |
| `RS_RECOVERY_SURRENDER` | Все стратегии исчерпаны, не рестартить |
| `RS_RECOVERY_ERROR` | Внутренняя ошибка |

#### Реализованные в Phase 5 (полный список)

| Стратегия | Реализация |
|-----------|------------|
| `STRAT_RESTART` | no-op (существующая логика) |
| `STRAT_RESTART_DEPS` | `cascade_restart()` из Level 3 |
| `STRAT_RESTART_CLEAN` | сброс `r_backoff` и `r_restarts` в 0 |
| `STRAT_RESTART_ISOLATE` | установка `RS_REINCARNATE` флага |
| `STRAT_RESTART_MINIMAL` | снижение `r_priority` (+5, cap 19) и уменьшение `r_quantum` (/2, min 1) |
| `STRAT_FREE_MEMORY` | `vm_info_stats()` → query free/cached → `vm_clear_cache(dev_nr)` для всех блок-драйверов → requery |
| `STRAT_CLEAR_CACHE` | `vm_clear_cache(dev_nr)` для всех активных блок-драйверов |
| `STRAT_USER_ALERT` | surrender notice с fail_reason, signal, uptime, attempts |
| `STRAT_SURRENDER` | `RS_EXITING` + audit log (`AUDIT_PRIV_CHANGE`) |

#### Исправленные по code review

1. **`//` comments → `/* */`** — strategy.c изначально использовал C++ стиль комментариев, несовместимый с K&R/C89. Заменены на C89-совместимые.
2. **Redundant `check_dependencies()` в `attempt_restart_deps()`** — внешняя проверка дублировала вызов внутри `cascade_restart()`. Удалена.
3. **`AUDIT_IPC_DENIED` → `AUDIT_PRIV_CHANGE`** — surrender использует семантически корректный audit event type.

---

### Level 6: "Anis — Console Surrender" ★ Phase 1 ✅ ~350 LOC

**Phase 1 (Console/Terminal)**: RS выводит красивый surrender отчёт с полной диагностикой в консоль перед тем как поднять белый флаг.

**Phase 2 (GUI)**: Графическая панель с историей падений, интерактивным просмотром диагностики и рекомендациями.

**Цель Phase 1**: Красивый, человекочитаемый отчёт в терминале, который даёт пользователю всю необходимую информацию для диагностики.

#### Архитектура Phase 1

```
surrender.h           ← struct rs_attempt_entry, struct rs_surrender_config, prototypes
surrender.c           ← attempt logging, CP437 box rendering, full diagnostic output

strategy.h            ← rs_recovery_data расширен attempt_log[]
strategy.c            ← integrates surrender framework: log each attempt, render on surrender
```

#### Структуры (surrender.h)

```c
/* Per-attempt log entry — хранит историю попыток восстановления. */
struct rs_attempt_entry {
    enum recovery_strategy strategy;        /* что пробовали */
    int result;                              /* OK = success, !OK = fail, EAGAIN = skip */
    char desc[RS_SURRENDER_DESC_LEN];        /* "Restart with isolation", etc */
};

/* Расширенное per-service recovery tracking. */
struct rs_recovery_data {
    int rrd_attempts;                        /* всего попыток */
    int rrd_current_strategy;                /* индекс следующей стратегии */
    int rrd_surrendered;                     /* 1 = surrender */
    struct rs_attempt_entry rrd_attempt_log[RS_MAX_ATTEMPT_LOG];  /* история */
    int rrd_attempt_count;                   /* количество записей в логе */
};
```

#### Console surrender box (CP437 + ANSI)

```
\x1B[1;31m  ← красный жирный для заголовка
\x1B[1;33m  ← жёлтый жирный для предупреждений
\x1B[1;34m  ← синий жирный для информации
\x1B[0m     ← сброс

┌─ Anis  ───────────────────────────────────────────────────┐
│                                                           │
│  ── Anis: "Я сделала всё, что могла..."                 ──│
│                                                           │
│  Service:   VFS (Virtual File System)                     │
│  PID:       1423                                          │
│  Uptime:    2h 34m                                        │
│                                                           │
│  Cause:     SIGSEGV (signal 11)                           │
│  Reason:    FAIL_SEGFAULT — software bug                  │
│                                                           │
│  Attempts:  5 (2 crash cycles)                            │
│    1. Restart with isolation           → FAIL             │
│    2. Restart with deps               → FAIL             │
│    3. Free memory                     → FAIL             │
│    4. Clear cache                     → SKIP             │
│    5. User alert                      → OK               │
│                                                           │
│  System state:                                            │
│    Memory: 12.4 GB free / 24 GB total                     │
│    All other services: healthy                            │
│                                                           │
│  Log: /var/log/rs/crash/vfs.20260701-143502.log           │
│                                                           │
│  "I've tried everything I could, but VFS keeps            │
│   crashing. Please check the log for details."            │
│                                                           │
│  Anis will wait for your instructions.                    │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

#### Surrender format (console output)

The box uses:
- **ANSI escape codes**: `\x1B[1;31m` (red), `\x1B[1;33m` (yellow), `\x1B[1;34m` (blue), `\x1B[0m` (reset)
- **CP437 box drawing**: `\xDA` (┌), `\xC4` (─), `\xBF` (┐), `\xB3` (│), `\xC0` (└), `\xD9` (┘)
- **Fallback ASCII**: When ANSI is not supported, uses plain `+`, `-`, `|` characters

#### Интеграция в recovery flow

```c
// В execute_recovery_plan(), после каждой попытки:
surrender_log_attempt(rp, strategy, result);

// При surrender, вместо старого attempt_surrender():
surrender_render(rp, reason, dp);
rp->r_flags |= RS_EXITING;
```

#### Files

| Файл | Изменения |
|------|-----------|
| `surrender.h` (NEW) | `struct rs_attempt_entry`, `enum rs_surrender_output`, prototypes |
| `surrender.c` (NEW) | `surrender_log_attempt()`, `surrender_render()`, `surrender_format_time()`, helpers |
| `strategy.h` | +attempt_log[] в `rs_recovery_data`, +`RS_MAX_ATTEMPT_LOG 20` |
| `strategy.c` | интегрировать surrender: логировать каждую попытку, вызывать render при surrender |
| `inc.h` | `#include "surrender.h"` |
| `Makefile` | `surrender.c` в `SRCS` |
| `proto.h` | +прототипы surrender.c |

#### Критерии готовности Phase 1

- [ ] Attempt log заполняется при каждой попытке восстановления
- [ ] Surrender box выводится при исчерпании всех стратегий
- [ ] Box содержит: service info, cause, attempt history, system state, recommendations
- [ ] CP437 box drawing работает на VGA console
- [ ] ANSI escape codes работают на serial/QEMU console
- [ ] ASCII fallback работает на всех консолях (выводится, если \x1B не поддерживается)

---

## 4. Изменения в IPC и протоколах

### Новые системные вызовы (IPC → RS)

```c
// Регистрация healthcheck'а
#define RS_REGISTER_HEALTHCHECK   (RS_RQ_BASE + 25)  // 0x725 (фактически)

// Регистрация/отмена зависимости
#define RS_REGISTER_DEP           (RS_RQ_BASE + 27)  // 0x727 (фактически)
#define RS_UNREGISTER_DEP         (RS_RQ_BASE + 28)  // 0x728 (фактически)

// Получение diagnostic log (Level 4)
#define RS_DIAG_REPORT            (RS_RQ_BASE + 29)  // 0x72D (фактически)
#define RS_DIAG_CLEAR             (RS_RQ_BASE + 30)  // 0x72E (фактически)

// Запрос на освобождение ресурсов (Level 5)
#define RS_FREE_RESOURCES         (RS_RQ_BASE + 22)
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

### Phase 1: Level 2 — Healthchecks ✅ РЕАЛИЗОВАН

**Новые файлы**:
```
minix/servers/rs/
  ├── health.c          ← healthcheck framework
  └── health.h          ← healthcheck structures
```

**Изменения**:
- `type.h`: +`struct rs_healthcheck *r_healthchecks` в `struct rproc`
- `const.h`: +`RS_HEALTHCHECK_FAIL 0x20000`
- `manager.c`: `free_slot()` (no free — shared ptr), `clone_slot()` (clear flag), `terminate_service()` (healthcheck branch)
- `request.c`: healthcheck loop в `do_period()` + `RS_HEALTHCHECK_FAIL` guard
- `main.c`: 2 новых case (REGISTER/UNREGISTER_HEALTHCHECK)
- `com.h`: +2 IPC сообщения (0x725, 0x726)
- `proto.h`: +5 прототипов
- `inc.h`: `#include "health.h"`
- `Makefile`: `health.c` в `SRCS`

**Итого**: ~410 LOC добавлено, 10 файлов модифицировано

### Phase 2: Level 3 — Dependency graph ✅ РЕАЛИЗОВАН

**Новые файлы**:
```
minix/servers/rs/
  ├── deps.c           ← dependency management (~350 LOC)
  └── deps.h           ← dependency structures
```

**Изменения**:
- `type.h`: +`struct rs_dep *r_deps`, `int r_num_deps` в `struct rproc`
- `const.h`: +`RS_DEP_FAIL 0x40000`
- `proto.h`: +8 прототипов
- `com.h`: +`RS_REGISTER_DEP` (0x727), `RS_UNREGISTER_DEP` (0x728)
- `main.c`: `deps_init_table()` в boot cycle, 2 новых IPC case
- `request.c`: `RS_DEP_FAIL` cleared в `do_init_ready`
- `manager.c`: `cascade_restart()` в `terminate_service` healthcheck branch
- `inc.h`: `#include "deps.h"`
- `Makefile`: `deps.c` в `SRCS`

**Исправленные баги**:
- `RS_DEP_FAIL` flag leak (не очищался)
- Dead code в `do_register_dep()`
- `MEM_PROC_NR` как placeholder для динамических block drivers

### Phase 3: Level 4 — Diagnostics ✅ РЕАЛИЗОВАН

**Новые файлы**:
```
minix/servers/rs/
  ├── diag.c           ← diagnostic collection (~300 LOC)
  ├── diag.h           ← diagnostic structures
  └── analyze.c        ← failure analysis (~200 LOC)
```

**Изменения**:
- `const.h`: `RS_DIAG_COLLECTED` (закомментирован)
- `proto.h`: +9 прототипов
- `com.h`: +`RS_DIAG_REPORT` (0x72D), `RS_DIAG_CLEAR` (0x72E)
- `main.c`: `diag_init()` в boot cycle, 2 новых IPC case
- `manager.c`: `collect_diagnostics()` + `save_diag_report()` в начале `terminate_service()`
- `inc.h`: `#include "diag.h"`
- `Makefile`: `diag.c analyze.c` в `SRCS`

**Исправленные баги**:
- `RS_DIAG_COLLECTED` флаг был мёртвым кодом (никогда не устанавливался) — убран
- Лишние `#include` в `diag.c` (`fcntl.h`, `unistd.h`, `errno.h` — уже в inc.h)
- `AUDIT_COUNT` → `DIAG_COUNT` (семантически чище)
- Комментарий `RTS_SLOT_FREE` ссылался на kernel-internal константу — исправлен

### Phase 4: Level 5 — Proactive recovery ✅ РЕАЛИЗОВАН

**Новые файлы**:
```
minix/servers/rs/
  ├── strategy.c       ← recovery strategies (~350 LOC)
  └── strategy.h       ← strategy definitions (~100 LOC)
```

**Изменения**:
- `type.h`: +`struct rs_recovery_data r_recovery` в `struct rproc`
- `proto.h`: +3 прототипа (recovery_get_plan, execute_recovery_plan, recovery_reset)
- `inc.h`: `#include "strategy.h"`
- `Makefile`: `strategy.c` в `SRCS`
- `manager.c`: `execute_recovery_plan()` в `terminate_service()` (после diagnostics, перед flag checks) + `recovery_reset()` в `end_srv_init()` (после успешного restart)

**Реализовано**:
- `STRAT_FREE_MEMORY` — `vm_clear_cache(dev_nr)` на всех блок-драйверах + `vm_info_stats()` для мониторинга
- `STRAT_CLEAR_CACHE` — `vm_clear_cache(dev_nr)` на всех блок-драйверах
- `STRAT_RESTART_MINIMAL` — снижение `r_priority`/`r_quantum`
- `STRAT_SURRENDER` — rich report + audit log

**Не реализовано (задел на Phase 6)**:
- `SCHED_RS_BOOST` IPC — отложено (приоритет/квант уже меняются через r_priority/r_quantum)
- Core dump / stack trace — отложено (требует kernel changes)
- VFS-level cache flush IPC — не требуется (VM cache clearing через `vm_clear_cache()` покрывает эту потребность)

**Исправленные баги**:
- `//` comments → `/* */` (C89/K&R совместимость)
- Redundant `check_dependencies()` вызов в `attempt_restart_deps()`
- `AUDIT_IPC_DENIED` → `AUDIT_PRIV_CHANGE` в surrender audit

### Phase 5: Level 6 P1 — "Anis: Console Surrender" ✅ РЕАЛИЗОВАН

**Новые файлы**:
```
minix/servers/rs/
  ├── surrender.c      ← surrender framework (~350 LOC)
  └── surrender.h      ← surrender structures
```

**Изменения**:
- `strategy.h`: +attempt_log[] в `rs_recovery_data`
- `strategy.c`: логирование попыток через `surrender_log_attempt()`, рендер бокса через `surrender_render()`
- `inc.h`: `#include "surrender.h"`
- `Makefile`: `surrender.c` в `SRCS`
- `proto.h`: +прототипы surrender.c

**Итого**: ~350 LOC добавлено, 6 файлов модифицировано

### Phase 6: Level 6 P2 — "Anis: GUI Surrender" 🟡 ЗАПЛАНИРОВАН

**Цель**: Графический интерфейс для просмотра истории падений и диагностики.

- GUI surrender panel
- Интерактивный просмотр diagnostic log
- Визуальные рекомендации

---

## 6. Оценка объёма работ

| Компонент | LOC | Фаза | Сложность |
|-----------|-----|------|-----------|
| Healthcheck framework | ~410 | P1 ✅ | 🟡 Средняя |
| **→ Level 2 итог** | **~410 LOC** | **P1 ✅** | |
| Dependency graph | ~350 | P2 ✅ | 🟡 Средняя |
| **→ Level 3 итог** | **~350 LOC** | **P2 ✅** | |
| Diagnostic collection | ~300 | P3 ✅ | 🔴 Высокая |
| Failure analysis | ~200 | P3 ✅ | 🟡 Средняя |
| **→ Level 4 итог** | **~580 LOC** | **P3 ✅** | |
| Recovery strategies | ~450 | P4 ✅ | 🔴 Высокая |
| **→ Level 5 итог** | **~450 LOC** | **P4 ✅** | |
| Surrender + refined UI (P1) | ~350 | P5 ✅ | 🟡 Средняя |
| GUI surrender (P2) | ~300 | P6 🟡 | 🟡 Средняя |
| **Итого (Level 4-6)** | **~2,300 LOC** | | |

---

## 7. Открытые вопросы

1. **Диагностика после падения** — как собрать stack trace мёртвого процесса? Сейчас `sys_diagctl_stacktrace()` работает только для живых процессов. Нужен `SIGSEGV` → kernel сохраняет стек перед убийством.

2. **Core dump** — MINIX 3 не имеет традиционного core dump механизма. Нужен новый сервис или расширение RS для дампа памяти сервиса при падении.

3. **Взаимодействие с VFS** — если VFS мёртв, как RS может скинуть отчёт на диск? Нужен fallback: писать в кольцевой буфер в памяти, который сохраняется при следующей загрузке.

4. **Graceful degradation** — что значит "минимальный режим" для сервиса? Нужно определить для каждого сервиса fallback-режим, который потребляет меньше ресурсов.

5. **Тестирование** — как тестировать RS? Нужны скрипты, которые убивают сервисы разными способами (SIGSEGV, SIGKILL, OOM, зависание) и проверяют корректность восстановления.

---

## 8. Критерии готовности

### Level 2 ✅ РЕАЛИЗОВАН
- [x] Healthcheck'и регистрируются и выполняются
- [x] Сервис, не прошедший healthcheck (>3 failures), перезапускается
- [x] Результаты healthcheck'ов логируются
- [x] `RS_HEALTHCHECK_FAIL` защищает от двойной обработки (healthcheck + heartbeat)
- [x] `r_healthchecks` корректно переживает restart (shared pointer, не free'ится)
- [x] IPC: `RS_REGISTER_HEALTHCHECK` + `RS_UNREGISTER_HEALTHCHECK`

### Level 3 ✅ РЕАЛИЗОВАН
- [x] Таблица зависимостей определена (built-in deps + runtime via IPC)
- [x] Каскадный рестарт работает (Phase 1: deps → Phase 2: self)
- [x] Критические vs некритические зависимости обрабатываются по-разному
- [x] IPC: `RS_REGISTER_DEP` + `RS_UNREGISTER_DEP` (safecopy pattern)
- [x] `RS_DEP_FAIL` очищается при успешной инициализации (`do_init_ready`)
- [x] Dependency graph загружается при boot через `deps_init_table()`

### Level 4 ✅ РЕАЛИЗОВАН
- [x] Diagnostic packet собирается для каждого падения (в начале `terminate_service()`)
- [x] Причина падения анализируется (11 типов fail_reason)
- [x] Diagnostic report сохраняется в ring buffer (64 entries, `RS_DIAG_REPORT` IPC)
- [x] Diagnostic report пишется на диск (best-effort, `/var/log/rs/crash/`)
- [x] Recommendation генерируется на основе анализа
- [x] IPC: `RS_DIAG_REPORT` + `RS_DIAG_CLEAR`
- [x] Signal-to-name и fail_reason-to-name конвертация
- [x] Ring buffer переживает рестарты RS (in-memory)

### Level 5 ✅ РЕАЛИЗОВАН
- [x] Recovery plan выбирается на основе причины (11 fail_reason → 11 планов)
- [x] Одна стратегия за crash cycle (индекс сохраняется между циклами)
- [x] Surrender устанавливает RS_EXITING (существующая логика не рестартит)
- [x] Recovery tracking сбрасывается после успешного restart (end_srv_init)
- [x] Стратегии: RESTART, DEPS, CLEAN, ISOLATE, MINIMAL, FREE_MEM, CACHE, ALERT, SURRENDER
- [x] C89/K&R совместимость (code review)
- [x] VM_FREE_MEM — реализовано через `vm_clear_cache()` на блок-драйверах
- [x] CACHE_CLEAR — реализовано через `vm_clear_cache()` на блок-драйверах
- [x] Minimal mode — снижение `r_priority`/`r_quantum` при рестарте
- [x] Surrender audit — лог через `SYS_AUDIT` с `AUDIT_PRIV_CHANGE`
- [x] SCHED_RS_BOOST IPC — `SCHEDULING_BOOST (SCHEDULING_BASE + 10)` + `sched_boost()` в libsys + `do_boost()` в scheduler

### Level 6 P1 ✅ Phase 1 — "Anis: Console Surrender"
- [x] Attempt log заполняется при каждой попытке восстановления
- [x] Surrender box выводится при исчерпании всех стратегий
- [x] Box содержит: service info, cause, attempt history, system state, recommendations
- [x] CP437 box drawing + ANSI colors
- [x] ASCII fallback

### Level 6 P2 🟡 Phase 2 — "Anis: GUI Surrender"
- [ ] GUI surrender panel
- [ ] Interactive diagnostic log viewer
- [ ] Visual recommendations

---

## 9. Связанные документы

- `minix/servers/rs/` — существующий RS код
- `minix/servers/pm/` — process manager (fork, exec, signal delivery)
- `minix/servers/vm/` — memory management (free memory, low memory detection)
- `minix/servers/vfs/` — file system (cache clearing)
