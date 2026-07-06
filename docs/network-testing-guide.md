# Network Testing Guide — GergiOS

> **Last updated**: July 2026
> **Related**: `scripts/run_net_test.sh`, `scripts/run_net_bench.sh`, `planning/25_network_stack_modernization.md`

## Overview

This guide explains how to use the network test infrastructure for developing and benchmarking the MINIX lwIP network stack. The test harness boots MINIX in QEMU with configurable networking and runs baseline benchmarks.

**Prerequisites**:
- `qemu-system-x86_64` — `apt install qemu-system-x86`
- `iperf3` — `apt install iperf3`
- A bootable MINIX image (see [BUILDING.md](./BUILDING.md))

---

## 1. Quick Start

### 1.1 Test MINIX Boot with Networking

```bash
# Build a MINIX image and boot with default (user/SLiRP) networking:
./scripts/run_net_test.sh --mode user --timeout 60

# Or with a pre-built image:
./scripts/run_net_test.sh --image ~/minix_x86.img --mode user
```

This boots MINIX, captures the serial console, and runs basic network checks:
- Interface detection (e1000 in boot log)
- IPv6 support (INET6 compile flag)
- Network errors in boot log

Results go to `net-test-results/`.

### 1.2 Run Baseline Benchmarks

```bash
# Quick benchmark (1 iteration, 5s per test):
./scripts/run_net_bench.sh --quick

# Full benchmark (3 iterations, 10s per test):
./scripts/run_net_bench.sh --full

# Specific benchmarks:
./scripts/run_net_bench.sh --tcp-only
./scripts/run_net_bench.sh --latency-only
```

Results go to `net-bench-results/`:
- `summary.md` — Markdown report
- `baseline.json` — machine-readable JSON for CI comparison

### 1.3 Compare Against Baseline

```bash
# Run benchmarks and compare with previous baseline:
./scripts/run_net_bench.sh --full --baseline net-bench-results/baseline.json

# This shows changes:
#   ping_guest_avg      0.15 ms     0.12 ms    -20.0%
#   tcp_host_to_guest   512.3 Mbps  687.1 Mbps  +34.1%
```

---

## 2. Network Modes

### 2.1 User Mode (SLiRP) — Default

QEMU's built-in SLiRP NAT. No host configuration needed.

```
┌─────────┐     ┌──────────────┐     ┌──────────────┐
│  Host   │────▶│  QEMU SLiRP  │────▶│  MINIX       │
│ iperf3  │     │  10.0.2.2    │     │  10.0.2.15   │
└─────────┘     └──────────────┘     └──────────────┘
```

**Limitations**:
- Host can initiate connections to guest only via port forwarding
- No raw packet access (no TUN/TAP)
- Higher latency than tap mode

**Port forwarding** (configured by default):
```bash
# Host port 2222 → Guest port 22 (SSH)
ssh -p 2222 root@localhost
```

### 2.2 Tap Bridge Mode

A Linux bridge connects the host and guest directly.

```bash
# Requires root:
sudo ./scripts/run_net_test.sh --mode tap

# Or customize the bridge:
sudo ./scripts/run_net_test.sh --mode tap --tap-bridge mybr0 --tap-iface mytap0
```

```
┌─────────┐     ┌──────────┐     ┌──────────┐
│  Host   │────▶│  br0     │────▶│  tap0    │────▶│  MINIX  │
│  192.168.100.1  │         │     │          │     │  192.168.100.2
└─────────┘     └──────────┘     └──────────┘
```

**Advantages**:
- Full bidirectional connectivity
- Raw packet access (for tcpdump on host)
- Lower latency

### 2.3 Isolated Mode

No network at all. Useful for testing that MINIX boots cleanly without networking.

```bash
./scripts/run_net_test.sh --mode isolated
```

---

## 3. Benchmark Suite

### 3.1 TCP Throughput

**Tool**: `iperf3` (host) → iperf3 server (MINIX guest)

**Procedure**:
```bash
# On MINIX guest:
iperf3 -s

# On host:
iperf3 -c 10.0.2.15 -t 10
```

**Expected baseline** (lwIP 2.1.x, QEMU e1000):
| Direction | Expected | Notes |
|-----------|----------|-------|
| Host→Guest | ~500-800 Mbps | QEMU e1000, single queue |
| Guest→Host | ~300-600 Mbps | lwIP TX path |
| Loopback | ~1-2 Gbps | Host reference |

### 3.2 UDP Throughput

**Tool**: `iperf3 -u` (host) → iperf3 server (MINIX guest)

```bash
# On host:
iperf3 -c 10.0.2.15 -u -b 100M -t 10
```

**Expected** (UDP 1400-byte packets):
| Direction | Expected | Packet Loss |
|-----------|----------|-------------|
| Host→Guest | ~300-500 Mbps | <1% (with pacing) |

### 3.3 Latency

**Tool**: `ping` (host → MINIX guest)

```bash
ping -c 100 10.0.2.15
```

**Expected**:
| Mode | RTT avg | Notes |
|------|---------|-------|
| User (SLiRP) | ~0.5-2 ms | Higher due to NAT |
| Tap | <0.3 ms | Direct bridge |

### 3.4 TCP Connection Rate

**Tool**: Custom python script (host loopback) → custom test (MINIX).

```bash
# Host-only reference (loopback):
./scripts/run_net_bench.sh --tcp-only
```

**Expected**: ~1000-5000 conn/s depending on mode.

---

## 4. Test Results in CI

### 4.1 CI Integration

The benchmark scripts output JSON in CI-compatible format:

```bash
./scripts/run_net_bench.sh --quick
cat net-bench-results/baseline.json
```

Example GitHub Actions workflow:
```yaml
- name: Network baseline benchmarks
  run: |
    ./scripts/run_net_test.sh --mode user --timeout 120
    ./scripts/run_net_bench.sh --quick --mode user
  continue-on-error: true
  timeout-minutes: 5

- name: Upload benchmark results
  uses: actions/upload-artifact@v4
  with:
    name: net-bench-results
    path: net-bench-results/
```

### 4.2 Regression Detection

Compare current results against a stored baseline:

```bash
# Store reference baseline:
cp net-bench-results/baseline.json net-bench-results/baseline-reference.json

# Compare later run:
./scripts/run_net_bench.sh --full \
  --baseline net-bench-results/baseline-reference.json
```

Significant regressions (>10% drop) should block the PR.

---

## 5. Common Issues

### 5.1 QEMU e1000 performance

The QEMU emulated e1000 (82540EM) has limited performance:

| Issue | Impact | Workaround |
|-------|--------|------------|
| Single TX/RX queue | ~500-800 Mbps max | Use tap mode for lower overhead |
| No TSO/GRO hardware offload | CPU-bound on host | Compare with virtio-net for reference |
| Emulated DMA | Slower than real HW | Use `-accel kvm` (Linux host) |

Always enable KVM for benchmarks:
```bash
qemu-system-x86_64 --enable-kvm ...
```

The `run_net_test.sh` script auto-detects KVM.

### 5.2 Guest Not Reachable

If the guest is not reachable:
```bash
# 1. Verify QEMU is running:
ps aux | grep qemu

# 2. Check serial output for boot progress:
tail -50 net-test-results/serial.txt

# 3. Check that MINIX has the e1000 driver:
# Look for "e1000" in boot log

# 4. Check MINIX networking:
grep -E "(e1000|eth|inet|IP)" net-test-results/serial.txt
```

### 5.3 iperf3 Not Available on MINIX

The benchmark script currently runs iperf3 on the host. For guest-side benchmarks, iperf3 must be cross-compiled for MINIX and included in the boot image:

```bash
# Cross-compile iperf3 for MINIX:
CC=x86_64-elf64-minix-gcc \
./configure --host=x86_64-elf64-minix
make
# Copy binary into the release image
```

---

## 6. File Layout Reference

```
scripts/
  run_net_test.sh           ← QEMU test harness
  run_net_bench.sh          ← Baseline benchmarks
  benchmark-data/           ← Benchmark input data

net-test-results/           ← Created by run_net_test.sh
  serial.txt                ← QEMU serial console output
  tests.log                 ← Test results log
  summary.txt               ← Test summary

net-bench-results/          ← Created by run_net_bench.sh
  summary.md                ← Markdown report
  baseline.json             ← JSON baseline (machine-readable)
  raw/                      ← Raw iperf output
```
