# Phase 3: Rust сетевые компоненты 🦀

> **Статус**: Планирование
> **Связанные**: `planning/25_network_stack_modernization.md` §Phase 3
> **Существующие Rust crates**: `rust/net-parse/` (TCP/UDP/DNS парсеры),
>   `rust/minix-rs/` (IPC bindings), `rust/minix-driver/` (safe MMIO/port I/O),
>   `rust/e1000/` (полный Rust драйвер e1000), `rust/virtio-blk/` (virtio pilot)
> **Build system**: CMake `add_rust_library()` + `add_rust_utility()` — Rust статически
>   линкуется в C targets через imported target `rust_<name>`

---

## Обзор

**Что есть сейчас**: Rust уже интегрирован в билд систему MINIX (CMake `add_rust_library()` +
`add_rust_utility()` + `add_rust_test()`). Существует инфраструктура для:
- `no_std` библиотек с FFI экспортом (Cargo.toml `crate-type = ["staticlib", "lib"]`)
- Утилит (Cargo.toml `[[bin]]`)
- Тестов (`cargo test --lib`)

**Существующие сетевые Rust crates**:
- **`net-parse`** — TCP/UDP/DNS парсеры (23 теста, zero unsafe, no_std)
- **`e1000`** — Полный Rust e1000 драйвер с MMIO/PCI/IRQ/MSI-X, interrupt moderation,
  work queue, netdriver callbacks (lib.rs, ffi.rs, driver.rs, desc.rs, reg.rs, eeprom.rs, pci_ids.rs)
- **`minix-rs`** — IPC message bindings (Message, syscall, endpoint/type validation)
- **`minix-driver`** — Safe MMIO/port I/O wrappers (используется e1000 и virtio-blk)
- **`virtio-blk`** — Rust virtio block driver (pilot, multi-threaded, MSI-X, multi-queue)

**Что Phase 3 добавляет**: Интеграцию safe Rust компонентов в работающий сетевой стек.

---

## Sub-phase 3a: net-parse FFI Bridge

**Цель**: Добавить C-совместимый FFI слой в `rust/net-parse/` для вызова парсеров
TCP/UDP заголовков из C кода lwIP service.

**Зачем**: Замена C-кода проверки заголовков на safe Rust код с нулевым unsafe.
Это первая фаза — верификация, не замена. Постепенная миграция, один протокол за раз.

### Изменения:

**`rust/net-parse/src/ffi.rs`** (НОВЫЙ):
```c
// C-совместимые функции для вызова из C lwIP service

// Проверка TCP заголовка: buf[buflen] → заполняет TcpHeaderFFI
// Возвращает: 0=OK, -1=Truncated, -2=InvalidData
int net_parse_tcp_header(const uint8_t *buf, size_t buflen,
    struct TcpHeaderFFI *out);

// Проверка UDP заголовка
int net_parse_udp_header(const uint8_t *buf, size_t buflen,
    struct UdpHeaderFFI *out);

// Проверка Internet checksum (RFC 1071) — уже есть в utils
uint16_t net_parse_checksum(const uint8_t *data, size_t len);
int net_parse_checksum_verify(const uint8_t *data, size_t len);
```

**`rust/net-parse/include/net_parse.h`** (НОВЫЙ):
```c
// C header для FFI функций net-parse
#ifndef NET_PARSE_H
#define NET_PARSE_H

#include <stdint.h>
#include <stddef.h>

struct TcpHeaderFFI {
    uint16_t src_port;
    uint16_t dst_port;
    uint32_t seq_num;
    uint32_t ack_num;
    uint8_t  data_offset;
    uint8_t  flags;        // TCP flags (SYN=0x02, ACK=0x10, etc.)
    uint16_t window_size;
    uint16_t checksum;
    uint16_t urgent_ptr;
};

struct UdpHeaderFFI {
    uint16_t src_port;
    uint16_t dst_port;
    uint16_t length;
    uint16_t checksum;
};

int net_parse_tcp_header(const uint8_t *buf, size_t buflen,
    struct TcpHeaderFFI *out);
int net_parse_udp_header(const uint8_t *buf, size_t buflen,
    struct UdpHeaderFFI *out);
uint16_t net_parse_checksum(const uint8_t *data, size_t len);
int net_parse_checksum_verify(const uint8_t *data, size_t len);

#endif /* NET_PARSE_H */
```

**`rust/net-parse/Cargo.toml`**: Добавить `crate-type = ["staticlib", "lib"]`

**Интеграция в lwIP service** (`minix/net/lwip/`):
- CMake: `add_rust_library(net-parse LINK_TO lwip)`
- В `tcpsock.c` или `lwip.h`: include `net_parse.h`
- Проверка: добавить assertion/validation hook при получении TCP сегментов
- Пока что только верификация (не замена) — `assert()` в debug build

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `rust/net-parse/src/ffi.rs` | ❌ Новый | ~80 |
| `rust/net-parse/include/net_parse.h` | ❌ Новый | ~50 |
| `rust/net-parse/Cargo.toml` | 🟡 Изменить | +2 стр |
| `minix/net/lwip/CMakeLists.txt` | 🟡 Изменить | +2 стр |
| `minix/net/lwip/tcpsock.c` | 🟡 Изменить | +15 стр (debug assertions) |

**Тестирование**: `cargo test --lib` (net-parse tests), `cargo test -p net-parse --test ffi`

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

## Sub-phase 3c: Rust BPF Verifier (packet-filter)

**Цель**: Создать safe Rust BPF инструкций верификатор, заменяющий `bpf_validate()`
в `minix/net/lwip/bpf_filter.c`.

**Зачем**: BPF верификатор — изолированный компонент, идеальный для safe Rust.
C `bpf_validate()` уже правильный, но Rust может предложить formal verification
потенциал в будущем и безопасную альтернативу.

### Новый crate: `rust/packet-filter/`

```rust
// rust/packet-filter/src/lib.rs — no_std, zero unsafe

/// BPF instruction (mirrors struct bpf_insn)
#[repr(C)]
pub struct BpfInsn {
    pub code: u16,
    pub jt: u8,
    pub jf: u8,
    pub k: u32,
}

/// Validate a BPF filter program
/// Returns true if the program is safe to execute
pub fn bpf_validate(insns: &[BpfInsn]) -> bool {
    // Reachability analysis
    // Store-verify: every memory load is preceded by a store
    // Division-by-zero check: DIV/MOD with k=0
    // Shift-amount check: LSH/RSH k >= 32
    // Jump-target check: all jumps stay within bounds
    // Termination guarantee: no infinite loops
}
```

**FFI export** (`rust/packet-filter/src/ffi.rs`):
```c
int packet_filter_validate(const struct BpfInsn *insns, int count);
```

**Интеграция**:
- `minix/net/lwip/bpf_filter.c`: заменить `bpf_validate()` на FFI вызов
- `bpfdev.c`: не меняется (вызывает `bpf_validate()` через `bpf_filter.c`)

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `rust/packet-filter/Cargo.toml` | ❌ Новый | ~10 |
| `rust/packet-filter/src/lib.rs` | ❌ Новый | ~150 |
| `rust/packet-filter/src/ffi.rs` | ❌ Новый | ~30 |
| `rust/packet-filter/include/packet_filter.h` | ❌ Новый | ~15 |
| `minix/net/lwip/bpf_filter.c` | 🟡 Изменить | +10 стр |
| `minix/net/lwip/CMakeLists.txt` | 🟡 Изменить | +2 стр |
| `rust/Cargo.toml` | 🟡 Изменить | +1 стр |

**Тестирование**: `cargo test`, существующие BPF тесты в test suite

---

## Sub-phase 3d: Rust virtio-net Pilot Driver

**Цель**: Создать Rust драйвер virtio-net как pilot для будущих Rust сетевых
драйверов, по аналогии с `rust/virtio-blk/`.

**Зачем**: Модель для будущих Rust сетевых драйверов. virtio-net проще e1000
и хорошо поддерживается в QEMU.

### Новый crate: `rust/virtio-net/`

```
rust/virtio-net/
├── Cargo.toml          — staticlib + lib, deps: minix-driver
├── src/
│   ├── lib.rs          — Netdriver callbacks + C entry
│   ├── ffi.rs          — FFI bindings (как в virtio-blk + e1000)
│   ├── device.rs       — VirtioDevice (переиспользовать из virtio-blk)
│   ├── queue.rs        — VirtQueue (переиспользовать из virtio-blk)
│   ├── net.rs          — Virtio-net protocol (config, header, features)
│   └── driver.rs       — Core driver logic (probe, init, send, recv)
```

**Переиспользование**: `device.rs` и `queue.rs` практически идентичны virtio-blk.
Можно вынести в отдельный `minix-virtio` crate, но для pilot — копирование проще.

**Netdriver callbacks**: как в C e1000 и Rust e1000:
- `ndr_init` — PCI probe, feature negotiation, alloc virtqueues (RX + TX)
- `ndr_send` — submit TX virtqueue: virtio-net header + packet data
- `ndr_recv` — collect RX virtqueue: strip virtio-net header, return data
- `ndr_intr` — MSI-X or legacy IRQ → process used rings
- `ndr_get_link` — read config space status

### Файлы:
| Файл | Статус | LOC |
|------|--------|-----|
| `rust/virtio-net/Cargo.toml` | ❌ Новый | ~15 |
| `rust/virtio-net/src/lib.rs` | ❌ Новый | ~200 |
| `rust/virtio-net/src/ffi.rs` | ❌ Новый | ~100 |
| `rust/virtio-net/src/device.rs` | ❌ Новый | ~250 |
| `rust/virtio-net/src/queue.rs` | ❌ Новый | ~250 |
| `rust/virtio-net/src/net.rs` | ❌ Новый | ~100 |
| `rust/virtio-net/src/driver.rs` | ❌ Новый | ~200 |
| `rust/virtio-net/include/virtio_net.h` | ❌ Новый | ~30 |
| `rust/Cargo.toml` | 🟡 Изменить | +1 стр |

**Тестирование**: QEMU с виртуальной сетью (`-device virtio-net-pci`),
`ping`, `iperf3` сравнение с e1000

---

## Sub-phase 3e: Rust Checksum в драйверах

**Цель**: Заменить `in_cksum()` в драйверах на Rust `net_parse::util::internet_checksum()`
через FFI.

**Зачем**: Проверенная безопасная checksum implementation (zero unsafe) через FFI.
Потенциально vectorized SIMD в будущем.

### Изменения:
- `rust/net-parse/src/util.rs`: уже есть `internet_checksum()` и `verify_checksum()` ✅
- FFI уже будет в Sub-phase 3a
- `minix/drivers/net/*/`: опциональная замена (low priority)

---

## Приоритет выполнения

```
Sub-phase 3a (net-parse FFI) ─────── Простейшая интеграция, высокий impact
        │
        ▼
Sub-phase 3b (Rust e1000) ────────── Уже полный код, только build + shim
        │
        ▼
Sub-phase 3c (BPF verifier) ──────── Изолированный, хорошо подходит для Rust
        │
        ▼
Sub-phase 3d (virtio-net pilot) ──── Самый большой, но важный для будущего
        │
        ▼
Sub-phase 3e (checksum) ──────────── Если уже есть FFI — тривиально
```

## Ключевые риски

| Риск | Impact | Mitigation |
|------|--------|------------|
| Rust e1000 не поддерживает все фичи C версии (TSO, CSO, batch) | Medium | Добавить в Rust e1000 перед интеграцией |
| No_std ограничения: нет Vec, нет String | Low | net-parse и e1000 уже no_std |
| cargo test на хосте ≠ поведение на MINIX | Low | Dual-platform FFI stubs (уже сделано в e1000) |
| virtio-net в QEMU user-mode сети не работает | Medium | Использовать tap/bridge для тестов |

---

## Оценка объёма работ

| Sub-phase | LOC (новый код) | Изменённые файлы |
|-----------|----------------|-------------------|
| 3a: net-parse FFI | ~130 | 5 |
| 3b: Rust e1000 интеграция | ~40 | 5 |
| 3c: BPF verifier | ~200 | 7 |
| 3d: virtio-net pilot | ~1100 | 9 |
| 3e: checksum | ~10 | 2 |
| **Итого** | **~1500** | **~28** |
