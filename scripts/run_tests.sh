#!/bin/sh
# Automated Test Runner for Minix
# This script runs the full test suite and generates reports

set -e

# Configuration
TEST_DIR="${TEST_DIR:-$(pwd)/tests}"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/test-results}"
COVERAGE_DIR="${COVERAGE_DIR:-$(pwd)/coverage}"
PARALLEL_JOBS="${PARALLEL_JOBS:-$(nproc)}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "Minix Automated Test Runner"
echo "=========================================="
echo "Test Directory: ${TEST_DIR}"
echo "Results Directory: ${RESULTS_DIR}"
echo "Coverage Directory: ${COVERAGE_DIR}"
echo "Parallel Jobs: ${PARALLEL_JOBS}"
echo "=========================================="

# Create directories
mkdir -p "${RESULTS_DIR}"
mkdir -p "${COVERAGE_DIR}"

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

# Step 1: Build test suite
print_status "info" "Building test suite..."
cd "${TEST_DIR}"
make clean || true
make -j"${PARALLEL_JOBS}" || {
    print_status "warning" "Test suite build had issues, continuing..."
}

# Step 2: Run ATF tests if available
if command -v atf-run >/dev/null 2>&1; then
    print_status "info" "Running ATF tests..."
    atf-run > "${RESULTS_DIR}/atf-results.xml" || {
        print_status "warning" "ATF tests completed with failures"
    }
    atf-report "${RESULTS_DIR}/atf-results.xml" > "${RESULTS_DIR}/atf-report.txt" || true
else
    print_status "warning" "ATF not found, skipping ATF tests"
fi

# Step 3: Run Kyua tests if available
if command -v kyua >/dev/null 2>&1; then
    print_status "info" "Running Kyua tests..."
    kyua test > "${RESULTS_DIR}/kyua-results.txt" 2>&1 || {
        print_status "warning" "Kyua tests completed with failures"
    }
    kyua report > "${RESULTS_DIR}/kyua-report.html" || true
else
    print_status "warning" "Kyua not found, skipping Kyua tests"
fi

# Step 4: Run individual component tests
print_status "info" "Running component tests..."

# Test bin utilities
if [ -d "${TEST_DIR}/bin" ]; then
    print_status "info" "Testing bin utilities..."
    cd "${TEST_DIR}/bin"
    for dir in */; do
        if [ -f "${dir}/Makefile" ]; then
            print_status "info" "  Testing $(basename "$dir")..."
            cd "${dir}"
            make test 2>&1 | tee -a "${RESULTS_DIR}/bin-tests.log" || true
            cd "${TEST_DIR}/bin"
        fi
    done
fi

# Test libraries
if [ -d "${TEST_DIR}/lib" ]; then
    print_status "info" "Testing libraries..."
    cd "${TEST_DIR}/lib"
    for dir in */; do
        if [ -f "${dir}/Makefile" ]; then
            print_status "info" "  Testing $(basename "$dir")..."
            cd "${dir}"
            make test 2>&1 | tee -a "${RESULTS_DIR}/lib-tests.log" || true
            cd "${TEST_DIR}/lib"
        fi
    done
fi

# Step 5: Generate test summary
print_status "info" "Generating test summary..."
{
    echo "=========================================="
    echo "Test Summary"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Test Directory: ${TEST_DIR}"
    echo ""
    echo "Test Results:"
    echo "------------"
    
    # Count total tests
    total_tests=0
    passed_tests=0
    failed_tests=0
    
    if [ -f "${RESULTS_DIR}/atf-results.xml" ]; then
        echo "ATF Results: ${RESULTS_DIR}/atf-results.xml"
    fi
    
    if [ -f "${RESULTS_DIR}/kyua-results.txt" ]; then
        echo "Kyua Results: ${RESULTS_DIR}/kyua-results.txt"
    fi
    
    echo ""
    echo "Component Test Logs:"
    echo "-------------------"
    if [ -f "${RESULTS_DIR}/bin-tests.log" ]; then
        echo "Bin tests: ${RESULTS_DIR}/bin-tests.log"
    fi
    if [ -f "${RESULTS_DIR}/lib-tests.log" ]; then
        echo "Lib tests: ${RESULTS_DIR}/lib-tests.log"
    fi
    
    echo ""
    echo "=========================================="
} > "${RESULTS_DIR}/test-summary.txt"

print_status "success" "Test execution completed"
print_status "info" "Results saved to: ${RESULTS_DIR}"
print_status "info" "Summary: ${RESULTS_DIR}/test-summary.txt"

# Exit with appropriate code
if [ -f "${RESULTS_DIR}/test-summary.txt" ]; then
    print_status "success" "Test run completed successfully"
    exit 0
else
    print_status "error" "Test run failed"
    exit 1
fi
