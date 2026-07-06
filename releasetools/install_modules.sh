#!/bin/sh
#
# install_modules.sh — GergiOS Module Hierarchy Setup
# ====================================================
#
# Creates the /lib/modules/$(uname -r)/ hierarchy following Linux convention
# and installs pre-built kernel modules from the flat /lib/modules/gergios/
# directory.
#
# Usage:
#   ./install_modules.sh                      # Auto-detect kernel version
#   ./install_modules.sh -r 5.15.0-gergios    # Specify kernel version
#   ./install_modules.sh -s /alt/source        # Alternate source dir
#   ./install_modules.sh -d /alt/dest          # Alternate dest dir
#   ./install_modules.sh -n                    # Dry run
#   ./install_modules.sh -c                    # Custom category mapping
#   ./install_modules.sh --run-depmod          # Also run depmod
#
# Category mapping (auto-detected by module name prefix/PCI class):
#   ahci*                     → ata/
#   nvme*                     → nvme/
#   e1000, rtl*, fxp, lance   → net/
#   xhci*, ehci*, ohci*       → usb/
#   hda*, snd*                → audio/
#   drm*, i915*, amdgpu*      → video/
#   hid*, usbhid              → hid/
#   i2c*, spi*                → char/
#   evdev, joydev, mousedev   → input/
#   *                         → extra/  (fallback)
#

set -e

# Defaults
KERNEL_VER="$(uname -r 2>/dev/null || echo 'gergios')"
SOURCE_DIR="/lib/modules/gergios"
DEST_DIR="/lib/modules/${KERNEL_VER}"
DRY_RUN=0
RUN_DEPMOD=0

# Color output
info()  { printf "  [INFO]  %s\\n" "$*"; }
ok()    { printf "  [ OK ]  %s\\n" "$*"; }
warn()  { printf "  [WARN]  %s\\n" "$*"; }
err()   { printf "  [ERR]   %s\\n" "$*"; exit 1; }

# Auto-categorize a module based on its filename
# $1: module filename (e.g., "ahci.ko")
# Returns: category subdirectory (e.g., "ata")
auto_category() {
    local name="$1"
    local base="${name%.*}"  # Strip .ko/.so

    case "${base}" in
        ahci*|ata_*|sata_*|libata*)          echo "ata";;
        nvme*|nvmet*)                         echo "nvme";;
        e1000|e1000e|igb|igc|ixgbe)          echo "net";;
        rtl8139|rtl8169|r8169|r8125)         echo "net";;
        fxp|lance|dp8390|dpeth)              echo "net";;
        virtio_net|virtio_blk|virtio_scsi)   echo "virtio";;
        xhci*|ehci*|ohci*|uhci*)             echo "usb";;
        usb_storage|usbhid|usbcore)          echo "usb";;
        hda*|snd_hda*|snd_*|soundcore)       echo "audio";;
        drm*|i915|amdgpu|nouveau|radeon)     echo "video";;
        hid*|usbhid|wacom)                   echo "hid";;
        i2c_*|spi_*)                         echo "char";;
        evdev|joydev|mousedev|serio*)        echo "input";;
        pcie*|acpi*|pci_*)                   echo "pci";;
        scsi_*|sd_mod|sr_mod|sg)             echo "scsi";;
        mmc*|sdhci|rtsx*)                    echo "mmc";;
        mtd*|nand|ubi*)                      echo "mtd";;
        ath*|iwl*|b43*|brcm*)               echo "net";  # WiFi
        btusb|bluetooth|hci_*)               echo "net";  # Bluetooth
        *)                                    echo "extra";;
    esac
}

# Create the /lib/modules/ hierarchy
create_hierarchy() {
    info "Creating module hierarchy: ${DEST_DIR}"

    if [ "${DRY_RUN}" -eq 1 ]; then
        echo "  Would create: ${DEST_DIR}/kernel/drivers/{ata,nvme,net,usb,audio,video,hid,char,input,pci,scsi,mmc,mtd,extra}"
        return
    fi

    mkdir -p "${DEST_DIR}/kernel/drivers/ata"
    mkdir -p "${DEST_DIR}/kernel/drivers/nvme"
    mkdir -p "${DEST_DIR}/kernel/drivers/net"
    mkdir -p "${DEST_DIR}/kernel/drivers/usb"
    mkdir -p "${DEST_DIR}/kernel/drivers/audio"
    mkdir -p "${DEST_DIR}/kernel/drivers/video"
    mkdir -p "${DEST_DIR}/kernel/drivers/hid"
    mkdir -p "${DEST_DIR}/kernel/drivers/char"
    mkdir -p "${DEST_DIR}/kernel/drivers/input"
    mkdir -p "${DEST_DIR}/kernel/drivers/pci"
    mkdir -p "${DEST_DIR}/kernel/drivers/scsi"
    mkdir -p "${DEST_DIR}/kernel/drivers/mmc"
    mkdir -p "${DEST_DIR}/kernel/drivers/mtd"
    mkdir -p "${DEST_DIR}/kernel/drivers/virtio"
    mkdir -p "${DEST_DIR}/kernel/drivers/extra"
    ok "Hierarchy created"
}

# Install modules from source to category directories
install_modules() {
    local count=0
    local skipped=0

    if [ ! -d "${SOURCE_DIR}" ]; then
        warn "Source directory '${SOURCE_DIR}' does not exist — nothing to install"
        return
    fi

    info "Installing modules from ${SOURCE_DIR}..."

    for f in "${SOURCE_DIR}"/*.ko "${SOURCE_DIR}"/*.so; do
        [ -f "${f}" ] || continue

        local base
        base="$(basename "${f}")"
        local category
        category="$(auto_category "${base}")"

        if [ "${DRY_RUN}" -eq 1 ]; then
            echo "  WOULD INSTALL: ${base} → kernel/drivers/${category}/"
            count=$((count + 1))
            continue
        fi

        # Install with file flags preserved
        cp -f "${f}" "${DEST_DIR}/kernel/drivers/${category}/" 2>/dev/null || {
            warn "Failed to install ${base}"
            skipped=$((skipped + 1))
            continue
        }
        count=$((count + 1))
    done

    if [ "${DRY_RUN}" -eq 0 ]; then
        ok "Installed ${count} module(s) (${skipped} skipped)"
    else
        info "Would install ${count} module(s)"
    fi
}

# Run depmod to generate dependency/alias/symbol files
run_depmod() {
    if ! command -v depmod >/dev/null 2>&1; then
        warn "depmod not found — skipping dependency generation"
        warn "  Manually run: depmod -a -o ${DEST_DIR}"
        return
    fi

    info "Running depmod for ${DEST_DIR}..."
    if [ "${DRY_RUN}" -eq 1 ]; then
        echo "  Would run: depmod -a -o ${DEST_DIR}"
        return
    fi

    depmod -a -o "${DEST_DIR}" 2>&1 || {
        warn "depmod returned non-zero — check errors above"
        return
    }
    ok "depmod complete: modules.dep, modules.alias, modules.symbols generated"
}

# ============================================================================
# Main
# ============================================================================

usage() {
    cat <<EOF
Usage: $0 [options]

Options:
  -r version    Kernel version (default: \$(uname -r))
  -s dir        Source module directory (default: /lib/modules/gergios/)
  -d dir        Destination directory (default: /lib/modules/\$(uname -r)/)
  -n            Dry run — show what would be done
  --run-depmod  Also run depmod after installation
  -h            Show this help

Examples:
  $0                                    # Auto-detect, install from /lib/modules/gergios/
  $0 -r 5.15.0-gergios -s /tmp/modules # Custom version and source
  $0 -n                                 # Dry run preview
  $0 --run-depmod                       # Install + run depmod
EOF
    exit 0
}

while [ $# -gt 0 ]; do
    case "$1" in
        -r) KERNEL_VER="$2"; shift 2;;
        -s) SOURCE_DIR="$2"; shift 2;;
        -d) DEST_DIR="$2"; shift 2;;
        -n) DRY_RUN=1; shift;;
        --run-depmod) RUN_DEPMOD=1; shift;;
        -h|--help) usage;;
        *) err "Unknown option: $1. Use -h for help.";;
    esac
done

echo "================================================"
echo "  GergiOS Module Installation Script"
echo "================================================"
echo "  Kernel version:  ${KERNEL_VER}"
echo "  Source:          ${SOURCE_DIR}"
echo "  Destination:     ${DEST_DIR}"
echo "  Dry run:         ${DRY_RUN}"
echo "  Run depmod:      ${RUN_DEPMOD}"
echo "================================================"

create_hierarchy
install_modules

if [ "${RUN_DEPMOD}" -eq 1 ]; then
    run_depmod
fi

echo ""
echo "Installation complete."
echo "  ${DEST_DIR}/kernel/drivers/{ata,nvme,net,usb,audio,...}"
echo ""
echo "Run 'depmod -a -o ${DEST_DIR}' to generate module metadata."
