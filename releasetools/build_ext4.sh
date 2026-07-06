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
#   1. MINIX cross-toolchain installed (e.g., /opt/minix/toolchain)
#   2. MINIX DESTDIR (sysroot) populated (e.g., /opt/minix/destdir)
#   3. Environment variables:
#       export MINIX_TOOLCHAIN=/opt/minix/toolchain
#       export MINIX_DESTDIR=/opt/minix/destdir
#
# Output:
#   rust/ext4-core/target/<target>/release/libext4_core.a
#   build/tools/nbmkfs.ext4 (host tool for release image creation)

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}"")" && pwd)"
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
    local toolchain_prefix
    case "${arch}" in
        x86_64)  toolchain_prefix="x86_64-elf64-minix" ;;
        aarch64) toolchain_prefix="aarch64-elf64-minix" ;;
        *)
            err "Unsupported architecture: ${arch} (use x86_64 or aarch64)"
            exit 1 ;;
    esac
    if [ -z "${MINIX_TOOLCHAIN:-}" ]; then
        err "MINIX_TOOLCHAIN not set!"; exit 1
    fi
    if [ -z "${MINIX_DESTDIR:-}" ]; then
        err "MINIX_DESTDIR not set!"; exit 1
    fi
    local linker="${MINIX_TOOLCHAIN}/bin/${toolchain_prefix}-gcc"
    if [ ! -x "${linker}" ]; then
        err "Linker not found: ${linker}"; exit 1
    fi
    ok "Cross-linker found: ${linker}"
    ok "MINIX sysroot: ${MINIX_DESTDIR}"
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
    local target_file="${RUST_DIR}/x86_64-unknown-minix.json"
    local toolchain_prefix="x86_64-elf64-minix"

    if [ "${arch}" = "aarch64" ]; then
        target_file="${RUST_DIR}/aarch64-unknown-minix.json"
        toolchain_prefix="aarch64-elf64-minix"
        if [ ! -f "${target_file}" ]; then
            err "Create ${target_file} first (see planning docs)"
            exit 1
        fi
    fi

    info "Cross-compiling ext4-core staticlib for MINIX ${arch}..."
    cd "${EXT4_CORE_DIR}"

    MINIX_TOOLCHAIN="${MINIX_TOOLCHAIN}" \
    MINIX_DESTDIR="${MINIX_DESTDIR}" \
    RUSTFLAGS="-C linker=${MINIX_TOOLCHAIN}/bin/${toolchain_prefix}-gcc" \
    cargo build --release --lib --target "${target_file}" 2>&1 | tail -10

    local target_dir; target_dir=$(basename "${target_file}" .json)
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
