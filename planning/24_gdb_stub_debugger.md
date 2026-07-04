# GDB Remote Serial Stub — KGDB для GergiOS

> **Статус**: 📋 Планирование (July 2026)
> **Целевой релиз**: **После стабилизации драйверов** (post-Phase 7, ~Q4 2026)
> **Приоритет**: Низкий — инструмент разработки, не влияет на загрузку/работу системы
> **Связанные**: `sys/sys/kgdb.h` (NetBSD KGDB протокол), `minix/kernel/debug.c` (debug utilities), `minix/kernel/system/do_diagctl.c` (SYS_DIAGCTL), `planning/22_rs_reincarnation_server.md` (диагностика RS), `planning/23_driver_model_modernization.md` §Phase 7 (Driver Manager — GDB stub как сервис)

---

## 1. Executive Summary

### 1.1 Что такое KGDB

KGDB (Kernel GDB) — это **remote serial stub**, встроенный в ядро, который позволяет отлаживать систему через GDB remote serial protocol. GDB общается с KGDB через последовательный порт (COM1/2 или QEMU serial), отправляя команды: read/write memory, read/write registers, continue, single-step, breakpoint set/clear.

### 1.2 Почему GDB stub лучше, чем собственный дебаггер

| Аспект | Свой TUI дебаггер (Rust) | GDB stub (KGDB) |
|--------|--------------------------|------------------|
| **LOC** | ~5,400 (DWARF + TUI + breakpoints) | **~1,200** (stub + serial + HW breaks) |
| **Языковая поддержка** | Придётся писать самому | **GDB уже поддерживает C, Rust, ASM** |
| **Symbol tables** | Свой ELF/DWARF парсер | **GDB читает .elf, .debug, dwarf** |
| **Source-level debug** | Свой file:line resolver | **GDB может показать исходник** |
| **Scripting** | Нет | **GDB Python scripting** |
| **Reverse debugging** | Нет | **GDB record/reverse-step (QEMU)** |
| **Телеметрия** | Только что напишем | **GDB tracepoints, agents, profiling** |
| **Risks** | DWARF парсер — огромный | **Protocol стабилен с 1988 года** |
| **Микроядро** | Надо понимать архитектуру | **NetBSD KGDB stub — адаптация** |

### 1.3 Микроядро — идеальный GDB target

Парадоксально, но микроядро MINIX подходит для GDB **лучше**, чем монолит:

| Аспект | Монолит (Linux) | Микроядро (MINIX/GergiOS) |
|--------|-----------------|---------------------------|
| **Цикл событий** | Нет (весь код плоский) | **Есть** — `sef_receive()` + `sys_call()` |
| **Прерывания** | Могут случиться **в любой момент** | Обрабатываются в процессах |
| **Остановка** | Надо atomic'но фризить **все** ядра | Достаточно приостановить target процесс |
| **KGDB stub** | Должен быть вшит в ядро | Можно сделать отдельным процессом/сервером |
| **GDB и так** | Нет userspace для GDB из коробки | **GDB умеет отлаживать процессы** |

### 1.4 Ключевое архитектурное решение

Вместо того чтобы вшивать KGDB stub в ядро (как Linux), мы можем сделать **GDB stub как userspace сервис** (`drivers/debug/gdb_stub/`), который:

1. **Слушает serial порт** (COM1/2, `-serial stdio` в QEMU)
2. **На каждый запрос GDB**:
   - Читает/пишет память target процесса через `sys_safecopy` / `data_copy`
   - Читает/пишет регистры через `SYS_DIAGCTL` + `proc_stacktrace()`
   - Ставит/убирает breakpoint'ы через INT3 injection
   - Управляет выполнением (continue, single-step через TF флаг)
3. **Не вешает ядро** — GDB stub работает как обычный процесс с приоритетом SCHED_FIFO 98

Это **не KGDB-костыль**, а естественное расширение архитектуры микроядра.

---

## 2. Архитектура GDB Stub для GergiOS

### 2.1 Общая схема

```
Хост (Linux/macOS/Windows)           Target (GergiOS в QEMU/bare metal)
┌─────────────────────┐              ┌─────────────────────────────────────┐
│                     │              │                                     │
│  $ gdb kernel.elf   │              │  ┌───────────────────────────────┐  │
│  (gdb) target       │◄────serial───►  │  gdb_stub (userspace сервис)  │  │
│        remote       │   $GDBSTART    │  │                             │  │
│        /dev/ttyS0   │   $M 0x1000    │  │  serial_init(COM1, 115200)  │  │
│                     │   ...  :8      │  │  gdb_main_loop() {          │  │
│  GDB client         │   g/G          │  │    recv_packet()            │  │
│  ─────────────────  │   c            │  │    dispatch(command) {      │  │
│  set breakpoint     │   s            │  │      'g' → get_regs()       │  │
│  read memory        │   Z0,1,4       │  │      'G' → set_regs()       │  │
│  write memory       │   z0,1,4       │  │      'm' → read_mem()       │  │
│  step/continue      │   k            │  │      'M' → write_mem()      │  │
│  ─────────────────  │               │  │      'c' → continue()       │  │
│                     │               │  │      's' → step()           │  │
│                     │               │  │      'Z' → set_bp()         │  │
│                     │               │  │      'z' → clear_bp()       │  │
│                     │               │  │      '?' → signal_query()   │  │
│                     │               │  │    }                         │  │
│                     │               │  │  }                           │  │
│                     │               │  └───────────────────────────────┘  │
│                     │               │                                     │
│                     │               │  ┌───────────────────────────────┐  │
│                     │               │  │  Target Process (любой)       │  │
│                     │               │  │  ─────────────────────────    │  │
│                     │               │  │  • INT3 breakpoints          │  │
│                     │               │  │  • TF single-step            │  │
│                     │               │  │  • DR0-DR3 watchpoints       │  │
│                     │               │  │  • p_reg (stackframe_s)      │  │
│                     │               │  └───────────────────────────────┘  │
│                     │               │                                     │
│                     │               │  ┌───────────────────────────────┐  │
│                     │               │  │  Kernel (через syscalls)      │  │
│                     │               │  │  ─────────────────────────    │  │
│                     │               │  │  • sys_safecopy() — память   │  │
│                     │               │  │  • SYS_DIAGCTL — стек/рег    │  │
│                     │               │  │  • cause_sig() — SIGTRAP     │  │
│                     │               │  │  • pci_scan.c — PCI BARs     │  │
│                     │               │  └───────────────────────────────┘  │
│                     │               │                                     │
└─────────────────────┘              └─────────────────────────────────────┘
```

### 2.2 GDB Remote Serial Protocol — ключевые команды

GDB общается с target через простой текстовый протокол (изобретён в 1988, стабилен навеки):

```
Формат пакета:  $<data>#<checksum>
Ответ:          + (ACK) / - (NAK) / $<data>#<checksum>

Основные команды:
  '?'         — запрос сигнала остановки (SIGTRAP, SIGSEGV, ...)
  'g'         — читать все регистры (как hex)
  'G XX...'   — записать все регистры
  'm addr,len'— читать память (hex)
  'M addr,len:XX...' — писать память
  'c [addr]'  — continue (опционально с адреса)
  's [addr]'  — single-step
  'k'         — kill target
  'D'         — detach
  'Z0,addr,kind'  — software breakpoint (INT3)
  'z0,addr,kind'  — clear software breakpoint
  'Z1,addr,kind'  — hardware breakpoint (DR0)
  'z1,addr,kind'  — clear hardware breakpoint
  'Z2,addr,kind'  — write watchpoint (DR1)
  'Z3,addr,kind'  — read watchpoint (DR2)
  'Z4,addr,kind'  — access watchpoint (DR3)
  'qSupported'    — feature query
  'qC'            — current thread ID
  'qfThreadInfo'  — list threads (first)
  'qsThreadInfo'  — list threads (subsequent)
  'Hc thread'     — set thread for 'c'/'s'
  'Hg thread'     — set thread for memory ops
  'T thread'      — thread is alive?
  'vCont'         — extended continue (multi-thread)
  'vCont?'        — query vCont support
  'qXfer:memory-map:read' — memory map (для GDB знает MMIO)
```

### 2.3 Как это работает на микроядре

#### 2.3.1 Остановка процесса

Когда GDB говорит `c` (continue), а потом процесс ловит INT3:

```
1. GDB:     target remote /dev/ttyS0
2. GDB:     'c' (continue)
3. GDB stub: ничего не шлёт — процесс работает
4. Процесс:  встречает INT3 (0xCC) → #BP exception (vec 3)
5. Kernel:   exception_handler() → is_nested == 0, !iskernelp(pr)
6. Kernel:   cause_sig(proc_nr, SIGTRAP) → сигнал процессу
7. PM:       обработка SIGTRAP → процесс останавливается
8. GDB stub: получает уведомление (SIGCHLD или через RS register)
   ИЛИ:      kernel уведомляет GDB stub напрямую (через новый syscall)
9. GDB stub: шлёт 'T05' (SIGTRAP) по serial
10. GDB:     видит 'T05', понимает — процесс остановился
11. GDB:     шлёт 'g' (get registers) → stub читает p_reg через SYS_DIAGCTL
12. GDB:     показывает пользователю исходник с highlight'ом
```

#### 2.3.2 Breakpoint'ы

Software breakpoint (INT3, `0xCC`):
```
1. GDB:     'Z0,0x1000,0' — поставить breakpoint на 0x1000
2. GDB stub: читает байт по адресу 0x1000 (sys_safecopy)
3. GDB stub: сохраняет оригинальный байт в BP table
4. GDB stub: пишет 0xCC по адресу 0x1000 (sys_safecopy)
5. .text:   проблема! MINIX часто мапит .text как read-only
6. Решение:  VM → vm_map_phys() с RW permission временно
             или использовать hardware breakpoints (DR0)
```

Hardware breakpoint (DR0-DR3, для .text read-only):
```
1. GDB:     'Z1,0x1000,0' — hardware breakpoint на 0x1000
2. GDB stub: kernel call: sys_hwbp_set(proc_nr, DR0, 0x1000, 1, DR7_LE)
3. Kernel:   программирует DR0 = 0x1000, DR7 |= (GE | LE | L0)
4. Процесс:  при выполнении 0x1000 → #DB exception (vec 1)
5. Kernel:   exception_handler() → #DB → cause_sig(SIGTRAP)
6. GDB stub: шлёт 'T05' → GDB показывает остановку
```

#### 2.3.3 Single-step

```
1. GDB:     's' — step one instruction
2. GDB stub: kernel call: sys_single_step(proc_nr, enable)
3. Kernel:   устанавливает TF (Trap Flag) в EFLAGS процесса
4. Kernel:   возобновляет процесс (continue)
5. Процесс:  выполняет одну инструкцию → #DB exception (TF)
6. Kernel:   exception_handler() → #DB с TF → SIGTRAP
7. GDB stub: шлёт 'T05'
8. GDB:      показывает следующую инструкцию
```

#### 2.3.4 Чтение памяти процессов

```
GDB:     'm 0x7f00,100'
GDB stub: вызывает sys_safecopy(target_ep, 0x7f00, stub_ep, buf, 100)
          ИЛИ data_copy_vmcheck(target_ep, 0x7f00, stub_ep, buf, 100)
          отправляет hex(buf, 100) → GDB
```

#### 2.3.5 Чтение памяти ядра

```
GDB:     'm 0xf0000000,100' (память ядра)
GDB stub: sys_safecopy(KERNEL, 0xf0000000, stub_ep, buf, 100)
          (SYS_SAFECOPY_FROM kernel → stub buffer)
```

### 2.4 Новые системные вызовы

Для GDB stub потребуются новые kernel calls в `do_diagctl.c`:

```c
// Новые DIAGCTL коды
#define DIAGCTL_CODE_GET_REGS     5  // читать p_reg процесса (stackframe_s)
#define DIAGCTL_CODE_SET_REGS     6  // писать p_reg процесса
#define DIAGCTL_CODE_SINGLE_STEP  7  // включить/выключить TF флаг
#define DIAGCTL_CODE_HWBP_SET     8  // установить DR0-DR3 + DR7
#define DIAGCTL_CODE_HWBP_CLEAR   9  // очистить DR0-DR3
#define DIAGCTL_CODE_BP_NOTIFY    10 // уведомить GDB stub о breakpoint'е
```

#### 2.4.1 `DIAGCTL_CODE_GET_REGS`

```c
case DIAGCTL_CODE_GET_REGS:
    // Копировать p_reg (stackframe_s) из struct proc в userspace буфер
    // Это даёт GDB stub полный доступ к: rax, rbx, rcx, rdx, rsi,
    // rdi, rbp, rsp, r8-r15, rip, eflags, cs, ss, ds, es, fs, gs
    if (!isokendpt(m_ptr->m_lsys_krn_sys_diagctl.endpt, &proc_nr))
        return EINVAL;
    pp = proc_addr(proc_nr);
    // Копировать pp->p_reg в буфер пользователя
    data_copy(KERNEL, (vir_bytes)&pp->p_reg,
              caller->p_endpoint, buf, sizeof(struct stackframe_s));
    return OK;
```

#### 2.4.2 `DIAGCTL_CODE_SET_REGS`

```c
case DIAGCTL_CODE_SET_REGS:
    // Копировать из userspace буфера в pp->p_reg
    // Нужно для restore после single-step и для set $rip
    data_copy(caller->p_endpoint, buf,
              KERNEL, (vir_bytes)&pp->p_reg, sizeof(struct stackframe_s));
    return OK;
```

#### 2.4.3 `DIAGCTL_CODE_SINGLE_STEP`

```c
case DIAGCTL_CODE_SINGLE_STEP:
    // Включить/выключить TF (Trap Flag) в p_reg.psw (EFLAGS)
    if (m_ptr->m_lsys_krn_sys_diagctl.len) {  // enable
        pp->p_reg.psw |= TRACEBIT;
    } else {  // disable
        pp->p_reg.psw &= ~TRACEBIT;
    }
    return OK;
```

#### 2.4.4 `DIAGCTL_CODE_HWBP_SET`

```c
case DIAGCTL_CODE_HWBP_SET:
    // Программировать DR0-DR3 + DR7 для hardware breakpoint/watchpoint
    // Параметры: регистр DR (0-3), адрес, длина (1/2/4/8), тип (execute/write/read)
    pp->p_debug.dr[dr_num] = address;
    // DR7: L0/G0, LEN, RW для каждого DR
    pp->p_debug.dr7 |= (1 << (dr_num * 2)) |  // L0/L1/L2/L3
                       (len_code << (16 + dr_num * 4)) |  // LEN
                       (type_code << (18 + dr_num * 4));  // RW
    return OK;
```

### 2.5 Структуры данных

```c
// Внутренняя таблица breakpoint'ов GDB stub
#define MAX_BREAKPOINTS 64

struct gdb_bp {
    uintptr_t addr;         // адрес breakpoint'а
    uint8_t saved_byte;     // оригинальный байт (для software BP)
    uint8_t type;           // 0=software, 1=hardware, 2=watch_write, ...
    uint8_t enabled;        // активен?
    endpoint_t target_ep;   // какой процесс
};

struct gdb_state {
    // Serial
    int serial_fd;          // fd для COM порта
    int serial_irq;         // IRQ номер (COM1=4, COM2=3)
    
    // Состояние
    int connected;          // GDB подключён?
    endpoint_t target;      // какой процесс отлаживаем
    int stopped;            // target остановлен?
    int signal;             // последний сигнал (SIGTRAP, SIGSEGV, ...)
    
    // Breakpoints
    struct gdb_bp bps[MAX_BREAKPOINTS];
    int num_bps;
    
    // Thread list (для info threads)
    endpoint_t thread_list[NR_PROCS + NR_TASKS];
    int num_threads;
    
    // Буферы
    uint8_t recv_buf[1024]; // приёмный буфер serial
    uint8_t send_buf[1024]; // передающий буфер
    uint8_t mem_buf[4096];  // буфер чтения памяти
};
```

---

## 3. План реализации (3 фазы)

### Фаза 1: Ядро GDB stub — serial + базовые команды 🟡 ~2-3 недели

**Цель**: GDB может подключиться, читать/писать память, читать регистры.

#### 3.1.1 Serial transport

```c
// drivers/debug/gdb_stub/serial.c (~150 LOC)

int gdb_serial_init(int port, int baud) {
    // Открыть COM порт
    // COM1 = /dev/tty00, IRQ 4, port 0x3F8
    // COM2 = /dev/tty01, IRQ 3, port 0x2F8
    //
    // В QEMU: -serial stdio, GergiOS видит как tty
    // В bare metal: UART 16550 инициализация
}

int gdb_serial_send(uint8_t *data, int len) {
    // write(fd, data, len)
}

int gdb_serial_recv(uint8_t *data, int timeout) {
    // read(fd, data, 1) с таймаутом
    // Для interrupt-driven: ожидание в цикле с select/poll
}
```

#### 3.1.2 GDB protocol parser

```c
// drivers/debug/gdb_stub/protocol.c (~250 LOC)

// Парсит $<data>#<checksum>, проверяет контрольную сумму
// Отвечает + или -
int gdb_recv_packet(uint8_t *buf, int max_len);

// Отправляет $<data>#<checksum>, ждёт ACK
int gdb_send_packet(uint8_t *data, int len);

// Кодирование/декодирование hex
int gdb_hex_encode(uint8_t *src, int len, char *dst);
int gdb_hex_decode(char *src, uint8_t *dst, int max_len);

// CRC: простой 8-bit XOR (исторический GDB checksum)
uint8_t gdb_checksum(uint8_t *data, int len);
```

#### 3.1.3 Memory ops

```c
// drivers/debug/gdb_stub/memory.c (~200 LOC)

// Читать память target процесса или ядра
int gdb_read_mem(endpoint_t ep, uintptr_t addr, uint8_t *buf, int len) {
    if (ep == KERNEL) {
        // sys_safecopy(KERNEL_SELF, addr, stub_ep, buf, len)
        return sys_safecopy(SELF, addr, stub_ep, (vir_bytes)buf, len);
    } else {
        // data_copy_vmcheck(ep, addr, stub_ep, buf, len)
        return data_copy_vmcheck(target, ep, addr,
                                 stub_ep, (vir_bytes)buf, len);
    }
}

// Писать память target процесса
int gdb_write_mem(endpoint_t ep, uintptr_t addr, uint8_t *buf, int len) {
    // Аналогично, но data_copy_vmcheck(stub_ep, buf, ep, addr, len)
    // Для .text: временно разрешить запись через VM
    //   vm_memctl(ep, addr, len, VM_MEMCTL_ALLOW_WRITE);
    //   data_copy(...);
    //   vm_memctl(ep, addr, len, VM_MEMCTL_PROTECT);
}
```

#### 3.1.4 Register ops

```c
// drivers/debug/gdb_stub/registers.c (~150 LOC)

// Читать все регистры target процесса
int gdb_get_regs(endpoint_t ep, uint8_t *buf, int *len) {
    // sys_diagctl_getregs(ep, buffer) → kernel копирует p_reg
    message m;
    m.m_type = SYS_DIAGCTL;
    m.m_lsys_krn_sys_diagctl.code = DIAGCTL_CODE_GET_REGS;
    m.m_lsys_krn_sys_diagctl.endpt = ep;
    m.m_lsys_krn_sys_diagctl.buf = buf_vir;
    m.m_lsys_krn_sys_diagctl.len = sizeof(struct stackframe_s);
    int r = sys_call(KERNEL, &m);
    
    // GDB ожидает регистры в порядке: rax, rbx, rcx, rdx, rsi, rdi,
    // rbp, rsp, r8-r15, rip, eflags, cs, ss, ds, es, fs, gs
    // stackframe_s на x86_64 хранит их именно так (см. procoffsets.h)
}

// Писать регистры (для restore после step, или set $rip)
int gdb_set_regs(endpoint_t ep, uint8_t *buf, int len) {
    // DIAGCTL_CODE_SET_REGS
}
```

#### 3.1.5 Main loop

```c
// drivers/debug/gdb_stub/main.c (~200 LOC)

void gdb_main_loop(void) {
    gdb_serial_init(COM1, 115200);
    
    while (1) {
        uint8_t pkt[1024];
        int len = gdb_recv_packet(pkt, sizeof(pkt));
        if (len < 0) continue;
        
        switch (pkt[0]) {
        case '?': // Signal query
            gdb_send_signal(g_state.signal);
            break;
        case 'g': // Get registers
            gdb_get_regs(g_state.target, buf, &buflen);
            gdb_send_packet(buf, buflen);
            break;
        case 'G': // Set registers
            gdb_set_regs(g_state.target, pkt+1, len-1);
            gdb_send_ok();
            break;
        case 'm': // Read memory: m addr,len
            gdb_parse_m(pkt, &addr, &len);
            gdb_read_mem(g_state.target, addr, buf, len);
            gdb_send_packet_hex(buf, len);
            break;
        case 'M': // Write memory
            gdb_parse_M(pkt, &addr, &len, &data);
            gdb_write_mem(g_state.target, addr, data, len);
            gdb_send_ok();
            break;
        case 'c': // Continue
            gdb_continue(pkt);
            break;
        case 's': // Step
            gdb_step(pkt);
            break;
        case 'k': // Kill
            gdb_kill();
            break;
        case 'D': // Detach
            gdb_detach();
            break;
        case 'Z': // Set breakpoint/watchpoint
            gdb_set_bp(pkt);
            break;
        case 'z': // Clear breakpoint/watchpoint
            gdb_clear_bp(pkt);
            break;
        case 'q': // Query
            gdb_handle_query(pkt);
            break;
        default:
            gdb_send_empty(); // пустой ответ = unsupported
            break;
        }
    }
}
```

#### 3.1.6 Файлы

```
drivers/debug/gdb_stub/
  ├── Makefile           — build + install
  ├── gdb_stub.h         — internal structures
  ├── main.c             — entry point + main loop (~200 LOC)
  ├── serial.c           — UART 16550 init + send/recv (~150 LOC)
  ├── protocol.c         — GDB packet encode/decode (~250 LOC)
  ├── memory.c           — sys_safecopy read/write (~200 LOC)
  └── registers.c        — DIAGCTL get/set regs (~150 LOC)
```

**Итого Фаза 1**: ~1,000 LOC C

---

### Фаза 2: Breakpoints + Single-step + Continue 🟡 ~2-3 недели

**Цель**: GDB может ставить breakpoint'ы, single-step, continue.

#### 3.2.1 Software breakpoints (INT3)

```c
// gdb_stub/breakpoint.c (~250 LOC)

// Сохранить байт по addr, записать 0xCC
int gdb_set_sw_bp(endpoint_t ep, uintptr_t addr) {
    // 1. sys_safecopy(ep, addr, stub_ep, &saved, 1)
    // 2. Если saved == 0xCC → уже breakpoint, return
    // 3. g_state.bps[idx].saved = saved
    // 4. Если .text read-only:
    //    vm_memctl(ep, addr, 1, VM_MEMCTL_ALLOW_WRITE);
    // 5. sys_safecopy(stub_ep, &0xCC, ep, addr, 1)
    // 6. Если .text был read-only:
    //    vm_memctl(ep, addr, 1, VM_MEMCTL_PROTECT);
    // 7. Отметить BP как enabled
}

// Восстановить оригинальный байт
int gdb_clear_sw_bp(endpoint_t ep, uintptr_t addr) {
    // sys_safecopy(stub_ep, &saved_byte, ep, addr, 1)
}

// При остановке на breakpoint'е: restore байт на место,
// откатить RIP на 1 (INT3 = 1 байт)
void gdb_handle_bp_hit(endpoint_t ep) {
    // 1. Определить адрес: RIP - 1
    // 2. Найти BP в таблице по ep + addr
    // 3. Восстановить оригинальный байт
    // 4. gdb_set_regs: RIP -= 1
    // 5. Отправить GDB: T05 (SIGTRAP)
}
```

#### 3.2.2 Hardware breakpoints (DR0-DR3)

```c
// Для .text который нельзя писать (read-only code) — используем DR0
// 
// x86_64 Debug Registers:
//   DR0-DR3: линейные адреса breakpoint'ов
//   DR6: статус (какой DR сработал)
//   DR7: контроль (L0/G0, LEN, RW для каждого DR)
//
// DR7 encoding:
//   L0 = bit 0, G0 = bit 1, L1 = bit 2, G1 = bit 3, ...
//   LEN0 = bits 18-19: 00=1 byte, 01=2 bytes, 10=8 bytes, 11=4 bytes
//   RW0  = bits 16-17: 00=execute, 01=write, 10=I/O, 11=read/write

int gdb_set_hw_bp(endpoint_t ep, uintptr_t addr, int kind) {
    // sys_diagctl_hwbp(ep, DIAGCTL_HWBP_SET, dr_num, addr, len, type)
}

int gdb_clear_hw_bp(endpoint_t ep, uintptr_t addr, int kind) {
    // sys_diagctl_hwbp(ep, DIAGCTL_HWBP_CLEAR, dr_num, 0, 0, 0)
}
```

#### 3.2.3 Continue

```c
// gdb_stub/continue.c (~150 LOC)

// После continue stub ничего не шлёт — ждёт следующего события
void gdb_continue(uint8_t *pkt) {
    // Разрешить процессу выполняться
    // sys_diagctl_single_step(ep, FALSE);  // снять TF
    // Надо уведомить kernel: "этот процесс больше не остановлен"
    // kernel возобновляет процесс
    //
    // Stub переходит в режим ожидания:
    //   select(fd, &g_state.serial_fd, ...) — ждёт serial OR уведомление
    //   Если пришло от serial → GDB прислал новую команду
    //   Если пришло уведомление → target процесс словил BP/SIG
}

// Уведомление от kernel о том, что target остановился:
// Вариант A: kernel шлёт сигнал SIGTRAP процессу, PM обрабатывает
//            → PM уведомляет GDB stub через IPC
// Вариант B: новый syscall SYS_BP_NOTIFY, kernel напрямую кидает
//            сообщение GDB stub'у
// Вариант C: GDB stub регистрируется через DIAGCTL_CODE_REGISTER
//            и получает SIGKMESS при breakpoint'е
```

#### 3.2.4 Single-step

```c
void gdb_step(uint8_t *pkt) {
    // 1. sys_diagctl_single_step(ep, TRUE);  // установить TF
    // 2. Возобновить процесс (как continue)
    // 3. Процесс выполнит 1 инструкцию → #DB → SIGTRAP
    // 4. Stub получает уведомление
    // 5. Stub шлёт 'T05' (SIGTRAP)
    // 6. GDB: показывает следующую инструкцию
    //
    // Проблема: single-step через syscall требует
    // установки TF + продолжения процесса атомарно.
    // Решение: DIAGCTL_CODE_SINGLE_STEP + continue в одном вызове
}
```

#### 3.2.5 Файлы

```
drivers/debug/gdb_stub/
  ├── breakpoint.c      — software + hardware BP management (~250 LOC)
  ├── continue.c        — continue + single-step + wait (~150 LOC)
  └── ...
```

**Итого Фаза 2**: ~400 LOC C

---

### Фаза 3: Multi-thread + QoL + Integration 🟡 ~2-3 недели

**Цель**: Полноценный GDB с multi-process, thread listing, symbol support.

#### 3.3.1 Query handler (qSupported, qXfer)

```c
// gdb_stub/query.c (~200 LOC)

// qSupported: какие фичи поддерживает stub
void gdb_qsupported(void) {
    // PacketSize=1024
    // qXfer:memory-map:read+
    // qXfer:features:read+
    // qXfer:threads:read+
    // qC, qfThreadInfo, qsThreadInfo
    // vCont+
    // multiprocess+
    // swbreak+
    // hwbreak+
}

// qXfer:memory-map:read — memory map для GDB
// Позволяет GDB знать, какие адреса MMIO, RAM, ROM
void gdb_memory_map(void) {
    // XML:
    // <memory-map>
    //   <memory type="ram" start="0x0" length="0x100000"/>
    //   <memory type="rom" start="0xE0000" length="0x20000"/>
    // </memory-map>
}

// qXfer:threads:read — список тредов/процессов
void gdb_thread_list(void) {
    // XML:
    // <threads>
    //   <thread id="1" core="0" name="kernel"/>
    //   <thread id="42" core="0" name="pm"/>
    //   <thread id="43" core="0" name="vfs"/>
    //   <thread id="55" core="0" name="ahci_rust"/>
    //   ...
    // </threads>
}
```

#### 3.3.2 Hardware watchpoints (DR1-DR3)

```c
// gdb_stub/watchpoint.c (~150 LOC)

// DR1 = write watchpoint
// DR2 = read watchpoint  
// DR3 = access watchpoint
//
// Используется для отслеживания изменения переменных:
//   (gdb) watch my_var
//   (gdb) rwatch my_var
//   (gdb) awatch my_var
```

#### 3.3.3 Kernel memory access (для отладки ядра)

```c
// Расширение DIAGCTL для чтения физической памяти ядра
// и памяти других процессов.
//
// Проблема: GergiOS — микроядро, процессы изолированы.
// GDB stub (userspace) может читать только свою и target память.
// Для чтения физической памяти нужен kernel call.
//
// Решение: DIAGCTL_CODE_READ_PHYS / DIAGCTL_CODE_WRITE_PHYS
//   m_lsys_krn_sys_diagctl.code = DIAGCTL_CODE_READ_PHYS
//   m_lsys_krn_sys_diagctl.buf = phys_addr
//   m_lsys_krn_sys_diagctl.len = size
//
// kernel: data_copy(KERNEL, phys_addr, stub_ep, buf, len)
// (требует SYS_PROC permission)
```

#### 3.3.4 Автоматическое обнаружение GDB при panic

```c
// В kernel/debug.c или kernel/panic.c:
// Если kgdb_active и GDB подключён — вместо panic вызова
// остановить систему и ждать GDB команд.
//
// Это позволяет вместо "panic: unhandled kernel exception"
// получить GDB промпт с полным состоянием:
//   (gdb) bt
//   (gdb) info registers
//   (gdb) list *$rip

void kgdb_panic_hook(void) {
    if (gdb_connected) {
        // Сохранить состояние
        // Отправить GDB 'S02' (SIGINT)
        // Ждать команд
        gdb_main_loop_interactive();
    } else {
        // Обычный panic
        panic("...");
    }
}
```

#### 3.3.5 Integration с RS (Reincarnation Server)

```c
// При падении сервиса RS может:
// 1. Проверить — подключён ли GDB?
// 2. Если да → вместо restart'а подвесить сервис для отладки
// 3. GDB stub уведомляет GDB: 'T05' (SIGTRAP)
// 4. Пользователь может отладить сервис перед рестартом
//
// RS integration:
//   if (gdb_stub_connected && rp->r_restarts > MAX_RESTARTS) {
//       gdb_stub_notify(rp->r_pub->endpoint, SIGSEGV);
//       // Ждать, пока GDB не скажет continue
//   }
```

#### 3.3.6 Файлы

```
drivers/debug/gdb_stub/
  ├── query.c           — qSupported, qXfer, thread list (~200 LOC)
  ├── watchpoint.c      — hardware watchpoints (~150 LOC)
  ├── phys_mem.c        — kernel physical memory access (~100 LOC)
  ├── panic_hook.c      — kgdb_panic integration (~100 LOC)
  └── rs_integration.c  — RS notify/crash handling (~150 LOC)
```

**Итого Фаза 3**: ~700 LOC C

---

## 4. Изменения в ядре

### 4.1 Новые DIAGCTL коды

```c
// minix/include/minix/com.h — новые коды для DIAGCTL
#define DIAGCTL_CODE_GET_REGS     5
#define DIAGCTL_CODE_SET_REGS     6
#define DIAGCTL_CODE_SINGLE_STEP  7
#define DIAGCTL_CODE_HWBP_SET     8
#define DIAGCTL_CODE_HWBP_CLEAR   9
#define DIAGCTL_CODE_BP_NOTIFY    10
#define DIAGCTL_CODE_READ_PHYS    11
#define DIAGCTL_CODE_WRITE_PHYS   12
```

### 4.2 Debug registers в struct proc

```c
// minix/kernel/proc.h — добавить debug registers
struct proc {
    // ... existing fields ...
    
    // Debug registers (x86_64 DR0-DR7)
    // Сохраняются/восстанавливаются при context switch
    uint64_t p_debug_dr[4];     // DR0-DR3
    uint64_t p_debug_dr6;       // DR6 (status)
    uint64_t p_debug_dr7;       // DR7 (control)
};
```

### 4.3 Context switch — save/restore DR

```c
// minix/kernel/arch/x86_64/switch.S
// При переключении процессов сохранять/восстанавливать DR0-DR7

switch_to:
    // Save current process DR
    mov     %dr0, %rax
    mov     %rax, PROC_DR0(%rdi)    // old proc
    mov     %dr1, %rax
    mov     %rax, PROC_DR1(%rdi)
    mov     %dr2, %rax
    mov     %rax, PROC_DR2(%rdi)
    mov     %dr3, %rax
    mov     %rax, PROC_DR3(%rdi)
    mov     %dr7, %rax
    mov     %rax, PROC_DR7(%rdi)
    
    // Restore new process DR
    mov     PROC_DR0(%rsi), %rax    // new proc
    mov     %rax, %dr0
    mov     PROC_DR1(%rsi), %rax
    mov     %rax, %dr1
    mov     PROC_DR2(%rsi), %rax
    mov     %rax, %dr2
    mov     PROC_DR3(%rsi), %rax
    mov     %rax, %dr3
    mov     PROC_DR7(%rsi), %rax
    mov     %rax, %dr7
```

### 4.4 Single-step handler

```c
// minix/kernel/arch/x86_64/exception.c
// В exception_handler() добавить обработку #DB с TF:

void exception_handler(int is_nested, struct exception_frame * frame) {
    // ...
    
    if (frame->vector == DEBUG_VECTOR) {
        // Check if TF was set (single-step)
        if (saved_proc->p_reg.psw & TRACEBIT) {
            // Clear TF
            saved_proc->p_reg.psw &= ~TRACEBIT;
            
            if (!is_nested && !iskernelp(saved_proc)) {
                // Userspace single-step → SIGTRAP
                cause_sig(proc_nr(saved_proc), SIGTRAP);
                return;
            }
        }
        
        // Check DR6 for hardware breakpoint/watchpoint
        if (saved_proc->p_debug_dr6) {
            saved_proc->p_debug_dr6 = 0;  // Clear status
            
            if (!is_nested && !iskernelp(saved_proc)) {
                cause_sig(proc_nr(saved_proc), SIGTRAP);
                return;
            }
        }
    }
    
    // ...
}
```

### 4.5 DIAGCTL изменения

```c
// minix/kernel/system/do_diagctl.c

int do_diagctl(struct proc * caller, message * m_ptr) {
    switch (m_ptr->m_lsys_krn_sys_diagctl.code) {
        // ... existing cases ...
        
        case DIAGCTL_CODE_GET_REGS: {
            int proc_nr;
            struct proc *pp;
            struct stackframe_s regs;
            
            if (!isokendpt(m_ptr->m_lsys_krn_sys_diagctl.endpt, &proc_nr))
                return EINVAL;
            pp = proc_addr(proc_nr);
            
            // Копировать p_reg в буфер вызывающего
            regs = pp->p_reg;  // struct copy
            return data_copy(KERNEL, (vir_bytes)&regs,
                           caller->p_endpoint,
                           m_ptr->m_lsys_krn_sys_diagctl.buf,
                           sizeof(regs));
        }
        
        case DIAGCTL_CODE_SET_REGS: {
            int proc_nr;
            struct proc *pp;
            struct stackframe_s regs;
            
            if (!isokendpt(m_ptr->m_lsys_krn_sys_diagctl.endpt, &proc_nr))
                return EINVAL;
            pp = proc_addr(proc_nr);
            
            // Копировать из буфера вызывающего в p_reg
            int r = data_copy(caller->p_endpoint,
                            m_ptr->m_lsys_krn_sys_diagctl.buf,
                            KERNEL, (vir_bytes)&regs,
                            sizeof(regs));
            if (r != OK) return r;
            
            pp->p_reg = regs;
            return OK;
        }
        
        case DIAGCTL_CODE_SINGLE_STEP: {
            int proc_nr;
            struct proc *pp;
            
            if (!isokendpt(m_ptr->m_lsys_krn_sys_diagctl.endpt, &proc_nr))
                return EINVAL;
            pp = proc_addr(proc_nr);
            
            if (m_ptr->m_lsys_krn_sys_diagctl.len) {
                pp->p_reg.psw |= TRACEBIT;
            } else {
                pp->p_reg.psw &= ~TRACEBIT;
            }
            return OK;
        }
        
        case DIAGCTL_CODE_HWBP_SET: {
            int proc_nr, dr_num;
            struct proc *pp;
            
            // Параметры: endpt, dr_num, addr, len, type
            // Упакованы в buf/len/LSB сообщения
            
            if (!isokendpt(m_ptr->m_lsys_krn_sys_diagctl.endpt, &proc_nr))
                return EINVAL;
            pp = proc_addr(proc_nr);
            
            dr_num = /* из параметров */;
            pp->p_debug_dr[dr_num] = /* addr */;
            
            // DR7 encoding:
            // L0 = 1 << (dr_num * 2)
            // LEN = len_code << (16 + dr_num * 4)
            // RW = type_code << (18 + dr_num * 4)
            pp->p_debug_dr7 |= /* L0 */;
            pp->p_debug_dr7 |= /* LEN */;
            pp->p_debug_dr7 |= /* RW */;
            
            return OK;
        }
        
        case DIAGCTL_CODE_BP_NOTIFY: {
            // GDB stub регистрируется как получатель уведомлений
            // о breakpoint'ах. При #BP или #DB kernel шлёт
            // сообщение GDB stub'у.
            
            priv(caller)->s_diag_sig = TRUE;
            return OK;
        }
    }
}
```

### 4.6 Новые/изменённые файлы ядра

| Файл | Изменения |
|------|-----------|
| `minix/include/minix/com.h` | DIAGCTL_CODE_GET_REGS, SET_REGS, SINGLE_STEP, HWBP_SET/CLEAR, BP_NOTIFY, READ/WRITE_PHYS |
| `minix/kernel/proc.h` | `p_debug_dr[4]`, `p_debug_dr6`, `p_debug_dr7` поля в struct proc |
| `minix/kernel/system/do_diagctl.c` | 6 новых case'ов |
| `minix/kernel/arch/x86_64/exception.c` | #DB handler для TF + HW breakpoints |
| `minix/kernel/arch/x86_64/switch.S` | DR save/restore в context switch |

---

## 5. Тестирование

### 5.1 В QEMU

```bash
# Запуск GergiOS с serial для GDB
qemu-system-x86_64 -kernel kernel.bin \
    -serial tcp::1234,server,nowait \
    -append "gdb=1 gdb_port=1234"

# В другом терминале:
gdb kernel.elf
(gdb) target remote localhost:1234
(gdb) break pm_init
(gdb) continue
```

### 5.2 Bare metal

```bash
# На физической машине с двумя COM портами
# COM1: console GergiOS
# COM2: GDB stub

qemu-system-x86_64 ... -serial stdio -serial tcp::1234,server,nowait

# GDB подключается к COM2
gdb kernel.elf
(gdb) target remote /dev/ttyS1  # или localhost:1234 для QEMU
```

### 5.3 Unit tests (через host stubs)

```c
// tests/gdb_stub_test.c — тесты GDB протокола
// • hex encode/decode
// • checksum compute
// • packet framing
// • memory read/write (через mock sys_safecopy)
// • register encoding (stackframe_s → GDB order)

// Тесты можно запускать на хосте (Linux), подменяя syscall stubs
```

### 5.4 Integration tests

```bash
# GDB script для автоматического тестирования
# test_gdb.gdb:
#   target remote localhost:1234
#   break main
#   continue
#   info registers
#   x/10i $pc
#   stepi
#   stepi
#   info threads
#   continue

gdb -batch -x test_gdb.gdb kernel.elf
```

---

## 6. LOC Estimate

| Компонент | Файлы | LOC | Фаза |
|-----------|-------|-----|------|
| **GDB Stub (userspace)** | | | |
| Serial init + UART | `serial.c` | ~150 | P1 |
| GDB protocol (packet, hex, CRC) | `protocol.c` | ~250 | P1 |
| Memory read/write (safecopy) | `memory.c` | ~200 | P1 |
| Register get/set (DIAGCTL) | `registers.c` | ~150 | P1 |
| Main loop + dispatch | `main.c` | ~200 | P1 |
| Breakpoints (sw + hw) | `breakpoint.c` | ~250 | P2 |
| Continue + wait | `continue.c` | ~150 | P2 |
| Query handler (qSupported, qXfer) | `query.c` | ~200 | P3 |
| Watchpoints (DR1-DR3) | `watchpoint.c` | ~150 | P3 |
| Physical memory access | `phys_mem.c` | ~100 | P3 |
| Panic hook | `panic_hook.c` | ~100 | P3 |
| RS integration | `rs_integration.c` | ~150 | P3 |
| **Итого GDB Stub** | **~2,100** | | |
| | | | |
| **Kernel изменения** | | | |
| do_diagctl.c (6 case'ов) | system/do_diagctl.c | ~150 | P1 |
| proc.h (DR поля) | proc.h | ~20 | P1 |
| exception.c (#DB handler) | arch/x86_64/exception.c | ~50 | P1 |
| switch.S (DR save/restore) | arch/x86_64/switch.S | ~40 | P1 |
| com.h (новые константы) | include/minix/com.h | ~15 | P1 |
| **Итого Kernel** | **~275** | | |
| | | | |
| **Всего** | **~2,375 LOC C** | | |

---

## 7. Приоритет и зависимость от других фаз

### 7.1 Зависимости

| Зависимость | Почему |
|-------------|--------|
| **Phase 7 (Drivers)** | GDB stub нужен для отладки драйверов, но драйверы — приоритет P0. Stub пишется после стабилизации драйверов, когда GDB будет нужен для отладки проблем в production. **НО**: базовая версия (Фаза 1) может быть написана раньше для отладки самих драйверов |
| **IRQ threads (Phase 6)** | Serial IRQ handler нужен для interrupt-driven GDB stub (не busy-wait) |
| **SMP стабильность** | DR save/restore в context switch на SMP требует корректной синхронизации |

### 7.2 Готовность после каждой фазы

| Фаза | Что можно делать |
|------|------------------|
| **P1** | Подключиться GDB, читать/писать память и регистры. **Достаточно для отладки багов** (смотреть переменные, стек) |
| **P2** | Breakpoint'ы + single-step. **Полноценная отладка** |
| **P3** | Multi-thread, watchpoints, kernel memory, panic hook, RS integration |

---

## 8. Открытые вопросы

1. **Уведомление GDB stub при breakpoint'е** — как kernel сообщает GDB stub'у, что процесс остановился?
   - Вариант A: kernel шлёт IPC stub'у напрямую (новый syscall + endpoint)
   - Вариант B: PM обрабатывает SIGTRAP → уведомляет stub через RS
   - Вариант C: stub polling — периодически проверяет статус процесса через `SYS_DIAGCTL`
   
2. **Read-only .text** — если кодовая страница read-only, software breakpoint (INT3) не сработает.
   - Решение 1: Hardware breakpoints (DR0) — не требует записи в .text
   - Решение 2: Временно разрешить запись через `vm_memctl()`
   - Решение 3: Всегда мапить .text как RW при включённом GDB

3. **Multiple processes** — GDB отлаживает один процесс. Как переключаться между процессами?
   - GDB `thread` command → `info threads` показывает все процессы
   - stub переключает `g_state.target` на выбранный endpoint
   - Проблема: одновременная остановка нескольких процессов

4. **Kernel отладка** — можно ли отлаживать само микроядро?
   - Да, GDB stub читает p_reg SYSTEM процесса
   - DIAGCTL_CODE_READ_PHYS для чтения физической памяти ядра
   - Проблема: при остановке ядра вся система виснет (нет IPC)

5. **Fast serial** — 115200 бод = ~11.5 KB/s. Для чтения 4KB памяти надо ~350ms.
   - Решение: использовать QEMU TCP (`-serial tcp::1234,server,nowait`) вместо serial
   - Для bare metal: 921600 бод если UART и кабель поддерживают

6. **Безопасность** — GDB stub может читать/писать любую память и регистры любого процесса.
   - Только процессы с `SYS_PROC` permission могут регистрироваться
   - При detach/отключении — все breakpoint'ы автоматически очищаются

---

## 9. Связанные документы

- `sys/sys/kgdb.h` — NetBSD KGDB заголовок (протокол)
- `minix/kernel/system/do_diagctl.c` — существующий SYS_DIAGCTL (SIGKMESS, stacktrace)
- `minix/kernel/arch/x86_64/exception.c` — exception handler (#DB, #BP)
- `minix/kernel/debug.c` — debug utilities (print_proc, stacktrace, IPC stats)
- `minix/kernel/proc.h` — struct proc (p_reg = stackframe_s)
- `planning/22_rs_reincarnation_server.md` — RS diagnostics (GDB integration для crash dump)
- `planning/23_driver_model_modernization.md` — Driver Manager (GDB stub как сервис)
- GDB Remote Protocol: [Sourceware GDB docs](https://sourceware.org/gdb/current/onlinedocs/gdb/Remote-Protocol.html)
- NetBSD KGDB: [NetBSD kgdb(4)](https://man.netbsd.org/kgdb.4)
