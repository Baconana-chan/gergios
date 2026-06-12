#!/bin/sh
# Static Analysis Runner for Minix
# This script runs clang-tidy and cppcheck on the codebase

set -e

# Configuration
SOURCE_DIR="${SOURCE_DIR:-$(pwd)}"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/static-analysis-results}"
PARALLEL_JOBS="${PARALLEL_JOBS:-$(nproc)}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "Static Analysis Runner"
echo "=========================================="
echo "Source Directory: ${SOURCE_DIR}"
echo "Results Directory: ${RESULTS_DIR}"
echo "Parallel Jobs: ${PARALLEL_JOBS}"
echo "=========================================="

# Create results directory
mkdir -p "${RESULTS_DIR}"

# Function to print status
print_status() {
    local status=$1
    local message=$2
    if [ "$status" = "success" ]; then
        echo -e "${GREEN}[SUCCESS]${NC} $message"
    elif [ "$status" = "warning" ]; then
        echo -e "${YELLOW}[WARNING]${NC} $message"
    elif [ "$status" = "error" ]; then
        echo -e "${RED}[ERROR]${NC} $message"
    else
        echo "[INFO] $message"
    fi
}

# Step 1: Run clang-tidy
if command -v clang-tidy >/dev/null 2>&1; then
    print_status "info" "Running clang-tidy..."
    
    # Find C/C++ files
    find "${SOURCE_DIR}" -type f \( -name "*.c" -o -name "*.cpp" -o -name "*.h" -o -name "*.hpp" \) \
        ! -path "*/external/*" \
        ! -path "*/gnu/*" \
        ! -path "*/obj/*" \
        ! -path "*/.git/*" \
        > "${RESULTS_DIR}/files_to_analyze.txt"
    
    # Run clang-tidy on files (limit to first 100 for CI speed)
    head -100 "${RESULTS_DIR}/files_to_analyze.txt" | \
        xargs -P"${PARALLEL_JOBS}" -I{} clang-tidy {} --warnings-as-errors='*' \
        > "${RESULTS_DIR}/clang-tidy-results.txt" 2>&1 || {
        print_status "warning" "Clang-tidy found issues"
    }
    
    print_status "success" "Clang-tidy completed: ${RESULTS_DIR}/clang-tidy-results.txt"
else
    print_status "warning" "clang-tidy not found, skipping"
fi

# Step 2: Run cppcheck
if command -v cppcheck >/dev/null 2>&1; then
    print_status "info" "Running cppcheck..."
    
    cppcheck --enable=all \
        --inconclusive \
        --std=c11 \
        --platform=unix64 \
        --xml \
        --xml-version=2 \
        --suppress=missingIncludeSystem \
        --suppress=unusedFunction \
        --suppress=uninitvar \
        -j "${PARALLEL_JOBS}" \
        "${SOURCE_DIR}" \
        2> "${RESULTS_DIR}/cppcheck-results.xml" || {
        print_status "warning" "Cppcheck found issues"
    }
    
    # Convert XML to text for easier reading
    if [ -f "${RESULTS_DIR}/cppcheck-results.xml" ]; then
        print_status "success" "Cppcheck completed: ${RESULTS_DIR}/cppcheck-results.xml"
    fi
else
    print_status "warning" "cppcheck not found, skipping"
fi

# Step 3: Generate summary
print_status "info" "Generating analysis summary..."
{
    echo "=========================================="
    echo "Static Analysis Summary"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Source Directory: ${SOURCE_DIR}"
    echo ""
    
    if [ -f "${RESULTS_DIR}/clang-tidy-results.txt" ]; then
        echo "Clang-tidy Results:"
        echo "-------------------"
        error_count=$(grep -c "error:" "${RESULTS_DIR}/clang-tidy-results.txt" || echo "0")
        warning_count=$(grep -c "warning:" "${RESULTS_DIR}/clang-tidy-results.txt" || echo "0")
        echo "Errors: ${error_count}"
        echo "Warnings: ${warning_count}"
        echo ""
    fi
    
    if [ -f "${RESULTS_DIR}/cppcheck-results.xml" ]; then
        echo "Cppcheck Results:"
        echo "-----------------"
        error_count=$(grep -c "error" "${RESULTS_DIR}/cppcheck-results.xml" || echo "0")
        warning_count=$(grep -c "warning" "${RESULTS_DIR}/cppcheck-results.xml" || echo "0")
        echo "Errors: ${error_count}"
        echo "Warnings: ${warning_count}"
        echo ""
    fi
    
    echo "Report Locations:"
    echo "-----------------"
    if [ -f "${RESULTS_DIR}/clang-tidy-results.txt" ]; then
        echo "Clang-tidy: ${RESULTS_DIR}/clang-tidy-results.txt"
    fi
    if [ -f "${RESULTS_DIR}/cppcheck-results.xml" ]; then
        echo "Cppcheck: ${RESULTS_DIR}/cppcheck-results.xml"
    fi
    echo ""
    echo "=========================================="
} > "${RESULTS_DIR}/analysis-summary.txt"

print_status "success" "Static analysis completed"
print_status "info" "Summary: ${RESULTS_DIR}/analysis-summary.txt"

# Display summary
cat "${RESULTS_DIR}/analysis-summary.txt"
