# cmake/ci-config.cmake
# CI/CD pipeline configuration for MINIX CMake build
#
# This file documents the CI/CD integration points. Actual CI/CD
# configuration (GitHub Actions, GitLab CI, Jenkins) should use
# this as a reference.

# ============================================================================
# Build Matrix
# ============================================================================

# Since Phase 3 of i386 deprecation, i386 is REMOVED from the default
# CI/CD pipeline. It was removed from the main build and requires
# explicit opt-in (-DMKI386=ON).
#
# i386 builds can still be performed on-demand or via scheduled jobs,
# but are no longer part of the standard CI/CD workflow.
#
# Architecture (i386 is Phase 3 hard deprecated — not in default CI)
set(CI_ARCHITECTURES
    x86_64      # Primary: default build target
    earm        # Secondary: ARM 32-bit support
    aarch64     # Future: ARM64 support (Phase 1: build infra)
)

# Build Types
set(CI_BUILD_TYPES
    Debug
    Release
    MinSizeRel
)

# Compiler configurations
set(CI_COMPILER_CONFIGS
    clang    # Primary: MINIX uses LLVM/Clang
    gcc      # Secondary: GCC compatibility
)

# ============================================================================
# On-Demand i386 Build (optional, for maintainers only)
# ============================================================================

# i386 builds are NOT part of the standard CI/CD pipeline. Maintainers
# can trigger them manually via workflow_dispatch with:
#
#   cmake -DMACHINE_ARCH=i386 -DMKI386=ON ..
#
# The on-demand workflow should NOT be used for:
#   - PR validation (use x86_64)
#   - Release builds (use x86_64)
#   - Development testing (use x86_64)
#
# The on-demand workflow MAY be used for:
#   - Verifying critical security patches for i386
#   - Pre-removal archival builds
#   - Community-contributed i386 fixes

# ============================================================================
# Standard CI Pipeline Stages
# ============================================================================

# Stage 1: Configure
#   cmake -G Ninja -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
#         -DMACHINE_ARCH=x86_64 -DCMAKE_BUILD_TYPE=Debug -S . -B build

# Stage 2: Build
#   cmake --build build --target kernel    # Kernel only
#   cmake --build build --target wolfssl   # wolfSSL only
#   cmake --build build                    # Full build

# Stage 3: Test
#   ctest --test-dir build --output-on-failure

# Stage 4: Package
#   cmake --install build --prefix destdir
#   cpack -G TGZ -B packages

# ============================================================================
# CI Environment Variables
# ============================================================================

set(CI_REQUIRED_ENV
    MINIX_DESTDIR=/opt/minix/destdir
    MINIX_TOOLCHAIN=/opt/minix/toolchain
)

# ============================================================================
# Validation Targets
# ============================================================================

# NOTE: To enable CI targets, include this file from the root CMakeLists.txt:
#   include(ci-config)
#
# Custom target to validate CMake configuration
# add_custom_target(ci-validate-config
#     COMMAND ${CMAKE_COMMAND} --system-information
#     COMMENT "Validating CI configuration"
# )
#
# Custom target to build all components
# add_custom_target(ci-build-all
#     COMMAND ${CMAKE_COMMAND} --build .
#     COMMENT "Building all components"
# )
#
# Custom target for quick smoke test
# add_custom_target(ci-smoke-test
#     COMMAND ${CMAKE_COMMAND} --build . --target kernel
#     COMMAND ${CMAKE_COMMAND} --build . --target wolfssl
#     COMMENT "Smoke test: kernel + wolfSSL"
# )

# ============================================================================
# GitHub Actions Workflow (reference)
# ============================================================================
#
# .github/workflows/minix-build.yml:
#
# ```yaml
# name: MINIX CMake Build
# on: [push, pull_request]
# jobs:
#   build:
#     strategy:
#       matrix:
#         arch: [x86_64, earm]    # i386 removed (Phase 3 hard deprecation)
#         build_type: [Debug, Release]
#     runs-on: ubuntu-latest
#     steps:
#       - uses: actions/checkout@v4
#       - name: Install dependencies
#         run: |
#           sudo apt-get update
#           sudo apt-get install -y ninja-build cmake
#       - name: Configure
#         run: |
#           cmake -G Ninja \
#             -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
#             -DMACHINE_ARCH=${{ matrix.arch }} \
#             -DCMAKE_BUILD_TYPE=${{ matrix.build_type }} \
#             -S . -B build
#       - name: Build
#         run: cmake --build build
#       - name: Test
#         run: ctest --test-dir build --output-on-failure
#
#   # On-demand i386 build (manual trigger for maintainers):
#   i386-legacy:
#     if: github.event_name == 'workflow_dispatch'
#     strategy:
#       matrix:
#         build_type: [Debug]
#     runs-on: ubuntu-latest
#     steps:
#       - uses: actions/checkout@v4
#       - name: Configure (i386 legacy)
#         run: |
#           cmake -G Ninja \
#             -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
#             -DMACHINE_ARCH=i386 -DMKI386=ON \
#             -DCMAKE_BUILD_TYPE=${{ matrix.build_type }} \
#             -S . -B build
#       - name: Build
#         run: cmake --build build
# ```
