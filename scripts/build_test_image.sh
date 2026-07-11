#!/usr/bin/env bash
# ============================================================================
# build_test_image.sh — Phase 9.2: Minimal QEMU Test Image Builder
#
# Builds a bootable disk image for QEMU testing with an optional custom
# rc.local test script injected into /etc/rc.local. Supports x86_64 and
# aarch64 architectures.
#
# Usage:
#   ./scripts/build_test_image.sh                          # Build x86_64
#   ./scripts/build_test_image.sh --arch aarch64           # Build ARM64
#   ./scripts/build_test_image.sh --rc-local myscript.sh   # Inject test script
#   ./scripts/build_test_image.sh --output test.img        # Custom output
#   ./scripts/build_test_image.sh --cache                  # Skip if exists
#   ./scripts/build_test_image.sh --list                   # List cached images
#   ./scripts/build_test_image.sh --help                   # Show help
#
# Environment variables:
#   BUILD_DIR     — Build output directory (default: ./build/qemu-test)
#   DESTDIR       — DESTDIR from MINIX build (default: auto-detect)
#   OBJ           — OBJ from MINIX build (default: auto-detect)
#   FORCE_REBUILD — Set to 1 to force rebuild (default: 0)
#
# Architecture support:
#   x86_64  — QEMU x86_64, Limine BIOS+UEFI boot (fully supported)
#   aarch64 — QEMU AArch64, Limine AAC64 UEFI boot (kernel pending)
#
# Requirements:
#   - Built MINIX release (build.sh release or cmake-build)
#   - Limine bootloader (for BIOS+UEFI boot)
#   - qemu-system-x86_64 / qemu-system-aarch64 (for testing)
#   - sgdisk, mkfs.fat, mtools (for image creation)
# ============================================================================

set -euo pipefail

# ─── Colors ─────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'

# ─── Script location ────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ─── Defaults ───────────────────────────────────────────────────────────
ARCH="${ARCH:-x86_64}"
BUILD_DIR="${BUILD_DIR:-${SCRIPT_DIR}/build/qemu-test}"
OUTPUT_IMG="${OUTPUT_IMG:-${BUILD_DIR}/minix-test-${ARCH}.img}"
RC_LOCAL_SCRIPT=""
USE_CACHE=false
LIST_ONLY=false

# Image sizes (bytes)
ESP_SIZE=$((64*(2**20)))          # 64 MB ESP
ROOT_SIZE=$((64*(2**20)))         # 64 MB root
IMG_SIZE=$((256*(2**20)))         # 256 MB total

# ─── Help ───────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $0 [options]

Builds a minimal bootable test image for QEMU testing.

Options:
  --arch <arch>       Target architecture: x86_64 (default) or aarch64
  --rc-local <file>   Inject rc.local test script into /etc/rc.local
  --output <path>     Output image path (default: build/qemu-test/minix-test-*.img)
  --cache             Skip build if image already exists
  --list              List cached test images
  --help              Show this help

Environment:
  BUILD_DIR     Build output directory
  FORCE_REBUILD Force rebuild even if image exists (set to 1)

Examples:
  $0                                          # Build x86_64 image
  $0 --arch aarch64                           # Build ARM64 image
  $0 --rc-local /tmp/test.sh                  # Build with test script
  $0 --cache --output test.img                # Cached build
  $0 --list                                   # List cached images
EOF
    exit 0
}

# ─── Parse arguments ────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)         ARCH="$2"; shift 2 ;;
        --rc-local)     RC_LOCAL_SCRIPT="$2"; shift 2 ;;
        --output)       OUTPUT_IMG="$2"; shift 2 ;;
        --cache)        USE_CACHE=true; shift ;;
        --list)         LIST_ONLY=true; shift ;;
        --help|-h)      usage ;;
        *)              echo "Unknown: $1"; usage ;;
    esac
done

# ─── List cached images ────────────────────────────────────────────────
if [ "$LIST_ONLY" = true ]; then
    echo -e "${CYAN}Cached test images:${NC}"
    find "${BUILD_DIR}" -name "minix-test-*.img" -exec ls -lh {} \; 2>/dev/null || echo "  (none)"
    exit 0
fi

# ─── Validate rc.local ──────────────────────────────────────────────────
if [ -n "$RC_LOCAL_SCRIPT" ]; then
    if [ ! -f "$RC_LOCAL_SCRIPT" ]; then
        echo -e "${RED}Error: rc.local script not found: ${RC_LOCAL_SCRIPT}${NC}"
        exit 1
    fi
    echo -e "${GREEN}Using rc.local: ${RC_LOCAL_SCRIPT}${NC}"
fi

# ─── Cache check ────────────────────────────────────────────────────────
if [ "$USE_CACHE" = true ] && [ -f "$OUTPUT_IMG" ]; then
    echo -e "${GREEN}Cached image exists: ${OUTPUT_IMG}${NC}"
    echo "$OUTPUT_IMG"
    exit 0
fi

# ─── Architecture-specific setup ────────────────────────────────────────
case "$ARCH" in
    x86_64)
        QEMU_SYSTEM="qemu-system-x86_64"
        LIMINE_ARCH="x86_64"
        OVMF_PATHS=(
            /usr/share/ovmf/OVMF_CODE.fd
            /usr/share/edk2-ovmf/OVMF_CODE.fd
            /usr/share/qemu/ovmf-x86_64.bin
            /usr/lib/edk2-ovmf/OVMF_CODE.fd
            /opt/homebrew/share/qemu/edk2-x86_64-code.fd
        )
        ;;
    aarch64|arm64)
        ARCH="aarch64"  # normalize
        QEMU_SYSTEM="qemu-system-aarch64"
        LIMINE_ARCH="aac64"
        OVMF_PATHS=(
            /usr/share/qemu-efi-aarch64/QEMU_EFI.fd
            /usr/share/AAVMF/AAVMF_CODE.fd
            /usr/share/edk2/aarch64/QEMU_EFI.fd
            /usr/share/ovmf/aarch64/QEMU_EFI.fd
        )
        ;;
    *)
        echo -e "${RED}Unsupported architecture: ${ARCH}${NC}"
        echo "Supported: x86_64, aarch64"
        exit 1
        ;;
esac

# ─── Check prerequisites ────────────────────────────────────────────────
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}GergiOS Test Image Builder (Phase 9.2)${NC}"
echo -e "${BLUE}========================================${NC}"
echo "Architecture: ${ARCH}"
echo "Output:       ${OUTPUT_IMG}"
echo ""

# Check QEMU
if ! command -v "$QEMU_SYSTEM" &>/dev/null; then
    echo -e "${YELLOW}Warning: ${QEMU_SYSTEM} not found (install qemu-system-*)${NC}"
fi

# Check image tools
for tool in sgdisk mkfs.fat mcopy mmd; do
    if ! command -v "$tool" &>/dev/null; then
        echo -e "${RED}Error: ${tool} not found. Install: gdisk dosfstools mtools${NC}"
        exit 1
    fi
done

# ─── Locate build artifacts ─────────────────────────────────────────────
# Auto-detect DESTDIR and MODDIR from common build locations
DESTDIR="${DESTDIR:-}"
if [ -z "$DESTDIR" ]; then
    for candidate in \
        "${SCRIPT_DIR}/destdir.x86_64" \
        "${SCRIPT_DIR}/destdir" \
        "${SCRIPT_DIR}/obj/destdir.x86_64" \
        "${SCRIPT_DIR}/obj.i386/destdir.i386" \
        "${BUILD_DIR}/destdir"; do
        if [ -d "$candidate" ]; then
            DESTDIR="$candidate"
            echo -e "${GREEN}Auto-detected DESTDIR: ${DESTDIR}${NC}"
            break
        fi
    done
fi

MODDIR="${DESTDIR}/boot/minix/.temp"
if [ ! -d "$MODDIR" ] || [ ! -f "$MODDIR/kernel" ]; then
    # Try alternative locations
    for candidate in \
        "${SCRIPT_DIR}/obj.x86_64/destdir.x86_64/boot/minix/.temp" \
        "${SCRIPT_DIR}/build/qemu/destdir/boot/minix/.temp" \
        "${DESTDIR}/boot/minix/.temp" \
        "${SCRIPT_DIR}/boot/minix/.temp"; do
        if [ -f "$candidate/kernel" ]; then
            MODDIR="$candidate"
            echo -e "${GREEN}Auto-detected MODDIR: ${MODDIR}${NC}"
            break
        fi
    done
fi

if [ ! -f "$MODDIR/kernel" ]; then
    echo -e "${YELLOW}Warning: kernel not found at ${MODDIR}/kernel${NC}"
    echo -e "${YELLOW}The image will be created without kernel/modules.${NC}"
    echo -e "${YELLOW}Build MINIX first: make do-kernel do-lib do-build${NC}"
    echo -e "${YELLOW}Or: ./releasetools/cmake-build.sh build${NC}"
    MODDIR=""
fi

# ─── Set up work directory ──────────────────────────────────────────────
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

ESP_DIR="${WORK_DIR}/esp"
ROOT_FS="${WORK_DIR}/root"
mkdir -p "$ESP_DIR" "$ROOT_FS"

# ─── Create minimal root filesystem ─────────────────────────────────────
echo ""
echo -e "${YELLOW}Creating minimal root filesystem...${NC}"

# Essential directories
for d in bin sbin dev etc home lib mnt proc root sbin sys tmp usr/bin \
         usr/sbin usr/lib usr/share var/log var/run var/tmp; do
    mkdir -p "${ROOT_FS}/${d}"
done

# Shell (busybox-style: link to /bin/sh — we use the MINIX shell)
# In MINIX, /bin/sh is ash; if available from DESTDIR, copy it
if [ -n "$DESTDIR" ] && [ -d "$DESTDIR" ]; then
    echo " * Copying essential binaries from DESTDIR..."
    # Copy key binaries for smoke tests
    for bin in bin/sh bin/ash bin/ls bin/cat bin/echo bin/ps bin/df \
               bin/uname bin/mount bin/umount bin/mkdir bin/touch \
               bin/dd bin/rm bin/cp bin/mv bin/sync bin/kill bin/shutdown \
               sbin/init sbin/mkfs.ext4 sbin/fsck.ext4 sbin/ifconfig \
               sbin/route sbin/ping sbin/bluetoothd sbin/bt-tool; do
        src="${DESTDIR}/${bin}"
        if [ -f "$src" ]; then
            mkdir -p "$(dirname "${ROOT_FS}/${bin}")"
            cp "$src" "${ROOT_FS}/${bin}"
        fi
    done

    # Copy shared libraries
    for lib in usr/lib/libc.so usr/lib/libsys.so usr/lib/libbluetooth.so \
               usr/lib/liblwip.so usr/lib/libm.so; do
        src="${DESTDIR}/${lib}"
        if [ -f "$src" ]; then
            mkdir -p "$(dirname "${ROOT_FS}/${lib}")"
            cp "$src" "${ROOT_FS}/${lib}"
        fi
    done

    # Copy device nodes
    # In MINIX, devman handles this; we provide minimal static nodes
    echo " * Creating minimal device nodes..."
    # /dev/console, /dev/tty, /dev/null, /dev/zero
    # (These are created by devman at runtime in real MINIX)
fi

# ─── Inject rc.local test script ──────────────────────────────────────
echo " * Creating /etc/rc.local..."
if [ -n "$RC_LOCAL_SCRIPT" ]; then
    # Copy user-provided test script
    cp "$RC_LOCAL_SCRIPT" "${ROOT_FS}/etc/rc.local"
    chmod 755 "${ROOT_FS}/etc/rc.local"
    echo -e "${GREEN}   → Injected: ${RC_LOCAL_SCRIPT}${NC}"
else
    # Default rc.local: smoke test placeholder
    cat >"${ROOT_FS}/etc/rc.local" <<'EOF'
#!/bin/sh
# Default GergiOS smoke test rc.local
# Override with --rc-local <script> to run custom tests.

echo ""
echo "============================================"
echo " GergiOS Smoke Test"
echo "============================================"
echo ""

# Record start time
START_TIME=$(date +%s 2>/dev/null || echo 0)

# Test: kernel version
echo "[TEST] uname -a"
uname -a 2>/dev/null || echo "[FAIL] uname command not available"

# Test: process list
echo "[TEST] ps"
ps -ef 2>/dev/null || ps 2>/dev/null || echo "[FAIL] ps command not available"

# Test: filesystem
echo "[TEST] df -h"
df -h 2>/dev/null || df 2>/dev/null || echo "[FAIL] df command not available"

# Test: /proc filesystem
echo "[TEST] /proc"
if [ -d /proc ]; then
    ls /proc/ 2>/dev/null
    echo "[PASS] /proc filesystem available"
else
    echo "[WARN] /proc not mounted"
fi

# Test: device nodes
echo "[TEST] /dev"
ls /dev/null /dev/tty /dev/console 2>/dev/null && \
    echo "[PASS] Essential device nodes present" || \
    echo "[WARN] Some device nodes missing"

# All tests complete
ELAPSED=$(( $(date +%s 2>/dev/null || echo 0) - START_TIME ))
echo ""
echo "============================================"
echo " Smoke Test Complete (${ELAPSED}s)"
echo "============================================"

# Signal test completion for serial parsing
echo "SMOKE_DONE"
sync
EOF
    chmod 755 "${ROOT_FS}/etc/rc.local"
    echo -e "${GREEN}   → Default smoke test rc.local injected${NC}"
fi

# Create /etc/fstab
cat >"${ROOT_FS}/etc/fstab" <<'EOF'
none    /sys        devman    rw,rslabel=devman    0    0
none    /dev/pts    ptyfs     rw,rslabel=ptyfs      0    0
EOF

# ─── Copy kernel + modules to ESP ───────────────────────────────────────
echo ""
echo -e "${YELLOW}Preparing boot files...${NC}"

if [ -n "$MODDIR" ] && [ -f "$MODDIR/kernel" ]; then
    cp "$MODDIR/kernel" "$ESP_DIR/kernel"
    echo " * Kernel: ${MODDIR}/kernel → ESP/kernel"

    # Copy modules
    for mod in "$MODDIR"/mod*; do
        if [ -f "$mod" ]; then
            cp "$mod" "$ESP_DIR/$(basename "$mod")"
        fi
    done
    MOD_COUNT=$(find "$ESP_DIR" -name 'mod*' -type f 2>/dev/null | wc -l)
    echo " * Modules: ${MOD_COUNT} files copied"
else
    echo -e "${YELLOW} * No kernel/modules available — creating stub image${NC}"
    # Create a minimal placeholder kernel for the ESP structure
    echo "placeholder" > "$ESP_DIR/kernel"
fi

# ─── Create limine.conf ─────────────────────────────────────────────────
echo " * Creating limine.conf..."

generate_modules_block() {
    local esp_dir="$1"
    if [ -d "$esp_dir" ]; then
        for mod in "$esp_dir"/mod*; do
            if [ -f "$mod" ]; then
                echo "    MODULE_PATH=boot:///$(basename "$mod")"
            fi
        done
    fi
}

MODS_BLOCK="$(generate_modules_block "$ESP_DIR")"

cat >"${ESP_DIR}/limine.conf" <<END_LIMINE
# GergiOS Test Image — generated by build_test_image.sh
TIMEOUT=3
:Test Boot
    PROTOCOL=limine
    KERNEL_PATH=boot:///kernel
${MODS_BLOCK}
    CMDLINE=rootdevname=c0d0p0

:Test Boot (Safe Mode)
    PROTOCOL=limine
    KERNEL_PATH=boot:///kernel
${MODS_BLOCK}
    CMDLINE=rootdevname=c0d0p0 bootopts=-s
END_LIMINE

# ─── Copy Limine bootloader ─────────────────────────────────────────────
echo " * Setting up Limine bootloader..."

# Try auto-detecting Limine
LIMINE_BIN="$(command -v limine 2>/dev/null || true)"
LIMINE_DATA=""

if [ -n "$LIMINE_BIN" ]; then
    LIMINE_VER="$("$LIMINE_BIN" --version 2>/dev/null || echo "unknown")"

    # Find Limine data directory
    LIMINE_DATA=$("$LIMINE_BIN" --print-data-dir 2>/dev/null || true)
    if [ -z "$LIMINE_DATA" ] || [ ! -d "$LIMINE_DATA" ]; then
        for d in /usr/share/limine /usr/local/share/limine /opt/limine/share; do
            [ -d "$d" ] && [ -f "$d/limine.sys" ] && { LIMINE_DATA="$d"; break; }
        done
    fi
fi

if [ -n "$LIMINE_DATA" ] && [ -d "$LIMINE_DATA" ]; then
    echo -e "${GREEN}   Limine: ${LIMINE_BIN} (v${LIMINE_VER}) at ${LIMINE_DATA}${NC}"

    # Copy stage files to ESP
    if [ -f "${LIMINE_DATA}/limine.sys" ]; then
        cp "${LIMINE_DATA}/limine.sys" "$ESP_DIR/limine.sys"
    fi

    # UEFI bootloader
    mkdir -p "$ESP_DIR/EFI/BOOT"
    if [ "$ARCH" = "x86_64" ] && [ -f "${LIMINE_DATA}/BOOTX64.EFI" ]; then
        cp "${LIMINE_DATA}/BOOTX64.EFI" "$ESP_DIR/EFI/BOOT/BOOTX64.EFI"
    elif [ "$ARCH" = "aarch64" ] && [ -f "${LIMINE_DATA}/BOOTAA64.EFI" ]; then
        cp "${LIMINE_DATA}/BOOTAA64.EFI" "$ESP_DIR/EFI/BOOT/BOOTAA64.EFI"
    fi

    HAS_LIMINE=true
else
    echo -e "${YELLOW}   Warning: Limine not found — image will not be bootable${NC}"
    echo -e "${YELLOW}   Install: https://github.com/limine-bootloader/limine${NC}"
    HAS_LIMINE=false
fi

# ─── Create disk image ──────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}Creating disk image...${NC}"

mkdir -p "$(dirname "$OUTPUT_IMG")"

# Calculate partition sizes (all aligned to 1MB)
ALIGN=$((1024*1024))
ESP_ALIGNED=$(( ((ESP_SIZE + ALIGN - 1) / ALIGN) * ALIGN ))
ROOT_ALIGNED=$(( ((ROOT_SIZE + ALIGN - 1) / ALIGN) * ALIGN ))

ESP_SECTORS=$((ESP_ALIGNED / 512))
ROOT_SECTORS=$((ROOT_ALIGNED / 512))

# FAT partition starting at sector 2048 (1MB alignment)
FAT_START=2048
ROOT_START=$((FAT_START + ESP_SECTORS))
TOTAL_SECTORS=$((ROOT_START + ROOT_SECTORS + 2048))  # extra padding

# Create sparse image
dd if=/dev/zero of="$OUTPUT_IMG" bs=512 count=1 seek=$((TOTAL_SECTORS - 1)) 2>/dev/null
echo -e "${GREEN} * Created ${TOTAL_SECTORS}-sector sparse image${NC}"

# Create GPT partition table
sgdisk -o "$OUTPUT_IMG" 2>/dev/null
sgdisk -n 1:${FAT_START}:+${ESP_SECTORS} -t 1:ef00 -c 1:"ESP" "$OUTPUT_IMG" 2>/dev/null
sgdisk -n 2:${ROOT_START}:+${ROOT_SECTORS} -t 2:8300 -c 2:"ROOT" "$OUTPUT_IMG" 2>/dev/null
sgdisk -A 1:set:2 "$OUTPUT_IMG" 2>/dev/null || true
echo -e "${GREEN} * GPT partition table created${NC}"

# ─── Format and populate ESP (FAT32) ────────────────────────────────────
echo " * Building ESP (FAT32, ${ESP_ALIGNED}B)..."

# Extract ESP region from image
ESP_IMG="${WORK_DIR}/esp.img"
dd if="$OUTPUT_IMG" of="$ESP_IMG" bs=512 skip=${FAT_START} count=${ESP_SECTORS} 2>/dev/null

# Format as FAT32
mkfs.fat -F 32 -n "GERGIOS-ESP" "$ESP_IMG" >/dev/null 2>&1
echo -e "${GREEN}   → Formatted as FAT32${NC}"

# Populate ESP
mcopy -D sG "$ESP_DIR/limine.conf" "$ESP_IMG" "limine.conf" 2>/dev/null

if [ -f "$ESP_DIR/limine.sys" ]; then
    mcopy -D sG "$ESP_DIR/limine.sys" "$ESP_IMG" "limine.sys" 2>/dev/null
fi
if [ -f "$ESP_DIR/kernel" ]; then
    mcopy -D sG "$ESP_DIR/kernel" "$ESP_IMG" "kernel" 2>/dev/null
fi
for mod in "$ESP_DIR"/mod*; do
    if [ -f "$mod" ]; then
        mcopy -D sG "$mod" "$ESP_IMG" "$(basename "$mod")" 2>/dev/null
    fi
done

# UEFI bootloader on ESP
if [ -d "$ESP_DIR/EFI" ]; then
    mmd -D sG "$ESP_IMG" "EFI" 2>/dev/null || true
    mmd -D sG "$ESP_IMG" "EFI/BOOT" 2>/dev/null || true
    for efi_file in "$ESP_DIR"/EFI/BOOT/*.EFI; do
        if [ -f "$efi_file" ]; then
            mcopy -D sG "$efi_file" "$ESP_IMG" "EFI/BOOT/$(basename "$efi_file")" 2>/dev/null
        fi
    done
fi

# Write ESP back to image
dd if="$ESP_IMG" of="$OUTPUT_IMG" bs=512 seek=${FAT_START} conv=notrunc 2>/dev/null
echo -e "${GREEN}   → ESP written${NC}"

# ─── Install Limine stage 1 (for BIOS boot) ─────────────────────────────
if [ "$HAS_LIMINE" = true ] && [ "$ARCH" = "x86_64" ]; then
    if "$LIMINE_BIN" bios-install "$OUTPUT_IMG" 2>/dev/null; then
        echo -e "${GREEN}   → Limine stage 1 installed (BIOS boot supported)${NC}"
    else
        echo -e "${YELLOW}   → limine bios-install skipped (UEFI-only on this host)${NC}"
    fi
fi

# ─── Create ext4 root filesystem ────────────────────────────────────────
echo " * Building ROOT filesystem (ext4, ${ROOT_ALIGNED}B)..."

ROOT_IMG="${WORK_DIR}/root.img"
truncate -s "${ROOT_ALIGNED}" "$ROOT_IMG"

# Use mke2fs to create ext4 with our root directory content
if command -v mke2fs >/dev/null 2>&1; then
    if command -v fakeroot >/dev/null 2>&1; then
        fakeroot mke2fs -t ext4 -b 4096 \
            -O ^has_journal,^metadata_csum,^64bit \
            -d "$ROOT_FS" \
            "$ROOT_IMG" >/dev/null 2>&1 && \
        echo -e "${GREEN}   → ext4 with fakeroot${NC}" || {
            echo -e "${YELLOW}   → fakeroot failed, retrying without...${NC}"
            mke2fs -t ext4 -b 4096 \
                -O ^has_journal,^metadata_csum,^64bit \
                -d "$ROOT_FS" \
                "$ROOT_IMG" >/dev/null 2>&1 && \
            echo -e "${GREEN}   → ext4 without fakeroot${NC}" || \
            echo -e "${YELLOW}   → ext4 fallback: empty filesystem${NC}"
        }
    else
        mke2fs -t ext4 -b 4096 \
            -O ^has_journal,^metadata_csum,^64bit \
            -d "$ROOT_FS" \
            "$ROOT_IMG" >/dev/null 2>&1 && \
        echo -e "${GREEN}   → ext4 (no fakeroot)${NC}" || \
        echo -e "${YELLOW}   → ext4 fallback: empty filesystem${NC}"
    fi
else
    # No mke2fs — create empty ext4 image
    echo -e "${YELLOW}   → mke2fs not found — creating empty ext4${NC}"
    echo -e "${YELLOW}   Install: apt install e2fsprogs${NC}"
fi

# Write ROOT partition to image
dd if="$ROOT_IMG" of="$OUTPUT_IMG" bs=512 seek=${ROOT_START} conv=notrunc 2>/dev/null
echo -e "${GREEN}   → ROOT partition written${NC}"

# ─── Clean up temporary files ──────────────────────────────────────────
rm -rf "$WORK_DIR"

# ─── Verify output ─────────────────────────────────────────────────────
IMG_SIZE_HUMAN=$(ls -lh "$OUTPUT_IMG" 2>/dev/null | awk '{print $5}')
echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Image created: ${OUTPUT_IMG}${NC}"
echo -e "${GREEN}Size:         ${IMG_SIZE_HUMAN}${NC}"
echo -e "${GREEN}Architecture: ${ARCH}${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "To boot in QEMU (BIOS):"
echo "  qemu-system-x86_64 -m 512M -drive file=${OUTPUT_IMG},format=raw -serial stdio"
echo ""
echo "To boot in QEMU (UEFI):"
echo "  qemu-system-x86_64 -m 512M -bios /usr/share/ovmf/OVMF_CODE.fd \\"
echo "    -drive file=${OUTPUT_IMG},format=raw -serial stdio"
echo ""

# Output image path for script chaining
echo "$OUTPUT_IMG"
