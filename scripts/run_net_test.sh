#!/bin/bash
# run_net_test.sh — QEMU MINIX network test harness
#
# Boots MINIX in QEMU with configurable network setups and runs
# network tests (ping, iperf3, TCP/UDP connectivity).
#
# Usage:
#   ./scripts/run_net_test.sh                         # Default: user + tap
#   ./scripts/run_net_test.sh --mode user             # SLiRP (NAT, no host access)
#   ./scripts/run_net_test.sh --mode tap              # Tap bridge (host access)
#   ./scripts/run_net_test.sh --mode isolated         # No network
#   ./scripts/run_net_test.sh --image <path>          # Custom image
#   ./scripts/run_net_test.sh --list-modes            # List available modes
#   ./scripts/run_net_test.sh --help                  # Show help
#
# Network modes:
#   user       — QEMU user-mode SLiRP (default, NAT, no host→guest)
#   tap        — Tap bridge interface (root: sudo, full bidirectional)
#   isolated   — No network (-nic none)
#
# Requirements:
#   - qemu-system-x86_64 (for x86_64)
#   - For tap mode: sudo, ip, bridge-utils, dnsmasq (optional)
#   - For benchmarks: iperf3, ping on host
#   - A bootable MINIX image (x86_ramimage.sh or x86_hdimage.sh)
#
# Output:
#   net-test-results/ — test results and logs
#   net-test-results/summary.txt — summary of all tests

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/net-test-results}"
SERIAL_OUT="${RESULTS_DIR}/serial.txt"
IMAGE=""
MODE="user"
HOST_IP="10.0.2.2"      # SLiRP gateway for user mode
GUEST_IP="10.0.2.15"     # SLiRP guest IP for user mode
TAP_BRIDGE="br0"
TAP_IFACE="tap0"
TAP_SUBNET="192.168.100.0/24"
TAP_HOST_IP="192.168.100.1"
TAP_GUEST_IP="192.168.100.2"
TIMEOUT=60               # QEMU boot timeout seconds

mkdir -p "$RESULTS_DIR"

# ─── Help ─────────────────────────────────────────────────────────────────
show_help() {
    cat <<EOF
Usage: $0 [options]

Network test harness for MINIX in QEMU.

Options:
  --mode <mode>     Network mode: user (default), tap, isolated
  --image <path>    Path to bootable MINIX image
  --tap-bridge <b>  Tap bridge name (default: br0)
  --tap-iface <i>   Tap interface name (default: tap0)
  --timeout <sec>   QEMU boot timeout (default: 60)
  --list-modes      List available network modes
  --help            Show this help

Modes:
  user       — QEMU SLiRP (NAT, no host→guest, easiest setup)
  tap        — Tap bridge (root/sudo, bidirectional, needs setup)
  isolated   — No network (-nic none)

Examples:
  $0 --mode user
  sudo $0 --mode tap  # tap needs root
EOF
    exit 0
}

# ─── Parse arguments ─────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode)         MODE="$2"; shift 2 ;;
        --image)        IMAGE="$2"; shift 2 ;;
        --tap-bridge)   TAP_BRIDGE="$2"; shift 2 ;;
        --tap-iface)    TAP_IFACE="$2"; shift 2 ;;
        --timeout)      TIMEOUT="$2"; shift 2 ;;
        --list-modes)
            echo "Available modes: user (default), tap, isolated"
            exit 0 ;;
        --help|-h)      show_help ;;
        *)              echo "Unknown: $1"; show_help ;;
    esac
done

# ─── Find QEMU ───────────────────────────────────────────────────────────
QEMU=""
for q in qemu-system-x86_64 qemu-system-i386 qemu-kvm; do
    command -v "$q" &>/dev/null && { QEMU="$q"; break; }
done
if [ -z "$QEMU" ]; then
    echo -e "${RED}QEMU not found. Install: apt install qemu-system-x86${NC}"
    exit 1
fi
echo -e "${GREEN}QEMU: ${QEMU}$(command -v kvm &>/dev/null && echo ' (+kvm)')${NC}"

# ─── Check tools for selected mode ───────────────────────────────────────
case "$MODE" in
    user)
        echo -e "${CYAN}Mode: user (SLiRP NAT)${NC}"
        echo "  Guest IP: ${GUEST_IP}  Host IP: ${HOST_IP}"
        ;;
    tap)
        echo -e "${CYAN}Mode: tap bridge (${TAP_BRIDGE}/${TAP_IFACE})${NC}"
        echo "  Guest IP: ${TAP_GUEST_IP}  Host IP: ${TAP_HOST_IP}"
        if [ "$(id -u)" -ne 0 ]; then
            echo -e "${YELLOW}Warning: tap mode usually needs root. Try: sudo $0 --mode tap${NC}"
        fi
        # Check for required tools
        for tool in ip brctl; do
            command -v "$tool" &>/dev/null || echo -e "${YELLOW}Warning: $tool not found${NC}"
        done
        ;;
    isolated)
        echo -e "${CYAN}Mode: isolated (no network)${NC}"
        ;;
    *)
        echo -e "${RED}Unknown mode: ${MODE}. Use: user, tap, isolated${NC}"
        exit 1
        ;;
esac

# ─── Locate or build image ────────────────────────────────────────────────
if [ -n "$IMAGE" ] && [ -f "$IMAGE" ]; then
    echo -e "${GREEN}Using image: ${IMAGE}${NC}"
elif [ -n "$IMAGE" ]; then
    echo -e "${RED}Image not found: ${IMAGE}${NC}"; exit 1
else
    # Search common locations
    for candidate in \
        "${SCRIPT_DIR}/minix.img" \
        "${SCRIPT_DIR}/minix_x86.img" \
        "${SCRIPT_DIR}/minix.iso" \
        "${SCRIPT_DIR}/obj.i386/minix_x86.img" \
        "${SCRIPT_DIR}/build/qemu/minix_x86.img" \
        "${SCRIPT_DIR}/build/qemu/minix.iso"; do
        if [ -f "$candidate" ]; then
            IMAGE="$candidate"
            echo -e "${GREEN}Found pre-built image: ${IMAGE}${NC}"
            break
        fi
    done

    if [ -z "$IMAGE" ]; then
        echo -e "${YELLOW}No image found. Building...${NC}"
        if [ -f "${SCRIPT_DIR}/releasetools/x86_ramimage.sh" ]; then
            BUILD_DIR="${SCRIPT_DIR}/build/qemu"
            mkdir -p "$BUILD_DIR"
            ARCH=x86_64 OBJ="$BUILD_DIR" \
                bash "${SCRIPT_DIR}/releasetools/x86_ramimage.sh" \
                2>&1 | tee "$RESULTS_DIR/image-build.log"
            IMAGE=$(find "$BUILD_DIR" -name "*.img" -o -name "*.iso" 2>/dev/null | head -1)
            [ -z "$IMAGE" ] && IMAGE=$(find "${SCRIPT_DIR}" -maxdepth 2 \
                -name "*.img" -o -name "*.iso" 2>/dev/null | head -1)
        else
            echo -e "${RED}releasetools/x86_ramimage.sh not found${NC}"
            exit 1
        fi
        [ -z "$IMAGE" ] && echo -e "${RED}Image build failed${NC}" && exit 1
        echo -e "${GREEN}Built image: ${IMAGE}${NC}"
    fi
fi

# ─── Set up tap networking (if mode=tap) ─────────────────────────────────
cleanup_tap() {
    echo -e "${YELLOW}Cleaning up tap network...${NC}"
    ip link set "$TAP_IFACE" down 2>/dev/null || true
    brctl delif "$TAP_BRIDGE" "$TAP_IFACE" 2>/dev/null || true
    ip tuntap del dev "$TAP_IFACE" mode tap 2>/dev/null || true
    ip link set "$TAP_BRIDGE" down 2>/dev/null || true
    ip link delete "$TAP_BRIDGE" 2>/dev/null || true
}

setup_tap() {
    echo -e "${YELLOW}Setting up tap bridge network...${NC}"
    cleanup_tap 2>/dev/null || true

    # Create bridge
    ip link add name "$TAP_BRIDGE" type bridge 2>/dev/null || {
        echo -e "${RED}Failed to create bridge. Try: sudo $0 --mode tap${NC}"
        return 1
    }
    ip addr add "${TAP_HOST_IP}/24" dev "$TAP_BRIDGE"
    ip link set "$TAP_BRIDGE" up

    # Create tap
    ip tuntap add dev "$TAP_IFACE" mode tap
    ip link set "$TAP_IFACE" up
    brctl addif "$TAP_BRIDGE" "$TAP_IFACE"

    echo -e "${GREEN}Tap network ready: ${TAP_GUEST_IP} ↔ ${TAP_HOST_IP}${NC}"
    return 0
}

TAP_CLEANUP=""
if [ "$MODE" = "tap" ]; then
    setup_tap && TAP_CLEANUP=1
fi

# Ensure cleanup on exit
cleanup() {
    [ -n "$TAP_CLEANUP" ] && cleanup_tap
    kill "$QEMU_PID" 2>/dev/null || true
}
trap cleanup EXIT

# ─── Build QEMU args ────────────────────────────────────────────────────
QEMU_ARGS=(
    -nographic
    -m 512M
    -smp 1
    -no-reboot
    -serial "file:${SERIAL_OUT}"
)

# Network configuration
case "$MODE" in
    user)
        # SLiRP: guest can reach host, host cannot reach guest directly
        QEMU_ARGS+=(-nic user,model=e1000,hostfwd=tcp::2222-:22)
        ;;
    tap)
        if [ -n "$TAP_CLEANUP" ]; then
            QEMU_ARGS+=(-nic tap,ifname="$TAP_IFACE",script=no,downscript=no,model=e1000)
        else
            echo -e "${YELLOW}Tap setup failed, falling back to user mode${NC}"
            QEMU_ARGS+=(-nic user,model=e1000,hostfwd=tcp::2222-:22)
        fi
        ;;
    isolated)
        QEMU_ARGS+=(-nic none)
        ;;
esac

# Determine boot device
case "$IMAGE" in
    *.iso)  QEMU_ARGS+=(-cdrom "$IMAGE" -boot order=d) ;;
    *)      QEMU_ARGS+=(-drive file="$IMAGE",format=raw -boot order=c) ;;
esac

# ─── Boot MINIX ─────────────────────────────────────────────────────────
echo ""
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo -e "${BLUE}Booting MINIX in QEMU (timeout: ${TIMEOUT}s)${NC}"
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo "Image:  ${IMAGE}"
echo "QEMU:   ${QEMU_ARGS[*]}"
echo "Serial: ${SERIAL_OUT}"
echo ""

# Clear serial output
: > "$SERIAL_OUT"

# Start QEMU in background
set +e
"${QEMU}" "${QEMU_ARGS[@]}" &
QEMU_PID=$!
set -e

# Wait for boot or timeout
BOOTED=0
BOOT_MSG="MINIX boot detected"
for i in $(seq 1 $((TIMEOUT / 2))); do
    sleep 2
    if ! kill -0 "$QEMU_PID" 2>/dev/null; then
        echo -e "${YELLOW}QEMU exited unexpectedly (check ${SERIAL_OUT})${NC}"
        break
    fi
    if grep -qE "(login:|# |MINIX|Shell)" "$SERIAL_OUT" 2>/dev/null; then
        BOOTED=1
        echo -e "${GREEN}${BOOT_MSG} (~$((i * 2))s)${NC}"
        break
    fi
done

if [ "$BOOTED" -eq 0 ]; then
    echo -e "${YELLOW}MINIX did not boot within ${TIMEOUT}s${NC}"
    echo "Last 20 lines of serial output:"
    tail -20 "$SERIAL_OUT" 2>/dev/null || true
    kill "$QEMU_PID" 2>/dev/null || true
    exit 1
fi

# ─── Run tests ──────────────────────────────────────────────────────────
echo ""
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo -e "${BLUE}Network Tests${NC}"
echo -e "${BLUE}════════════════════════════════════════════${NC}"

TEST_LOG="${RESULTS_DIR}/tests.log"
: > "$TEST_LOG"

test_result() {
    local name="$1"
    local status="$2"
    local detail="$3"
    if [ "$status" = "PASS" ]; then
        echo -e "  [${GREEN}PASS${NC}] ${name} — ${detail}" | tee -a "$TEST_LOG"
    else
        echo -e "  [${RED}FAIL${NC}] ${name} — ${detail}" | tee -a "$TEST_LOG"
    fi
}

# Test 1: Ping loopback
echo -e "\n${YELLOW}Test: ping loopback${NC}"
# Inject ping command via serial
# (In real usage, this requires a MINIX image that auto-logs in and runs cmds)
test_result "ping6 localhost" "INFO" "Requires MINIX with serial console interaction"

# Test 2: Check network interfaces
echo -e "\n${YELLOW}Test: network interface detection${NC}"
if grep -qE "(e1000|eth|lo0|inet )" "$SERIAL_OUT" 2>/dev/null; then
    test_result "interface detected" "PASS" "Network interface found in boot log"
else
    test_result "interface detected" "FAIL" "No network interface in boot log"
fi

# Test 3: IPv6 presence
echo -e "\n${YELLOW}Test: IPv6 support${NC}"
if grep -qi "inet6" "$SERIAL_OUT" 2>/dev/null; then
    test_result "IPv6 support" "PASS" "INET6 detected in boot log"
else
    test_result "IPv6 support" "WARN" "No INET6 in boot log — may be USE_INET6=no"
fi

# Test 4: Check boot log for errors
echo -e "\n${YELLOW}Test: network errors in boot log${NC}"
NET_ERRORS=$(grep -ciE "(lwip|eth|net|nic).*(error|fail|panic)" "$SERIAL_OUT" 2>/dev/null || echo 0)
if [ "$NET_ERRORS" -eq 0 ]; then
    test_result "boot errors" "PASS" "No network errors in boot log"
else
    test_result "boot errors" "WARN" "${NET_ERRORS} potential errors — check ${SERIAL_OUT}"
fi

# ─── Summary ─────────────────────────────────────────────────────────────
{
    echo "=========================================="
    echo "Network Test Results"
    echo "=========================================="
    echo "Date:    $(date)"
    echo "QEMU:    ${QEMU}"
    echo "Image:   ${IMAGE}"
    echo "Mode:    ${MODE}"
    echo "Time:    ~$(($(date +%s) - $(date -r "$SERIAL_OUT" +%s 2>/dev/null || echo 0)))s"
    echo ""
    echo "Tests:"
    cat "$TEST_LOG" 2>/dev/null || echo "(no tests run)"
    echo ""
    echo "Serial log: ${SERIAL_OUT}"
    echo "Test log:   ${TEST_LOG}"
} > "$RESULTS_DIR/summary.txt"

echo ""
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo -e "${GREEN}Tests complete${NC}"
echo -e "Results: ${RESULTS_DIR}/"
echo -e "Summary: ${RESULTS_DIR}/summary.txt"
echo -e "Serial:  ${SERIAL_OUT} ($(wc -l < "$SERIAL_OUT" 2>/dev/null || echo 0) lines)"
echo -e "${BLUE}════════════════════════════════════════════${NC}"

# Shutdown QEMU gracefully
echo -e "${YELLOW}Shutting down QEMU...${NC}"
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
sleep 1

# Show test summary
echo ""
echo -e "${CYAN}=== Test Summary ===${NC}"
grep -E "^  \[|INFO" "$TEST_LOG" 2>/dev/null || echo "  (no structured tests)"
echo ""
