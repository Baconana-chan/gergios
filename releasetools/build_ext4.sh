#!/usr/bin/env bash
# build_ext4.sh — Build ext4 Rust staticlib, mkfs.ext4, and nbmkfs.ext4
#
# Usage:
#   ./releasetools/build_ext4.sh                    # Build native (staticlib + mkfs)
#   ./releasetools/build_ext4.sh cross x86_64       # Cross-compile for MINIX x86_64
#   ./releasetools/build_ext4.sh cross aarch64      # Cross-compile for MINIX aarch64
#   ./releasetools/build_ext4.sh host               # Build host tools (nbmkfs.ext4 for release images)
#   ./releasetools/build_ext4.sh clean              # Clean all build artifacts
#
# Prerequisites for cross-compilation:
#   1. Nightly Rust with rust-src component installed:
#       rustup toolchain install nightly --component rust-src
#   2. MINIX target spec installed to nightly sysroot:
#       bash releasetools/setup_minix_sysroot.sh install-target
#   3. (Optional) MINIX DESTDIR for linking executables:
#       export MINIX_DESTDIR=/opt/minix/destdir
#
# Output:
#   rust/target/<arch>-unknown-minix/release/libext4_core.a
#   build/tools/nbmkfs.ext4 (host tool for release image creation)

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUST_DIR="${PROJECT_DIR}/rust"
EXT4_CORE_DIR="${RUST_DIR}/ext4-core"
MKFS_C_SRC="${PROJECT_DIR}/minix/usr.sbin/mkfs.ext4/main.c"
BUILD_TOOLS_DIR="${PROJECT_DIR}/build/tools"

# ─── Colors ──────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}   $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }

# ─── Check prerequisites ────────────────────────────────
check_native() {
    if ! command -v cargo &>/dev/null; then
        err "cargo not found. Install Rust: https://rustup.rs"
        exit 1
    fi
    info "Rust $(rustc --version)"
}

check_cross() {
    local arch="$1"

    # Verify nightly Rust is available (required for -Zbuild-std)
    if ! cargo +nightly --version &>/dev/null; then
        err "Nightly Rust not available! Run: rustup toolchain install nightly --component rust-src"
        exit 1
    fi

    # Verify target spec is installed
    local sysroot; sysroot="$(rustc +nightly --print sysroot 2>/dev/null)"
    if [ ! -f "${sysroot}/lib/rustlib/${arch}-unknown-minix/target.json" ]; then
        err "Target spec not installed for ${arch}!"
        err "Run: ./releasetools/setup_minix_sysroot.sh install-target"
        exit 1
    fi

    ok "Nightly Rust: $(cargo +nightly --version 2>&1 | head -1)"
    ok "Target spec installed: ${arch}-unknown-minix"
    if [ -n "${MINIX_DESTDIR:-}" ]; then
        ok "MINIX sysroot: ${MINIX_DESTDIR}"
    else
        warn "MINIX_DESTDIR not set — needed for linking executables, not for --lib builds"
    fi
}

# ─── Build functions ─────────────────────────────────────
build_native() {
    info "Building ext4-core staticlib (native)..."
    cd "${EXT4_CORE_DIR}"
    cargo build --release --lib 2>&1 | tail -5
    local lib="target/release/libext4_core.a"
    if [ -f "${lib}" ]; then
        local size; size=$(du -h "${lib}" | cut -f1)
        ok "Static library: ${lib} (${size})"
    else
        local lib_win="target/release/ext4_core.lib"
        if [ -f "${lib_win}" ]; then
            local size; size=$(du -h "${lib_win}" | cut -f1)
            ok "Static library: ${lib_win} (${size})"
        else
            err "Static library not found!"; exit 1
        fi
    fi

    # Build native mkfs.ext4 (links against the Rust staticlib)
    local cc="${CC:-cc}"
    local mkfs_out="target/release/mkfs.ext4"
    info "Building native mkfs.ext4..."
    ${cc} -std=c11 -O2 -o "${mkfs_out}" "${MKFS_C_SRC}" \
        -L target/release -lext4_core -lm 2>&1 | tail -5
    if [ -f "${mkfs_out}" ]; then
        ok "Native mkfs.ext4: ${mkfs_out}"
    fi

    cd "${PROJECT_DIR}"
}

build_host() {
    info "Building nbmkfs.ext4 host tool..."
    mkdir -p "${BUILD_TOOLS_DIR}"

    # Build Rust staticlib for host
    cd "${EXT4_CORE_DIR}"
    cargo build --release --lib 2>&1 | tail -5
    local lib="target/release/libext4_core.a"

    if [ ! -f "${lib}" ]; then
        err "Host staticlib not found at ${lib}!"; exit 1
    fi
    ok "Host staticlib: ${lib}"

    # Compile mkfs.ext4 as host tool → nbmkfs.ext4
    local cc="${CC:-cc}"
    local nbmkfs="${BUILD_TOOLS_DIR}/nbmkfs.ext4"
    info "Compiling: ${cc} -std=c11 -O2 -o ${nbmkfs} ${MKFS_C_SRC} -L${EXT4_CORE_DIR}/target/release -lext4_core -lm"

    ${cc} -std=c11 -O2 -o "${nbmkfs}" "${MKFS_C_SRC}" \
        -L"${EXT4_CORE_DIR}/target/release" -lext4_core -lm 2>&1 | tail -5

    if [ -f "${nbmkfs}" ]; then
        local size; size=$(du -h "${nbmkfs}" | cut -f1)
        ok "Host tool: ${nbmkfs} (${size})"

        # Copy to CROSS_TOOLS location if set
        if [ -n "${CROSS_TOOLS:-}" ] && [ -d "${CROSS_TOOLS}" ]; then
            cp "${nbmkfs}" "${CROSS_TOOLS}/nbmkfs.ext4"
            ok "Installed to: ${CROSS_TOOLS}/nbmkfs.ext4"
        fi
    else
        err "nbmkfs.ext4 build failed!"; exit 1
    fi

    cd "${PROJECT_DIR}"
}

build_cross() {
    local arch="$1"
    info "Cross-compiling ext4-core staticlib for MINIX ${arch}..."
    cd "${EXT4_CORE_DIR}"

    # NOTE: Requires nightly Rust with target spec installed.
    # Run: ./releasetools/setup_minix_sysroot.sh install-target
    RUSTFLAGS="-Zunstable-options" \
    cargo +nightly build -Zbuild-std=core --release --lib --target "${arch}-unknown-minix" 2>&1 | tail -10

    local target_dir="${arch}-unknown-minix"
    local lib="target/${target_dir}/release/libext4_core.a"

    if [ -f "${lib}" ]; then
        local size; size=$(du -h "${lib}" | cut -f1)
        ok "Cross-compiled staticlib: ${lib} (${size})"

        # Copy to ext4 server CMake build directory
        local cmake_lib_dir="${PROJECT_DIR}/build/minix/fs/ext4"
        mkdir -p "${cmake_lib_dir}"
        cp "${lib}" "${cmake_lib_dir}/libext4_core.a"
        ok "Copied to CMake build directory"
    else
        err "Static library not found at ${lib}!"; exit 1
    fi

    cd "${PROJECT_DIR}"
}

build_clean() {
    info "Cleaning build artifacts..."
    cd "${EXT4_CORE_DIR}" && cargo clean && rm -rf target
    ok "Cleaned Rust build artifacts"
    rm -rf "${BUILD_TOOLS_DIR}"
    ok "Cleaned host tools"
    find "${PROJECT_DIR}/build" -name "libext4_core.a" -delete 2>/dev/null || true
    ok "Cleaned CMake build artifacts"
    cd "${PROJECT_DIR}"
}

# ─── Main ────────────────────────────────────────────────
main() {
    local cmd="${1:-native}"
    local arch="${2:-x86_64}"

    echo ""
    echo "╔══════════════════════════════════════════════╗"
    echo "║      ext4 Build Script (GergiOS)             ║"
    echo "╚══════════════════════════════════════════════╝"
    echo ""

    case "${cmd}" in
        native|"")
            check_native
            build_native
            ;;
        cross)
            check_cross "${arch}"
            build_cross "${arch}"
            ;;
        host)
            check_native
            build_host
            ;;
        clean)
            build_clean
            ;;
        help|*)
            echo "Usage:"
            echo "  ${BASH_SOURCE[0]}                    # Build native + mkfs.ext4"
            echo "  ${BASH_SOURCE[0]} cross x86_64       # Cross-compile for MINIX"
            echo "  ${BASH_SOURCE[0]} cross aarch64      # Cross-compile for ARM64"
            echo "  ${BASH_SOURCE[0]} host               # Build nbmkfs.ext4 host tool"
            echo "  ${BASH_SOURCE[0]} clean              # Clean artifacts"
            echo ""
            exit 0 ;;
    esac

    echo ""
    ok "Done."
}

main "$@"
