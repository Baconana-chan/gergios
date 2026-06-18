#!/bin/bash
# run_rust_tests.sh — Rust workspace test runner for CI/CD
#
# Runs all Rust tests with optional sanitizer support.
# Usage:
#   ./scripts/run_rust_tests.sh              # Standard test run
#   ./scripts/run_rust_tests.sh --asan        # With AddressSanitizer
#   ./scripts/run_rust_tests.sh --ubsan       # With UndefinedBehaviorSanitizer
#   ./scripts/run_rust_tests.sh --coverage    # With code coverage
#   ./scripts/run_rust_tests.sh --fuzz        # Run fuzz targets (needs nightly)

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

CARGO="${CARGO:-cargo}"
RUST_DIR="${RUST_DIR:-$(cd "$(dirname "$0")/../rust" && pwd)}"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/rust-test-results}"

mkdir -p "$RESULTS_DIR"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Rust Workspace Test Runner${NC}"
echo -e "${BLUE}========================================${NC}"
echo "Rust directory: $RUST_DIR"
echo "Results: $RESULTS_DIR"
echo ""

# Parse arguments
SANITIZER=""
MODE="test"

while [[ $# -gt 0 ]]; do
    case $1 in
        --asan)    SANITIZER="address"; shift ;;
        --ubsan)   SANITIZER="undefined"; shift ;;
        --tsan)    SANITIZER="thread"; shift ;;
        --coverage) MODE="coverage"; shift ;;
        --fuzz)    MODE="fuzz"; shift ;;
        --build-only) MODE="build"; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# Build all crates
echo -e "${YELLOW}[1/4] Building all crates...${NC}"
if [ -n "$SANITIZER" ]; then
    RUSTFLAGS="-Z sanitizer=$SANITIZER" \
        $CARGO build --manifest-path "$RUST_DIR/Cargo.toml" --workspace 2>&1 | \
        tee "$RESULTS_DIR/build.log"
else
    $CARGO build --manifest-path "$RUST_DIR/Cargo.toml" --workspace 2>&1 | \
        tee "$RESULTS_DIR/build.log"
fi

if [ "${PIPESTATUS[0]}" -ne 0 ]; then
    echo -e "${RED}Build failed — see $RESULTS_DIR/build.log${NC}"
    exit 1
fi
echo -e "${GREEN}Build OK${NC}"

# Run all tests
echo -e "${YELLOW}[2/4] Running all tests...${NC}"
if [ -n "$SANITIZER" ]; then
    RUSTFLAGS="-Z sanitizer=$SANITIZER" \
        $CARGO test --manifest-path "$RUST_DIR/Cargo.toml" --workspace 2>&1 | \
        tee "$RESULTS_DIR/test.log"
else
    $CARGO test --manifest-path "$RUST_DIR/Cargo.toml" --workspace 2>&1 | \
        tee "$RESULTS_DIR/test.log"
fi

TEST_EXIT=${PIPESTATUS[0]}
if [ "$TEST_EXIT" -eq 0 ]; then
    echo -e "${GREEN}All tests passed${NC}"
else
    echo -e "${RED}Some tests failed (exit code: $TEST_EXIT)${NC}"
fi

# Generate test summary
echo -e "${YELLOW}[3/4] Generating test summary...${NC}"
{
    echo "=========================================="
    echo "Rust Test Summary"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Sanitizer: ${SANITIZER:-none}"
    echo "Mode: $MODE"
    echo ""
    echo "Test Results:"
    echo "------------"
    grep -E "(test result|running [0-9]+ test|error\[|warning\[)" "$RESULTS_DIR/test.log" 2>/dev/null || echo "(no test output)"
    echo ""
    echo "Crates:"
    $CARGO metadata --manifest-path "$RUST_DIR/Cargo.toml" --format-version 1 2>/dev/null | \
        python3 -c "import sys,json; ws=json.load(sys.stdin); [print(f'  {pkg[\"name\"]} ({pkg[\"version\"]})') for pkg in ws['packages']]" 2>/dev/null || \
        echo "  (metadata unavailable)"
} > "$RESULTS_DIR/summary.txt"

# Run fuzz targets
if [ "$MODE" = "fuzz" ]; then
    echo -e "${YELLOW}[4/4] Running fuzz targets...${NC}"
    if ! $CARGO +nightly fuzz --help &>/dev/null; then
        echo -e "${YELLOW}cargo-fuzz not installed or nightly toolchain missing.${NC}"
        echo "Install with:"
        echo "  rustup toolchain install nightly"
        echo "  cargo install cargo-fuzz"
        echo ""
        echo "To run a specific fuzz target on nightly:"
        echo "  cd rust && cargo +nightly fuzz run fuzz_minixrs_message -- -max_total_time=600"
        exit 0
    fi

    # Define all fuzz targets
    FUZZ_TARGETS=(
        "fuzz_minixrs_message:600"   # 10 min
        "fuzz_netparse_tcp:300"       # 5 min
        "fuzz_netparse_udp:300"       # 5 min
        "fuzz_netparse_dns:300"       # 5 min
        "fuzz_audiobuf_ringpos:300"   # 5 min
        "fuzz_procfspath_pid:300"     # 5 min
    )

    for entry in "${FUZZ_TARGETS[@]}"; do
        target="${entry%%:*}"
        duration="${entry##*:}"
        echo ""
        echo -e "${BLUE}Fuzzing: ${target} (${duration}s)${NC}"
        cd "$RUST_DIR"
        $CARGO +nightly fuzz run "$target" -- \
            -max_total_time="$duration" 2>&1 | \
            tee -a "$RESULTS_DIR/fuzz.log" || echo -e "${YELLOW}Fuzz target ${target} exited non-zero${NC}"
        cd - >/dev/null
    done

    # Check for crashes
    if ls "$RUST_DIR/fuzz/artifacts/"* 2>/dev/null | head -5; then
        echo -e "${RED}Crash artifacts found!${NC}" | tee -a "$RESULTS_DIR/summary.txt"
    else
        echo "No crash artifacts found." | tee -a "$RESULTS_DIR/summary.txt"
    fi
    exit 0
fi

# Optionally generate coverage
if [ "$MODE" = "coverage" ]; then
    echo -e "${YELLOW}[4/4] Generating code coverage...${NC}"
    if command -v cargo-llvm-cov &>/dev/null; then
        $CARGO llvm-cov --manifest-path "$RUST_DIR/Cargo.toml" --workspace \
            --lcov --output-path "$RESULTS_DIR/lcov.info" 2>&1 | \
            tee "$RESULTS_DIR/coverage.log"
        echo -e "${GREEN}Coverage report: $RESULTS_DIR/lcov.info${NC}"
    else
        echo -e "${YELLOW}cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov${NC}"
    fi
fi

echo -e "${BLUE}========================================${NC}"
echo -e "Results saved to: ${RESULTS_DIR}/"
echo -e "Summary: ${RESULTS_DIR}/summary.txt"
echo -e "${BLUE}========================================${NC}"

exit $TEST_EXIT
