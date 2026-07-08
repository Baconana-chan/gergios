# Network Performance — GergiOS

> **Last updated**: July 2026
> **Related**: `docs/networking-guide.md`, `docs/network-architecture.md`,
>   `docs/network-security.md`, `man netstat(8)`, `planning/25_phase2_performance_detailed.md`

## Table of Contents

1. [Overview](#1-overview)
2. [Performance Architecture](#2-performance-architecture)
3. [Buffer Configuration](#3-buffer-configuration)
4. [TCP Tuning](#4-tcp-tuning)
5. [Checksum Offload and TSO](#5-checksum-offload-and-tso)
6. [Interrupt Moderation](#6-interrupt-moderation)
7. [Jumbo Frames](#7-jumbo-frames)
8. [Multi-Queue](#8-multi-queue)
9. [Loopback Fast Path](#9-loopback-fast-path)
10. [Monitoring with netstat](#10-monitoring-with-netstat)
11. [Performance Alerts](#11-performance-alerts)
12. [Latency Histograms](#12-latency-histograms)
13. [Benchmark Methodology](#13-benchmark-methodology)

---

## 1. Overview

GergiOS networking performance is driven by the **lwIP 2.2.1** stack with
several enhancements from Phase 2 of the modernization plan:

| Feature | Phase | Impact |
|---------|-------|--------|
| Checksum offload (e1000) | 2a | IPv4 checksum: 100% CPU → 0% HW offload |
| Jumbo frames | 2a | MTU 1500 → 9000, fewer interrupts per byte |
| Software multi-queue | 2b | 1 → 2 send queues, round-robin |
| TSO (Legacy TSE) | 2c | Super-segments up to 64KB |
| Loopback fast path | 2d | Zero-copy synchronous delivery |
| Batch processing | 2e | Up to 8× IPC reduction for small packets |

### Expected Throughput (QEMU e1000, KVM)

| Direction | Before | After (Phase 2) |
|-----------|--------|-----------------|
| TCP host→guest | ~500-800 Mbps | ~1-2 Gbps |
| TCP guest→host | ~300-600 Mbps | ~800 Mbps-1.5 Gbps |
| UDP host→guest | ~300-500 Mbps | ~500-800 Mbps |
| Loopback | ~1-2 Gbps | ~5-10 Gbps |

**Note**: These are QEMU-emulated e1000 (82540EM) figures. Real hardware
(Intel PRO/1000 PT, etc.) may show different performance.

---

## 2. Performance Architecture

```
┌─────────────────────────────────────────────────┐
│                 Application                       │
│  send(buf, len) → write(fd)                      │
└────────────────────┬────────────────────────────┘
                     │ IPC (socket write)
                     ▼
┌─────────────────────────────────────────────────┐
│              lwIP Service (lwip.c)               │
│                                                   │
│  tcpsock_write()                                 │
│    ├── tcp_write(pcb, data, len, COPY)           │
│    ├── tcp_output(pcb)                           │
│    │   ├── Single segment (MSS)                  │
│    │   └── TSO super-segment (up to 64KB)        │
│    └── ip4_output() → ethif_output()             │
│                                                   │
│  ethif_output()                                  │
│    ├── Checksum offload (IP/TCP)                 │
│    └── Send queue selection (round-robin)         │
│                                                   │
│  ndev_send() / ndev_send_batch()                 │
│    └── IPC to driver                             │
└────────────────────┬────────────────────────────┘
                     │ NDEV message
                     ▼
┌─────────────────────────────────────────────────┐
│              e1000 Driver                        │
│                                                   │
│  TX descriptor ring (512 entries)                │
│    ├── Legacy descriptor (data + length)         │
│    ├── TSE descriptor (CMD+CSS/CSO+MSS)          │
│    └── Interrupt moderation (EITR/RDTR/RADV)     │
└─────────────────────────────────────────────────┘
```

---

## 3. Buffer Configuration

### 3.1 Static Allocation

lwIP on GergiOS uses **static memory pools** for all network buffers.
This avoids runtime allocation overhead but limits flexibility.

### 3.2 Key Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `MEMP_NUM_TCP_PCB` | 50 | Active TCP connections |
| `MEMP_NUM_TCP_PCB_LISTEN` | 16 | Listening TCP sockets |
| `MEMP_NUM_UDP_PCB` | 16 | Active UDP sockets |
| `MEMP_NUM_RAW_PCB` | 8 | Active RAW sockets |
| `MEMP_NUM_NETBUF` | 128 | Network buffers |
| `MEMP_NUM_NETCONN` | 32 | Network connections |
| `TCP_SND_BUF` | 16384 (16K) | Per-connection send buffer |
| `TCP_WND` | 32768 (32K) | Per-connection receive window |
| `PBUF_POOL_SIZE` | 64 | pbuf pool entries |
| `PBUF_POOL_BUFSIZE` | 1568 | pbuf pool size (MTU + headers) |
| `MEM_SIZE` | 32768 (32K) | Heap size for PBUF_RAM |

### 3.3 Tuning Buffers

If you see `ENOBUFS` errors or poor throughput:

```sh
# Increase per-socket send buffer:
sysctl -w net.inet.tcp.sendspace=65536

# Increase per-socket receive buffer:
sysctl -w net.inet.tcp.recvspace=65536

# Increase UDP buffers:
sysctl -w net.inet.udp.sendspace=16384
sysctl -w net.inet.udp.recvspace=65536
```

**Note**: These values are TCP send/recv space. The effect is similar to
`SO_SNDBUF` / `SO_RCVBUF` on other systems.

### 3.4 Memory Budget

| Component | Size |
|-----------|------|
| lwIP heap (MEM_SIZE) | 32 KB |
| pbuf pool (64 × 1568) | 98 KB |
| TCP PCB (50 × ~300) | ~15 KB |
| Total service memory | ~150-200 KB |

---

## 4. TCP Tuning

### 4.1 Send Buffer vs Throughput

The relationship between send buffer and throughput:

```
Throughput ≈ send_buf / RTT
```

For example:
- Send buffer = 16 KB, RTT = 1 ms → ~16 MB/s (~128 Mbps)
- Send buffer = 64 KB, RTT = 1 ms → ~64 MB/s (~512 Mbps)
- Send buffer = 256 KB, RTT = 1 ms → ~256 MB/s (~2 Gbps)

Increase `TCP_SND_BUF` (recompile) for long-fat pipes.

### 4.2 Receive Window

Similar to send buffer for receive path:

```sh
sysctl -w net.inet.tcp.recvspace=65536
```

### 4.3 Window Scaling (RFC 1323)

lwIP automatically negotiates window scaling if both ends support it.
The scaling factor depends on `TCP_WND`:
- Default (32 KB): scale factor 0
- 64 KB: scale factor 1
- 128 KB+: scale factor 2+

### 4.4 Selective ACK (SACK)

Enabled by default (`LWIP_TCP_SACK_OUT = 1`). SACK improves throughput
on lossy links by allowing the receiver to report non-contiguous data
received:

```sh
# Verify SACK is active:
sysctl net.inet.tcp.sack.enabled
```

### 4.5 Nagle Algorithm

Enabled by default. Disable for low-latency interactive applications:

```c
int flag = 1;
setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &flag, sizeof(flag));
```

### 4.6 TCP Keepalive

```c
int keepalive = 1;
setsockopt(fd, SOL_SOCKET, SO_KEEPALIVE, &keepalive, sizeof(keepalive));

int idle = 60;     /* seconds before probes start */
setsockopt(fd, IPPROTO_TCP, TCP_KEEPIDLE, &idle, sizeof(idle));

int interval = 10; /* seconds between probes */
setsockopt(fd, IPPROTO_TCP, TCP_KEEPINTVL, &interval, sizeof(interval));

int count = 5;     /* probes before death */
setsockopt(fd, IPPROTO_TCP, TCP_KEEPCNT, &count, sizeof(count));
```

---

## 5. Checksum Offload and TSO

### 5.1 Checksum Offload (CSO)

The e1000 driver supports IPv4 header checksum offload for both TX and RX:

- **TX**: Driver computes the IP checksum in hardware
- **RX**: Driver validates the IP checksum in hardware

Enabled by driver capability flags `NDEV_CAP_CS_IP4_TX | NDEV_CAP_CS_IP4_RX`.

**Effect**: Removes ~1-2% CPU overhead from checksum calculation.

### 5.2 TCP Segmentation Offload (TSO)

TSO (also called LSO/GSO) allows the TCP layer to build super-segments
up to 64KB, which the network driver segments into MTU-sized packets.

#### Implementation Detail

TSO uses **Legacy TSE** (TCP Segmentation Engine) descriptors on e1000:

```
TX descriptor (Legacy TSE)
  Buffer addr: ────────────────────────────▶ 64KB data
  Length:      65535
  CMD:         CMD_EOP | CMD_RS | CMD_TSE | CMD_IC
  CSS:         14          (IP header offset)
  CSO:         24          (TCP checksum offset for pseudo-header)
  Special:     1460        (MSS = segment size)
```

#### Enabling TSO

TSO is enabled by default on e1000. Verify:

```sh
# Check driver capabilities:
sysctl minix.lwip.drivers.info
# Look for "tso" in capabilities

# Check TCP is using TSO:
netstat
# Large cwnd + MSS = 1460 indicates TSO active
```

#### TSO Constraints

| Constraint | Reason |
|-----------|--------|
| Max super-segment | 64KB | TCP send buffer limit |
| Min MSS | 536 bytes | lwIP default minimum |
| Only TCP | Protocol limitation | UDP is not segmented |
| e1000 only | Driver support | Other drivers lack TSE |

---

## 6. Interrupt Moderation

### 6.1 e1000 Interrupt Parameters

| Register | Value | Effect |
|----------|-------|--------|
| `EITR` | 500 | Interrupt rate: ~500 Hz target |
| `RDTR` | 128 | RX delay: ~131 µs before interrupt |
| `RADV` | 256 | RX absolute max delay: ~262 µs |

### 6.2 Tuning

Lower values = lower latency, higher CPU usage. Higher values = higher
throughput, higher latency.

```sh
# For latency-sensitive workloads (e.g., NFS):
# Reduce EITR (increase interrupt rate)
# This requires driver recompile
```

### 6.3 Effect on Performance

| Setting | Latency | Throughput | CPU Usage |
|---------|---------|------------|-----------|
| No moderation | ~50 µs | ~300 Mbps | 100% (one core) |
| Default (500/128/256) | ~200 µs | ~1 Gbps | ~60% |
| Aggressive (100/512/1024) | ~500 µs | ~1.2 Gbps | ~30% |

---

## 7. Jumbo Frames

### 7.1 Enabling

```sh
# Enable on e1000 interface:
ifconfig e0 mtu 9000

# Verify:
ifconfig e0
# Expected: "mtu 9000"
```

### 7.2 Requirements

| Component | Must Support |
|-----------|-------------|
| NIC | e1000: jumbo frames support (MTU up to 16110) |
| Switch | All switches on path must support jumbo frames |
| Peer | Remote host must also use jumbo frames |
| lwIP | `NDEV_ETH_PACKET_MAX` = 65535 (configured) |

### 7.3 Benefits

- **Fewer interrupts per byte**: ~6× fewer packets for bulk transfer
- **Less CPU overhead**: Reduced per-packet processing
- **Better throughput**: Less protocol overhead (headers/ACKs per byte)

### 7.4 Limitations

- Not supported on all drivers (e1000 only)
- Requires infrastructure support
- No benefit for small-packet workloads (DNS, SSH, chat)

---

## 8. Multi-Queue

### 8.1 Software Multi-Queue

Two send queues with round-robin selection:

```
ndev_send(pbuf, len, queue_index)
    │
    ├── queue_index = 0:  sendq[0]
    ├── queue_index = 1:  sendq[1]
    │
    ▼
ethif_get_sendq() → 0 → 1 → 0 → 1 ...
```

### 8.2 Benefits

- **Reduced contention**: Multiple streams can use different queues
- **Better cache behavior**: Each queue's descriptors stay hot in cache
- **Fairness**: Round-robin prevents one stream from starving others

### 8.3 Caveats

- **Software only**: Not hardware RSS — queues are software constructs
- **Single-threaded**: lwIP service is single-threaded, so queues
  serialize at the driver level
- **e1000 only**: Other drivers use single queue

---

## 9. Loopback Fast Path

### 9.1 Synchronous Delivery

Loopback traffic (lo0) bypasses the IPC/driver path entirely:

```c
static err_t
loopif_output(struct netif *netif, struct pbuf *p)
{
    struct ifdev *ifdev = netif->ifdev;
    struct loopif *loop = (struct loopif *)ifdev;

    if (loop->fast_depth < LOOPIF_FAST_DEPTH_MAX) {
        loop->fast_depth++;
        pbuf_ref(p);              /* Zero-copy reference */
        err = ifdev_input(ifdev, p);  /* Direct delivery */
        loop->fast_depth--;
    } else {
        /* Fallback to async path via ndev_send/recv */
        err = ndev_send(ifdev_ndev(ifdev), p, 0);
    }
}
```

### 9.2 Performance

| Metric | Before | After (Fast Path) |
|--------|--------|-------------------|
| Loopback throughput | ~1-2 Gbps | ~5-10 Gbps |
| Latency | ~100 µs (IPC round-trip) | ~10 µs (direct call) |
| CPU usage | 100% (IPC context switches) | ~20% (direct execution) |

### 9.3 Depth Guard

The `LOOPIF_FAST_DEPTH_MAX = 8` guard prevents stack overflow from
recursive delivery (e.g., TCP send → loopback input → TCP ACK → ...).

---

## 10. Monitoring with netstat

### 10.1 netstat Modes

```sh
# TCP connections with congestion metrics:
netstat
# Example output:
# Proto Recv-Q Send-Q  Local Address Foreign Address  State   cwnd   rtt   rto  nrtx   mss
# tcp   0      49728  192.168.1.100:22 10.0.2.2:34567  EST    24672   145   600     0  1460
#
# Columns:
#   cwnd  — Congestion window (bytes)
#   rtt   — Round-trip time (ms)
#   rto   — Retransmission timeout (ms)
#   nrtx  — Number of retransmissions
#   mss   — Maximum segment size

# Interface statistics:
netstat -i
# Example:
# Name  MTU   Network     Address        Ipkts Ierrs Opkts Oerrs Collis Drops
# e0    1500  192.168.1.0 192.168.1.100  45032     0 89211     0     0     2
# lo0   16384 127.0.0.0   127.0.0.1      1205      0  1205     0     0     0

# Protocol statistics:
netstat -s
# TCP
#   89211 packets sent
#   45032 packets received
#   12345 segments sent
#   91234 segments received
#   0 retransmit timeouts
#   0 connections reset
#   3 connections established
#   send buffer: 16384 bytes
#   recv buffer: 32768 bytes

# Driver information:
netstat -d
# Driver: e1000
#   Endpoint: 12345
#   SendQ[0]: 0/512
#   SendQ[1]: 0/512
#   RecvQ:    0/512
#   Caps:     cs_ip4_tx, cs_ip4_rx, tso, jumbo, multiq
```

### 10.2 Key Metrics to Watch

| Metric | netstat column | Good | Bad |
|--------|---------------|------|-----|
| Retransmissions | nrtx | 0 | >5 (congestion) |
| Interface errors | Ierrs/Oerrs | 0 | >0 (hardware) |
| Collisions | Collis | 0 | >0 (half-duplex) |
| Drops | Drops | 0 | >0 (congestion/buffer) |
| Send buffer | Send-Q | 0 | >0 (bottleneck) |
| CWND | cwnd | = ssthresh | < ssthresh (loss) |

### 10.3 Using sysctl

```sh
# Full per-interface statistics:
sysctl minix.lwip.ifaces

# TCP extended metrics (per-connection):
sysctl minix.lwip.tcp_ext

# Latency histograms:
sysctl minix.lwip.latency
```

---

## 11. Performance Alerts

### 11.1 Alert Types

| Alert | Threshold (default) | Level | Source |
|-------|--------------------|-------|--------|
| `packet-drop` | 100 events/tick | WARNING | `ifdev_input()` |
| `tcp-rst` | 50 events/tick | WARNING | `tcpsock_event_err()` |
| `oom` | 10 events/tick | ERROR | `tcpsock_alloc_buf()` |
| `high-latency` | 100 ms | WARNING | All latency recordings |
| `rate-limit-hit` | 100 events/tick | WARNING | ICMP/ARP/NDP rate limiter |

### 11.2 Configuration

```sh
# Disable all alerts:
sysctl minix.lwip.alerts.enabled=0

# Change drop threshold to 50:
sysctl minix.lwip.alerts.drop_thresh=50

# Change latency threshold to 200ms:
sysctl minix.lwip.alerts.latency_us=200000

# Change RST threshold to 20:
sysctl minix.lwip.alerts.rst_thresh=20

# Change OOM threshold to 5:
sysctl minix.lwip.alerts.oom_thresh=5

# Change rate-limit threshold to 50:
sysctl minix.lwip.alerts.rate_limit_thresh=50
```

### 11.3 Alert Output

Alerts go to **syslog** (facility `LOG_DAEMON`):

```
lwip[123]: perf: packet-drop on e0 threshold exceeded (100 events)
lwip[123]: perf: high-latency 152.345 ms (threshold 100 ms)
lwip[123]: perf: oom threshold exceeded (10 events)
lwip[123]: perf: tcp-rst threshold exceeded (50 events)
lwip[123]: perf: rate-limit-hit threshold exceeded (100 events)
```

### 11.4 Cooldown

Each alert type has a **30-second cooldown** to prevent log flooding.
After an alert fires, the same type will not fire again for 30 seconds.

**Counters reset every second** (driven by `perf_alerts_tick()`),
triggered by the CLOCK notification in the main event loop.

---

## 12. Latency Histograms

### 12.1 Buckets

| Bucket | Range | Label |
|--------|-------|-------|
| 0 | < 100 µs | microsecond |
| 1 | 100-500 µs | fast |
| 2 | 500 µs-1 ms | moderate |
| 3 | 1-5 ms | slow |
| 4 | 5-20 ms | very slow |
| 5 | 20-100 ms | degraded |
| 6 | > 100 ms | critical |

### 12.2 Per-Protocol

| Protocol | Stat name | Operations recorded |
|----------|-----------|-------------------|
| UDP | `minix.lwip.latency.udp_send` | `udpsock_send()` |
| TCP | `minix.lwip.latency.tcp_connect` | `tcpsock_connect()` |
| TCP | `minix.lwip.latency.tcp_send` | `tcpsock_send()` |

### 12.3 Viewing

```sh
sysctl minix.lwip.latency
# Example output:
# udp_send:  [0: 8932] [1: 1045] [2: 123] [3: 45] [4: 12] [5: 3] [6: 1]
# tcp_connect: [0: 234] [1: 567] [2: 89] [3: 12] [4: 2] [5: 0] [6: 0]
# tcp_send:  [0: 45001] [1: 8234] [2: 1456] [3: 234] [4: 56] [5: 7] [6: 2]
```

---

## 13. Benchmark Methodology

### 13.1 Tools

| Tool | Use | Source |
|------|-----|--------|
| `iperf3` | TCP/UDP throughput | Host or guest |
| `ping` | Round-trip latency | Host |
| `netstat` | Connection metrics | Guest (built-in) |
| `sysctl` | Perf alerts, latency | Guest (built-in) |

### 13.2 Test Procedure

```sh
# 1. Start iperf3 server on one end:
iperf3 -s

# 2. Run throughput test (10 seconds, 4 parallel streams):
iperf3 -c 192.168.1.100 -t 10 -P 4

# 3. Check retransmissions:
netstat | grep EST | awk '{print $NF}'

# 4. Check interface errors:
netstat -i

# 5. Check latency histogram:
sysctl minix.lwip.latency

# 6. Check performance alerts:
sysctl minix.lwip.alerts
```

### 13.3 Baseline Script

Use `scripts/run_net_bench.sh` for automated baselines:

```bash
# Quick test (1 iteration, 5s per test):
./scripts/run_net_bench.sh --quick

# Full test (3 iterations, 10s per test):
./scripts/run_net_bench.sh --full

# Compare against previous baseline:
./scripts/run_net_bench.sh --full --baseline net-bench-results/baseline.json
```

---

> **See also**: `docs/networking-guide.md` for practical setup,
> `docs/network-architecture.md` for internal architecture,
> `docs/network-security.md` for security features.
