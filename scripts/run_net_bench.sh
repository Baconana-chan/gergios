#!/bin/bash
# run_net_bench.sh — Baseline network benchmarks for MINIX in QEMU
#
# Measures TCP/UDP throughput, latency, and connection rate for
# the MINIX lwIP network stack. Results are saved as baseline
# for comparison after stack optimizations (Phase 1+).
#
# Usage:
#   ./scripts/run_net_bench.sh                      # Full benchmark suite
#   ./scripts/run_net_bench.sh --quick               # Quick (1 iteration each)
#   ./scripts/run_net_bench.sh --tcp-only            # TCP only
#   ./scripts/run_net_bench.sh --mode tap            # Use tap networking
#   ./scripts/run_net_bench.sh --baseline <file>     # Compare against baseline
#   ./scripts/run_net_bench.sh --list                # List available benchmarks
#   ./scripts/run_net_bench.sh --help                # Show help
#
# Requirements:
#   - qemu-system-x86_64 (for QEMU mode)
#   - iperf3, ping on host
#   - bash 4+ (for arrays)
#   - MINIX bootable image with iperf3 (optional, for guest benchmarks)
#
# Output:
#   net-bench-results/ — raw data and summary
#   net-bench-results/summary.md — Markdown report
#   net-bench-results/baseline.json — machine-readable baseline

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/net-bench-results}"
BASELINE_FILE="${RESULTS_DIR}/baseline.json"
MODE="quick"
FILTER=""
COMPARE_BASELINE=""
GUEST_IP="10.0.2.15"  # SLiRP guest IP
HOST_IP="10.0.2.2"    # SLiRP gateway

mkdir -p "$RESULTS_DIR"

# ─── Help ─────────────────────────────────────────────────────────────────
show_help() {
    cat <<EOF
Usage: $0 [options]

Baseline network benchmarks for MINIX lwIP stack.

Options:
  --quick              Quick mode (1 iteration per test)
  --full               Full suite (3+ iterations)
  --tcp-only           TCP benchmarks only
  --udp-only           UDP benchmarks only
  --latency-only       Latency (ping) benchmarks only
  --mode <mode>        Network mode (user/tap, default: user)
  --guest-ip <ip>      Guest IP address
  --host-ip <ip>       Host IP address
  --baseline <file>    Compare results against baseline JSON
  --list               List available benchmark groups
  --help               Show this help

Output:
  Results in \${RESULTS_DIR:-net-bench-results/}
  Summary: summary.md + baseline.json
EOF
    exit 0
}

# ─── Parse arguments ─────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick)         MODE="quick"; shift ;;
        --full)          MODE="full"; shift ;;
        --tcp-only)      FILTER="tcp"; shift ;;
        --udp-only)      FILTER="udp"; shift ;;
        --latency-only)  FILTER="latency"; shift ;;
        --mode)          NET_MODE="$2"; shift 2 ;;
        --guest-ip)      GUEST_IP="$2"; shift 2 ;;
        --host-ip)       HOST_IP="$2"; shift 2 ;;
        --baseline)      COMPARE_BASELINE="$2"; shift 2 ;;
        --list)
            echo "Available benchmark groups: tcp, udp, latency, connect"
            exit 0 ;;
        --help|-h)       show_help ;;
        *) echo "Unknown: $1"; show_help ;;
    esac
done

: ${NET_MODE:=user}

# ─── Find QEMU (for system info in JSON output) ──────────────────────────
QEMU_BIN=""
for q in qemu-system-x86_64 qemu-system-i386 qemu-kvm; do
    command -v "$q" &>/dev/null && { QEMU_BIN="$q"; break; }
done
QEMU_VER="${QEMU_BIN:+$($QEMU_BIN --version 2>/dev/null | head -1)}"

# ─── Check tools ─────────────────────────────────────────────────────────
BENCH_TOOLS=""
for tool in ping iperf3; do
    if command -v "$tool" &>/dev/null; then
        BENCH_TOOLS+=" $tool"
    else
        echo -e "${YELLOW}Warning: $tool not found — skipping related benchmarks${NC}"
    fi
done
echo -e "${GREEN}Available tools:${BENCH_TOOLS}${NC}"

# ─── Benchmark configuration ─────────────────────────────────────────────
# Number of iterations
if [ "$MODE" = "full" ]; then
    IPERF_ITER=3
    PING_COUNT=100
    CONNECT_ITER=5
else  # quick
    IPERF_ITER=1
    PING_COUNT=10
    CONNECT_ITER=1
fi

# iperf3 durations
IPERF_TIME=10  # seconds per test
if [ "$MODE" = "quick" ]; then
    IPERF_TIME=5
fi

# ─── Results file ────────────────────────────────────────────────────────
SUMMARY_MD="${RESULTS_DIR}/summary.md"
SUMMARY_TXT="${RESULTS_DIR}/summary.txt"
RAW_DIR="${RESULTS_DIR}/raw"
mkdir -p "$RAW_DIR"

{
    echo "# MINIX Network Baseline Benchmarks"
    echo ""
    echo "**Date**: $(date)"
    echo "**Mode**: ${MODE}"
    echo "**Network**: ${NET_MODE} (guest=${GUEST_IP}, host=${HOST_IP})"
    echo "**Host**: $(uname -a 2>/dev/null || echo '?')"
    echo ""
    echo "## Results"
    echo ""
    echo "| Benchmark | Result | Unit | Notes |"
    echo "|-----------|--------|------|-------|"
} > "$SUMMARY_MD"

# JSON baseline start
echo "{" > "$BASELINE_FILE"
echo '  "date": "'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'",' >> "$BASELINE_FILE"
echo '  "mode": "'"${MODE}"'",' >> "$BASELINE_FILE"
echo '  "network_mode": "'"${NET_MODE}"'",' >> "$BASELINE_FILE"
echo '  "results": [' >> "$BASELINE_FILE"
FIRST_RESULT=true

# Helper: record a measurement
record() {
    local name="$1" value="$2" unit="$3" notes="${4:-}"

    if [ "$FIRST_RESULT" = true ]; then
        FIRST_RESULT=false
    else
        echo "," >> "$BASELINE_FILE"
    fi
    echo -n "    {\"name\":\"${name}\",\"value\":${value},\"unit\":\"${unit}\"" >> "$BASELINE_FILE"
    [ -n "$notes" ] && echo -n ",\"notes\":\"${notes}\"" >> "$BASELINE_FILE"
    echo "}" >> "$BASELINE_FILE"

    echo "| ${name} | ${value} | ${unit} | ${notes} |" >> "$SUMMARY_MD"
}

# ─── Check if we can reach the guest ─────────────────────────────────────
check_guest() {
    if command -v ping &>/dev/null; then
        ping -c 1 -W 2 "$GUEST_IP" &>/dev/null && return 0
    fi
    return 1
}

echo ""
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo -e "${BLUE}MINIX Network Baseline Benchmarks${NC}"
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo "Mode: ${MODE} | Network: ${NET_MODE} | Iterations: TCP/UDP=${IPERF_ITER}, ping=${PING_COUNT}"
echo "Guest: ${GUEST_IP}  Host: ${HOST_IP}"
echo ""

# ─── Latency benchmarks ──────────────────────────────────────────────────
if [ -z "$FILTER" ] || [ "$FILTER" = "latency" ]; then
    echo -e "${YELLOW}[1/4] Latency benchmarks${NC}"

    if command -v ping &>/dev/null; then
        # Ping loopback (host-side, for reference)
        echo -n "  ping loopback (host): "
        LO_RESULT=$(ping -c "$PING_COUNT" 127.0.0.1 2>/dev/null | tail -1 | \
            sed -n 's/.* = \([0-9.]*\)\/\([0-9.]*\)\/\([0-9.]*\).*/\1 \2 \3/p' || echo "0 0 0")
        LO_MIN=$(echo "$LO_RESULT" | awk '{print $1}')
        LO_AVG=$(echo "$LO_RESULT" | awk '{print $2}')
        LO_MAX=$(echo "$LO_RESULT" | awk '{print $3}')
        echo -e "${CYAN}min=${LO_MIN}ms avg=${LO_AVG}ms max=${LO_MAX}ms${NC}"
        record "ping_lo_avg" "$LO_AVG" "ms" "loopback latency (host)"
        record "ping_lo_min" "$LO_MIN" "ms"
        record "ping_lo_max" "$LO_MAX" "ms"

        # Ping guest (if reachable)
        if check_guest; then
            echo -n "  ping guest (${GUEST_IP}): "
            GUEST_RESULT=$(ping -c "$PING_COUNT" -W 2 "$GUEST_IP" 2>/dev/null | tail -1 | \
                sed -n 's/.* = \([0-9.]*\)\/\([0-9.]*\)\/\([0-9.]*\).*/\1 \2 \3/p' || echo "0 0 0")
            GU_MIN=$(echo "$GUEST_RESULT" | awk '{print $1}')
            GU_AVG=$(echo "$GUEST_RESULT" | awk '{print $2}')
            GU_MAX=$(echo "$GUEST_RESULT" | awk '{print $3}')
            echo -e "${CYAN}min=${GU_MIN}ms avg=${GU_AVG}ms max=${GU_MAX}ms${NC}"
            record "ping_guest_avg" "${GU_AVG:-0}" "ms" "QEMU user mode RTT"
            record "ping_guest_min" "${GU_MIN:-0}" "ms"
            record "ping_guest_max" "${GU_MAX:-0}" "ms"

            # Packet loss
            LOSS=$(ping -c "$PING_COUNT" -W 2 "$GUEST_IP" 2>/dev/null | \
                grep -oP '\d+(?=% packet loss)' || echo "0")
            record "ping_guest_loss" "$LOSS" "%" "packet loss to guest"
        else
            echo -e "  ${YELLOW}Guest ${GUEST_IP} unreachable — skipping ping benchmarks${NC}"
            echo -e "  ${YELLOW}(MINIX must be running in QEMU with networking)${NC}"
        fi
    fi
fi

# ─── TCP throughput benchmarks ──────────────────────────────────────────
if [ -z "$FILTER" ] || [ "$FILTER" = "tcp" ]; then
    echo -e "${YELLOW}[2/4] TCP throughput benchmarks${NC}"

    if command -v iperf3 &>/dev/null; then
        # Host loopback reference
        echo -n "  iperf TCP loopback (host): "
        HOST_TCP=$(iperf3 -c 127.0.0.1 -t "$IPERF_TIME" --json 2>/dev/null || true)
        TCP_LO_BITS=$(echo "$HOST_TCP" | python3 -c "
import json,sys
try:
    d=json.load(sys.stdin)
    print(d['end']['sum_received']['bits_per_second'])
except: print(0)" 2>/dev/null || echo 0)
        TCP_LO_MBPS=$(echo "scale=1; $TCP_LO_BITS / 1000000" | bc 2>/dev/null || echo "0")
        echo -e "${CYAN}${TCP_LO_MBPS} Mbps${NC}"
        record "tcp_loopback" "$TCP_LO_MBPS" "Mbps" "host loopback reference"

        # Host to guest (if reachable, iperf server on guest)
        if check_guest; then
            # Requires iperf3 server running on guest
            echo -n "  iperf TCP host→guest (${GUEST_IP}): "
            H2G_TCP=$(iperf3 -c "$GUEST_IP" -t "$IPERF_TIME" --json 2>/dev/null || true)
            H2G_BITS=$(echo "$H2G_TCP" | python3 -c "
import json,sys
try:
    d=json.load(sys.stdin)
    print(d['end']['sum_received']['bits_per_second'])
except: print(0)" 2>/dev/null || echo 0)
            H2G_MBPS=$(echo "scale=1; $H2G_BITS / 1000000" | bc 2>/dev/null || echo "0")
            echo -e "${CYAN}${H2G_MBPS} Mbps${NC}"
            record "tcp_host_to_guest" "$H2G_MBPS" "Mbps" \
                "MINIX lwIP TCP receive (iperf3 client→server)"
        else
            echo -e "  ${YELLOW}Guest unreachable — skipping iperf TCP benchmarks${NC}"
        fi
    else
        echo -e "  ${YELLOW}iperf3 not found — skipping TCP throughput${NC}"
    fi
fi

# ─── UDP throughput benchmarks ──────────────────────────────────────────
if [ -z "$FILTER" ] || [ "$FILTER" = "udp" ]; then
    echo -e "${YELLOW}[3/4] UDP throughput benchmarks${NC}"

    if command -v iperf3 &>/dev/null; then
        # Host loopback reference
        echo -n "  iperf UDP loopback (host): "
        HOST_UDP=$(iperf3 -c 127.0.0.1 -t "$IPERF_TIME" -u -b 1000M --json 2>/dev/null || true)
        UDP_LO_BITS=$(echo "$HOST_UDP" | python3 -c "
import json,sys
try:
    d=json.load(sys.stdin)
    print(d['end']['sum']['bits_per_second'])
except: print(0)" 2>/dev/null || echo 0)
        UDP_LO_MBPS=$(echo "scale=1; $UDP_LO_BITS / 1000000" | bc 2>/dev/null || echo "0")
        echo -e "${CYAN}${UDP_LO_MBPS} Mbps${NC}"
        record "udp_loopback" "$UDP_LO_MBPS" "Mbps" "host UDP loopback reference"

        if check_guest; then
            echo -n "  iperf UDP host→guest (${GUEST_IP}): "
            H2G_UDP=$(iperf3 -c "$GUEST_IP" -t "$IPERF_TIME" -u -b 100M --json 2>/dev/null || true)
            H2G_BITS=$(echo "$H2G_UDP" | python3 -c "
import json,sys
try:
    d=json.load(sys.stdin)
    print(d['end']['sum']['bits_per_second'])
except: print(0)" 2>/dev/null || echo 0)
            H2G_MBPS=$(echo "scale=1; $H2G_BITS / 1000000" | bc 2>/dev/null || echo "0")
            H2G_LOSS=$(echo "$H2G_UDP" | python3 -c "
import json,sys
try:
    d=json.load(sys.stdin)
    print(d['end']['sum']['packets'],d['end']['sum']['lost'])
except: print('0 0')" 2>/dev/null || echo "0 0")
            echo -e "${CYAN}${H2G_MBPS} Mbps${NC}"
            record "udp_host_to_guest" "$H2G_MBPS" "Mbps" \
                "MINIX lwIP UDP receive"
            record "udp_packet_loss" "$(echo $H2G_LOSS | awk '{if($1>0) print $2/$1*100; else print 0}')" "%"
        else
            echo -e "  ${YELLOW}Guest unreachable — skipping iperf UDP benchmarks${NC}"
        fi
    fi
fi

# ─── TCP connection rate benchmark ───────────────────────────────────────
if [ -z "$FILTER" ] || [ "$FILTER" = "connect" ] || [ "$FILTER" = "tcp" ]; then
    echo -e "${YELLOW}[4/4] TCP connection rate${NC}"

    # Use a simple bash-based connect test
    if command -v python3 &>/dev/null; then
        echo -n "  TCP connect rate (host loopback): "

        # Minimal TCP connection rate test using python
        CONNECT_RATE=$(python3 -c "
import socket, time

def test_connect_rate(host, port=0):
    # Simple test: open/close connections as fast as possible
    # (Uses a dummy listener on localhost)
    import threading

    results = []

    # Start a listener
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(('127.0.0.1', 0))
    _, port = listener.getsockname()
    listener.listen(128)

    def accept_loop():
        while True:
            try:
                conn, _ = listener.accept(timeout=0.1)
                conn.close()
            except:
                break

    t = threading.Thread(target=accept_loop, daemon=True)
    t.start()

    # Measure connection rate
    count = 0
    start = time.time()
    end = start + 2.0
    while time.time() < end:
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.settimeout(0.1)
            s.connect(('127.0.0.1', port))
            s.close()
            count += 1
        except:
            pass

    elapsed = time.time() - start
    listener.close()
    return count / elapsed if elapsed > 0 else 0

rate = test_connect_rate()
print(f'{rate:.0f}')
" 2>/dev/null || echo "0")
        echo -e "${CYAN}${CONNECT_RATE} conn/s${NC}"
        record "tcp_connect_rate_host" "$CONNECT_RATE" "conn/s" "host loopback reference"

        if check_guest; then
            echo -n "  TCP connect rate guest→host: "
            # Placeholder: requires MINIX-side test tool
            echo -e "${YELLOW}skipped (requires MINIX test binary)${NC}"
            record "tcp_connect_rate_guest" "0" "conn/s" "TODO: requires MINIX-side tool"
        fi
    else
        echo -e "  ${YELLOW}python3 not found — skipping connect rate${NC}"
    fi
fi

# ─── Finalize baseline JSON ─────────────────────────────────────────────
echo "" >> "$BASELINE_FILE"
echo "  ]," >> "$BASELINE_FILE"
echo '  "system": {' >> "$BASELINE_FILE"
echo '    "hostname": "'"$(hostname 2>/dev/null || echo '?')"'",' >> "$BASELINE_FILE"
echo '    "kernel": "'"$(uname -r 2>/dev/null || echo '?')"'",' >> "$BASELINE_FILE"
echo '    "cpu": "'"$(nproc 2>/dev/null || echo '?')"' cores",' >> "$BASELINE_FILE"
echo '    "qemu": "'"${QEMU_VER:-?}"'",' >> "$BASELINE_FILE"
echo '    "iperf3": "'"$(iperf3 --version 2>/dev/null | head -1)"'"' >> "$BASELINE_FILE"
echo '  }' >> "$BASELINE_FILE"
echo "}" >> "$BASELINE_FILE"

# ─── Summary ─────────────────────────────────────────────────────────────
echo ""
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo -e "${GREEN}Benchmarks complete${NC}"
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo ""
echo -e "${CYAN}=== Results ===${NC}"
grep "^|" "$SUMMARY_MD" | head -20
echo ""
echo "Detailed: ${SUMMARY_MD}"
echo "JSON:     ${BASELINE_FILE}"
echo "Raw:      ${RAW_DIR}/"

# Compare against previous baseline if requested
if [ -n "$COMPARE_BASELINE" ] && [ -f "$COMPARE_BASELINE" ]; then
    echo ""
    echo -e "${YELLOW}Comparison with previous baseline:${NC}"
    python3 <<PYEOF 2>/dev/null || echo "  (comparison failed)"
import json

with open('${BASELINE_FILE}') as f:
    new_data = json.load(f)
with open('${COMPARE_BASELINE}') as f:
    old_data = json.load(f)

new_results = {r['name']: r for r in new_data['results']}
old_results = {r['name']: r for r in old_data['results']}

print(f"{'Benchmark':<30} {'Old':>12} {'New':>12} {'Change':>12}")
print("-"*68)
for name in sorted(set(list(old_results.keys()) + list(new_results.keys()))):
    old = old_results.get(name, {})
    new = new_results.get(name, {})
    old_v = old.get('value', 0)
    new_v = new.get('value', 0)
    unit = new.get('unit', old.get('unit', ''))
    change = ''
    if old_v and old_v > 0:
        pct = ((new_v - old_v) / old_v) * 100
        sign = '+' if pct >= 0 else ''
        change = f'{sign}{pct:.1f}%'
    if name in old and name in new:
        print(f"{name:<30} {old_v:>10.2f} {unit:<3} {new_v:>10.2f} {unit:<3} {change:>12}")
    elif name in old:
        print(f"{name:<30} {old_v:>10.2f} {unit:<3} {'(removed)':>23}")
    else:
        print(f"{name:<30} {'(new)':>15} {new_v:>10.2f} {unit:<3}")
PYEOF
fi

echo ""
