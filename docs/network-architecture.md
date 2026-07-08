# Network Architecture — GergiOS

> **Last updated**: July 2026
> **Related**: `docs/networking-guide.md`, `docs/network-performance.md`,
>   `docs/network-security.md`, `planning/25_network_stack_modernization.md`

## Table of Contents

1. [Overview](#1-overview)
2. [Service Architecture](#2-service-architecture)
3. [Module Map](#3-module-map)
4. [IPC Model](#4-ipc-model)
5. [Data Flow](#5-data-flow)
6. [Socket Layer](#6-socket-layer)
7. [Interface Management](#7-interface-management)
8. [Driver Framework](#8-driver-framework)
9. [Timer and Event System](#9-timer-and-event-system)
10. [Configuration and Sysctl](#10-configuration-and-sysctl)

---

## 1. Overview

GergiOS networking is built around **lwIP 2.2.1** (lightweight IP), a TCP/IP
stack designed for embedded systems with minimal resource consumption.
The stack runs as a **user-space service** (`minix/net/lwip/`) in the MINIX
microkernel architecture, communicating with the VFS (Virtual File System)
service and network drivers via **synchronous IPC messages**.

### Design Principles

| Principle | Implementation |
|-----------|---------------|
| **Single-threaded event loop** | All network processing in one thread, no locks needed |
| **Zero-copy where possible** | pbuf chains shared between lwIP and driver layers |
| **No syscall overhead** | All socket operations are IPC messages to the service |
| **Static memory allocation** | TCP PCBs, buffers, and pools allocated at init time |
| **Minimal footprint** | ~10,000 LOC service code, ~50,000 LOC lwIP dist |

### Key Architecture Decisions

- **`NO_SYS = 0`**: lwIP runs in native OS mode (not raw mode), using
  MINIX IPC for synchronization rather than lwIP's own locking primitives.
- **`LWIP_SOCKET = 0`**: GergiOS does NOT use lwIP's POSIX socket emulation.
  Instead, it implements its own socket layer via IPC to the VFS service.
- **`LWIP_NETCONN = 0`**: The Netconn API is not used. All socket operations
  are handled by the service's custom socket layer (`tcpsock.c`, `udpsock.c`,
  etc.).

---

## 2. Service Architecture

```
                     ┌──────────────────────────┐
                     │       VFS (VFS_PROC_NR)   │
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

### 2.1 Main Event Loop (`lwip.c`)

The main loop runs in `main()` and cycles through:

1. **`sef_receive()`** — Wait for and receive an IPC message
2. **Dispatch** — Determine message type and route to handler:
   - Notification → timer events (ARP, TCP, DHCP, SYN cookie, rate limit)
   - MIB request → `rmib_process()`
   - Socket event → `sockevent_process()`
   - BPF request → `bpfdev_process()`
   - Driver reply → `ndev_process()`
3. **`sockevent_process()`** — Forward socket events to the appropriate
   protocol handler (TCP/UDP/RAW/Packet)

### 2.2 Notification Handlers

The service registers for several system notifications:

| Source | Type | Handler | Interval |
|--------|------|---------|----------|
| CLOCK | `NOTIFY_ALARM` | `expire_timers()` | ~50ms |
| kernel | `NOTIFY_SIGS` | Signal delivery | On demand |
| kernel | `NOTIFY_BOOT` | Boot-time init | Once |

The clock tick drives periodic tasks:
- TCP slow timer (every 500ms): retransmissions, keepalives
- ARP timer (every 10s): ARP cache cleanup
- DHCP timer (every 250ms): DHCP state machine
- SYN cookie timer (every 64s): secret rotation
- Rate limit tick (every 100ms): token bucket refill
- Performance alerts tick: counter reset

---

## 3. Module Map

### 3.1 Core Service Modules

| Module | Files | LOC | Function |
|--------|-------|-----|----------|
| **Main loop** | `lwip.c` | ~300 | Event loop, dispatch, timers, init |
| **IP sockets** | `ipsock.c`, `ipsock.h` | ~700 | PF_INET/PF_INET6 socket operations, sysctl, V6ONLY |
| **TCP sockets** | `tcpsock.c` | ~1000 | TCP connect/listen/accept/send/recv/options |
| **UDP sockets** | `udpsock.c` | ~800 | UDP sendto/recvfrom, connected sockets, DTLS |
| **RAW sockets** | `rawsock.c` | ~1200 | ICMP, custom protocols, raw IPv4/IPv6 |
| **Packet sockets** | `pktsock.c`, `pktsock.h` | ~500 | AF_PACKET, cooked headers |
| **Routing** | `route.c`, `route.h`, `rtsock.c`, `rtsock.h`, `rttree.c`, `rttree.h` | ~1500 | Routing table, routing sockets, radix tree |
| **Interfaces** | `ifdev.c`, `ifdev.h`, `ifaddr.c`, `ifaddr.h`, `ifconf.c` | ~1500 | Interface management, addressing, configuration |
| **Ethernet** | `ethif.c`, `ethif.h`, `loopif.c`, `loopif.h` | ~800 | Ethernet/loopback netif drivers |
| **Driver IPC** | `ndev.c`, `ndev.h` | ~400 | Network device driver communication |
| **Link sockets** | `lnksock.c`, `lldata.c` | ~500 | PF_LINK socket for link-level access |
| **BPF** | `bpfdev.c`, `bpf_filter.c` | ~400 | Packet filter, BPF character device |
| **Address** | `addr.c`, `addr.h`, `addrpol.c` | ~600 | Address parsing, policy |
| **MIB tree** | `mibtree.c` | ~200 | sysctl MIB tree |
| **Misc** | `mempool.c`, `pchain.c`, `tcpisn.c`, `mcast.c`, `util.c` | ~500 | Memory pool, multicast, TCP ISN |

### 3.2 Extended Monitoring Modules

| Module | Files | LOC | Function |
|--------|-------|-----|----------|
| **Interface stats** | `ifstat.c`, `ifstat.h` | ~120 | Per-interface packet/byte counters |
| **TCP extended** | `tcp_ext.c`, `tcp_ext.h` | ~180 | Per-connection CWND/RTT/RTO metadata |
| **Latency hist.** | `latency.c`, `latency.h` | ~130 | Per-protocol latency histogram (7 buckets) |
| **Perf alerts** | `perf_alerts.c`, `perf_alerts.h` | ~360 | Syslog alerts with thresholds and cooldown |

### 3.3 Security Modules

| Module | Files | LOC | Function |
|--------|-------|-----|----------|
| **SYN cookies** | `lwipsyncookie.c`, `lwipsyncookie.h` (lib) | ~420 | RFC 4987 SYN flood protection |
| **TCP MD5** | `lwip_tcp_md5.c`, `lwip_tcp_md5.h` (lib) | ~500 | RFC 2385 TCP MD5 signature |
| **Rate limit** | `lwip_ratelimit.c`, `lwip_ratelimit.h` (lib) | ~120 | Token bucket for ICMP/ARP/NDP |
| **WireGuard** | (12 files in lib) | ~4000 | VPN tunnel, ChaCha20-Poly1305 |
| **IPsec** | `lwip_ipsec.c`, `lwip_ipsec.h` (lib) | ~1100 | ESP/AH transport mode |
| **DTLS** | `lwip_dtls.c`, `lwip_dtls.h` (lib) | ~600 | DTLS 1.2/1.3 over UDP via wolfSSL |

### 3.4 Rust Modules

| Module | Files | LOC | Function |
|--------|-------|-----|----------|
| **net-parse** | `rust/net-parse/` | ~200+ | TCP/UDP/DNS safe parsers (FFI) |
| **packet-filter** | `rust/packet-filter/` | ~570 | BPF verifier (zero unsafe) |
| **e1000** | `rust/e1000/` | ~1000+ | Rust e1000 driver with C shim |
| **virtio-net** | `rust/virtio-net/` | ~1400 | Pilot virtio-net driver |

### 3.5 User-space Utilities

| Utility | Files | LOC | Function |
|---------|-------|-----|----------|
| **netstat** | `minix/usr.bin/netstat/` | ~280 | Socket/interface/protocol stats |
| **pcapng** | `minix/usr.bin/pcapng/` | ~300 | pcapng capture writer |
| **rpcapd** | `minix/usr.bin/rpcapd/` | ~600 | Wireshark remote capture daemon |

---

## 4. IPC Model

### 4.1 Message Types

The lwIP service handles three categories of IPC messages:

#### Socket Events (from VFS)

| Message | Handler | Description |
|---------|---------|-------------|
| `SDEV_RQ_SOCKET` | `tcpsock_create()` / `udpsock_create()` etc. | Create socket |
| `SDEV_RQ_CLOSE` | `tcpsock_close()` etc. | Close socket |
| `SDEV_RQ_READ` | `tcpsock_read()` etc. | Read data |
| `SDEV_RQ_WRITE` | `tcpsock_write()` etc. | Write data |
| `SDEV_RQ_IOCTL` | `tcpsock_ioctl()` etc. | Socket ioctl |
| `SDEV_RQ_SETOPT` | `tcpsock_setsockopt()` | Set socket option |
| `SDEV_RQ_GETOPT` | `tcpsock_getsockopt()` | Get socket option |
| `SDEV_RQ_CONNECT` | `tcpsock_connect()` | Connect socket |
| `SDEV_RQ_LISTEN` | `tcpsock_listen()` | Listen on socket |
| `SDEV_RQ_ACCEPT` | `tcpsock_accept()` | Accept connection |
| `SDEV_RQ_FCNTL` | `tcpsock_fcntl()` | File control |

#### Driver Replies (from network drivers)

| Message | Handler | Description |
|---------|---------|-------------|
| `NDEV_RS_SEND` | `ndev_process_send_reply()` | Packet transmission complete |
| `NDEV_RS_RECV` | `ndev_process_recv_reply()` | Packet received |
| `NDEV_RS_STOP` | `ndev_process_stop_reply()` | Driver stopped |
| `NDEV_RS_GETSTAT` | `ndev_process_stat_reply()` | Driver statistics |

#### MIB Requests (from sysctl/mib service)

| Message | Handler | Description |
|---------|---------|-------------|
| `RMIB_RQ_GET` | `rmib_get()` | Read sysctl variable |
| `RMIB_RQ_SET` | `rmib_set()` | Write sysctl variable |
| `RMIB_RQ_FUNC` | `rmib_func()` | Execute MIB function |

### 4.2 Synchronous IPC Flow

```
VFS                     lwIP service              Driver
 │                         │                        │
 │── SDEV_RQ_CONNECT ──────▶                        │
 │                         │── NDEV_RS_RECV (wait) ─▶
 │                         │   ...                  │
 │                         │◀── NDEV_RS_RECV ────────┤
 │                         │ TCP SYN → SYN-ACK       │
 │                         │── NDEV_RS_RECV (wait) ─▶│
 │                         │◀── NDEV_RS_RECV ────────┤
 │                         │ TCP ACK (connection)    │
 │◀── Status: OK ──────────┤                         │
```

All IPC is **synchronous** — the caller blocks until the callee responds.
For network drivers, this means `ndev_send()` waits for the driver to
complete transmission before returning.

### 4.3 Batch Processing

Phase 2e introduced batch processing for single-fragment packets:

- **`NDEV_SEND_BATCH`**: Up to 8 single-fragment packets in one `asynsend()`
- **`ethif_poll()`**: Batch processing for single-fragment packets
- **Fallback**: Returns `EBUSY` if packets don't fit batch criteria

This reduces IPC overhead by up to **8×** for small packets (ACKs, DNS).

---

## 5. Data Flow

### 5.1 Packet Reception

```
Driver RX ring
    │
    ▼
ndev_process_recv_reply()     ← IPC from driver
    │
    ▼
ifdev_input()                 ← Interface dispatch
    │
    ├── BPF tap (if bpf attached)
    │
    ▼
ip4_input() / ip6_input()    ← lwIP IP layer
    │
    ├── Demux by protocol
    │
    ▼
tcp_input() / udp_input()    ← lwIP TCP/UDP
    │
    ▼
Socket buffer / notification
    │
    ▼
VFS: select/poll wakeup      ← IPC to VFS
```

### 5.2 Packet Transmission

```
VFS: write() / sendto()
    │
    ▼
tcpsock_write() / udpsock_send()
    │
    ▼
tcp_write() / udp_send()     ← lwIP TCP/UDP
    │
    ▼
ip4_output() / ip6_output()  ← lwIP IP layer
    │
    ▼
ethif_output()               ← Ethernet framing
    │
    ▼
ndev_send()                  ← IPC to driver
    │
    ▼
Driver TX ring
```

### 5.3 Loopback Fast Path

Loopback uses a **synchronous fast path** (Phase 2d):

```
TCP send on lo0
    │
    ▼
loopif_output()
    │
    ├── if depth < LOOPIF_FAST_DEPTH_MAX (8):
    │       pbuf_ref() + direct ifdev_input()
    │       → zero IPC, zero context switch
    │
    └── else:
            ndev_send() → ndev_recv()
            → fallback to async path
```

### 5.4 pbuf Architecture

lwIP uses `pbuf` (packet buffer) chains for all packet data:

```
                ┌──────────────┐
                │   pbuf      │
                │ payload ────────▶ Data buffer
                │ len = 1460  │
                │ tot_len     │
                │ next ───────▶ ┌──────────────┐
                │ type=PBUF_RAM│ │   pbuf      │
                └──────────────┘ │ payload ────────▶ TCP options
                                 │ len = 12    │
                                 │ tot_len     │
                                 │ next ───────▶ NULL
                                 └──────────────┘
```

Types used:

| Type | Allocation | Use |
|------|------------|-----|
| `PBUF_RAM` | `malloc()` (heap) | Application data, outbound packets |
| `PBUF_POOL` | Fixed-size pool | Inbound packets, reassembly |
| `PBUF_REF` | Reference to external | Zero-copy, loopback fast path |

---

## 6. Socket Layer

### 6.1 Socket Creation Flow

```
VFS: socket(PF_INET, SOCK_STREAM, 0)
    │
    ▼
VFS creates VFS-side socket structure
    │
    ▼
IPC: SDEV_RQ_SOCKET(PF_INET, SOCK_STREAM, 0)
    │
    ▼
lwIP: sockevent_rq_socket()
    │
    ├── pf == PF_INET || PF_INET6
    │      ├── type == SOCK_STREAM  → tcpsock_create()
    │      ├── type == SOCK_DGRAM   → udpsock_create()
    │      ├── type == SOCK_RAW     → rawsock_create()
    │      └── type == SOCK_PACKET  → pktsock_create()
    │
    └── pf == PF_LINK
           → lnksock_create()
```

### 6.2 Socket State Machines

#### TCP

```
CLOSED → LISTEN → SYN_RCVD → ESTABLISHED → CLOSE_WAIT → LAST_ACK
  │        │                      │                       │
  │        │                      ├── FIN_WAIT_1 ──→ TIME_WAIT
  │        │                      ├── FIN_WAIT_2
  │        │                      └── CLOSING
  │        │
  └────────┴──────────────────→ CLOSED (abort/timeout)
```

#### UDP

```
UNCONNECTED → CONNECTED
```

### 6.3 Socket Options

Supported per-protocol socket options:

#### TCP

| Option | Type | Description |
|--------|------|-------------|
| `TCP_NODELAY` | int | Disable Nagle algorithm |
| `TCP_KEEPALIVE` | int | Enable keepalive probes |
| `TCP_KEEPIDLE` | int | Idle time before probes (seconds) |
| `TCP_KEEPINTVL` | int | Interval between probes (seconds) |
| `TCP_KEEPCNT` | int | Number of probes before death |
| `TCP_MD5SIG` | struct | TCP MD5 signature key |
| `SO_REUSEADDR` | int | Reuse local addresses |
| `SO_REUSEPORT` | int | Reuse local port |
| `SO_KEEPALIVE` | int | Enable TCP keepalive |
| `SO_LINGER` | struct | Close behavior |
| `SO_RCVTIMEO` | struct | Receive timeout |
| `SO_SNDTIMEO` | struct | Send timeout |
| `TCP_INFO` | struct | TCP connection info |

#### UDP

| Option | Type | Description |
|--------|------|-------------|
| `SO_REUSEADDR` | int | Reuse local addresses |
| `SO_REUSEPORT` | int | Reuse local port |
| `UDP_DTLS` | int | Enable DTLS over this socket |

#### IP

| Option | Type | Description |
|--------|------|-------------|
| `IP_TTL` | int | Time-to-live |
| `IP_MULTICAST_TTL` | int | Multicast TTL |
| `IP_MULTICAST_IF` | struct | Multicast interface |
| `IP_ADD_MEMBERSHIP` | struct | Join multicast group |
| `IP_DROP_MEMBERSHIP` | struct | Leave multicast group |
| `IP_IPSEC_SA` | int | Attach IPsec SA to socket |

---

## 7. Interface Management

### 7.1 Interface Abstraction

All network interfaces are managed through the `ifdev` abstraction:

```c
struct ifdev {
    char        if_name[IFNAMSIZ];      /* Interface name */
    int         if_index;               /* Unique index */
    struct if_data if_data;             /* Statistics */
    struct ifdev_ops *ops;              /* Driver operations */
    struct netif *netif;                /* lwIP netif */
    unsigned int if_flags;              /* IFF_* flags */
};
```

### 7.2 Interface Discovery

Interfaces are discovered at boot time via driver registration:

1. Driver starts, registers with `ndev_register()`
2. lwIP service probes driver capabilities
3. System creates `ifdev` entry + corresponding `netif`
4. Interface becomes visible to `ifconfig` and routing

Available interfaces can be enumerated at runtime:

```sh
# All interfaces:
sysctl minix.lwip.ifaces

# Specific:
ifconfig e0
```

### 7.3 Interface Types

| Type | ifdev_ops | Description |
|------|-----------|-------------|
| Ethernet | `ethif_ops` | Wired Ethernet (e1000, rtl8139, etc.) |
| Loopback | `loopif_ops` | Local loopback |
| WireGuard | `wgif_ops` | VPN tunnel |
| Any (BPF) | — | Virtual "any" interface for capture |

### 7.4 Interface Flags

Standard `IFF_*` flags:

| Flag | Meaning |
|------|---------|
| `IFF_UP` | Interface is running |
| `IFF_BROADCAST` | Broadcast address valid |
| `IFF_LOOPBACK` | Loopback interface |
| `IFF_PROMISC` | Promiscuous mode |
| `IFF_MULTICAST` | Multicast support |
| `IFF_OACTIVE` | Output queue active |

---

## 8. Driver Framework

### 8.1 Driver Model

```
┌──────────────────────────────┐
│      lwIP service            │
│  ┌────────────────────────┐  │
│  │     ndev layer         │  │
│  │  ndev_register()       │  │
│  │  ndev_send()           │  │
│  │  ndev_recv()           │  │
│  └──────┬─────────────────┘  │
└─────────┼────────────────────┘
          │ IPC messages
┌─────────▼────────────────────┐
│      Net Driver Process       │
│  ┌────────────────────────┐  │
│  │  e1000 / rtl8139 /     │  │
│  │  fxp / lance / dp8390  │  │
│  └────────────────────────┘  │
└──────────────────────────────┘
```

### 8.2 Driver Capabilities

Drivers advertise capabilities via bitmask:

| Capability | Meaning |
|------------|---------|
| `NDEV_CAP_CS_IP4_TX` | IPv4 checksum offload (TX) |
| `NDEV_CAP_CS_IP4_RX` | IPv4 checksum offload (RX) |
| `NDEV_CAP_TSO` | TCP segmentation offload |
| `NDEV_CAP_JUMBO` | Jumbo frame support |
| `NDEV_CAP_MULTIQ` | Multi-queue support |

Currently supported:

| Driver | CSO | TSO | Jumbo | MultiQ | Rust |
|--------|-----|-----|-------|--------|------|
| e1000 | ✅ | ✅ | ✅ | ✅ | ✅ (shim) |
| rtl8139 | — | — | — | — | — |
| fxp | — | — | — | — | — |
| lance | — | — | — | — | — |
| dp8390 | — | — | — | — | — |

### 8.3 Send Queues (Multi-Queue)

Phase 2b introduced software multi-queue:

- **2 send queues** per interface (`NDEV_NUM_SENDQ = 2`)
- **Round-robin** selection: queue 0 → queue 1 → queue 0
- **Per-queue depth**: tracked in `ndev` layer
- **Driver-agnostic**: works with any physical driver

### 8.4 Interrupt Moderation

For e1000 driver:

| Parameter | Value | Effect |
|-----------|-------|--------|
| `EITR` | 500 (interrupts/sec) | Throttle interrupt rate |
| `RDTR` | 128 (×1.024 µs) | RX delay timer |
| `RADV` | 256 (×1.024 µs) | RX absolute max delay |

---

## 9. Timer and Event System

### 9.1 Periodic Timers

All driven by the CLOCK notification tick (~50ms):

| Timer | Interval | Purpose |
|-------|----------|---------|
| `tcp_slowtmr()` | 500ms | TCP retransmit, keepalive, delayed ACK |
| `tcp_fasttmr()` | 250ms | TCP fast retransmit |
| `etharp_tmr()` | 10s | ARP cache expiry |
| `dhcp_fine_tmr()` | 250ms | DHCP state machine |
| `dhcp_coarse_tmr()` | 60s | DHCP renew/rebind |
| `lwip_ratelimit_tick()` | 100ms | Token bucket refill |
| `syn_cookie_rotate()` | 64s | SYN cookie secret rotation |
| `perf_alerts_tick()` | 1s | Alert counter reset |
| `nd6_tmr()` | 1s | NDP reachability |

### 9.2 Signal Handling

The service handles `SIGTERM` for graceful shutdown and `SIGCHLD` for
reaping child processes (used by `rpcapd`).

---

## 10. Configuration and Sysctl

### 10.1 MIB Tree Layout

```
net.inet.tcp.*            — TCP parameters (RFC standard names)
net.inet.udp.*            — UDP parameters
net.inet.ip.*             — IP parameters
net.inet.raw.*            — RAW socket parameters
net.inet6.ip6.*           — IPv6 parameters
net.inet6.icmp6.*         — ICMPv6 parameters

minix.lwip.*              — MINIX-specific extensions
minix.lwip.ifaces         — Per-interface statistics (RO)
minix.lwip.tcp_ext        — TCP extended metrics (RO)
minix.lwip.latency        — Latency histograms (RO)
minix.lwip.alerts         — Performance alert config (RW)
minix.lwip.wireguard      — WireGuard config (RW/FUNC)
minix.lwip.ipsec          — IPsec config (RW)
minix.lwip.dtls           — DTLS config (RW)
minix.lwip.drivers        — Driver info (RO)
```

### 10.2 Key TCP Sysctl Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `net.inet.tcp.sendspace` | 16384 | TCP send buffer |
| `net.inet.tcp.recvspace` | 32768 | TCP receive buffer |
| `net.inet.tcp.mssdflt` | 1460 | Default MSS |
| `net.inet.tcp.sack.enabled` | 1 | Selective ACK (RFC 2018) |
| `net.inet.tcp.syncookies` | 1 | SYN cookies (RW) |
| `net.inet.tcp.md5sig` | 1 | TCP MD5 signature (RW) |
| `net.inet.ip.forwarding` | 0 | IP forwarding (RW) |
| `net.inet.ip.ingress_filter` | 0 | BCP 38 filtering (RW) |
| `net.inet6.ip6.forwarding` | 0 | IPv6 forwarding (RW) |
| `net.inet6.ip6.ingress_filter` | 0 | IPv6 ingress filter (RW) |

### 10.3 Watchdog / Live Update

**Status**: TODO — not yet implemented.

The lwIP service should support MINIX live update for zero-downtime
network stack upgrades. The planned approach:

1. **State serialization**: Serialize TCP PCB state, routing table,
   interface state into a memory buffer
2. **`sef_lu_prepare()`**: Save state, pause all timers
3. **`sef_lu_continuation()`**: Restore state, resume timers
4. **Limitation**: In-flight packets may be lost; TCP should recover
   via retransmission

---

> **See also**: `docs/networking-guide.md` for practical setup,
> `docs/network-performance.md` for tuning, `docs/network-security.md`
> for security features.
