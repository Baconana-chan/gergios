#!/usr/bin/env bash
# Phase 2 — Limine UEFI/BIOS Boot Test Image Generator
#
# Creates a bootable disk image with Limine bootloader supporting both
# BIOS (Legacy) and UEFI boot on QEMU.
#
# Prerequisites:
#   - Limine binaries (https://github.com/limine-bootloader/limine)
#   - qemu-system-x86_64
#   - Built GergiOS kernel + modules (build.sh release, MKLIMINE=yes)
#   - sgdisk, mkfs.fat, mtools
#   - OVMF (for UEFI boot): apt install ovmf / pacman -S edk2-ovmf
#
# Usage:
#   # BIOS test:
#   ./releasetools/make_limine_test_image.sh -O ../obj.x86_64 -r
#
#   # UEFI test:
#   ./releasetools/make_limine_test_image.sh -O ../obj.x86_64 -u -r
#
#   # With custom kernel:
#   ./releasetools/make_limine_test_image.sh -o test.img \
#       -k /path/to/kernel -m /path/to/modules -r

set -euo pipefail

# ============================================================================
# Defaults
# ============================================================================

: ${ARCH=x86_64}
: ${OBJ=../obj.${ARCH}}
: ${DESTDIR=${OBJ}/destdir.${ARCH}}
: ${MODDIR=${DESTDIR}/boot/minix/.temp}
: ${LIMINE_BIN=limine}
: ${LIMINE_DIR=/usr/local/share/limine}
: ${IMG=minix_limine_test.img}
: ${QEMU=qemu-system-x86_64}
: ${OVMF_CODE=/usr/share/ovmf/OVMF_CODE.fd}
: ${OVMF_VARS=/usr/share/ovmf/OVMF_VARS.fd}

# IMG sizes (bytes)
: ${ESP_SIZE=$((64*(2**20)))}  # 64MB ESP (FAT32)
: ${IMG_SIZE=$((256*(2**20)))} # 256MB total

# ============================================================================
# Functions
# ============================================================================

usage() {
    cat <<EOF
Usage: $0 [options]

Options:
  -o IMG       Output image path (default: ${IMG})
  -O OBJDIR    Build object directory (for -k/-m defaults)
  -k KERNEL    Path to kernel binary (overrides OBJDIR)
  -m MODDIR    Path to modules directory (overrides OBJDIR)
  -b LIMINE    Path to limine binary (default: ${LIMINE_BIN})
  -L LIMDIR    Path to Limine data directory (default: ${LIMINE_DIR})
  -r           Run in QEMU after creation
  -u           Test UEFI boot (needs OVMF)
  -f           Force re-creation even if image exists
  -h           Show this help

Examples:
  # Quick BIOS test from build:
  $0 -O ../obj.x86_64 -r

  # UEFI test:
  $0 -O ../obj.x86_64 -u -r

  # Custom kernel:
  $0 -o test.img -k ./kernel -m ./modules -r
EOF
    exit 0
}

# Try to locate OVMF firmware
find_ovmf() {
    # Try common paths
    for path in \
        "${OVMF_CODE}" \
        /usr/share/ovmf/OVMF_CODE.fd \
        /usr/share/edk2-ovmf/OVMF_CODE.fd \
        /usr/share/qemu/ovmf-x86_64.bin \
        /usr/share/qemu/ovmf-x86_64-4m.bin \
        /usr/lib/edk2-ovmf/OVMF_CODE.fd \
        /opt/homebrew/share/qemu/edk2-x86_64-code.fd \
        "C:/Program Files/qemu/share/edk2-x86_64-code.fd" \
        "C:/Program Files (x86)/qemu/share/edk2-x86_64-code.fd"; do
        if [ -f "$path" ]; then
            OVMF_CODE="$path"
            OVMF_VARS="${path%/*}/OVMF_VARS.fd"
            [ -f "$OVMF_VARS" ] || OVMF_VARS=""
            return 0
        fi
    done
    return 1
}

# ============================================================================
# Parse options
# ============================================================================

RUN_QEMU=0
UEFI=0
FORCE=0

while getopts "o:O:k:m:b:L:rufh" c; do
    case "$c" in
        o) IMG="$OPTARG" ;;
        O) OBJ="$OPTARG"
           DESTDIR="${OBJ}/destdir.${ARCH}"
           MODDIR="${DESTDIR}/boot/minix/.temp" ;;
        k) KERNEL="$OPTARG" ;;
        m) MODULE_DIR="$OPTARG" ;;
        b) LIMINE_BIN="$OPTARG" ;;
        L) LIMINE_DIR="$OPTARG" ;;
        r) RUN_QEMU=1 ;;
        u) UEFI=1 ;;
        f) FORCE=1 ;;
        h) usage ;;
        *) usage ;;
    esac
done

# ============================================================================
# Validate inputs
# ============================================================================

echo "=== Limine Boot Test Image Generator (Phase 2) ==="
echo ""

# Limine
if ! command -v "${LIMINE_BIN}" >/dev/null 2>&1; then
    cat >&2 <<EOF
ERROR: Limine binary '${LIMINE_BIN}' not found.
Install: https://github.com/limine-bootloader/limine
  git clone https://github.com/limine-bootloader/limine.git
  cd limine && make && sudo make install
EOF
    exit 1
fi

# Limine data
if [ ! -d "${LIMINE_DIR}" ]; then
    LIMINE_DIR=$("${LIMINE_BIN}" --print-data-dir 2>/dev/null || echo "")
    if [ -z "${LIMINE_DIR}" ] || [ ! -d "${LIMINE_DIR}" ]; then
        echo >&2 "ERROR: Cannot find Limine data directory."
        echo >&2 "Specify with -L /path/to/limine/share"
        exit 1
    fi
fi

LIMINE_VER=$("${LIMINE_BIN}" --version 2>/dev/null || echo "unknown")
echo "Limine: ${LIMINE_BIN} (v${LIMINE_VER})"
echo "Data:   ${LIMINE_DIR}"

# Kernel
KERNEL="${KERNEL:-${MODDIR}/kernel}"
if [ ! -f "${KERNEL}" ]; then
    echo >&2 "ERROR: Kernel not found at ${KERNEL}"
    echo >&2 "Specify with -k /path/to/kernel or -O OBJDIR"
    exit 1
fi
echo "Kernel: ${KERNEL}"

# Modules
MODULE_DIR="${MODULE_DIR:-${MODDIR}}"
if [ ! -d "${MODULE_DIR}" ]; then
    echo "WARNING: Module directory '${MODULE_DIR}' not found."
    echo "Will create image with kernel only (no boot modules)."
    MODULE_DIR=""
else
    MOD_COUNT=$(ls "${MODULE_DIR}"/mod* 2>/dev/null | wc -l)
    echo "Modules: ${MODULE_DIR} (${MOD_COUNT} modules)"
fi

# QEMU
if [ "${RUN_QEMU}" -eq 1 ]; then
    if ! command -v "${QEMU}" >/dev/null 2>&1; then
        echo >&2 "ERROR: ${QEMU} not found. Install qemu-system-x86_64."
        exit 1
    fi
    if [ "${UEFI}" -eq 1 ] && ! find_ovmf; then
        echo >&2 "ERROR: OVMF firmware not found for UEFI boot."
        echo >&2 "Install: apt install ovmf (or pacman -S edk2-ovmf)"
        echo >&2 "Or specify OVMF_CODE env var."
        exit 1
    fi
fi

# Tools
for tool in sgdisk mkfs.fat mcopy mmd; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo >&2 "ERROR: ${tool} not found."
        echo >&2 "Install: apt install gdisk dosfstools mtools"
        echo >&2 "   or:  pacman -S gdisk dosfstools mtools"
        exit 1
    fi
done

# Check Limine files
if [ ! -f "${LIMINE_DIR}/limine.sys" ]; then
    echo >&2 "ERROR: ${LIMINE_DIR}/limine.sys not found."
    echo >&2 "Is Limine properly installed?"
    exit 1
fi

# ============================================================================
# Create disk image
# ============================================================================

WORK_DIR=$(mktemp -d)
trap "rm -rf ${WORK_DIR}" EXIT

echo ""
echo "=== Creating disk image ==="
echo ""

# Align ESP to 1MB boundary
ESP_ALIGNED=$(( ((ESP_SIZE + (1024*1024) - 1) / (1024*1024)) * (1024*1024) ))
ESP_SECTORS=$((ESP_ALIGNED / 512))
DATA_START=$((ESP_SECTORS + (2048)))  # ESP + padding
DATA_SECTORS=$(( (IMG_SIZE - DATA_START*512) / 512 ))
TOTAL_SECTORS=$((DATA_START + DATA_SECTORS))

# Clean old image
[ "${FORCE}" = "1" ] && rm -f "${IMG}"
if [ -f "${IMG}" ]; then
    echo "Image exists: ${IMG}"
    echo "Use -f to overwrite."
    exit 1
fi

# Create empty image
dd if=/dev/zero of="${IMG}" bs=512 count="${TOTAL_SECTORS}" 2>/dev/null
echo " * Created ${TOTAL_SECTORS}-sector image (${IMG_SIZE}B)"

# Create GPT with ESP partition
sgdisk -o "${IMG}" 2>/dev/null || true
if ! sgdisk -n 1:2048:+${ESP_SECTORS} -t 1:ef00 "${IMG}" 2>/dev/null; then
    echo "ERROR: sgdisk failed to create partition table"
    exit 1
fi
echo " * Created GPT with ESP partition (${ESP_ALIGNED}B)"

# Offset and size for ESP
ESP_OFFSET=$((2048 * 512))
ESP_LIMIT=$((ESP_SECTORS * 512))

# Format ESP as FAT32
ESP_IMG="${WORK_DIR}/esp.img"
dd if="${IMG}" of="${ESP_IMG}" bs=512 skip=2048 count=${ESP_SECTORS} 2>/dev/null
if ! mkfs.fat -F 32 "${ESP_IMG}" >/dev/null 2>&1; then
    echo "ERROR: mkfs.fat failed to format ESP"
    exit 1
fi
echo " * Formatted ESP as FAT32"

# ============================================================================
# Populate ESP
# ============================================================================

echo " * Copying files to ESP..."

# Create directory structure
mmd -D sG "${ESP_IMG}" "EFI" 2>/dev/null || true
mmd -D sG "${ESP_IMG}" "EFI/BOOT" 2>/dev/null || true

# Copy Limine stage 2 (BIOS boot)
mcopy -D sG "${LIMINE_DIR}/limine.sys" "${ESP_IMG}" "limine.sys" 2>/dev/null

# Copy Limine UEFI bootloader
if [ -f "${LIMINE_DIR}/BOOTX64.EFI" ]; then
    mcopy -D sG "${LIMINE_DIR}/BOOTX64.EFI" "${ESP_IMG}" "EFI/BOOT/BOOTX64.EFI" 2>/dev/null
    echo "   → ESP/EFI/BOOT/BOOTX64.EFI (UEFI)"
elif [ -f "${LIMINE_DIR}/limine-uefi-cd.bin" ]; then
    echo "   WARNING: No BOOTX64.EFI found, using limine-uefi-cd.bin"
fi

# Copy kernel
mcopy -D sG "${KERNEL}" "${ESP_IMG}" "kernel" 2>/dev/null
echo "   → ESP/kernel"

# Copy modules
if [ -n "${MODULE_DIR}" ]; then
    for mod in "${MODULE_DIR}"/mod*; do
        if [ -f "${mod}" ]; then
            mcopy -D sG "${mod}" "${ESP_IMG}" "$(basename ${mod})" 2>/dev/null
        fi
    done
    echo "   → ESP/mod* (${MOD_COUNT} modules)"
fi

# Generate limine.conf
cat >"${WORK_DIR}/limine.conf" <<END_LIMINE_CONF
# GergiOS Limine Configuration
# Generated by make_limine_test_image.sh
TIMEOUT=5

:GergiOS (BIOS/UEFI)
    PROTOCOL=limine
    KERNEL_PATH=boot:///kernel
END_LIMINE_CONF

if [ -n "${MODULE_DIR}" ]; then
    for mod in "${MODULE_DIR}"/mod*; do
        if [ -f "${mod}" ]; then
            echo "    MODULE_PATH=boot:///$(basename ${mod})" >> "${WORK_DIR}/limine.conf"
        fi
    done
fi

cat >>"${WORK_DIR}/limine.conf" <<END_LIMINE_CONF
    CMDLINE=rootdevname=c0d0p0

:GergiOS (Safe Mode)
    PROTOCOL=limine
    KERNEL_PATH=boot:///kernel
END_LIMINE_CONF

if [ -n "${MODULE_DIR}" ]; then
    for mod in "${MODULE_DIR}"/mod*; do
        if [ -f "${mod}" ]; then
            echo "    MODULE_PATH=boot:///$(basename ${mod})" >> "${WORK_DIR}/limine.conf"
        fi
    done
fi

cat >>"${WORK_DIR}/limine.conf" <<END_LIMINE_CONF
    CMDLINE=rootdevname=c0d0p0 bootopts=-s
END_LIMINE_CONF

mcopy -D sG "${WORK_DIR}/limine.conf" "${ESP_IMG}" "limine.conf" 2>/dev/null
echo "   → ESP/limine.conf"

# Write ESP back to image
dd if="${ESP_IMG}" of="${IMG}" bs=512 seek=2048 count=${ESP_SECTORS} conv=notrunc 2>/dev/null
echo " * ESP written to image"

# ============================================================================
# Install Limine stage 1 (for BIOS compatibility)
# ============================================================================

# Limine bios-install writes stage 1 into the MBR/GPT protective area.
# This step requires root on Linux; on other platforms it may fail silently.
if "${LIMINE_BIN}" bios-install "${IMG}" 2>/dev/null; then
    echo " * Limine stage 1 installed (BIOS boot supported)"
else
    echo "   NOTE: limine bios-install not possible on this platform"
    echo "   UEFI boot still works via ESP/EFI/BOOT/BOOTX64.EFI"
fi

# Mark ESP as bootable
sgdisk -A 1:set:2 "${IMG}" 2>/dev/null || true

echo ""
echo "=== Image created: ${IMG} ==="
echo ""
echo "Boot modes:"
echo "  1. UEFI:  UEFI firmware → ESP/EFI/BOOT/BOOTX64.EFI → Limine"
echo "  2. BIOS:  MBR stage 1 → limine.sys → Limine"
echo ""

# ============================================================================
# Run in QEMU
# ============================================================================

if [ "${RUN_QEMU}" -eq 1 ]; then
    echo "=== Starting QEMU ==="

    QEMU_OPTS=(
        -m 512M
        -drive "file=${IMG},if=ide,format=raw"
        -serial stdio
        -no-reboot
        -no-shutdown
    )

    if [ "${UEFI}" -eq 1 ]; then
        echo " * UEFI boot (OVMF: ${OVMF_CODE})"
        QEMU_OPTS+=(-bios "${OVMF_CODE}")
    else
        echo " * Legacy BIOS boot"
    fi

    echo ""
    echo "Starting QEMU (Ctrl+A X to exit)..."

    # Use -nographic for headless, or remove for graphical
    # "${QEMU}" -nographic "${QEMU_OPTS[@]}"
    "${QEMU}" "${QEMU_OPTS[@]}"
else
    echo "To boot in QEMU (BIOS):"
    echo "  ${QEMU} -m 512M -drive file=${IMG},if=ide,format=raw -serial stdio"
    echo ""
    echo "To boot in QEMU (UEFI):"
    echo "  ${QEMU} -m 512M -bios ${OVMF_CODE} -drive file=${IMG},if=ide,format=raw -serial stdio"
    echo ""
    echo "Run with -r to auto-start."
fi
