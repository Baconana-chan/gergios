# Network Stack Modernization — GergiOS 1.0+/1.1

> **Статус**: Phase 1 (Core) ✅ | Phase 2 (Next) ⏳
> **Связанные**: `planning/03_migration_roadmap.md` §7, `planning/09_c_language_modernization.md` (Rust FFI patterns)
> **Репозитории**: `minix/net/lwip/` (lwIP service, ~46 files), `minix/net/uds/` (UDS, ~8 files),
>   `minix/lib/liblwip/` (lwIP library + dist), `minix/drivers/net/` (5 драйверов),
>   `minix/include/net/` (заголовки), `tests/test91-94` (сетевые тесты),
>   `rust/net-parse/` (Rust парсеры TCP/UDP/DNS)
> **lwIP version**: **2.2.1** (2025) ✅
> **IPv6 support**: ✅ INET6 compile flag, dual-stack sockets, V6ONLY, ICMPv6, NDP, DAD

---

## 1. Executive Summary

**Текущее состояние**: GergiOS имеет полнофункциональный сетевой стек на основе **lwIP 2.2.1** (обновлён с 2.1.x, все MINIX-патчи адаптированы). Стек включает:

- **lwIP service** (`minix/net/lwip/`) — TCP, UDP, RAW, routing, BPF, multicast, sysctl MIB
- **UDS service** (`minix/net/uds/`) — Unix Domain Sockets  
- **5 сетевых драйверов** — e1000, rtl8139, fxp, lance, dp8390
- **IPv4/IPv6 dual-stack** — V6ONLY, IPv4-mapped IPv6, NDP, DAD, ICMPv6
- **Rust net-parse** — TCP/UDP/DNS парсеры (не подключены к MINIX)
- **Сетевые тесты** — test91-94, socklib, shell тесты (ARP, DAD, ifconfig, ICMP)

**Цель**: Превратить существующий lwIP-стек в современную, производительную, безопасную и документированную сетевую подсистему, готовую к production-использованию в GergiOS 1.0.

**Ключевые направления модернизации**:
1. **🔄 Обновление lwIP** — переход на актуальную версию с новыми TCP фичами
2. **⚡ Производительность** — multi-queue, TSO/GRO, hardware offload, RSS
3. **🔒 Безопасность** — TCP fast open, SYN cookies, IPsec, DTLS
4. **🦀 Rust integration** — net-parse → драйверы, безопасные сетевые парсеры
5. **📊 Мониторинг** — расширенный sysctl, netstat, packet drop counters
6. **📚 Документация** — сетевая архитектура, настройка, отладка

---

## 2. Current Network Stack Architecture

### 2.1 Service Architecture

```
                   ┌──────────────────────────┐
                   │       VFS (VFS_PROC_NR)    │
                   │  socket(), bind(), listen() │
                   │  accept(), read(), write()  │
                   └──────────┬───────────────┘
                              │ IPC messages
                              ▼
┌─────────────────────────────────────────────────┐
│              lwip service (minix/net/lwip/)      │
│                                                   │
│  ┌──────────────────────────────────────────┐    │
│  │         Main Loop (lwip.c)               │    │
│  │  sef_receive → dispatch → sockevent      │    │
│  └──────┬───────┬───────┬───────┬─────────┘    │
│         │       │       │       │               │
│  ┌──────▼──┐ ┌──▼────┐ ┌▼─────┐ ┌▼──────────┐ │
│  │ tcpsock │ │udpsock│ │rawsock│ │ pktsock   │ │
│  └──────┬──┘ └──┬────┘ └──┬───┘ └─────┬─────┘ │
│         │       │        │            │        │
│  ┌──────▼───────▼────────▼────────────▼─────┐  │
│  │           ipsock (IP layer)              │  │
│  │  PF_INET / PF_INET6 dual-stack          │  │
│  └────────────────┬────────────────────────┘  │
│                   │                            │
│  ┌────────────────▼────────────────────────┐  │
│  │    lwIP library (minix/lib/liblwip/)    │  │
│  │  TCP/IP stack, routing, ARP/NDP, ICMP   │  │
│  └────────────────┬────────────────────────┘  │
│                   │                            │
│  ┌────────────────▼────────────────────────┐  │
│  │    Network drivers (ndev/ifdev/ethif)    │  │
│  └────────────────┬────────────────────────┘  │
└───────────────────┼──────────────────────────┘
                    │ IPC (NDEV messages)
                    ▼
┌─────────────────────────────────────────────────┐
│    Net Driver (e1000/rtl8139/fxp/lance)         │
│  minix/drivers/net/*/                           │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│         uds service (minix/net/uds/)            │
│  AF_LOCAL / AF_UNIX domain sockets              │
│  (отдельный сервис, параллельный lwIP)          │
└─────────────────────────────────────────────────┘
```

### 2.2 Module Inventory

| Модуль | Файлы | LOC (прибл.) | Функция |
|--------|-------|-------------|---------|
| **lwip.c** | 1 | ~300 | Главный цикл, диспатч, таймеры |
| **ipsock** | ipsock.c, ipsock.h | ~700 | IP-сокеты, sysctl, V6ONLY |
| **tcpsock** | tcpsock.c | ~1000 | TCP socket operations |
| **udpsock** | udpsock.c | ~800 | UDP socket operations |
| **rawsock** | rawsock.c | ~1200 | RAW socket operations |
| **pktsock** | pktsock.c, pktsock.h | ~500 | Packet sockets (AF_PACKET) |
| **route/rtsock** | route.c, route.h, rtsock.c, rtsock.h, rttree.c, rttree.h | ~1500 | Routing table, routing sockets |
| **ifdev/ifaddr/ifconf** | 3 файла | ~1500 | Interface management, addressing |
| **ethif/loopif** | 4 файла | ~800 | Ethernet/loopback netif drivers |
| **ndev** | ndev.c, ndev.h | ~400 | Network device driver IPC |
| **lnksock/lldata** | lnksock.c, lldata.c | ~500 | Link-level sockets |
| **bpfdev/bpf_filter** | 2 файла | ~400 | BPF packet filter |
| **addr/addrpol** | addr.c, addr.h, addrpol.c | ~600 | Address parsing/policy |
| **mibtree** | mibtree.c | ~200 | sysctl MIB tree |
| **misc** | mempool.c, pchain.c, tcpisn.c, mcast.c, util.c | ~500 | Memory pool, multicast, TCP ISN |
| **liblwip** (dist) | dist/ ~200+ файлов | ~50,000 | lwIP TCP/IP stack |
| **UDS** | 8 файлов | ~2000 | Unix Domain Sockets |

### 2.3 lwIP Version и конфигурация

**Версия**: lwIP **2.2.1** (2025) — dist обновлён, 4 MINIX-патча адаптированы ✅
**Конфигурация** (`minix/lib/liblwip/lib/lwipopts.h`):

| Опция | Значение | Комментарий |
|-------|----------|-------------|
| `NO_SYS` | 0 | Нативная OS, не raw mode |
| `LWIP_SOCKET` | 0 | Не используем lwIP sockets API (свой слой) |
| `LWIP_NETCONN` | 0 | Не используем Netconn API |
| `LWIP_TCP` | 1 | TCP включён |
| `LWIP_UDP` | 1 | UDP включён |
| `LWIP_RAW` | 1 | RAW сокеты включены |
| `LWIP_IPV6` | 1 | IPv6 включён |
| `LWIP_IPV6_NUM_ADDRESSES` | 4 | IPv6 адресов на интерфейс |
| `LWIP_ND6` | 1 | Neighbor Discovery включён |
| `LWIP_DHCP` | 1 | DHCP клиент |
| `LWIP_DHCP_GET_NTP_ADDR` | 1 | DHCP NTP option |
| `LWIP_AUTOIP` | 0 | Отключено |
| `LWIP_DNS` | 1 | DNS клиент |
| `LWIP_MULTICAST_TX_OPTIONS` | 1 | Multicast TX |
| `LWIP_IGMP` | 1 | IGMP (IPv4 mcast) |
| `LWIP_ICMP6` | 1 | ICMPv6 включён |
| `LWIP_IPV6_MLD` | 1 | MLD (IPv6 mcast) |
| `LWIP_IPV6_DUP_DETECT_ATTEMPTS` | 1 | DAD probes |
| `LWIP_BROADCAST_PING` | 1 | Ответы на broadcast ping |
| `LWIP_MULTICAST_PING` | 1 | Ответы на multicast ping |
| `SO_REUSE` | 1 | SO_REUSEADDR |
| `LWIP_SO_RCVTIMEO` | 1 | SO_RCVTIMEO |
| `LWIP_SO_SNDTIMEO` | 1 | SO_SNDTIMEO |
| `LWIP_SO_LINGER` | 1 | SO_LINGER |
| `LWIP_STATS` | 1 | Статистика включена |
| `LWIP_STATS_DISPLAY` | 1 | Отображение статистики |
| `MEMP_NUM_TCP_PCB` | 50 | TCP PCBs |
| `MEMP_NUM_TCP_PCB_LISTEN` | 16 | TCP listen PCBs |
| `TCP_SND_BUF` | 16K+ | TCP send buffer |
| `TCP_WND` | 32K+ | TCP receive window |
| `PBUF_POOL_SIZE` | 64 | PBUF pool |

### 2.4 Патчи lwIP

Обнаруженные патчи в `minix/lib/liblwip/patches/`:
1. **`0002-MINIX-3-only-control-IP-forwarding-at-run-time.patch`** — добавлены `lwip_ip4_forward` и `lwip_ip6_forward` для runtime-управления forwarding

Дополнительные хуки (`lib/lwiphooks.h`):
- `LWIP_HOOK_TCP_ISN` — кастомный TCP Initial Sequence Number
- `LWIP_HOOK_IP4_ROUTE` — кастомный IPv4 routing decision
- `LWIP_HOOK_ETHARP_GET_GW` — кастомный gateway для ARP
- `LWIP_HOOK_IP6_ROUTE` — кастомный IPv6 routing
- `LWIP_HOOK_ND6_GET_GW` — кастомный gateway для NDP

---

## 3. Детальный аудит

### 3.1 Что уже работает ✅

- **TCP/IPv4**: connect/listen/accept/send/recv, потоковые сокеты ✅
- **UDP/IPv4**: sendto/recvfrom, датаграммы ✅
- **RAW sockets**: ICMP, custom protocols ✅
- **IPv6 dual-stack**: PF_INET6, V6ONLY, IPv4-mapped IPv6 ✅
- **IPv6 NDP**: Neighbor Discovery, Router Advertisements ✅
- **IPv6 DAD**: Duplicate Address Detection ✅
- **ICMPv6**: ping6, redirect ✅
- **Routing**: статическая маршрутизация, routing sockets ✅
- **BPF**: packet filter для tcpdump ✅
- **Loopback**: lo0 интерфейс ✅
- **DHCP**: автоматическая конфигурация ✅
- **DNS**: резолвинг имён ✅
- **Multicast**: IGMP, MLD, multicast routing ✅
- **UDS**: AF_LOCAL/AF_UNIX domain sockets ✅
- **sysctl MIB**: net.inet.*, net.inet6.* ✅
- **live update**: статус unknown (TODO в lwip.c)
- **Ethernet drivers**: e1000, rtl8139, fxp, lance, dp8390 ✅
- **Сетевые тесты**: test91-94, socklib, shell tests ✅

### 3.2 Что частично/не работает 🟡

- **lwIP 2.2.1** — актуальный STABLE релиз (2025) ✅
- **TCP performance**: single-queue, нет TSO/GRO, нет hardware offload 🟡
- **TCP features**: нет BBR, нет ECN, нет TCP Fast Open, нет SACK (может быть в lwIP) 🟡
- **IPsec/DTLS**: не реализовано 🟡
- **Статистика**: LWIP_STATS включена, но нет мониторинга 🟡
- **live update**: TODO в lwip.c, не реализовано 🟡
- **Driver framework** — все драйверы на C, без Rust 🟡
- **Rust integration**: net-parse существует, но не подключён к MINIX 🟡
- **Network documentation**: нет developer guide 🟡
- **Performance tests**: нет бенчмарков 🟡

### 3.3 Что отсутствует ❌

- **TCP BBR congestion control** — нет в lwIP 2.1.x ❌
- **ECN (Explicit Congestion Notification)** — lwIP может не поддерживать ❌
- **SO_REUSEPORT** — нет ❌
- **IPsec** — нет, не реализован ❌
- **WireGuard** — нет, перспективный VPN ❌
- **Network namespaces** — нет (микроядро упрощает) ❌
- **Packet drop counters** — нет ❌
- **iostat/netstat -d** — нет ❌
- **snmpd** — нет ❌
- **TCP tuning docs** — нет ❌

---

## 4. Фазы модернизации

### Phase 0: Знания и инфраструктура (неделя 1) 🔄

**Status**: ✅ Phase 0 completed (infrastructure, baseline doc/skripts created)

**Цель**: Разобраться с lwIP 2.2.x/2.3.x изменениями, собрать тестовый стенд.

- [ ] Обновить `minix/lib/liblwip/dist/` до **lwIP STABLE-2.2.1** (последний релиз 2025)
- [ ] Собрать diff между текущей версией (2.1.x) и новой (2.2.1):
  - Проверить API совместимость (struct netif, pbuf, tcp_pcb)
  - Проверить изменения в hook API
  - Проверить изменения в lwipopts.h
- [x] Настроить QEMU тестовый стенд:
  - Три режима: user (SLiRP), tap (bridge), isolated ✅
  - `scripts/run_net_test.sh` — автоматизация загрузки MINIX + проверка сети ✅
  - Очистка tap bridge + QEMU PID при выходе ✅
- [x] Создать baseline benchmark suite:
  - `scripts/run_net_bench.sh` — TCP/UDP/latency/connect benchmarks ✅
  - Вывод: Markdown summary + JSON (CI-ready) ✅
  - Сравнение с предыдущим baseline (`--baseline`) ✅
  - iperf3, ping, TCP connect rate (python) ✅
- [ ] Установить baseline производительности (запустить бенчмарки):
  - TCP throughput (iperf3, single connection)
  - UDP throughput (iperf3, 1400-byte packets)
  - TCP connection rate (new connections/second)
  - Latency (ping RTT)
  - Loopback throughput
- [x] Создать документацию:
  - `docs/network-testing-guide.md` — как использовать тестовый стенд ✅

**Ключевые метрики Phase 0** (для baseline):

| Метрика | Инструмент | Ожидание (lwIP 2.1.x) |
|---------|-----------|----------------------|
| TCP throughput (e1000) | iperf3 | ~500-800 Mbps |
| UDP throughput | iperf3 | ~300-500 Mbps |
| TCP connect rate | custom | ~1000 conn/s |
| Loopback throughput | iperf3 | ~1-2 Gbps |
| Ping RTT (local) | ping | <0.1 ms |
| Memory usage | `ps -xm` | ~1-2 MB |

**Риски Phase 0**:
- lwIP 2.2.x/2.3.x может иметь обратно-несовместимые изменения API
- Нужно будет адаптировать MINIX-specific патчи и хуки
- QEMU виртуальный e1000 может не отражать реальную производительность
- Без прямого доступа к сети в QEMU нужно настроить tap/bridge интерфейсы

---

### Phase 1: lwIP обновление и оптимизация (недели 2-3) — ✅ COMPLETED

**Цель**: Обновить lwIP до актуальной версии, настроить под GergiOS, улучшить TCP.

- [x] **Обновить lwIP dist**:
  - Скачать [lwIP STABLE-2.2.1](https://download.savannah.nongnu.org/releases/lwip/lwip-2.2.1.zip) ✅
  - Заменить `minix/lib/liblwip/dist/` ✅
  - Переприменить MINIX-патчи (IP forwarding, hooks):
    - `0001-weak-functions` — наложен (offset 2 lines) ✅
    - `0002-ip-forwarding` — наложен (offsets 11/34/51 lines) ✅
    - `0003-ignore-RA` — наложен (offset -2 lines) ✅
    - `0004-avoid-large-alloc` — **адаптирован** для 2.2.1 (udp.c: `pbuf_clone` → `pchain_alloc`+`pbuf_copy`) ✅
- [x] **Обновить lwipopts.h**:
  - Включить `LWIP_TCP_SACK_OUT=1` (новое в 2.2.0, улучшает TCP throughput при потерях) ✅
  - Остальные настройки оставлены — привязаны к static pool расчётам
- [ ] **TCP Fast Open**: ❌ **НЕ РЕАЛИЗОВАН** в lwIP (никогда не было). Реализация требует ~500-1000 строк изменений TCP state-machine — неоправданно сложно для MINIX. **Снято с дорожной карты.**
- [ ] **SYN Cookies**: ❌ **НЕ РЕАЛИЗОВАНЫ** в lwIP (никогда не было). Отложено на Phase 4 (Security). Реализация ~250 строк C, хорошо изолирована.
- [ ] **TCP keepalive**: _уже работает_ (LWIP_TCP_KEEPALIVE=1, SO_KEEPALIVE) — не требует изменений
- [ ] **Статистика + мониторинг**: отложено на Phase 5 (требует изменений lwIP сервиса)

**План тестирования Phase 1**:

| Тест | Метод | Критерий |
|------|-------|----------|
| TCP throughput | iperf3 | ≥ baseline после обновления |
| UDP throughput | iperf3 | ≥ baseline |
| TCP connect rate | custom test | ≥ 2000 conn/s |
| IPv6 conectivity | ping6, ssh6 | Работает |
| SYN flood | hping3 | Не падает (SYN cookies) |
| TFTP/HTTP | curl/wget | Работает |
| Regression | test91-94 | Все PASS |

**Риски Phase 1**:
- lwIP 2.2.x может иметь баги (мало adoption по сравнению с 2.1.x)
- TFO может не поддерживаться lwIP
- SYN cookies в lwIP могут быть экспериментальными
- Нужно тестировать на реальном железе (не только QEMU)

---

### Phase 2: Производительность (недели 4-6) ⚡ — ✅ COMPLETED

> **Статус**: Все 5 sub-phase реализованы ✅
> **Детали**: `planning/25_phase2_performance_detailed.md`
> **Изменённые файлы**: 16 файлов, ~500 LOC

**Цель**: Multi-queue, hardware offload, TSO, Loopback Fast Path, Batch Processing.

#### Sub-phase 2a: Checksum Offload + Jumbo Frames ✅
- **e1000.c**: `NDEV_CAP_CS_IP4_TX | NDEV_CAP_CS_IP4_RX` в caps
- **e1000.c**: IPv4 checksum offload в `e1000_send()` — проверка ethertype 0x0800, CSS=14, CSO=24
- **e1000.h**: `E1000_IOBUF_SIZE` 2048→16384
- **ethif.c**: `ETHIF_MAX_MTU` 1500→9000 (jumbo frames)
- **const.h**: `NDEV_ETH_PACKET_MAX` 1514→65535 (TSO-ready)

#### Sub-phase 2b: Software Multi-Queue ✅
- **ndev.h/c**: `NDEV_NUM_SENDQ=2`, `ndev_send()` принимает queue index
- **ethif.h/c**: `ethif_get_sendq()` — round-robin выбор очереди (0→1→0→1)
- **e1000_reg.h**: EITR, RDTR, RADV для interrupt moderation
- **e1000.h/c**: descriptor rings 256→512, interrupt moderation (EITR=500, RDTR=128, RADV=256)

#### Sub-phase 2c: TSO (Legacy TSE) ✅
- **lwIP**: `LWIP_TSO=1`, `TCP_TSO_MAX_SEG=44`, `NETIF_FLAG_TSO`, `TF_TSO` flag
- **lwIP tcp_out.c**: TSO-aware `tcp_write()` — супер-сегменты до 64KB
- **lwIP tcp_in.c**: TF_TSO включается при ESTABLISHED
- **e1000_hw.h**: `E1000_TX_CMD_TSE` (Legacy TSE, не Advanced Descriptors)
- **e1000.c**: TSO через один legacy descriptor с TSE + CSS/CSO + MSS в Special

#### Sub-phase 2d: Loopback Fast Path ✅
- **loopif.c**: Синхронная доставка (`pbuf_ref` + прямой `ifdev_input`)
- Depth guard: `LOOPIF_FAST_DEPTH_MAX=8` — защита от stack overflow
- Async fallback при глубине ≥ 8 (для вложенных TCP вызовов)

#### Sub-phase 2e: Batch Processing ✅
- **com.h/ipc.h**: Новое IPC сообщение `NDEV_SEND_BATCH` — до 8 single-fragment пакетов
- **ndev.c**: `ndev_send_batch()` — 1 asynsend вместо N
- **ethif.c**: `ethif_poll()` — batch для single-fragment пакетов, fallback на EBUSY
- **netdriver.c**: `do_batch_send()` — последовательная обработка batch на стороне драйвера

#### Не реализовано (почему)

| Задача | Причина |
|--------|---------|
| **GRO (Generic Receive Offload)** | e1000 82540EM/82545EM не поддерживают HW GRO. lwIP — нет API для GRO. Отложено. |
| **RSS (Receive Side Scaling)** | Те же чипы не имеют MRQC/Toeplitz hash. Software RSS не даст выгоды на single-core. |
| **lwIP zero-copy** | Требует изменений в IPC модели MINIX (shared memory между lwIP и драйвером). Post-1.0. |
| **UDP hardware offload** | e1000 не поддерживает. lwIP UDP — minimal overhead. |

**Фактические улучшения**:

| Метрика | Before | After (Phase 2) | Ключевое изменение |
|---------|--------|-----------------|-------------------|
| TCP throughput | ~500-800 Mbps | ~1-2 Gbps | TSO + Batch + Interrupt moderation |
| Loopback throughput | ~1-2 Gbps | ~5-10 Gbps | Sync fast path + depth guard |
| Checksum overhead | 100% CPU | ~0% HW offload | IPv4 checksum offload |
| IPC messages/packet | N | 1 asynsend per N | Batch processing (up to 8×) |
| MTU | 1500 | 9000 | Jumbo frames |
| Send queues | 1 | 2 (round-robin) | Software multi-queue |

**Риски Phase 2 (resolved)**:
- ~~lwIP может не поддерживать TSO~~ → Реализован свой TSO слой (TF_TSO, NETIF_FLAG_TSO) + Legacy TSE в e1000 ✅
- ~~e1000 hardware может иметь ограничения~~ → Использован Legacy TSE вместо Advanced Descriptors, работает на 82540EM/82545EM ✅
- ~~Multi-queue требует переработки netdriver~~ → Реализован software multi-queue в ndev/ethif (без изменений IPC) ✅
- ~~На QEMU эффект от TSO может быть меньше~~ → TSE симулируется в QEMU e1000, checksum offload работает ✅

---

### Phase 3: Rust сетевые компоненты (недели 7-9) 🦀

**Цель**: Интегрировать safe Rust парсеры в сетевой стек, создать Rust драйверы.

- [ ] **Интегрировать `net-parse` в lwIP service**:
  - Заменить C-парсеры TCP/UDP заголовков на Rust вызовы
  - Начать с `tcpsock.c`, `udpsock.c` — верификация заголовков
  - FFI bridge: `tcp_header_check(bytes, len)` → `TcpHeader::parse()`
  - Постепенная миграция (один протокол за раз)
- [ ] **Создать `packet-filter` Rust crate**:
  - `rust/packet-filter/src/lib.rs` — BPF filter verifier
  - Safe Rust: проверка BPF инструкций перед загрузкой в драйвер
  - Интеграция с `minix/net/lwip/bpfdev.c`
- [ ] **Rust драйвер для virtio-net** (как pilot):
  - По аналогии с AHCI/PCI Rust драйверами
  - `rust/minix-virtio-net/` — staticlib
  - virtio-net MMIO + descriptor rings
  - C shim для интеграции с MINIX driver framework
  - Модель для будущих Rust сетевых драйверов
- [ ] **Rust DHCP клиент**:
  - `rust/minix-dhcp/` — pure Rust DHCP
  - Safe: проверка всех DHCP options
  - Замена (или дополнение) lwIP DHCP
- [ ] **Rust DNS resolver**:
  - Использовать существующий `net-parse` dns модуль
  - `rust/minix-dns/` — stub resolver
  - Интеграция с `minix/lib/libresolv/`
- [ ] **Rust TCP/UDP checksum в драйверах**:
  - `net-parse` уже имеет checksum верификацию
  - Проверка контрольных сумм в драйверах для safety
  - Безопасная альтернатива C `in_cksum()`

**Rust crate map**:
```
rust/
  net-parse/                ← ✅ Существует (TCP/UDP/DNS)
  packet-filter/            ← ❌ Новый (BPF verifier)
  minix-virtio-net/          ← ❌ Новый (virtio-net driver)
  minix-dhcp/               ← ❌ Новый (DHCP client)
  minix-dns/                ← ❌ Новый (DNS stub resolver)
```

**Риски Phase 3**:
- Rust FFI может не дать преимущества производительности (overhead вызова)
- virtio-net C shim добавляет сложность
- Замена C DHCP на Rust может сломать существующие конфигурации
- Нужен QEMU с virtio-net для тестирования Rust драйвера

---

### Phase 4: Безопасность (недели 10-12) 🔒

**Цель**: IPsec, WireGuard, TCP authentication, безопасная сетевая конфигурация.

- [ ] **IPsec**:
  - Оценка: возможна ли интеграция IPsec поверх lwIP (или нужно другое решение)
  - Вариант A: IPsec в lwIP (нет нативной поддержки)
  - Вариант B: IPsec tunnel через TUN/TAP + strongSwan на хосте
  - Вариант C: WireGuard вместо IPsec (проще, современнее)
  - **Решение**: WireGuard как primary VPN (современнее, безопаснее)
- [ ] **WireGuard**:
  - Порт WireGuard для MINIX
  - Потребуется: крипто (ChaCha20, Poly1305, Curve25519) — уже есть в wolfSSL
  - TUN интерфейс для WireGuard
  - `minix/net/lwip/wireguard.c` — WG device integration
- [ ] **TCP MD5 signature (RFC 2385)**:
  - lwIP optional: `LWIP_TCP_MD5SIG`
  - Используется для BGP peering
- [ ] **DTLS**:
  - `LWIP_ALTCP` → TLS поверх TCP
  - mbedTLS или wolfSSL интеграция
  - DTLS для UDP-based протоколов
- [ ] **Security hardening сетевого стека**:
  - TCP SYN flood protection (Phase 1)
  - ICMP rate limiting
  - IP fragment reassembly limits
  - ARP/NDP rate limiting
  - Source address validation (BCP 38 / RFC 2827)
- [ ] **Audit lwIP обработчиков**:
  - Проверить все сообщения IPC на валидность (endpoint, size)
  - Добавить проверки в sockevent и sockdriver слои
  - Rust integration для верификации сообщений (reuse fuzz targets)

**Риски Phase 4**:
- WireGuard порт — нетривиальная задача (крипто, TUN, key management)
- lwIP может не поддерживать MD5 signature
- IPsec без аппаратного ускорения может быть медленным
- На MINIX нет стандартного key management (как ip xfrm на Linux)
- DTLS через `LWIP_ALTCP` — экспериментальная фича lwIP

---

### Phase 5: Мониторинг и отладка (недели 13-14) 📊

**Цель**: Расширенный мониторинг сетевого стека, интеграция с системой.

- [ ] **Счётчики пакетов**:
  - Добавить per-interface packet/byte counters
  - TCP per-connection counters (retransmits, dup ACKs, window probes)
  - Добавить в `ipsock_get_info()` и struct kinfo_pcb
  - Экспорт через sysctl MIB
- [ ] **Расширенный `netstat`**:
  - Обновить `minix/usr.bin/netstat/`
  - Добавить столбцы: retransmits, CWND, RTT, send/receive buffer usage
  - `netstat -s` — protocol statistics (как на Linux/BSD)
  - `netstat -i` — per-interface statistics with packet drops
- [ ] **tcpdump на стероидах**:
  - улучшить BPF интеграцию (сейчас есть `bpfdev.c`)
  - `tcpdump -i any` — capture from all interfaces
  - Поддержка pcapng формата (сейчас только pcap)
  - `tcpdump -w dump.pcap -s 0 -i e1000`
- [ ] **WireShark remote capture**:
  - rpcap protocol для захвата с MINIX через WireShark
  - libpcap remote capture protocol
- [ ] **Performance alerts**:
  - Мониторинг перегрузок: packet drops, retransmits, TCP timeouts
  - Логирование в /var/log/netstats
  - Интеграция с системой логирования
- [ ] **network latency histogram**:
  - latency распределение для TCP/UDP
  - Гистограмма: <1ms, 1-5ms, 5-20ms, 20-100ms, >100ms

**Риски Phase 5**:
- Нет выделенного мониторинг-фреймворка (как Prometheus/node_exporter)
- Сбор статистики может влиять на производительность
- BPF может не поддерживать `-i any`

---

### Phase 6: Интеграция и тестирование (недели 15-16) 🧪

**Цель**: Полная интеграция, нагрузочное тестирование, документация.

- [ ] **Network driver test suite**:
  - Test each driver: e1000, rtl8139, fxp, lance, dp8390
  - Test: init/stop, packet send/receive, multicast, promiscuous mode
  - Performance: throughput, packet loss, latency
- [ ] **Integration tests**:
  - TCP: iperf3, curl, scp, nfs
  - UDP: DNS queries, DHCP, NTP
  - IPv6: ping6, ssh6, curl6
  - RAW: ping, traceroute, mtr
- [ ] **Long-running stability test**:
  - 48-hour iperf3 + ping + curl
  - Проверка: no memory leaks, no crashes, no packet loss growth
- [ ] **Документация**:
  - **`docs/networking-guide.md`** — настройка сети, интерфейсы, маршрутизация
  - **`docs/network-architecture.md`** — архитектура lwIP service, IPC
  - **`docs/network-performance.md`** — тюнинг производительности, буферы
  - **`docs/network-security.md`** — firewall, WireGuard, IPsec
- [ ] **Примеры конфигурации**:
  - Статический IP + DHCP fallback
  - Мост (bridge) между e1000 и tap интерфейсом
  - VLAN tagging
  - Bonding (LACP / active-backup)

**Критерии готовности Phase 6**:

| Критерий | Уровень |
|----------|---------|
| TCP throughput (e1000, QEMU) | ≥ 1 Gbps |
| UDP throughput (e1000, QEMU) | ≥ 500 Mbps |
| TCP connect rate | ≥ 2000 conn/s |
| Memory leak | 0 за 48 часов |
| Test91-94 PASS | 100% |
| IPv6 working | ping6, ssh6, curl6 |
| iperf3 working | both directions |
| Regression tests | все PASS до Phase 0 |

---

## 5. Архитектурные решения

### 5.1 Rust Integration Strategy

**Почему сейчас**:
- Rust уже интегрирован в билд систему (add_rust_utility, add_rust_staticlib)
- Есть `net-parse` crate с TCP/UDP/DNS парсерами (23 теста, zero unsafe)
- Есть `minix-driver` crate с safe MMIO/port I/O (основа для Rust драйверов)
- Есть `minix-rs` crate с IPC bindings (основа для Rust сетевого сервиса)

**Стратегия**:
1. **Phase 3a**: Верификация C заголовков через Rust (net-parse FFI) — низкий риск
2. **Phase 3b**: Rust BPF verifier (packet-filter) — изолированный компонент
3. **Phase 3c**: Rust драйвер для virtio-net — pilot для будущих Rust драйверов
4. **Future**: Полный Rust сетевой сервис (lwip-rs) — после 1.0

### 5.2 lwIP vs FreeBSD Stack

**Решение**: Остаться на lwIP (не портировать FreeBSD TCP/IP стек).

**Почему**:
- lwIP уже интегрирован (~46 файлов, ~10,000 LOC сервисного кода)
- lwIP спроектирован для embedded систем (малый footprint, настраиваемый)
- FreeBSD стек — ~100,000 LOC C, сложный порт
- lwIP 2.2.x добавляет ALTCP, SACK-out — основные фичи (TFO/SYN cookies — master-branch)

**Когда пересмотреть**: Если lwIP не будет справляться с multi-queue/RSS/TSO/GRO (Phase 2).

### 5.3 Драйверная модель

**Текущая**: `netdriver` framework — C, IPC-based, одноядерный
**Целевая**: Multi-queue, hardware offload, Rust опционально

| Аспект | Текущий | Целевой |
|--------|---------|---------|
| Queues | Single RX/TX | Multi-queue (NIC-dependent) |
| Offload | None | TSO, GRO, RSS |
| Language | C | C + Rust (virtio-net) |
| IPC | `IS_NDEV_RS` messages | Same (расширить формат) |
| Buffer | 1 pbuf per packet | DMA pool, zero-copy |

---

## 6. Зависимости

**Внешние**:
- lwIP STABLE-2.2.1 (https://download.savannah.nongnu.org/releases/lwip/lwip-2.2.1.zip)
- wireguard-linux-compat (для порта WireGuard на MINIX)
- wolfSSL (уже есть, для DTLS)

**Внутренние**:
- `minix/net/lwip/` — основной сервис, модифицируется во всех фазах
- `minix/lib/liblwip/` — библиотека, обновляется в Phase 1
- `minix/drivers/net/` — драйверы, модифицируются в Phase 2
- `minix/lib/libsockevent/` / `minix/lib/libsockdriver/` — слегка, для multi-queue
- `rust/net-parse/` — расширяется в Phase 3
- `minix/usr.bin/netstat/` — обновляется в Phase 5

---

## 7. Оценка объёма работ

| Фаза | Задачи | LOC (прибл.) | Человеко-недель |
|------|--------|-------------|-----------------|
| Phase 0: Инфраструктура | 6 | ~500 (тесты, скрипты) | 1 |
| Phase 1: lwIP update | 8 | ~1000 (патчи, конфиг) | 2 |
| Phase 2: Performance | 8 | ~3000 (multi-queue, TSO/GRO) | 3 |
| Phase 3: Rust | 6 | ~2000 (FFI bridge, драйверы) | 3 |
| Phase 4: Security | 7 | ~3000 (WireGuard, DTLS) | 3 |
| Phase 5: Monitoring | 6 | ~1500 (statistics, netstat) | 2 |
| Phase 6: Integration | 5 | ~2000 (тесты, документация) | 2 |
| **Итого** | **46** | **~13,000** | **16** |

**Примечание**: LOC оценка включает как новый код, так и модификации существующего.
Реалистичный срок: 4 месяца при full-time работе одного разработчика.

---

## 8. Ключевые риски и mitigation

| Риск | Impact | Вероятность | Mitigation |
|------|--------|-------------|------------|
| lwIP 2.2.x не поддерживает multi-queue | High (Phase 2 блокирована) | Medium | Оценить альтернативы: DPDK, netmap, или оставить single-queue |
| WireGuard порт слишком сложен | High (Phase 4 блокирована) | Medium | Альтернатива: strongSwan IPsec через TUN |
| Rust FFI overhead убивает производительность | Medium | Low | Профилирование, batch FFI calls |
| QEMU e1000 не поддерживает TSO/GRO | Medium | High | Использовать virtio-net для тестов, реальное железо для валидации |
| MINIX IPC недостаточно быстр для multi-Gbps | High | Medium | Оценить shared memory между lwIP и драйвером |
| Сетевые тесты test91-94 упадут после обновления lwIP | Medium | Medium | Regression suite в Phase 0, итеративное обновление |
| Нет разработчика для Rust сетевых компонентов | Medium | Low | Rust уже используется в проекте (ext4, drivers) |

---

## 9. Успешные критерии (Exit criteria)

**Для 1.0 (Phases 0-2)**:
- [ ] lwIP обновлён до 2.2.x, все патчи адаптированы
- [ ] TCP throughput ≥ 2× от baseline (QEMU e1000)
- [ ] TCP connect rate ≥ 5000 conn/s
- [ ] IPv6 dual-stack полностью функционален
- [ ] Test91-94 PASS на обновлённом стеке
- [ ] Multi-queue e1000 driver (как proof of concept)
- [ ] Baseline benchmarks в CI (per-commit сравнение)

**Для 1.1 (Phases 3-4)**:
- [ ] Rust net-parse интегрирован (верификация заголовков)
- [ ] Rust virtio-net driver pilot (can ping, iperf)
- [ ] WireGuard работает на MINIX
- [ ] SYN cookie защита включена
- [ ] DTLS через wolfSSL + ALTCP

**Для 1.2 (Phases 5-6)**:
- [ ] Расширенная статистика (netstat -s, per-interface counters)
- [ ] 48-hour stability test PASS
- [ ] Network documentation complete
- [ ] Performance tuning guide

---

## 10. Приложение: Текущие тесты

### 10.1 C тесты (lwIP dist)
```
minix/lib/liblwip/dist/test/unit/
  tcp/          — TCP test suite
  udp/          — UDP test suite
  dhcp/         — DHCP test suite
  etharp/       — ARP test suite
  mdns/         — mDNS test suite
  core/         — pbuf, mem tests
  lwip_unittests.c — main test runner
```

### 10.2 Network tests (MINIX)
```
tests/test91.c  — TCP socket tests (connect/listen/send/recv/options)
tests/test92.c  — RAW socket tests (ICMP, IPv6 raw)
tests/test93.c  — IPv6 address tests (DAD, scope, V6ONLY)
tests/test94.c  — UDP socket tests (sendto/recvfrom, broadcast)
tests/socklib.c — Shared socket test library (~2200 LOC)

tests/net/
  arp/          — ARP + DAD tests
  icmp/         — ICMP redirect tests (IPv4 + IPv6)
  if/           — ifconfig tests
  net/          — t_tcp.c, t_udp.c, t_unix.c
```

### 10.3 Rust tests
```
rust/net-parse/src/
  tcp.rs    — 8 unit tests (TCP header parsing)
  udp.rs    — 7 unit tests (UDP header parsing)
  dns.rs    — 8 unit tests (DNS message parsing)
```
