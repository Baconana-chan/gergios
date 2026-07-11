# Phase 8 — Rust Bluetooth Stack (BlueZ Userspace Port)

## Overview

Phase 8 implements a native Rust Bluetooth userspace stack for GergiOS,
replacing the need for a full BlueZ/D-Bus/GLib port from Linux.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│               Bluetooth Applications                          │
│  (file transfer, headset, keyboard/mouse, IoT)                │
├──────────────────────────────────────────────────────────────┤
│         Bluetooth C Library (libbluetooth)                    │
│  ┌────────┐ ┌─────────┐ ┌──────────┐ ┌────────────────┐     │
│  │ scan() │ │pair()   │ │connect()  │ │send/receive() │     │
│  └────┬───┘ └───┬─────┘ └────┬─────┘ └───────┬────────┘     │
├───────┼──────────┼────────────┼───────────────┼──────────────┤
│       │     Bluetooth Daemon (bluetoothd)      │              │
│       │  ┌─────────────────────────────────┐   │              │
│       │  │     GATT (Generic Attribute)     │   │              │
│       │  ├─────────────────────────────────┤   │              │
│       │  │     RFCOMM (Serial Emulation)    │   │              │
│       │  ├─────────────────────────────────┤   │              │
│       │  │     SDP (Service Discovery)      │   │              │
│       │  ├─────────────────────────────────┤   │              │
│       │  │     L2CAP (Logical Link Control) │   │              │
│       │  └──────────────┬──────────────────┘   │              │
│       └─────────────────┼──────────────────────┘              │
│                         │ HCI                                  │
│              ┌──────────▼──────────┐                          │
│              │    /dev/hci0        │                          │
│              │  (minix-bt-hci)     │                          │
│              └─────────────────────┘                          │
└──────────────────────────────────────────────────────────────┘
```

## Protocol Layers

### L2CAP (Logical Link Control and Adaptation Protocol)
- Channel multiplexing for upper-layer protocols
- Segmentation and reassembly of packets
- Connection-oriented and connectionless channels
- Configuration: MTU, flush timeout, QoS
- LE Credit-Based Flow Control (BT 4.0+)
- Enhanced Credit-Based Flow Control (BT 5.0+)

### SDP (Service Discovery Protocol)
- Service records stored in the daemon
- Service search by UUID
- Service attribute browsing
- Service registration from local applications

### RFCOMM (Serial Port Emulation)
- RS-232 serial port emulation over L2CAP
- Multiple virtual serial ports (up to 30)
- Modem status signals (CTS, DSR, DTR, RTS, RI, DCD)
- Credit-based flow control

### GATT (Generic Attribute Profile)
- Attribute Protocol (ATT) for BLE
- Services, characteristics, descriptors
- Client and server roles
- Notifications and indications
- Support for all GATT-defined profiles

## Implementation Plan

### Phase 8.1 — L2CAP Protocol ✅
- [x] Create `rust/minix-bt-stack/` crate
- [x] L2CAP packet format (B-frame, S-frame, I-frame)
- [x] Channel state machine (CLOSED, WAIT_CONNECT, CONFIG, OPEN, etc.)
- [x] Connection-oriented channel management
- [x] Connectionless channel (CID 0x0002)
- [x] LE Credit-Based Flow Control
- [x] Unit tests for packet parsing/channel management

### Phase 8.2 — SDP Protocol ✅
- [x] Service record format and storage
- [x] PDU parsing (SearchRequest, AttributeRequest, etc.)
- [x] UUID → service lookup
- [x] Service registration
- [x] Unit tests

### Phase 8.3 — RFCOMM Protocol ✅
- [x] Multiplexer commands (SABM, UA, DM, DISC, UIH)
- [x] DLC (Data Link Connection) management
- [x] Modem signal handling
- [x] Credit-based flow control
- [x] Unit tests

### Phase 8.4 — GATT Protocol ✅
- [x] Attribute Protocol (ATT) PDUs
- [x] Server: Read/Write/Notify/Indicate
- [x] Client: Discover services, read/write characteristics
- [x] BLE connection management
- [x] Unit tests

### Phase 8.5 — Bluetooth Daemon ✅
- [x] HCI device management (open /dev/hci0, ioctl)
- [x] Connection manager (inquiry, pairing, connections)
- [x] Protocol multiplexing (L2CAP ↔ HCI)
- [x] Local service registration
- [x] Configuration file

### Phase 8.6 — C Library + bt-tool ✅
- [x] `libbluetooth` — C API for Bluetooth operations
- [x] `bt-tool` CLI — scan, pair, connect, info, send/receive
- [x] Man pages

## File Layout

```
rust/minix-bt-stack/
├── Cargo.toml
└── src/
    ├── lib.rs              — Crate root, re-exports
    ├── hci_mgr.rs          — HCI device management
    ├── l2cap.rs            — L2CAP protocol
    ├── l2cap_sig.rs        — L2CAP signaling
    ├── sdp.rs              — SDP protocol
    ├── sdp_record.rs       — SDP record representation
    ├── rfcomm.rs           — RFCOMM protocol
    ├── rfcomm_mux.rs       — RFCOMM multiplexer
    ├── gatt.rs             — GATT/ATT protocol
    ├── gatt_profile.rs     — GATT profile definitions
    ├── bt_daemon.rs        — Bluetooth daemon main loop
    ├── hci.rs              — Re-export HCI types from minix-bt-hci
    ├── types.rs            — Common types (BD_ADDR, UUID, etc.)
    └── ffi.rs              — FFI bindings for MINIX

minix/lib/libbluetooth/
├── Makefile
├── bluetooth.h            — C API header
└── bluetooth.c            — C API implementation

usr.bin/bt-tool/
├── Makefile
└── src/
    ├── main.rs            — CLI tool
    └── commands.rs         — Command implementations
```

## Key Design Decisions

1. **Single daemon architecture** — one process manages all Bluetooth connections
2. **Synchronous IPC** — applications communicate with daemon via chardev or message queue
3. **No D-Bus dependency** — lightweight IPC instead of full D-Bus
4. **Rust-native** — entire stack in Rust for memory safety
5. **BlueZ-compatible ioctls** — reuse /dev/hci0 interface from Phase 7.8
6. **`minix-driver` crate** — reuse existing FFI and platform abstractions

## Dependencies

- ✅ `rust/minix-bt-hci/` — HCI transport (/dev/hci0)
- ✅ `rust/minix-driver/` — MINIX FFI and driver abstractions
- 🆕 `rust/minix-bt-stack/` — this crate (in progress)

## Risks

- L2CAP signaling over HCI requires async handling of Command Complete events
- RFCOMM multiplexer state machine is complex
- BLE GATT requires LE Connection management (Phase 7.8 has limited LE support)
- Testing requires real Bluetooth hardware or HCI emulation
