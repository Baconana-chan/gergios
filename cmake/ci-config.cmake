# cmake/ci-config.cmake
# CI/CD pipeline configuration for MINIX CMake build
#
# This file documents the CI/CD integration points. Actual CI/CD
# configuration (GitHub Actions, GitLab CI, Jenkins) should use
# this as a reference.

# ============================================================================
# Build Matrix
# ============================================================================

# Architecture
set(CI_ARCHITECTURES
    i386
    earm
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
# Standard CI Pipeline Stages
# ============================================================================

# Stage 1: Configure
#   cmake -G Ninja -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
#         -DMACHINE_ARCH=i386 -DCMAKE_BUILD_TYPE=Debug -S . -B build

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
#         arch: [i386, earm]
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
# ```
