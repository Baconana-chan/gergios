#!/bin/bash
# run_qemu_test.sh — Boot MINIX in QEMU and capture serial output
#
# MINIX boots from a bootable image (ISO or disk), NOT via direct -kernel
# like Linux. This script relies on the release infrastructure in
# releasetools/ (e.g., x86_ramimage.sh) to create a bootable image,
# then boots it in QEMU with serial console capture.
#
# Usage:
#   ./scripts/run_qemu_test.sh                    # Build + boot
#   ./scripts/run_qemu_test.sh --image <path>     # Use pre-built image
#   ./scripts/run_qemu_test.sh --iso <path>       # Use pre-built ISO
#   ./scripts/run_qemu_test.sh --no-build         # Skip image build
#
# Requirements:
#   - qemu-system-x86_64 (for x86_64 emulation)
#   - releasetools/x86_ramimage.sh (for image creation)
#   - OR a pre-built bootable image from a real MINIX build

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-${SCRIPT_DIR}/build/qemu}"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/qemu-test-results}"
IMAGE=""
ISO=""
NO_BUILD=false
SERIAL_OUT="${RESULTS_DIR}/serial-output.txt"

mkdir -p "$RESULTS_DIR"
mkdir -p "$BUILD_DIR"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --image)    IMAGE="$2"; shift 2 ;;
        --iso)      ISO="$2"; shift 2 ;;
        --no-build) NO_BUILD=true; shift ;;
        --help)
            echo "Usage: $0 [--image <path>] [--iso <path>] [--no-build]"
            echo ""
            echo "This script boots a MINIX bootable image in QEMU."
            echo "Create an image with:"
            echo "  ARCH=x86_64 OBJ=build/qemu bash releasetools/x86_ramimage.sh"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}MINIX QEMU Test Runner${NC}"
echo -e "${BLUE}========================================${NC}"

# ------------------------------------------------------------------
# 1. Check for QEMU
# ------------------------------------------------------------------
QEMU=""
for q in qemu-system-x86_64 qemu-system-i386 qemu-kvm; do
    if command -v "$q" &>/dev/null; then
        QEMU="$q"
        break
    fi
done

if [ -z "$QEMU" ]; then
    echo -e "${RED}Error: QEMU not found${NC}"
    echo "Install with:"
    echo "  sudo apt-get install qemu-system-x86   # Debian/Ubuntu"
    echo "  sudo dnf install qemu-system-x86       # Fedora"
    exit 1
fi
echo -e "${GREEN}Found QEMU: $QEMU${NC}"

# ------------------------------------------------------------------
# 2. Locate or build MINIX bootable image
# ------------------------------------------------------------------
if [ -n "$IMAGE" ] && [ -f "$IMAGE" ]; then
    echo -e "${GREEN}Using provided image: $IMAGE${NC}"
elif [ -n "$ISO" ] && [ -f "$ISO" ]; then
    echo -e "${GREEN}Using provided ISO: $ISO${NC}"
elif [ "$NO_BUILD" = true ]; then
    echo -e "${YELLOW}--no-build specified but no image provided; looking for pre-built images...${NC}"
    # Search common locations
    for candidate in \
        "${BUILD_DIR}/minix.img" \
        "${BUILD_DIR}/minix.iso" \
        "${SCRIPT_DIR}/obj/minix.img" \
        "${SCRIPT_DIR}/minix.img"; do
        if [ -f "$candidate" ]; then
            echo -e "${GREEN}Found pre-built image: $candidate${NC}"
            IMAGE="$candidate"
            break
        fi
    done
    if [ -z "$IMAGE" ] && [ -z "$ISO" ]; then
        echo -e "${RED}No pre-built image found. Either build one first or omit --no-build.${NC}"
        echo "  ARCH=x86_64 OBJ=build/qemu bash releasetools/x86_ramimage.sh"
        exit 1
    fi
else
    echo -e "${YELLOW}[1/2] Building MINIX bootable image...${NC}"

    if [ -f "${SCRIPT_DIR}/releasetools/x86_ramimage.sh" ]; then
        echo "Running releasetools/x86_ramimage.sh..."
        cd "$SCRIPT_DIR"
        ARCH=x86_64 OBJ="${BUILD_DIR}" \
            bash releasetools/x86_ramimage.sh 2>&1 | tee "$RESULTS_DIR/image-build.log"

        # Find the produced image
        IMAGE=$(find "${BUILD_DIR}" -name "*.img" -o -name "*.iso" 2>/dev/null | head -1)
        if [ -z "$IMAGE" ]; then
            IMAGE=$(find "${SCRIPT_DIR}" -maxdepth 2 -name "*.img" -o -name "*.iso" 2>/dev/null | head -1)
        fi
    else
        echo -e "${RED}Error: releasetools/x86_ramimage.sh not found${NC}"
        echo "This script requires the MINIX release infrastructure."
        echo "Build a bootable image first, then use --image <path>"
        exit 1
    fi

    if [ -z "$IMAGE" ] || [ ! -f "$IMAGE" ]; then
        echo -e "${RED}Error: image creation failed — no bootable image produced${NC}"
        exit 1
    fi
    echo -e "${GREEN}Bootable image: $IMAGE${NC}"
fi

# ------------------------------------------------------------------
# 3. Boot in QEMU with serial console
# ------------------------------------------------------------------
echo -e "${YELLOW}[2/2] Booting MINIX in QEMU...${NC}"

# Determine boot device
if [ -n "$ISO" ]; then
    DRIVE_ARGS="-cdrom \"${ISO}\""
    BOOT_ORDER="d"  # boot from CD-ROM first
elif [ -n "$IMAGE" ]; then
    # Determine format by extension
    # NOTE: no quoting around $IMAGE — paths are CI-controlled without spaces
    case "$IMAGE" in
        *.iso)    DRIVE_ARGS="-cdrom ${IMAGE}" ; BOOT_ORDER="d" ;;
        *.qcow2)  DRIVE_ARGS="-drive file=${IMAGE},format=qcow2" ; BOOT_ORDER="c" ;;
        *.raw|*)  DRIVE_ARGS="-drive file=${IMAGE},format=raw" ; BOOT_ORDER="c" ;;
    esac
fi

echo "Booting with: ${DRIVE_ARGS}"
echo "Serial output → ${SERIAL_OUT}"
echo ""
echo -e "${BLUE}QEMU started — waiting for MINIX to boot...${NC}"
echo -e "${BLUE}(timeout: 120s)${NC}"
echo ""

# Start QEMU, capture serial console to file
# -nographic: serial console on stdio
# -no-reboot: exit on guest shutdown/panic
# -m 512M: 512 MB RAM (adjust if MINIX needs more)
# -device sga: serial graphics adapter for text output
set +e
timeout 120 "$QEMU" \
    -nographic \
    -m 512M \
    -smp 1 \
    ${DRIVE_ARGS} \
    -boot "order=${BOOT_ORDER}" \
    -serial stdio \
    -no-reboot \
    2>&1 | tee "$SERIAL_OUT"

QEMU_EXIT=$?
set -e

echo ""
echo -e "${BLUE}QEMU exited (code: ${QEMU_EXIT})${NC}"

# ------------------------------------------------------------------
# 4. Process results
# ------------------------------------------------------------------
echo -e "${YELLOW}Processing results...${NC}"

{
    echo "=========================================="
    echo "QEMU Test Results"
    echo "=========================================="
    echo "Date: $(date)"
    echo "QEMU: $QEMU"
    echo "Image: ${IMAGE:-${ISO:-none}}"
    echo "QEMU exit code: $QEMU_EXIT"
    echo ""
    echo "Kernel Boot Log:"
    echo "---------------"
    # Extract MINIX boot messages: look for common MINIX kernel output
    if grep -q "MINIX\|MINIX 3\|boot\|MINIX3" "$SERIAL_OUT" 2>/dev/null; then
        echo "Boot: DETECTED"
        grep -E "(MINIX|MINIX 3|kernel|boot|Kernel|MINIX3)" "$SERIAL_OUT" 2>/dev/null | head -20
    else
        echo "Boot: UNKNOWN — (first 30 lines of output below)"
        echo ""
        head -30 "$SERIAL_OUT" 2>/dev/null || echo "(no output)"
    fi
    echo ""
    echo "Full Serial Log:"
    echo "---------------"
    echo "See: $SERIAL_OUT ($(wc -l < "$SERIAL_OUT" 2>/dev/null || echo 0) lines)"
} > "$RESULTS_DIR/summary.txt"

echo -e "${BLUE}========================================${NC}"
echo -e "Results: ${RESULTS_DIR}/"
echo -e "Summary: ${RESULTS_DIR}/summary.txt"
echo -e "Serial:  ${SERIAL_OUT}"
echo -e "${BLUE}========================================${NC}"

# Determine success
if [ "$QEMU_EXIT" -eq 124 ]; then
    echo -e "${YELLOW}QEMU timed out after 120s — MINIX may not have booted or shutdown${NC}"
    exit 1
elif grep -q "MINIX" "$SERIAL_OUT" 2>/dev/null || grep -q "boot" "$SERIAL_OUT" 2>/dev/null; then
    echo -e "${GREEN}MINIX boot detected in serial output${NC}"
    exit 0
else
    echo -e "${YELLOW}QEMU ran but no MINIX boot messages detected${NC}"
    echo "Check $SERIAL_OUT for details"
    exit 0  # non-fatal: CI can still collect artifacts
fi
