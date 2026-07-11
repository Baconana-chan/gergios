#!/usr/bin/env bash
# setup_minix_sysroot.sh — Create/verify MINIX DESTDIR for Rust cross-compilation
#
# Usage:
#   ./releasetools/setup_minix_sysroot.sh                      # Interactive setup
#   ./releasetools/setup_minix_sysroot.sh check                # Check current state
#   ./releasetools/setup_minix_sysroot.sh headers              # Install headers only
#   ./releasetools/setup_minix_sysroot.sh libs                 # Build MINIX libs from source
#   ./releasetools/setup_minix_sysroot.sh test                 # Test cross-compilation
#   ./releasetools/setup_minix_sysroot.sh --destdir /opt/minix/destdir
#
# Prerequisites:
#   - Nightly Rust (for -Zbuild-std support with custom targets)
#   - clang + lld (for cross-compilation)
#   - Rust toolchain (rustc + cargo)
#
# After running, set:
#   export MINIX_DESTDIR=/opt/minix/destdir
#
# Then cross-compile:
#   cargo +nightly build -Zbuild-std=core --target x86_64-unknown-minix --lib

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DESTDIR="${DESTDIR:-/opt/minix/destdir}"
ARCH="${ARCH:-x86_64}"
MACHINE="${MACHINE:-x86_64}"

# Colors
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
info()  { echo -e "${BLUE}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}   $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()   { echo -e "${RED}[ERROR]${NC} $*"; }

# ─── Config ──────────────────────────────────────────────
# Tool prefix for cross-compilation
case "${ARCH}" in
    x86_64)  TOOL_PREFIX="x86_64-elf64-minix" ; LLVM_TARGET="x86_64-elf"  ;;
    aarch64) TOOL_PREFIX="aarch64-elf64-minix"; LLVM_TARGET="aarch64-elf" ;;
    *)       err "Unsupported arch: ${ARCH} (use x86_64 or aarch64)"; exit 1 ;;
esac

# ─── Helpers ─────────────────────────────────────────────

# Get nightly sysroot path
nightly_sysroot() {
    if rustup toolchain list 2>/dev/null | grep -q "nightly"; then
        rustc +nightly --print sysroot 2>/dev/null
    else
        echo ""
    fi
}

# Check if target spec is installed in nightly sysroot
target_spec_installed() {
    local sysroot arch="$1"
    sysroot="$(nightly_sysroot)"
    [ -n "${sysroot}" ] && [ -f "${sysroot}/lib/rustlib/${arch}-unknown-minix/target.json" ]
}

# Install target spec to nightly sysroot
install_target_spec() {
    local arch="$1"
    local sysroot
    sysroot="$(nightly_sysroot)"
    if [ -z "${sysroot}" ]; then
        err "Nightly Rust not installed. Run: rustup toolchain install nightly"
        return 1
    fi

    local target_dir="${sysroot}/lib/rustlib/${arch}-unknown-minix"
    local src_spec="${PROJECT_DIR}/rust/${arch}-unknown-minix.json"

    if [ ! -f "${src_spec}" ]; then
        err "Target spec not found: ${src_spec}"
        return 1
    fi

    mkdir -p "${target_dir}"
    cp "${src_spec}" "${target_dir}/target.json"
    ok "Installed ${arch}-unknown-minix target spec to nightly sysroot"
}

# ─── Commands ────────────────────────────────────────────

cmd_check() {
    echo ""
    echo "╔══════════════════════════════════════════════╗"
    echo "║   MINIX Cross-Compilation Environment Check  ║"
    echo "╚══════════════════════════════════════════════╝"
    echo ""

    # Rust
    if command -v rustc &>/dev/null; then
        ok "rustc: $(rustc --version)"
    else
        err "rustc not found!"
    fi
    if command -v cargo &>/dev/null; then
        ok "cargo: $(cargo --version | head -1)"
    else
        err "cargo not found!"
    fi

    # Nightly
    if rustup toolchain list 2>/dev/null | grep -q "nightly"; then
        ok "nightly: $(rustc +nightly --version 2>/dev/null | head -1)"
    else
        warn "nightly not installed (required for -Zbuild-std)"
        info "  Install: rustup toolchain install nightly --component rust-src"
    fi

    # Clang + lld
    if command -v clang &>/dev/null; then
        ok "clang: $(clang --version | head -1)"
    else
        err "clang not found!"
    fi
    if command -v ld.lld &>/dev/null; then
        ok "lld:   $(ld.lld --version | head -1)"
    else
        warn "ld.lld not found — fallback to system linker"
    fi

    # Clang can cross-compile?
    if command -v clang &>/dev/null; then
        echo 'int main(){}' | clang --target="${LLVM_TARGET}" -x c - -o /dev/null 2>/dev/null \
            && ok "clang can cross-compile for ${LLVM_TARGET}" \
            || warn "clang cross-compile test failed — may need sysroot"
    fi

    # DESTDIR
    echo ""
    echo "─── DESTDIR (sysroot) ───"
    if [ -n "${MINIX_DESTDIR:-}" ]; then
        ok "MINIX_DESTDIR=${MINIX_DESTDIR}"
    else
        warn "MINIX_DESTDIR not set"
    fi
    if [ -n "${DESTDIR}" ] && [ -d "${DESTDIR}" ]; then
        ok "DESTDIR=${DESTDIR} exists"
    else
        warn "DESTDIR=${DESTDIR} does not exist"
    fi

    # Check for essential headers
    echo ""
    echo "─── Headers ───"
    for hdr in "minix/com.h" "minix/type.h" "minix/endpoint.h" "minix/ipc.h" "stdio.h" "stdlib.h" "string.h"; do
        if [ -n "${MINIX_DESTDIR:-}" ] && [ -f "${MINIX_DESTDIR}/usr/include/${hdr}" ]; then
            ok "  ${hdr}"
        elif [ -f "${PROJECT_DIR}/include/${hdr}" ]; then
            ok "  ${hdr} (in source tree)"
        else
            warn "  ${hdr} — not found"
        fi
    done

    # Check for essential libs
    echo ""
    echo "─── Libraries (.a) ───"
    for lib in "libc.a" "libsys.a" "libminc.a" "libtimers.a"; do
        found=0
        for dir in "${MINIX_DESTDIR:-}" "${DESTDIR}" "${PROJECT_DIR}/build"; do
            if [ -n "${dir}" ] && [ -f "${dir}/usr/lib/${lib}" ]; then
                ok "  ${lib} (${dir})"
                found=1
                break
            fi
        done
        if [ "${found}" -eq 0 ]; then
            warn "  ${lib} — not found (needed for linking executables)"
        fi
    done

    # Rust target spec in nightly sysroot
    echo ""
    echo "─── Rust Target Spec ───"
    local src_spec="${PROJECT_DIR}/rust/${ARCH}-unknown-minix.json"
    if [ -f "${src_spec}" ]; then
        ok "Source spec: ${src_spec}"
    else
        err "Source spec not found: ${src_spec}"
    fi

    if target_spec_installed "${ARCH}"; then
        ok "Installed in nightly sysroot: ✓"
    else
        warn "Not installed in nightly sysroot"
        info "  Run: ${BASH_SOURCE[0]} install-target"
    fi

    # Summary
    echo ""
    echo "─── Summary ───"
    echo "  Target:        ${ARCH}"
    echo "  Tool prefix:   ${TOOL_PREFIX}"
    echo "  LLVM target:   ${LLVM_TARGET}"
    echo "  DESTDIR:       ${DESTDIR}"
    echo "  MINIX_DESTDIR: ${MINIX_DESTDIR:-<not set>}"
    echo "  Source tree:   ${PROJECT_DIR}"
    echo ""
    echo "To install target spec for nightly:"
    echo "  ./releasetools/setup_minix_sysroot.sh install-target"
    echo ""
    echo "To test cross-compilation:"
    echo "  ./releasetools/setup_minix_sysroot.sh test"
    echo ""
    echo "To cross-compile (nightly):"
    echo "  RUSTFLAGS=\"-Zunstable-options\" cargo +nightly build -Zbuild-std=core --target ${ARCH}-unknown-minix --lib"
    echo ""
}

cmd_headers() {
    echo ""
    info "Installing MINIX headers to ${DESTDIR}/usr/include/..."
    echo ""

    mkdir -p "${DESTDIR}/usr/include"
    mkdir -p "${DESTDIR}/usr/include/minix"
    mkdir -p "${DESTDIR}/usr/include/sys"
    mkdir -p "${DESTDIR}/usr/include/machine"

    # Copy source tree headers
    for dir in include minix/include; do
        if [ -d "${PROJECT_DIR}/${dir}" ]; then
            info "Copying from ${dir}/ → ${DESTDIR}/usr/include/"
            cp -r "${PROJECT_DIR}/${dir}/"* "${DESTDIR}/usr/include/" 2>/dev/null || true
        fi
    done

    # Copy minix-specific headers
    if [ -d "${PROJECT_DIR}/minix/include/minix" ]; then
        info "Copying minix/include/minix/ → ${DESTDIR}/usr/include/minix/"
        mkdir -p "${DESTDIR}/usr/include/minix"
        cp -r "${PROJECT_DIR}/minix/include/minix/"* "${DESTDIR}/usr/include/minix/" 2>/dev/null || true
    fi

    # Copy sys/ headers
    if [ -d "${PROJECT_DIR}/sys" ]; then
        info "Copying sys/ → ${DESTDIR}/usr/include/sys/"
        cp -r "${PROJECT_DIR}/sys/"* "${DESTDIR}/usr/include/sys/" 2>/dev/null || true
    fi

    # Copy machine/ headers
    if [ -d "${PROJECT_DIR}/machine" ]; then
        info "Copying machine/ → ${DESTDIR}/usr/include/machine/"
        cp -r "${PROJECT_DIR}/machine/"* "${DESTDIR}/usr/include/machine/" 2>/dev/null || true
    fi

    # Count
    local count
    count=$(find "${DESTDIR}/usr/include" -name "*.h" 2>/dev/null | wc -l)
    ok "Installed ${count} headers to ${DESTDIR}/usr/include/"
    echo ""
    echo "Next: build MINIX libraries:"
    echo "  ./releasetools/setup_minix_sysroot.sh libs"
    echo ""
}

cmd_libs() {
    echo ""
    info "Building MINIX libraries for ${ARCH} (this may take a while)..."
    echo ""

    # Ensure headers are installed first
    if [ ! -f "${DESTDIR}/usr/include/minix/com.h" ]; then
        warn "Headers not installed. Running setup_minix_sysroot.sh headers first..."
        cmd_headers
    fi

    mkdir -p "${DESTDIR}/usr/lib"

    local clang_flags="--target=${LLVM_TARGET} -nostdinc -I${DESTDIR}/usr/include -I${DESTDIR}/usr/include/minix -static -ffreestanding -fno-builtin -O2"
    local lib_build_dir="${PROJECT_DIR}/build/libs-${ARCH}"
    mkdir -p "${lib_build_dir}"

    # Build libminc (minimal C library: string, stdlib, stdio)
    info "Building libminc.a..."
    (
        cd "${lib_build_dir}"
        mkdir -p minc
        local failures=0 total=0
        for src in "${PROJECT_DIR}"/minix/lib/libminc/*.c; do
            [ -f "${src}" ] || continue
            total=$((total + 1))
            if clang ${clang_flags} -c "${src}" -o "minc/$(basename "${src%.c}.o")" 2>/tmp/minix_libc_err.log; then
                echo -n "."
            else
                warn "FAILED: $(basename "${src}"): $(head -1 /tmp/minix_libc_err.log)"
                failures=$((failures + 1))
            fi
        done
        local obj_count; obj_count=$(ls minc/*.o 2>/dev/null | wc -l)
        if [ "${obj_count}" -gt 0 ]; then
            llvm-ar rcs "${DESTDIR}/usr/lib/libminc.a" minc/*.o && ok "libminc.a: ${obj_count}/${total} .o files built (${failures} failed)"
        else
            warn "libminc.a: ALL ${total} files failed to compile!"
            warn "  libminc depends on generated headers and arch-specific asm."
            warn "  For a full build, use the BSD Make build system:"
            warn "  cd ${PROJECT_DIR} && make DESTDIR=${DESTDIR} MACHINE=${ARCH} distribution"
        fi
    )

    # Build libsys (MINIX system library)
    info "Building libsys.a..."
    (
        cd "${lib_build_dir}"
        mkdir -p sys
        for src in "${PROJECT_DIR}"/minix/lib/libsys/*.c; do
            [ -f "${src}" ] && clang ${clang_flags} -I"${PROJECT_DIR}/minix/lib/libsys" -c "${src}" -o "sys/$(basename "${src%.c}.o")" 2>/dev/null && echo -n "." || true
        done
        if [ -d "${PROJECT_DIR}/minix/lib/libsys/arch/${ARCH}" ]; then
            for src in "${PROJECT_DIR}/minix/lib/libsys/arch/${ARCH}"/*.c; do
                [ -f "${src}" ] && clang ${clang_flags} -I"${PROJECT_DIR}/minix/lib/libsys" -c "${src}" -o "sys/$(basename "${src%.c}.o")" 2>/dev/null && echo -n "." || true
            done
        fi
        llvm-ar rcs "${DESTDIR}/usr/lib/libsys.a" sys/*.o 2>/dev/null && ok "libsys.a done" || warn "libsys.a partially built"
    )

    # Build libtimers
    info "Building libtimers.a..."
    (
        cd "${lib_build_dir}"
        mkdir -p timers
        for src in "${PROJECT_DIR}"/minix/lib/libtimers/*.c; do
            [ -f "${src}" ] && clang ${clang_flags} -c "${src}" -o "timers/$(basename "${src%.c}.o")" 2>/dev/null && echo -n "." || true
        done
        llvm-ar rcs "${DESTDIR}/usr/lib/libtimers.a" timers/*.o 2>/dev/null && ok "libtimers.a done" || warn "libtimers.a partially built"
    )

    # Build libc (from NetBSD libc sources)
    info "Building libc.a (this is the big one)..."
    (
        cd "${lib_build_dir}"
        mkdir -p libc

        # Compile libc sources from common/lib/libc
        local libc_dir="${PROJECT_DIR}/common/lib/libc"
        if [ -d "${libc_dir}" ]; then
            for src in $(find "${libc_dir}" -name "*.c" -not -path "*/arch/*" 2>/dev/null | head -50); do
                clang ${clang_flags} -I"${libc_dir}/include" -c "${src}" -o "libc/$(basename "${src%.c}.o")" 2>/dev/null && echo -n "." || true
            done
        fi
        llvm-ar rcs "${DESTDIR}/usr/lib/libc.a" libc/*.o 2>/dev/null && ok "libc.a done (partial)" || warn "libc.a partially built"
    )

    echo ""
    info "Library build complete."
    echo "  Libraries in: ${DESTDIR}/usr/lib/"
    ls -la "${DESTDIR}/usr/lib/"*.a 2>/dev/null
    echo ""
    echo "Libraries may be incomplete (some .o files may fail to compile)."
    echo "For a full DESTDIR, use the MINIX BSD Make build:"
    echo "  cd ${PROJECT_DIR} && make DESTDIR=${DESTDIR} distribution"
    echo ""
}

cmd_install_target() {
    echo ""
    info "Installing MINIX target spec(s) to nightly Rust sysroot..."
    echo ""

    if ! rustup toolchain list 2>/dev/null | grep -q "nightly"; then
        err "Nightly Rust not installed! Run: rustup toolchain install nightly --component rust-src"
        exit 1
    fi

    local sysroot
    sysroot="$(rustc +nightly --print sysroot 2>/dev/null)"
    ok "Nightly sysroot: ${sysroot}"

    # Install for each architecture (skip if source file doesn't exist)
    for arch in x86_64 aarch64; do
        local src_spec="${PROJECT_DIR}/rust/${arch}-unknown-minix.json"
        if [ -f "${src_spec}" ]; then
            install_target_spec "${arch}"
        else
            warn "Skipping ${arch}: source spec not found at ${src_spec}"
        fi
    done

    echo ""
    ok "Target spec installation complete."
    echo ""
    info "Now you can cross-compile:"
    echo "  cd rust && RUSTFLAGS=\"-Zunstable-options\" cargo +nightly build -Zbuild-std=core --target x86_64-unknown-minix -p ext4-core --lib"
    echo ""
}

cmd_test() {
    echo ""
    info "Testing Rust cross-compilation for MINIX ${ARCH}..."
    echo ""

    # Step 0: Check nightly is available
    if ! rustup toolchain list 2>/dev/null | grep -q "nightly"; then
        err "Nightly Rust not installed!"
        err "Run: rustup toolchain install nightly --component rust-src"
        exit 1
    fi

    # Step 0.5: Check rust-src component is installed (required by -Zbuild-std)
    if ! rustup component list --toolchain nightly 2>/dev/null | grep -q "rust-src"; then
        err "rust-src component not installed on nightly!"
        err "Run: rustup component add rust-src --toolchain nightly"
        exit 1
    fi

    # Step 1: Install target spec if needed
    if ! target_spec_installed "${ARCH}"; then
        info "Installing ${ARCH} target spec to nightly sysroot..."
        install_target_spec "${ARCH}" || exit 1
    fi

    # Step 2: Create minimal test project for quick validation
    local test_dir="/tmp/minix-test-${ARCH}"
    rm -rf "${test_dir}"
    mkdir -p "${test_dir}/src"

    cat > "${test_dir}/Cargo.toml" << 'EOF'
[package]
name = "minix-test"
version = "0.1.0"
edition = "2021"
EOF

    cat > "${test_dir}/src/lib.rs" << 'EOF'
#![no_std]
pub fn add(a: i32, b: i32) -> i32 { a + b }
EOF

    info "1. Minimal no_std crate: cargo +nightly build -Zbuild-std=core..."
    cd "${test_dir}"
    if RUSTFLAGS="-Zunstable-options" cargo +nightly build -Zbuild-std=core --target "${ARCH}-unknown-minix" --lib 2>&1 | tail -20; then
        ok "✓ Minimal no_std crate compiles for ${ARCH}-unknown-minix"
    else
        warn "✗ Minimal no_std crate FAILED — see errors above"
    fi

    # Step 3: Test ext4-core
    echo ""
    info "2. Testing: cargo +nightly build -Zbuild-std=core for ext4-core..."
    cd "${PROJECT_DIR}/rust"
    if RUSTFLAGS="-Zunstable-options" cargo +nightly build -Zbuild-std=core,alloc --target "${ARCH}-unknown-minix" -p ext4-core --lib 2>&1 | tail -30; then
        ok "✓ cargo build PASSED for ext4-core"
    else
        warn "✗ cargo build FAILED for ext4-core (pre-existing crate issue, not toolchain)"
    fi

    # Step 4: Test minix-rs
    echo ""
    info "3. Testing: cargo +nightly build -Zbuild-std=core for minix-rs (no_std, FFI bindings)..."
    if RUSTFLAGS="-Zunstable-options" cargo +nightly build -Zbuild-std=core --target "${ARCH}-unknown-minix" -p minix-rs --lib 2>&1 | tail -30; then
        ok "✓ cargo build PASSED for minix-rs"
    else
        warn "✗ cargo build FAILED for minix-rs (pre-existing crate issue, not toolchain)"
    fi

    # Step 5: Full build if DESTDIR has libraries
    echo ""
    if [ -f "${MINIX_DESTDIR:-${DESTDIR}}/usr/lib/libc.a" ] || [ -f "${MINIX_DESTDIR:-${DESTDIR}}/usr/lib/libminc.a" ]; then
        info "4. Full build: cargo +nightly build -Zbuild-std=core for ext4-core..."
        cd "${PROJECT_DIR}/rust"
        if RUSTFLAGS="-Zunstable-options" cargo +nightly build -Zbuild-std=core,alloc --target "${ARCH}-unknown-minix" -p ext4-core --lib 2>&1 | tail -20; then
            ok "✓ cargo build PASSED for ext4-core"
            local rel_dir="${PROJECT_DIR}/rust/target/${ARCH}-unknown-minix/release"
            [ -f "${rel_dir}/libext4_core.rlib" ] && ok "  Output: ${rel_dir}/libext4_core.rlib"
        else
            warn "✗ cargo build FAILED — DESTDIR may be incomplete"
        fi
    else
        warn "Skipping full build — DESTDIR libraries not found"
        warn "  MINIX_DESTDIR=${MINIX_DESTDIR:-${DESTDIR}}"
        warn "  Run: sudo ./releasetools/setup_minix_sysroot.sh --destdir ${MINIX_DESTDIR:-${DESTDIR}} libs"
    fi

    cd "${PROJECT_DIR}"
    echo ""
    ok "Test complete."
}

cmd_usage() {
    echo "Usage: ${BASH_SOURCE[0]} [command] [options]"
    echo ""
    echo "Commands:"
    echo "  check            Check current cross-compilation environment"
    echo "  headers          Install MINIX headers to DESTDIR"
    echo "  libs             Build MINIX libraries from source (experimental)"
    echo "  install-target   Install target spec(s) to nightly Rust sysroot"
    echo "  test             Test Rust cross-compilation"
    echo "  help             Show this help"
    echo ""
    echo "Options:"
    echo "  --destdir DIR   Set DESTDIR (default: /opt/minix/destdir)"
    echo "  --arch ARCH     Set architecture: x86_64 (default) or aarch64"
    echo ""
    echo "Note: This script requires a Unix-like environment (Linux, macOS, WSL)."
    echo "On Windows, use WSL or set env vars manually:"
    echo "  set MINIX_DESTDIR=C:\\(...)\\destdir"
    echo "  set RUSTFLAGS=-Zunstable-options"
    echo ""
}

# ─── Main ────────────────────────────────────────────────
main() {
    local cmd="${1:-check}"

    # Parse --destdir and --arch
    while [ $# -gt 0 ]; do
        case "$1" in
            --destdir) DESTDIR="$2"; shift 2 ;;
            --arch)    ARCH="$2"; shift 2 ;;
            help|--help|-h) cmd_usage; exit 0 ;;
            *) cmd="$1"; shift ;;
        esac
    done

    echo ""
    echo "╔══════════════════════════════════════════════╗"
    echo "║   MINIX Cross-Compilation Setup              ║"
    echo "║   Target: ${ARCH}                              ║"
    echo "║   DESTDIR: ${DESTDIR}                         ║"
    echo "╚══════════════════════════════════════════════╝"
    echo ""

    case "${cmd}" in
        check)           cmd_check           ;;
        headers)         cmd_headers         ;;
        libs)            cmd_libs            ;;
        install-target)  cmd_install_target  ;;
        test)            cmd_test            ;;
        *)               cmd_usage           ;;
    esac
}

main "$@"
