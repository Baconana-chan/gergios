#!/bin/sh
# Code Coverage Generator for Minix
# This script generates code coverage reports using gcov/lcov

set -e

# Configuration
SOURCE_DIR="${SOURCE_DIR:-$(pwd)}"
COVERAGE_DIR="${COVERAGE_DIR:-$(pwd)/coverage}"
BUILD_DIR="${BUILD_DIR:-$(pwd)/obj}"
REPORT_FORMATS="${REPORT_FORMATS:-html xml}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "Code Coverage Generator"
echo "=========================================="
echo "Source Directory: ${SOURCE_DIR}"
echo "Coverage Directory: ${COVERAGE_DIR}"
echo "Build Directory: ${BUILD_DIR}"
echo "Report Formats: ${REPORT_FORMATS}"
echo "=========================================="

# Create coverage directory
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

# Check if lcov is installed
if ! command -v lcov >/dev/null 2>&1; then
    print_status "error" "lcov is not installed. Please install it first."
    exit 1
fi

# Check if gcov is installed
if ! command -v gcov >/dev/null 2>&1; then
    print_status "error" "gcov is not installed. Please install it first."
    exit 1
fi

# Step 1: Clean previous coverage data
print_status "info" "Cleaning previous coverage data..."
lcov --directory "${BUILD_DIR}" --zerocounters
rm -f "${COVERAGE_DIR}/coverage.info"
rm -f "${COVERAGE_DIR}/coverage.xml"

# Step 2: Capture coverage data
print_status "info" "Capturing coverage data..."
lcov --directory "${BUILD_DIR}" --base-directory "${SOURCE_DIR}" \
    --capture --output-file "${COVERAGE_DIR}/coverage.info" || {
    print_status "warning" "Coverage capture had issues, continuing..."
}

# Step 3: Filter out system and test files
print_status "info" "Filtering coverage data..."
lcov --remove "${COVERAGE_DIR}/coverage.info" \
    '/usr/*' \
    '/opt/*' \
    '*/tests/*' \
    '*/test/*' \
    '*/external/*' \
    '*/gnu/*' \
    --output-file "${COVERAGE_DIR}/coverage.filtered.info" || {
    print_status "warning" "Coverage filtering had issues, using original data..."
    cp "${COVERAGE_DIR}/coverage.info" "${COVERAGE_DIR}/coverage.filtered.info"
}

# Step 4: Generate HTML report
if echo "${REPORT_FORMATS}" | grep -q "html"; then
    print_status "info" "Generating HTML coverage report..."
    genhtml "${COVERAGE_DIR}/coverage.filtered.info" \
        --output-directory "${COVERAGE_DIR}/html" \
        --title "Minix Code Coverage Report" \
        --legend --show-details || {
        print_status "warning" "HTML report generation had issues..."
    }
    print_status "success" "HTML report generated: ${COVERAGE_DIR}/html/index.html"
fi

# Step 5: Generate XML report (for CI integration)
if echo "${REPORT_FORMATS}" | grep -q "xml"; then
    print_status "info" "Generating XML coverage report..."
    lcov --list "${COVERAGE_DIR}/coverage.filtered.info" > "${COVERAGE_DIR}/coverage.txt"
    
    # Convert to XML format (basic conversion)
    {
        echo '<?xml version="1.0" encoding="UTF-8"?>'
        echo '<coverage>'
        echo '  <packages>'
        # Parse and convert coverage data
        grep -E "^SF:" "${COVERAGE_DIR}/coverage.filtered.info" | while read -r line; do
            file=$(echo "$line" | cut -d: -f2-)
            echo "    <package name=\"$file\">"
            echo '      <classes>'
            echo "        <class name=\"$file\" filename=\"$file\">"
            echo '          <methods/>'
            echo '          <lines/>'
            echo '        </class>'
            echo '      </classes>'
            echo '    </package>'
        done
        echo '  </packages>'
        echo '</coverage>'
    } > "${COVERAGE_DIR}/coverage.xml"
    print_status "success" "XML report generated: ${COVERAGE_DIR}/coverage.xml"
fi

# Step 6: Generate coverage summary
print_status "info" "Generating coverage summary..."
{
    echo "=========================================="
    echo "Code Coverage Summary"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Source Directory: ${SOURCE_DIR}"
    echo ""
    echo "Coverage Statistics:"
    echo "--------------------"
    lcov --summary "${COVERAGE_DIR}/coverage.filtered.info" 2>&1 || echo "Could not generate summary"
    echo ""
    echo "Report Locations:"
    echo "-----------------"
    if [ -d "${COVERAGE_DIR}/html" ]; then
        echo "HTML Report: ${COVERAGE_DIR}/html/index.html"
    fi
    if [ -f "${COVERAGE_DIR}/coverage.xml" ]; then
        echo "XML Report: ${COVERAGE_DIR}/coverage.xml"
    fi
    if [ -f "${COVERAGE_DIR}/coverage.txt" ]; then
        echo "Text Report: ${COVERAGE_DIR}/coverage.txt"
    fi
    echo ""
    echo "=========================================="
} > "${COVERAGE_DIR}/coverage-summary.txt"

print_status "success" "Coverage report generation completed"
print_status "info" "Summary: ${COVERAGE_DIR}/coverage-summary.txt"

# Display summary
cat "${COVERAGE_DIR}/coverage-summary.txt"
