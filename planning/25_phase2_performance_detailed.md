# Phase 2: Network Performance — Детальный план реализации

> **Статус**: ⏳ План
> **База**: `planning/25_network_stack_modernization.md` Phase 2
> **Связанные**: `minix/drivers/net/e1000/`, `minix/lib/libnetdriver/`, `minix/net/lwip/ndev.c`, `minix/net/lwip/ethif.c`, `minix/net/lwip/loopif.c`

---

## 1. Текущее состояние (аудит)

### 1.1 e1000 драйвер (`minix/drivers/net/e1000/`)

```
┌─────────────────────────────────────┐
│          e1000 драйвер               │
│                                      │
│  RX Ring: 256 desc (single queue)    │ ← E1000_RXDESC_NR
│  TX Ring: 256 desc (single queue)    │ ← E1000_TXDESC_NR
│  Buffer: 2048 bytes per desc         │ ← E1000_IOBUF_SIZE
│                                      │
│  e1000_send() — polling, copyin      │ → netdriver_copyin(data → tx_buffer)
│  e1000_recv() — polling, copyout     │ → netdriver_copyout(rx_buffer → data)
│  e1000_intr() — ICR-based dispatch   │ → netdriver_send/recv/link
│                                      │
│  DMA: gergios_dma_alloc_coherent     │ ✅
│  PM: runtime_suspend/resume          │ ✅
│  Multicast: basic (RAL/RAH, MTA)     │ ✅
│  Checksum: НЕ ИСПОЛЬЗУЕТСЯ          │ ❌ (нет NDEV_CAP_CS_*)
└─────────────────────────────────────┘
```

**Ограничения:**
- Одна RX/TX очередь (регистры RDBAL/TDBAL — один набор)
- Нет TSO — все пакеты ≤ 2048 байт
- Нет RSS — одно ядро CPU обрабатывает все прерывания
- Нет checksum offload — lwIP вычисляет вручную
- Нет jumbo frames — MTU ≤ 1500

### 1.2 netdriver framework (`minix/lib/libnetdriver/`)

```
┌────────────────────────────────────────────┐
│           netdriver framework               │
│                                             │
│  NETDRIVER_SENDQ_MAX = 8   ← pending sends │
│  NETDRIVER_RECVQ_MAX = 2   ← pending recvs │
│  NETDRIVER_MCAST_MAX = 16  ← mcast list    │
│                                             │
│  struct netdriver {                         │
│    ndr_send(data, size)    ← одно API       │
│    ndr_recv(data, max)     ← одно API       │
│    ndr_init, ndr_stop, ...                  │
│  }                                          │
│                                             │
│  netdriver_task() — main loop               │
│  netdriver_process() — dispatch             │
│  netdriver_copyin/out — grant-based copy    │
└─────────────────────────────────────────────┘
```

**Ограничения:**
- Одна send/recv функция на драйвер
- Нет multi-queue в API
- Нет batch processing
- Нет event-driven модели (только polling после каждого сообщения)

### 1.3 ndev слой (`minix/net/lwip/ndev.c`)

```
┌───────────────────────────────────────┐
│      ndev (driver communication)       │
│                                        │
│  ndev_array[NR_NDEV] = 8              │
│                                        │
│  Каждый ndev:                         │
│    ndev_sendq (send + conf)           │
│    ndev_recvq (receive only)          │
│    ndev_endpt — endpoint драйвера     │
│                                        │
│  ndev_send() — grant ipc              │
│  ndev_recv() — grant ipc              │
│  ndev_conf() — config ipc             │
└────────────────────────────────────────┘
```

**Ограничения:**
- Одна очередь send, одна recv на драйвер
- `nq_head` — последовательные ID для tracking
- Нет multi-queue в протоколе IPC

### 1.4 ethif слой (`minix/net/lwip/ethif.c`)

```
┌─────────────────────────────────────────┐
│     ethif (ethernet interface)           │
│                                          │
│  ethif_array[NR_NDEV]                    │
│                                          │
│  ethif_snd — send queue (pbuf chain)     │
│  ethif_rcv — recv queue (pbuf chain)     │
│                                          │
│  ETHIF_MAX_MTU = 1500                    │
│  ETHIF_PBUF_MIN = 8 (NDEV_IOV_MAX)      │
│  ETHIF_MCAST_MAX = 8                     │
│                                          │
│  ethif_poll() — dispatch after each msg  │
│    → ethif_can_conf → ndev_conf          │
│    → ethif_can_send → ndev_send          │
│    → ethif_can_recv → ndev_recv          │
└──────────────────────────────────────────┘
```

**Ограничения:**
- Нет jumbo frames (MTU ≤ 1500)
- Send queue — linked list с spare tokens
- Нет multi-queue awareness
- Poll-based, не interrupt-driven

### 1.5 Loopback (`minix/net/lwip/loopif.c`)

```
┌──────────────────────────────────────┐
│        loopif (loopback)              │
│                                        │
│  NR_LOOPIF = 2                        │
│  LOOPIF_DEF_MTU = 65531               │
│                                        │
│  loopif_output() → pchain_alloc + copy │
│  loopif_poll() → ifdev_input          │
│                                        │
│  Async: пакеты ставятся в очередь     │
│  и обрабатываются на следующем poll   │
└──────────────────────────────────────┘
```

**Ограничения:**
- Полная копия pbuf на каждый пакет
- Async очередь → latency
- Нет синхронного fast-path

---

## 2. План реализации

### 2.1 Sub-phase 2a: Checksum Offload + Jumbo Frames

**Цель**: Включить аппаратный checksum offload в e1000, поднять MTU до 9000 (jumbo frames).

**Файлы для изменения:**
- `minix/drivers/net/e1000/e1000.c` → e1000_init: установить `NDEV_CAP_CS_*` флаги
- `minix/drivers/net/e1000/e1000.h` → увеличить `E1000_IOBUF_SIZE` для jumbo frames
- `minix/net/lwip/ethif.c` → `ETHIF_MAX_MTU` = 9000 (с конфигом)
- `minix/include/minix/netdriver.h` → `NDEV_CAP_CS_*` уже есть, не нужно менять

**Изменения в e1000 (e1000.c):**

```c
// В e1000_init() — добавить checksum capabilities:
*caps = NDEV_CAP_MCAST | NDEV_CAP_BCAST | NDEV_CAP_HWADDR |
    NDEV_CAP_CS_IP4_TX | NDEV_CAP_CS_IP4_RX |
    NDEV_CAP_CS_TCP_TX | NDEV_CAP_CS_TCP_RX |
    NDEV_CAP_CS_UDP_TX | NDEV_CAP_CS_UDP_RX;

// В e1000_init_hw() — включить checksum offload в RCTL/TCTL:
// RCTL.SECRC = 1 (strip Ethernet CRC on receive)
// TX descriptor: CSS=14 (TCP), CSU=IP+TCP, CST=0
```

**Изменения в ethif (ethif.c):**
- `ETHIF_MAX_MTU` = 9000 (по умолчанию `ETHIF_DEF_MTU` = 1500, включается по `ifconfig mtu 9000`)
- `ethif_output()` — проверить, что `pbuf->tot_len` ≤ `ETHIF_MAX_MTU + ETH_HDR_LEN`

**Изменения в lwipopts.h:**
- Нет изменений — lwIP уже поддерживает jumbo через MTU настройку netif

**Проверка:**
- `ifconfig em0 mtu 9000` — работает
- `iperf3 -M 9000` — TCP throughput улучшен
- checksum error counters не растут

**Ожидаемый эффект**: +10-20% throughput (за счёт HW checksum), jumbo frames (больше данных на пакет)

---

### 2.2 Sub-phase 2b: Multi-Queue e1000 + RSS

**Цель**: Использовать 2+ RX/TX очереди e1000 для параллельной обработки.

**Архитектурное решение:**
e1000 hardware (82540EM/82545EM) поддерживает до 4 TX очередей (TXDCTL, TDBAH/L) и до 4 RX очередей (RXDCTL, RDBAH/L). В MINIX драйвер — один процесс, но очереди могут быть распределены в нём.

**Подход**: Multiple queue pairs внутри одного драйвера.
- Каждая очередь — отдельный набор регистров
- RSS (Receive Side Scaling) — Toeplitz hash для распределения потоков
- IRQ coalescing: один IRQ на несколько очередей

**Изменения в e1000 (e1000.c/h):**

```c
// e1000.h — добавить:
#define E1000_NUM_QUEUES  2  /* 2 TX + 2 RX queues */

typedef struct e1000_queue {
    e1000_tx_desc_t *tx_desc;
    phys_bytes tx_desc_phys;
    char *tx_buffer;
    int tx_desc_count;

    e1000_rx_desc_t *rx_desc;
    phys_bytes rx_desc_phys;
    char *rx_buffer;
    int rx_desc_count;

    uint32_t tx_regs[4];  /* TDBAL, TDBAH, TDLEN, TDT */
    uint32_t rx_regs[4];  /* RDBAL, RDBAH, RDLEN, RDT */
} e1000_queue_t;

// e1000_state:
typedef struct e1000 {
    ...
    e1000_queue_t queues[E1000_NUM_QUEUES];
    uint32_t rss_key[10];   /* Toeplitz random key */
    ...
} e1000_t;
```

**RSS (Receive Side Scaling):**
- e1000 82545EM+ has MRQC register for RSS
- Toeplitz hash on (src_ip, dst_ip, src_port, dst_port)
- Hash result → queue index: `queue = hash % E1000_NUM_QUEUES`
- Generate random key in `e1000_init_hw()` (sys_rand())

**Изменения в netdriver API:**
- Текущий API: одна `ndr_send`/`ndr_recv` на драйвер
- Нужно расширить для multi-queue:
  - Добавить `unsigned int queue` параметр в `ndr_send`/`ndr_recv`
  - Или передавать queue через `netdriver_data` (поле `id`)

**Вариант**: Использовать младшие биты `netdriver_data.id` для queue number.

**Изменения в ndev (ndev.c):**
- Каждая queue получает свой набор слотов сообщений
- Queue ID кодируется в старших битах sequence number

**Изменения в ethif (ethif.c):**
- `ethif_output()` — выбирает TX queue по flow hash
- `ethif_poll()` — опрашивает все очереди

**Ожидаемый эффект**: 1.5-2× throughput на многоядерных системах.

**Риски:**
- e1000 QEMU может не поддерживать multi-queue
- Усложнение кода драйвера
- Требуется тестирование на реальном железе

**Альтернатива**: virtio-net multi-queue (если e1000 не поддерживает).

---

### 2.3 Sub-phase 2c: TSO (TCP Segmentation Offload)

**Цель**: Передать TCP сегментацию в e1000 hardware для снижения CPU нагрузки.

**Архитектура TSO в lwIP + e1000:**

```
lwIP TCP                       lwIP TCP
  │                               │
  ▼                               ▼
netif->linkoutput()           netif->linkoutput()
  │  один большой пакет          │  несколько сегментов
  │  (≤ 64KB)                    │  (≤ MTU)
  ▼                               ▼
e1000_send()                   e1000_send()
  │                               │
  ▼                               ▼
[HW сегментирует]             [SW копирует каждый]
[меньше прерываний]           [больше CPU]
```

**Что требует lwIP:**
- `LWIP_TSO=1` в `lwipopts.h`
- `NETIF_FLAG_TSO` в netif flags
- TCP может отправлять `tcp_mss * 44` (до 64KB) одним `tcp_write()`

**Изменения в lwipopts.h:**
```c
#define LWIP_TSO           1
```
(Проверить, поддерживает ли lwIP 2.2.1 TSO)

**Изменения в e1000 (e1000.c):**
- TX descriptor: `E1000_TX_CMD_TSE` (TCP Segmentation Enable)
- `E1000_TX_CMD_IPCSS`, `E1000_TX_CMD_IPCSO`, `E1000_TX_CMD_TCPCSS` — offset for segmentation
- `desc->mss` = MSS value
- Драйвер копирует заголовок + данные, HW разбивает на сегменты

**Изменения в ethif (ethif.c):**
- `ethif_output()` — не фрагментировать, если TSO включён
- Проверка `netif->flags & NETIF_FLAG_TSO`

**Изменения в ndev (ndev.c):**
- Увеличить `NDEV_IOV_MAX` для больших пакетов (сейчас 8, нужно > 44 для 64KB)
- Или: увеличить grant count в `ndev_transfer()`

**Ожидаемый эффект**: 1.3-2× throughput (меньше прерываний, больше данных за вызов).

**Риски:**
- QEMU e1000 может не поддерживать TSE
- lwIP TSO может быть нестабильным
- Увеличение `NDEV_IOV_MAX` требует больше грантов

---

### 2.4 Sub-phase 2d: Loopback Fast Path

**Цель**: Ускорить loopback — синхронная доставка вместо async очереди.

**Текущая архитектура:**
```
TCP send() → loopif_output()
  → pchain_alloc() + pbuf_copy()      ← копия!
  → enqueue в loopif_head             ← очередь!
  → ... ждёт следующего poll()        ← latency!
  → loopif_poll() → ifdev_input()     ← доставка
```

**Целевая архитектура:**
```
TCP send() → loopif_output()
  → pbuf_ref()                        ← зафиксировать
  → ifdev_input() directly            ← синхронно!
  → pbuf_free() после обработки
```

**Изменения в loopif (loopif.c):**
- Добавить `loopif_fast_output()` — альтернатива для синхронной доставки
- Триггер: если в очереди нет пакетов и не было троттлинга
- Фолбэк: async queue для нагрузки (предотвращение stack overflow)
- Полностью убрать копию pbuf для loopback (использовать pbuf_ref)

**Изменения в ifdev:**
- Возможно нужен флаг `IOP_OUTPUT_FAST` для синхронной доставки
- Если `iop_output` возвращает `ERR_INPROGRESS`, вызывать `iop_input` напрямую

**Ожидаемый эффект**: 2-5× loopback throughput, снижение latency для localhost.

**Риски:**
- Реентерабельность lwIP: `tcp_write → loopif_output → ifdev_input → tcp_input → tcp_output`
- Нужно убедиться, что lwIP не блокируется (no locks, re-entrant обработка)
- Stack depth: может быть большой при вложенных вызовах

---

### 2.5 Sub-phase 2e: Batch Processing in netdriver

**Цель**: Сократить IPC overhead — несколько пакетов за одно сообщение.

**Текущая архитектура:**
```
lwIP → ndev_send(pbuf1) → grant + asynsend → драйвер
lwIP → ndev_send(pbuf2) → grant + asynsend → драйвер
lwIP → ndev_send(pbuf3) → grant + asynsend → драйвер
                                              → 3 IPC сообщения
```

**Целевая архитектура:**
```
lwIP → ndev_send_batch({pbuf1, pbuf2, pbuf3})
  → grant + asynsend → драйвер
                      → 1 IPC сообщение, 3 пакета
```

**Изменения в ndev:**
- Добавить `ndev_send_batch()` — массив pbuf в одном вызове
- Новый тип сообщения `NDEV_SEND_BATCH` (или флаг в `NDEV_SEND`)
- Увеличить `NDEV_IOV_MAX` с 8 до, скажем, 32

**Изменения в netdriver (netdriver.c):**
- Обработка `NDEV_SEND_BATCH` — batch вызов `ndr_send`
- Driver может обработать несколько пакетов за один вызов

**Изменения в e1000 (e1000.c):**
- `e1000_send_batch()` — multiple descriptors за один проход
- Меньше MMIO операций (регистр TDT пишется один раз)

**Изменения в ethif (ethif.c):**
- `ethif_output()` — накопить пакеты, отправить batch
- Триггер: по таймеру или при заполнении квоты

**Ожидаемый эффект**: 1.5-3× throughput (меньше IPC сообщений).

**Риски:**
- Batch сообщение может быть слишком большим для MINIX IPC (ограничение ~64KB)
- Увеличение latency (пакет ждёт заполнения batch)
- Сложнее отладка (больше данных за сообщение)

---

## 3. Приоритеты реализации

| Sub-phase | Эффект | Сложность | Риск | Статус |
|-----------|--------|-----------|------|--------|
| **2a: Checksum Offload + Jumbo** | +10-20% | ★☆☆ | Низкий | **✅ Готово** |
| **2d: Loopback Fast Path** | 2-5× (loopback) | ★★☆ | Средний | 🥇 **Следующая** |
| **2b: Multi-Queue e1000** | 1.5-2× | ★★★ | Высокий | **✅ Готово** |
| **2c: TSO** | 1.3-2× | ★★★ | Высокий | **✅ Готово** (Legacy TSE) |
| **2e: Batch Processing** | 1.5-3× | ★★☆ | Средний | **✅ Готово** |

**Рекомендация**: Начать с **P0** (Checksum Offload + Loopback), затем **P1** (TSO → Multi-queue), затем **P2** (Batch).

---

## 4. Конкретные файлы и LOC

| Sub-phase | Файл | Изменения | LOC |
|-----------|------|-----------|-----|
| **2a** | `minix/drivers/net/e1000/e1000.c` | checksum caps + init | +30 |
| **2a** | `minix/drivers/net/e1000/e1000.h` | E1000_IOBUF_SIZE 2048→9216 | +1 |
| **2a** | `minix/net/lwip/ethif.c` | ETHIF_MAX_MTU 1500→9000 | +5 |
| **2b** | `minix/drivers/net/e1000/e1000.h` | e1000_queue_t, E1000_NUM_QUEUES | +40 |
| **2b** | `minix/drivers/net/e1000/e1000.c` | multi-queue init, RSS, dispatch | +200 |
| **2b** | `minix/net/lwip/ndev.c` | multi-queue support | +80 |
| **2b** | `minix/net/lwip/ethif.c` | flow-based queue selection | +50 |
| **2b** | `minix/lib/libnetdriver/netdriver.c` | multi-queue dispatch | +30 |
| **2c** | `minix/lib/liblwip/lib/lwipopts.h` | LWIP_TSO=1 | +1 |
| **2c** | `minix/drivers/net/e1000/e1000.c` | TSO descriptor setup | +60 |
| **2c** | `minix/drivers/net/e1000/e1000.h` | TSO constants | +10 |
| **2d** | `minix/net/lwip/loopif.c` | fast path + pbuf_ref | +60 |
| **2e** | `minix/net/lwip/ndev.c` | ndev_send_batch | +50 |
| **2e** | `minix/net/lwip/ethif.c` | batch accumulation | +40 |
| **2e** | `minix/lib/libnetdriver/netdriver.c` | batch processing | +30 |
| **2e** | `minix/drivers/net/e1000/e1000.c` | batch send | +30 |

**Итого**: ~700 LOC новых/изменённых (против ~3000 LOC в планировщике).

---

## 5. Тестирование

### 5.1 Checksum Offload (2a)
```
# Тест 1: Проверить, что checksums правильные (tcpdump не жалуется)
iperf3 -c 10.0.2.2 -t 10 -M 9000

# Тест 2: Jumbo frames
ifconfig em0 mtu 9000
ping -s 8000 10.0.2.2
iperf3 -c 10.0.2.2 -t 10 -M 9000

# Тест 3: Regression — test91 (TCP)
test91
```

### 5.2 Multi-Queue (2b)
```
# Тест: Нагрузка на 2+ потоках
iperf3 -c 10.0.2.2 -t 30 -P 4
# Сравнить с baseline (single-queue)
# Ожидание: лучшее распределение по queue, меньше lock contention
```

### 5.3 Loopback Fast Path (2d)
```
# Тест: loopback throughput
iperf3 -c 127.0.0.1 -t 10
# Сравнить с baseline (Phase 0)
# Ожидание: 2-5× improvement
```

### 5.4 Regression
```
test91 (TCP)
test92 (RAW)
test93 (IPv6)
test94 (UDP)
tests/net/arp/
tests/net/icmp/
tests/net/if/
```

---

## 6. Критерии готовности Phase 2

| Метрика | Baseline | Цель | Измерение |
|---------|----------|------|-----------|
| TCP throughput (e1000, single) | ~500-800 Mbps | ≥1 Gbps | `scripts/run_net_bench.sh --tcp-only` |
| TCP throughput (e1000, 4 потоков) | ~500 Mbps | ≥2 Gbps | `iperf3 -P 4` |
| Loopback throughput | ~1-2 Gbps | ≥5 Gbps | `iperf3 -c 127.0.0.1` |
| TCP connect rate | ~1000 conn/s | ≥5000 conn/s | `scripts/run_net_bench.sh --connect-only` |
| Packet latency (ping) | <0.1 ms | <0.05 ms | `ping -c 1000 -f` |
| CPU utilization (iperf) | 1 core 100% | <60% 1 core | `ps -xm` |
| Test91-94 | PASS | PASS | `test91; test92; test93; test94` |
| Jumbo frames (MTU 9000) | N/A | Работает | `ping -s 8000` |
| Memory usage | ~1-2 MB | <4 MB | `ps -xm` |
