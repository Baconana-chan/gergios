#!/bin/sh
# Security Scanning Runner for Minix
# This script runs various security scanning tools

set -e

# Configuration
SOURCE_DIR="${SOURCE_DIR:-$(pwd)}"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/security-results}"
PARALLEL_JOBS="${PARALLEL_JOBS:-$(nproc)}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "Security Scanning Runner"
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

# Step 1: Run scan-build (Clang static analyzer)
if command -v scan-build >/dev/null 2>&1; then
    print_status "info" "Running scan-build (Clang static analyzer)..."
    
    scan-build --use-cc=clang --use-c++=clang++ \
        -o "${RESULTS_DIR}/scan-build" \
        -stats \
        make -j"${PARALLEL_JOBS}" do-build > "${RESULTS_DIR}/scan-build.log" 2>&1 || {
        print_status "warning" "Scan-build found issues or build failed"
    }
    
    print_status "success" "Scan-build completed: ${RESULTS_DIR}/scan-build"
else
    print_status "warning" "scan-build not found, skipping"
fi

# Step 2: Run safety check for Python dependencies
if command -v python3 >/dev/null 2>&1; then
    print_status "info" "Running safety check for Python dependencies..."
    
    pip3 install safety --quiet 2>/dev/null || true
    
    if command -v safety >/dev/null 2>&1; then
        safety check --json > "${RESULTS_DIR}/safety-report.json" 2>&1 || {
            print_status "warning" "Safety check found vulnerabilities"
        }
        print_status "success" "Safety check completed: ${RESULTS_DIR}/safety-report.json"
    else
        print_status "warning" "safety tool not available"
    fi
fi

# Step 3: Check for common security issues in C code
print_status "info" "Checking for common security issues..."

# Check for dangerous functions
{
    echo "Dangerous Function Usage:"
    echo "-------------------------"
    grep -r "strcpy\|strcat\|sprintf\|gets\|scanf" --include="*.c" --include="*.h" \
        "${SOURCE_DIR}" 2>/dev/null | head -20 || echo "None found"
    echo ""
    
    echo "Hardcoded Credentials:"
    echo "---------------------"
    grep -ri "password\|secret\|api_key\|token" --include="*.c" --include="*.h" \
        "${SOURCE_DIR}" 2>/dev/null | grep -v "//" | head -10 || echo "None found"
    echo ""
    
    echo "Debug Statements:"
    echo "----------------"
    grep -r "printf\|printk" --include="*.c" "${SOURCE_DIR}" 2>/dev/null | \
        grep -i "debug\|test" | head -10 || echo "None found"
    echo ""
    
    echo "TODO/FIXME Comments:"
    echo "--------------------"
    grep -ri "TODO\|FIXME\|XXX\|HACK" --include="*.c" --include="*.h" \
        "${SOURCE_DIR}" 2>/dev/null | head -20 || echo "None found"
} > "${RESULTS_DIR}/security-audit.txt"

print_status "success" "Security audit completed: ${RESULTS_DIR}/security-audit.txt"

# Step 4: Generate security summary
print_status "info" "Generating security summary..."
{
    echo "=========================================="
    echo "Security Scanning Summary"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Source Directory: ${SOURCE_DIR}"
    echo ""
    
    echo "Scan Results:"
    echo "-------------"
    if [ -d "${RESULTS_DIR}/scan-build" ]; then
        echo "Scan-build: ${RESULTS_DIR}/scan-build"
    fi
    if [ -f "${RESULTS_DIR}/safety-report.json" ]; then
        echo "Safety Report: ${RESULTS_DIR}/safety-report.json"
    fi
    if [ -f "${RESULTS_DIR}/security-audit.txt" ]; then
        echo "Security Audit: ${RESULTS_DIR}/security-audit.txt"
    fi
    echo ""
    echo "=========================================="
} > "${RESULTS_DIR}/security-summary.txt"

print_status "success" "Security scanning completed"
print_status "info" "Summary: ${RESULTS_DIR}/security-summary.txt"

# Display summary
cat "${RESULTS_DIR}/security-summary.txt"
