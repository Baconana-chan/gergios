#!/usr/bin/env bash
# QEMU AArch64 MINIX test script
#
# Usage:
#   ./scripts/qemu-aarch64.sh --kernel <kernel>          # Direct kernel boot
#   ./scripts/qemu-aarch64.sh --uefi --image <image>     # UEFI/Limine boot
#   ./scripts/qemu-aarch64.sh --debug                    # GDB debug mode
#
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Defaults
: ${KERNEL="${PROJECT_DIR}/build-aarch64/minix/kernel/kernel"}
: ${QEMU=qemu-system-aarch64}
: ${MACHINE=virt}
: ${CPU=cortex-a72}
: ${MEM=256M}
: ${SMP=1}
: ${GDB_PORT=1234}

usage() {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --kernel <file>     Kernel binary (default: build-aarch64/kernel)"
    echo "  --uefi              Boot via UEFI (Limine AAC64)"
    echo "  --image <file>      Disk image (for UEFI boot)"
    echo "  --debug             Start QEMU with GDB server"
    echo "  --gdb-port <port>   GDB port (default: 1234)"
    echo "  --smp <n>           Number of CPU cores (default: 1)"
    echo "  --mem <size>        RAM size (default: 256M)"
    echo "  -h, --help          Show this help"
    exit 0
}

# Parse arguments
DIRECT_BOOT=1
UEFI_BOOT=0
DEBUG=0

while [ $# -gt 0 ]; do
    case "$1" in
        --kernel)
            KERNEL="$2"
            shift 2
            ;;
        --uefi)
            UEFI_BOOT=1
            DIRECT_BOOT=0
            shift
            ;;
        --image)
            IMAGE="$2"
            shift 2
            ;;
        --debug)
            DEBUG=1
            shift
            ;;
        --gdb-port)
            GDB_PORT="$2"
            shift 2
            ;;
        --smp)
            SMP="$2"
            shift 2
            ;;
        --mem)
            MEM="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use -h for help"
            exit 1
            ;;
    esac
done

# Check QEMU availability
if ! command -v ${QEMU} &>/dev/null; then
    echo "ERROR: ${QEMU} not found"
    echo "Install with: sudo apt install qemu-system-arm"
    exit 1
fi

# Common QEMU arguments
QEMU_ARGS=(
    -M "${MACHINE}"
    -cpu "${CPU}"
    -m "${MEM}"
    -smp "${SMP}"
    -nographic
    -serial mon:stdio
)

# GDB debug mode
if [ "${DEBUG}" = "1" ]; then
    QEMU_ARGS+=(-s -S)
    echo "QEMU waiting for GDB on port ${GDB_PORT}..."
    echo "Connect with: aarch64-linux-gnu-gdb ${KERNEL} -ex 'target remote :${GDB_PORT}'"
fi

if [ "${DIRECT_BOOT}" = "1" ]; then
    # ── Direct kernel boot ──
    if [ ! -f "${KERNEL}" ]; then
        echo "ERROR: Kernel not found: ${KERNEL}"
        echo "Build with: cmake --build build-aarch64 --target kernel"
        exit 1
    fi

    echo "=== MINIX ARM64 (AArch64) — Direct Boot ==="
    echo "Kernel: ${KERNEL}"
    echo "QEMU:   ${QEMU}"
    echo "Machine: ${MACHINE}, CPU: ${CPU}, RAM: ${MEM}, SMP: ${SMP}"
    echo ""

    QEMU_ARGS+=(-kernel "${KERNEL}")

elif [ "${UEFI_BOOT}" = "1" ]; then
    # ── UEFI / Limine boot ──
    if [ ! -f "${IMAGE}" ]; then
        echo "ERROR: Disk image not found: ${IMAGE}"
        exit 1
    fi

    # Find OVMF AArch64 firmware
    OVMF_CODE=""
    for path in \
        /usr/share/qemu-efi-aarch64/QEMU_EFI.fd \
        /usr/share/edk2/aarch64/QEMU_EFI.fd \
        /usr/share/ovmf/aarch64/QEMU_EFI.fd \
        /usr/share/AAVMF/AAVMF_CODE.fd; do
        if [ -f "${path}" ]; then
            OVMF_CODE="${path}"
            break
        fi
    done

    if [ -z "${OVMF_CODE}" ]; then
        echo "ERROR: OVMF firmware not found for AArch64"
        echo "Install with: sudo apt install qemu-efi-aarch64"
        exit 1
    fi

    echo "=== MINIX ARM64 (AArch64) — UEFI Boot ==="
    echo "Image:  ${IMAGE}"
    echo "OVMF:   ${OVMF_CODE}"
    echo "QEMU:   ${QEMU}"
    echo ""

    QEMU_ARGS+=(
        -bios "${OVMF_CODE}"
        -drive file="${IMAGE}",if=none,format=raw,id=hd0
        -device virtio-blk-device,drive=hd0
    )
fi

# Print and run
echo "Command: ${QEMU} ${QEMU_ARGS[*]}"
echo ""
echo "Press Ctrl-A X to exit QEMU"
echo ""

exec "${QEMU}" "${QEMU_ARGS[@]}"
