#!/usr/bin/env sh
# releasetools/cmake-build.sh
# CMake build wrapper for MINIX
#
# This script provides a convenient interface for building MINIX with CMake.
# It wraps the standard cmake/ninja commands with MINIX-specific defaults.
#
# Usage:
#   ./releasetools/cmake-build.sh configure [arch]     # Configure CMake
#   ./releasetools/cmake-build.sh build [target]        # Build (all or target)
#   ./releasetools/cmake-build.sh test                   # Run tests
#   ./releasetools/cmake-build.sh clean                  # Clean build dir
#   ./releasetools/cmake-build.sh kernel                 # Build kernel only
#   ./releasetools/cmake-build.sh wolfssl                # Build wolfSSL only
#   ./releasetools/cmake-build.sh list                   # List available targets
#
# Examples:
#   ./releasetools/cmake-build.sh configure i386
#   ./releasetools/cmake-build.sh build kernel
#   ./releasetools/cmake-build.sh test

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_DIR="${PROJECT_DIR}/build"
DEFAULT_ARCH="i386"

# Color output helpers
info()  { printf "  [INFO]  %s\\n" "$*"; }
ok()    { printf "  [ OK ]  %s\\n" "$*"; }
err()   { printf "  [ ERR]  %s\\n" "$*"; exit 1; }

# Check prerequisites
check_prereqs() {
    cmake --version >/dev/null 2>&1 || err "cmake not found. Install cmake >= 3.20."
    ninja --version >/dev/null 2>&1 || info "ninja not found. Will use make instead."
}

# Detect available generator
detect_generator() {
    if ninja --version >/dev/null 2>&1; then
        echo "Ninja"
    else
        echo "Unix Makefiles"
    fi
}

# Configure CMake
cmd_configure() {
    local arch="${1:-${DEFAULT_ARCH}}"
    local generator="$(detect_generator)"

    check_prereqs
    shift  # remove arch from $@, pass remaining args to cmake

    info "Configuring MINIX CMake build for ${arch} using ${generator}..."

    mkdir -p "${BUILD_DIR}"
    cd "${BUILD_DIR}"

    cmake "${PROJECT_DIR}" \
        -G "${generator}" \
        -DCMAKE_TOOLCHAIN_FILE="${PROJECT_DIR}/cmake/toolchain-minix.cmake" \
        -DMACHINE_ARCH="${arch}" \
        -DCMAKE_BUILD_TYPE=Debug \
        "$@"

    ok "Configured for ${arch} in ${BUILD_DIR}"
    info "Run: $0 build"
}

# Build
cmd_build() {
    local target="$1"

    if [ ! -d "${BUILD_DIR}" ]; then
        err "Build directory not found. Run '$0 configure' first."
    fi

    cd "${BUILD_DIR}"

    if [ -n "${target}" ]; then
        info "Building target: ${target}..."
        cmake --build . --target "${target}"
        ok "Built: ${target}"
    else
        info "Building all targets..."
        cmake --build .
        ok "Build complete"
    fi
}

# Test
cmd_test() {
    if [ ! -d "${BUILD_DIR}" ]; then
        err "Build directory not found. Run '$0 configure' first."
    fi

    cd "${BUILD_DIR}"
    info "Running tests..."
    ctest --output-on-failure "$@"
    ok "Tests complete"
}

# Clean
cmd_clean() {
    if [ -d "${BUILD_DIR}" ]; then
        info "Removing build directory: ${BUILD_DIR}"
        rm -rf "${BUILD_DIR}"
        ok "Cleaned"
    else
        info "Nothing to clean"
    fi
}

# List targets
cmd_list() {
    if [ ! -d "${BUILD_DIR}" ]; then
        err "Build directory not found. Run '$0 configure' first."
    fi
    cd "${BUILD_DIR}"
    cmake --build . --target help 2>/dev/null || ninja -t targets 2>/dev/null || make help
}

# Main
case "${1:-help}" in
    configure)
        shift
        cmd_configure "$@"
        ;;
    build)
        shift
        cmd_build "$@"
        ;;
    test)
        shift
        cmd_test "$@"
        ;;
    clean)
        cmd_clean
        ;;
    list)
        cmd_list
        ;;
    kernel)
        cmd_build "kernel"
        ;;
    wolfssl)
        cmd_build "wolfssl"
        ;;
    help|--help|-h)
        echo "MINIX CMake Build Wrapper"
        echo ""
        echo "Usage:"
        echo "  $0 configure [arch]     Configure CMake (default arch: ${DEFAULT_ARCH})"
        echo "  $0 build [target]       Build (all or specific target)"
        echo "  $0 test                 Run tests"
        echo "  $0 clean                Clean build directory"
        echo "  $0 kernel               Build kernel only"
        echo "  $0 wolfssl              Build wolfSSL only"
        echo "  $0 list                 List available targets"
        echo "  $0 help                 Show this help"
        echo ""
        echo "Examples:"
        echo "  $0 configure i386"
        echo "  $0 build"
        echo "  $0 build kernel"
        echo "  $0 test -R wolfssl"
        ;;
    *)
        err "Unknown command: $1. Use '$0 help' for usage."
        ;;
esac
