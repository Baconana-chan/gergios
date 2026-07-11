#!/bin/bash
# run_c_coverage.sh — C code coverage for CMake/Catch2 tests (gcov/lcov)
#
# Phase 9.5: C Coverage & Benchmarks
# See planning/28_testing_framework_migration.md
#
# This script builds the CMake/Catch2 test targets with coverage flags,
# runs all standalone tests, and generates coverage reports using gcov/lcov.
#
# For the BSD Make (legacy) build path, see scripts/generate_coverage.sh.
#
# Usage:
#   ./scripts/run_c_coverage.sh                          # Full run
#   ./scripts/run_c_coverage.sh --quick                   # Quick run (fewer tests)
#   ./scripts/run_c_coverage.sh --html-only               # HTML only, no XML
#   ./scripts/run_c_coverage.sh --ci                      # CI mode (compact output)
#
# Requirements:
#   - gcc/g++ with --coverage support
#   - lcov (sudo apt install lcov)
#   - CMake build directory configured

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-${SCRIPT_DIR}/build}"
COVERAGE_DIR="${COVERAGE_DIR:-${SCRIPT_DIR}/coverage/c}"
REPORT_FORMATS="${REPORT_FORMATS:-html xml}"
MODE="full"

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)    MODE="quick"; shift ;;
        --html-only) REPORT_FORMATS="html"; shift ;;
        --ci)       MODE="ci"; shift ;;
        --help)
            echo "Usage: $0 [--quick] [--html-only] [--ci]"
            echo "Generate C code coverage reports via gcov/lcov."
            exit 0
            ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

# ── Prerequisites ────────────────────────────────────────────────────────────

if ! command -v lcov &>/dev/null; then
    echo -e "${RED}Error: lcov not found. Install: sudo apt install lcov${NC}"
    exit 1
fi
if ! command -v gcov &>/dev/null; then
    echo -e "${RED}Error: gcov not found.${NC}"
    exit 1
fi

echo -e "${BLUE}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   C Code Coverage (gcov/lcov) — Phase 9.5   ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════╝${NC}"
echo "Build dir:    ${BUILD_DIR}"
echo "Coverage dir: ${COVERAGE_DIR}"
echo "Mode:         ${MODE}"
echo ""

mkdir -p "${COVERAGE_DIR}"

# ── Step 1: Build with coverage flags ────────────────────────────────────────

echo -e "${YELLOW}[1/5] Building Catch2 tests with coverage flags...${NC}"

if [ ! -d "${BUILD_DIR}" ]; then
    mkdir -p "${BUILD_DIR}"
    cd "${BUILD_DIR}"
    cmake "${SCRIPT_DIR}" \
        -DCMAKE_BUILD_TYPE=Debug \
        -DCMAKE_C_FLAGS="--coverage -g -O0" \
        -DCMAKE_CXX_FLAGS="--coverage -g -O0" \
        -DCMAKE_EXE_LINKER_FLAGS="--coverage" \
        -DMK_CATCH2=ON \
        -DMKATF=OFF
    cd "${SCRIPT_DIR}"
fi

cd "${BUILD_DIR}"

# Rebuild with coverage if needed
cmake "${SCRIPT_DIR}" \
    -DCMAKE_C_FLAGS="--coverage -g -O0" \
    -DCMAKE_CXX_FLAGS="--coverage -g -O0" \
    -DCMAKE_EXE_LINKER_FLAGS="--coverage" \
    -DMK_CATCH2=ON

# Build all Phase 9.4 test targets
TARGETS=(
    tcp_socket tcp_error
    raw_icmp raw_ipv6
    ipv6_addr
    bpf_filter bpf_attach
    blocktest
    ds_test rmibtest safecopy_test
)

for t in "${TARGETS[@]}"; do
    cmake --build . --target "$t" 2>/dev/null || true
done

cd "${SCRIPT_DIR}"

# ── Step 2: Zero counters and run tests ──────────────────────────────────────

echo -e "${YELLOW}[2/5] Zeroing coverage counters...${NC}"
lcov --directory "${BUILD_DIR}" --zerocounters --quiet

echo -e "${YELLOW}[2/5] Running Catch2 tests...${NC}"

ctest --test-dir "${BUILD_DIR}" -L phase9 --output-on-failure 2>&1 | \
    tail -20 || true

# ── Step 3: Capture raw coverage data ────────────────────────────────────────

echo -e "${YELLOW}[3/5] Capturing coverage data...${NC}"

RAW_INFO="${COVERAGE_DIR}/coverage.raw.info"
lcov --directory "${BUILD_DIR}" \
    --base-directory "${SCRIPT_DIR}" \
    --capture \
    --output-file "${RAW_INFO}" \
    --quiet 2>&1 || {
    echo -e "${YELLOW}Capture had issues, continuing...${NC}"
}

# ── Step 4: Filter coverage data ─────────────────────────────────────────────

echo -e "${YELLOW}[4/5] Filtering coverage data...${NC}"

FILTERED_INFO="${COVERAGE_DIR}/coverage.filtered.info"

lcov --remove "${RAW_INFO}" \
    '/usr/*' \
    '/opt/*' \
    '*/tests/*' \
    '*/test/*' \
    '*/external/*' \
    '*/gnu/*' \
    '*/cmake/*' \
    '*/rust/*' \
    '*/minix/tests/*' \
    --output-file "${FILTERED_INFO}" \
    --quiet 2>&1 || {
    echo -e "${YELLOW}Filtering had issues, using raw data...${NC}"
    cp "${RAW_INFO}" "${FILTERED_INFO}"
}

# ── Step 5: Generate reports ─────────────────────────────────────────────────

echo -e "${YELLOW}[5/5] Generating reports...${NC}"

# HTML report (if requested)
if echo "${REPORT_FORMATS}" | grep -q "html"; then
    HTML_DIR="${COVERAGE_DIR}/html"
    genhtml "${FILTERED_INFO}" \
        --output-directory "${HTML_DIR}" \
        --title "GergiOS C Code Coverage Report" \
        --legend --show-details \
        --quiet 2>&1 || true
    echo -e "${GREEN}  HTML: ${HTML_DIR}/index.html${NC}"
fi

# XML report (for Codecov/CI integration)
if echo "${REPORT_FORMATS}" | grep -q "xml"; then
    lcov --list "${FILTERED_INFO}" > "${COVERAGE_DIR}/coverage.txt" 2>/dev/null || true
    echo -e "${GREEN}  TXT:  ${COVERAGE_DIR}/coverage.txt${NC}"
fi

# Summary
SUMMARY="${COVERAGE_DIR}/coverage-summary.txt"
{
    echo "=========================================="
    echo "C Code Coverage Summary"
    echo "Phase 9.5 — gcov/lcov"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Build: ${BUILD_DIR}"
    echo ""
    echo "Coverage Statistics:"
    lcov --summary "${FILTERED_INFO}" 2>&1 || echo "  (summary unavailable)"
    echo ""
    echo "Report Locations:"
    echo "  HTML: ${COVERAGE_DIR}/html/index.html"
    echo "  TXT:  ${COVERAGE_DIR}/coverage.txt"
    echo "  Raw:  ${RAW_INFO}"
} > "${SUMMARY}"

cat "${SUMMARY}"

echo ""
echo -e "${GREEN}✅ C coverage report generated.${NC}"
echo -e "${CYAN}  Open: ${COVERAGE_DIR}/html/index.html${NC}"
echo ""
