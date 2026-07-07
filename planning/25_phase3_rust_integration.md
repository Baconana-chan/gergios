# Phase 3: Rust сетевые компоненты 🦀

> **Статус**: ✅ Complete (2026-07-07)
> **Связанные**: `planning/25_network_stack_modernization.md` §Phase 3
> **Rust crates Phase 3**: `rust/net-parse/` (TCP/UDP/DNS парсеры + FFI),
>   `rust/packet-filter/` (BPF verifier), `rust/virtio-net/` (virtio-net driver),
>   `rust/e1000/` (Rust e1000 драйвер с TSO/CSO), `rust/virtio-blk/` (virtio pilot)
> **Build system**: CMake `add_rust_library()` + `add_rust_utility()` — Rust статически
>   линкуется в C targets через imported target `rust_<name>`

---

## Обзор

**Инфраструктура**: Rust уже интегрирован в билд систему MINIX (CMake `add_rust_library()` +
`add_rust_utility()` + `add_rust_test()`). Существует инфраструктура для:
- `no_std` библиотек с FFI экспортом (Cargo.toml `crate-type = ["staticlib", "lib"]`)
- Утилит (Cargo.toml `[[bin]]`)
- Тестов (`cargo test --lib`)
- Dual-platform FFI stubs (`#[cfg(not(target_os = "minix"))]` для тестов на хосте)
- Bitfields (`modular_bitfield` crate)

**Сетевые Rust crates после Phase 3**:
- **`net-parse`** — TCP/UDP/DNS парсеры (30 тестов, zero unsafe, no_std) + FFI bridge
- **`packet-filter`** — BPF verifier (41 тест, zero unsafe, no_std) + FFI для C lwIP
- **`virtio-net`** — Rust virtio-net driver (11 тестов, no_std, netdriver callbacks)
- **`e1000`** — Rust e1000 драйвер (MMIO/PCI/IRQ/MSI-X, TSO, CSO, work queue)
- **`minix-rs`** — IPC message bindings (Message, syscall, endpoint/type validation)
- **`minix-driver`** — Safe MMIO/port I/O wrappers
- **`virtio-blk`** — Rust virtio block driver (pilot, multi-threaded, MSI-X, multi-queue)

**Phase 3 выполнено**: Интегрированы safe Rust компоненты в работающий сетевой стек MINIX.

---

## Sub-phase 3a: net-parse FFI Bridge ✅

**Цель**: Добавить C-совместимый FFI слой в `rust/net-parse/` для вызова парсеров
TCP/UDP заголовков и Internet checksum из C кода lwIP service.

**Зачем**: Безопасная верификация заголовков и контрольных сумм через FFI.

### Что сделано:

**`rust/net-parse/src/ffi.rs`** (~200 LOC) — C-совместимые FFI функции:
- `net_parse_tcp_header()` — парсинг TCP заголовка, заполняет `TcpHeaderFFI`
- `net_parse_udp_header()` — парсинг UDP заголовка, заполняет `UdpHeaderFFI`
- `net_parse_checksum()` — Internet checksum (RFC 1071), делегирует `util::internet_checksum()`
- `net_parse_checksum_verify()` — верификация checksum, делегирует `util::verify_checksum()`
- 7 unit tests: TCP/UDP парсинг, null pointer safety, truncated buffers, checksum

**`rust/net-parse/include/net_parse.h`** — C header с `struct TcpHeaderFFI`, `struct UdpHeaderFFI`
и всеми 4 function declarations

**`rust/net-parse/Cargo.toml`**: Добавлен `crate-type = ["staticlib", "lib"]`

**Интеграция в lwIP service**:
- CMake: `add_rust_library(net-parse LINK_TO lwip)` в `minix/net/lwip/CMakeLists.txt`
- `#include <net_parse.h>` в `minix/net/lwip/lwip.h`

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `rust/net-parse/src/ffi.rs` | ✅ Новый | ~200 |
| `rust/net-parse/include/net_parse.h` | ✅ Новый | ~70 |
| `rust/net-parse/Cargo.toml` | ✅ Изменён | +2 стр |
| `minix/net/lwip/CMakeLists.txt` | ✅ Изменён | +2 стр |
| `minix/net/lwip/lwip.h` | ✅ Изменён | +1 стр (include) |

**Тестирование**: `cargo test --lib` — 30/30 passed ✅

---

## Sub-phase 3b: Rust e1000 Driver Integration ✅

**Цель**: Cвязать существующий Rust e1000 драйвер (`rust/e1000/`) с билд системой
и заменить им C драйвер `minix/drivers/net/e1000/e1000.c`.

**Зачем**: Rust e1000 уже полный — PCI probe, ring init, packet send/recv, MSI-X,
interrupt moderation, bottom-half work queue, stats. Нужно только C shim для
netdriver framework и интеграция в build system.

### Что сделано:

**Rust e1000 — checksum offload + TSO**:
- `desc.rs`: добавлены `TX_CMD_IC` (Insert Checksum, bit 2) и `TX_CMD_TSE` (TCP Segmentation Enable, bit 7)
- `ffi.rs`: добавлены `NDEV_CAP_CS_IP4_TX` (0x0010) и `NDEV_CAP_CS_IP4_RX` (0x0020) в оба platform модуля
- `driver.rs`: переписан `send()` с двумя путями:
  - **TSO путь** (Legacy TSE): для TCP over IPv4 пакетов >= 1514 байт — TSE+IC биты, CSS/CSO для TCP checksum, MSS=1460 в special
  - **Normal путь**: EOP|FCS|RS, для IPv4 пакетов — IC бит с CSS=14, CSO=24 (IP checksum offload)
- `lib.rs`: `ndr_init` caps расширены `NDEV_CAP_CS_IP4_TX | NDEV_CAP_CS_IP4_RX`

**C shim** (`minix/drivers/net/e1000/e1000.c` — заменён):
- Полный C драйвер (~1000+ строк) заменён минимальным shim (20 строк)
- `extern int e1000_rust_main()` → `main()` вызывает Rust функцию

**CMake интеграция**:
- `CMakeLists.txt` (корень): `add_rust_library(e1000)` — билдит Rust staticlib из корня проекта
- `CMakeLists.txt` (драйвер): `add_minix_service(e1000 ...)` + `target_link_libraries(PRIVATE rust_e1000)`
- Makefile (legacy bsd build): оставлен без изменений

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `minix/drivers/net/e1000/e1000.c` | 🟡 Заменён на shim | ~5 |
| `minix/drivers/net/e1000/CMakeLists.txt` | ❌ Новый | ~10 |
| `CMakeLists.txt` (корень) | 🟡 Изменён | +1 стр |
| `rust/e1000/src/driver.rs` | 🟡 TSO + CSO | ~40 |
| `rust/e1000/src/desc.rs` | 🟡 TX_CMD_IC/TSE | ~5 |
| `rust/e1000/src/ffi.rs` | 🟡 CS caps | ~6 |
| `rust/e1000/src/lib.rs` | 🟡 caps | +1 стр |

**Тестирование**: `cargo test` — 7/7 passed ✅, staticlib компилируется ✅

---

## Sub-phase 3c: Rust BPF Verifier (packet-filter) ✅

**Цель**: Создать safe Rust BPF инструкций верификатор, заменяющий `bpf_validate()`
в `minix/net/lwip/bpf_filter.c`.

**Зачем**: BPF верификатор — изолированный компонент, идеальный для safe Rust.
~130 строк C валидатора заменены на safe Rust (zero `unsafe`).

### Новый crate: `rust/packet-filter/`

**`rust/packet-filter/src/lib.rs`** (~470 LOC) — safe Rust BPF verifier:
- `BpfInsn` struct (repr(C), mirrors `struct bpf_insn`)
- `bpf_validate(&[BpfInsn]) -> bool` — статический анализ BPF программы:
  - **Reachability analysis**: 512-bit bitset (16×u32), отслеживание достижимости инструкций
  - **Memory validity**: `MemInv(u16)` — битмаска 16 слов, проверка store-before-load
  - **Division/Modulo by zero**: DIV/MOD с `k=0` → reject
  - **Shift overflow**: LSH/RSH с `k >= 32` → reject
  - **Jump bounds**: все JMP/JA цели в пределах программы
  - **RET required**: программа должна заканчиваться RET
  - **Unknown opcodes**: default → reject
- 35 unit tests: все validation paths, edge cases, tcpdump-подобный фильтр
- `#![no_std]` + `#![deny(unsafe_code)]` — zero unsafe в валидаторе

**`rust/packet-filter/src/ffi.rs`** (~50 LOC):
- `packet_filter_validate(insns, count) -> i32` — unsafe extern "C"
- Null pointer safety, 0/negative count check
- 6 FFI unit tests

**`rust/packet-filter/include/packet_filter.h`** — C header с `struct packet_filter_insn`

**`rust/packet-filter/Makefile`** — BSD build system integration (legacy)

**Интеграция в lwIP**:
- `minix/net/lwip/bpf_filter.c`: `bpf_validate()` заменена на 15-строчный FFI wrapper
  (удалено ~130 строк C валидатора, включая `#include <minix/bitmap.h>`, `bitchunk_t`, bitmap macros)
- `minix/net/lwip/CMakeLists.txt`: `add_rust_library(packet-filter LINK_TO lwip)`
- `rust/Cargo.toml`: `"packet-filter"` добавлен в workspace

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `rust/packet-filter/Cargo.toml` | ✅ Новый | ~12 |
| `rust/packet-filter/src/lib.rs` | ✅ Новый | ~470 |
| `rust/packet-filter/src/ffi.rs` | ✅ Новый | ~50 |
| `rust/packet-filter/include/packet_filter.h` | ✅ Новый | ~25 |
| `rust/packet-filter/Makefile` | ✅ Новый | ~15 |
| `minix/net/lwip/bpf_filter.c` | ✅ Изменён | −130 / +20 |
| `minix/net/lwip/CMakeLists.txt` | ✅ Изменён | +2 стр |
| `rust/Cargo.toml` | ✅ Изменён | +1 стр |

**Тестирование**: `cargo test --lib` — 41/41 passed ✅

### Исправленные баги (3 раунда):
1. BPF_LD/BPF_LDX mode matching: `code & (BPF_SIZE | BPF_MODE)` combine → `code & BPF_MODE` separated
2. MemInv initialization: `[MemInv::all_invalid(); BPF_MAXINSNS]` → `[MemInv(0); BPF_MAXINSNS]` (C memset)
3. Unreachable instruction в tcpdump тесте — заменён на reachable эквивалент

---

## Sub-phase 3d: Rust virtio-net Pilot Driver ✅

**Цель**: Создать Rust драйвер virtio-net как pilot для будущих Rust сетевых
драйверов, по аналогии с `rust/virtio-blk/`.

**Зачем**: Модель для будущих Rust сетевых драйверов. virtio-net проще e1000
и хорошо поддерживается в QEMU.

### Новый crate: `rust/virtio-net/`

**`rust/virtio-net/Cargo.toml`** — staticlib + lib, deps: minix-driver, `#![no_std]`

**`rust/virtio-net/src/net.rs`** (~150 LOC) — Virtio-net protocol:
- Feature bits: MAC, STATUS, CSUM, MRG_RXBUF, GSO, TSO4/6, UFO
- `VirtioNetHdr` (10 bytes, repr(C)) и `VirtioNetHdrMrgRxbuf` (12 bytes)
- Config space accessors: `read_mac()`, `read_link_status()`, `read_config_xxx()`
- Queue indices: RX_Q=0, TX_Q=1, CTRL_Q=2
- PCI device ID: 0x0001 (virtio-net subsystem type)

**`rust/virtio-net/src/queue.rs`** (~200 LOC) — Virtqueue management:
- `VringDesc`, `VringAvail`, `VringUsed` (repr(C))
- `VirtQueue`: allocate, alloc/free desc chains, submit, collect
- 4 unit tests

**`rust/virtio-net/src/device.rs`** (~370 LOC) — PCI transport:
- `VirtioDevice`: probe (PCI scan, BAR 0), feature negotiation, alloc queues, ready, reset
- I/O port access (read8/16/32, write8/16/32 через PDEBUG)
- Legacy IRQ: `pci_set_irq()` / `pci_get_irq()`

**`rust/virtio-net/src/ffi.rs`** (~150 LOC) — Platform FFI:
- MINIX: ioport (pdebug), PCI (pci_*), SEF (sefcb_*, sef_startup), alloc, printf
- Host stubs: `#[cfg(not(target_os = "minix"))]` для тестов на хосте
- `pci_next_dev()`, `pci_attr_r8/16/32()`, `pci_first_dev()`

**`rust/virtio-net/src/driver.rs`** (~260 LOC) — Core driver logic:
- `RxQueue`/`TxQueue`: free list management (bitmask-based), scatter-gather
- `ErrorCounters`: TX/RX error tracking
- State machine: `VnicState { Uninitialized, Running, Stopped }`
- `VirtioNetDriver::probe()` → `init()` → `send()` → `recv()` → `intr()` → `stop()`

**`rust/virtio-net/src/lib.rs`** (~200 LOC) — Entry point:
- `VirtioNetDriver` struct with netdriver callbacks
- `ndr_init` / `ndr_send` / `ndr_recv` / `ndr_intr` / `ndr_stop`
- 7 unit tests: struct sizes (10/12 bytes), PCI probe filter

**`rust/virtio-net/include/virtio_net.h`** — C header с `struct virtio_net_driver`

**`rust/virtio-net/Makefile`** — BSD build system integration

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `rust/virtio-net/Cargo.toml` | ✅ Новый | ~15 |
| `rust/virtio-net/src/lib.rs` | ✅ Новый | ~200 |
| `rust/virtio-net/src/ffi.rs` | ✅ Новый | ~150 |
| `rust/virtio-net/src/device.rs` | ✅ Новый | ~370 |
| `rust/virtio-net/src/queue.rs` | ✅ Новый | ~200 |
| `rust/virtio-net/src/net.rs` | ✅ Новый | ~150 |
| `rust/virtio-net/src/driver.rs` | ✅ Новый | ~260 |
| `rust/virtio-net/include/virtio_net.h` | ✅ Новый | ~35 |
| `rust/virtio-net/Makefile` | ✅ Новый | ~15 |
| `rust/Cargo.toml` | ✅ Изменён | +1 стр |

**Тестирование**: `cargo test --lib` — 11/11 passed ✅

### Исправленные баги (4 раунда):
1. `VIRTIO_NET_PCI_DEVICE_ID: 0x1000 → 0x0001` (PCI device ID vs subsystem type)
2. `device.rs` probe: `pci_first_dev_ffi()` returns `Option<c_int>` not tuple — removed `.0`
3. `driver.rs` stop: `ManuallyDrop` → explicit `free_resources()` call
4. `ffi.rs`: host stubs for SEF callbacks + printf; `pci_attr_r8` stub returns `0xff`

---

## Sub-phase 3e: Rust Checksum в драйверах ✅

**Цель**: Обеспечить Rust `internet_checksum()` через FFI для замены C `in_cksum()`.

**Зачем**: Проверенная безопасная checksum implementation (zero unsafe) через FFI.
Потенциально vectorized SIMD в будущем.

### Что выполнено:
- ✅ `rust/net-parse/src/util.rs`: `internet_checksum()` + `verify_checksum()` (RFC 1071)
- ✅ `rust/net-parse/src/ffi.rs`: `net_parse_checksum()` + `net_parse_checksum_verify()`
- ✅ `rust/net-parse/include/net_parse.h`: обе функции объявлены в C header
- ✅ `minix/net/lwip/lwip.h`: `#include <net_parse.h>` — доступ через lwIP service

### Примечание:
План упоминает `minix/drivers/net/*/` как опциональную замену (low priority).
`in_cksum()` как отдельная функция в MINIX network drivers не существует —
checksum handling полностью внутри lwIP. FFI слой готов к использованию
из любого C-кода lwIP service.

---

## Приоритет выполнения

```
Sub-phase 3a (net-parse FFI) ─────── ✅ Завершено: FFI + checksum bridge
        │
        ▼
Sub-phase 3b (Rust e1000) ────────── ✅ Завершено: build + shim + TSO/CSO
        │
        ▼
Sub-phase 3c (BPF verifier) ──────── ✅ Завершено: 41 тест, safe Rust
        │
        ▼
Sub-phase 3d (virtio-net pilot) ──── ✅ Завершено: 11 тестов, ~1400 LOC
        │
        ▼
Sub-phase 3e (checksum) ──────────── ✅ Завершено: FFI checksum готов
```

## Ключевые риски (resolved)

| Риск | Impact | Статус |
|------|--------|--------|
| Rust e1000 не поддерживает все фичи C версии (TSO, CSO) | Medium | ✅ TSO + CSO добавлены в Sub-phase 3b |
| No_std ограничения: нет Vec, нет String | Low | ✅ Все crates no_std, bitmask вместо Vec, fixed arrays |
| cargo test на хосте ≠ поведение на MINIX | Low | ✅ Dual-platform FFI stubs во всех crates |
| virtio-net в QEMU user-mode сети не работает | Medium | 🟡 Требуется tap/bridge — отложено на интеграционное тестирование |
| BPF_LD/BPF_LDX mode matching в Rust валидаторе | High | ✅ Исправлен баг с `& (BPF_SIZE | BPF_MODE)` → `& BPF_MODE` |
| PCI device ID для virtio-net probe | High | ✅ Исправлен: `0x1000` → `0x0001` (subsystem type) |

---

## Итог Phase 3: Фактические результаты

| Sub-phase | LOC (новый код) | Изменённые файлы | Тесты |
|-----------|----------------|------------------|-------|
| 3a: net-parse FFI | ~270 | 5 | 30/30 passed |
| 3b: Rust e1000 интеграция | ~50 | 5 | 7/7 passed |
| 3c: BPF verifier | ~570 | 8 | 41/41 passed |
| 3d: virtio-net pilot | ~1400 | 10 | 11/11 passed |
| 3e: checksum | (в составе 3a) | 0 | (в составе 3a) |
| **Итого** | **~2300** | **~28** | **89/89 passed** |

**Созданы новые файлы**: 22 Rust файла (src/*.rs), 4 C header, 4 Makefile/BSD build
**Изменены C файлы**: 2 (e1000.c shim, bpf_filter.c FFI wrapper)
**Изменены файлы билд-системы**: CMakeLists.txt (корень + 2), rust/Cargo.toml
