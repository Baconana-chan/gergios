#!/bin/bash
# run_benchmarks.sh — Rust vs C utility performance benchmarks
#
# Uses hyperfine (https://github.com/sharkdp/hyperfine) to benchmark
# Rust port versions of core MINIX utilities.
#
# C comparison: The native C utilities are MINIX-specific and require
# a MINIX build environment or QEMU to compile. When C binaries are
# detected (from a pre-built MINIX sysroot), they are included in the
# comparison. Otherwise, only Rust absolute times are reported.
#
# Usage:
#   ./scripts/run_benchmarks.sh                        # Full run
#   ./scripts/run_benchmarks.sh --quick                # Fewer iterations
#   ./scripts/run_benchmarks.sh --utility grep,seq     # Filter by name
#   ./scripts/run_benchmarks.sh --json-only            # JSON export only
#   ./scripts/run_benchmarks.sh --ci                   # CI mode (compact, JSON)
#
# Requirements:
#   - hyperfine (cargo install hyperfine)
#   - Rust utilities built in release mode (cargo build --release)

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RUST_TARGET="${SCRIPT_DIR}/rust/target/release"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/benchmark-results}"
DATA_DIR="${SCRIPT_DIR}/scripts/benchmark-data"
MODE="full"
FILTER=""
JSON_ONLY=false
CI_MODE=false

mkdir -p "$RESULTS_DIR"
mkdir -p "$DATA_DIR"

# ------------------------------------------------------------------
# Parse arguments
# ------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)    MODE="quick"; shift ;;
        --utility)  FILTER="$2"; shift 2 ;;
        --json-only) JSON_ONLY=true; shift ;;
        --ci)       CI_MODE=true; JSON_ONLY=true; shift ;;
        --help)
            echo "Usage: $0 [--quick] [--utility grep,seq] [--json-only] [--ci]"
            echo ""
            echo "Benchmarks Rust utilities. C comparison when MINIX binaries detected."
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ------------------------------------------------------------------
# Check requirements
# ------------------------------------------------------------------
if ! command -v hyperfine &>/dev/null; then
    echo -e "${RED}Error: hyperfine not found. Install with: cargo install hyperfine${NC}"
    exit 1
fi

HYPERFINE_VER=$(hyperfine --version 2>&1 | head -1)
echo -e "${BLUE}Using ${HYPERFINE_VER}${NC}"

# ------------------------------------------------------------------
# Detect Rust binaries (handle .exe extension on Windows)
# ------------------------------------------------------------------
rust_bin() {
    local name="$1"
    if [ -f "${RUST_TARGET}/${name}.exe" ]; then
        echo "${RUST_TARGET}/${name}.exe"
    elif [ -f "${RUST_TARGET}/${name}" ]; then
        echo "${RUST_TARGET}/${name}"
    else
        echo ""
    fi
}

# ------------------------------------------------------------------
# Detect C binaries (MINIX native — optional)
# ------------------------------------------------------------------
c_bin() {
    local name="$1"
    # Check common locations for pre-built MINIX binaries
    for dir in \
        "${SCRIPT_DIR}/usr.bin/${name}/${name}" \
        "${SCRIPT_DIR}/bin/${name}/${name}" \
        "${SCRIPT_DIR}/usr.bin/${name}/${name}.minix" \
        "${SCRIPT_DIR}/destdir/usr/bin/${name}" \
        "${SCRIPT_DIR}/destdir/bin/${name}"; do
        if [ -f "$dir" ] && [ -x "$dir" ]; then
            echo "$dir"
            return
        fi
    done
    # On Windows, also check for .exe
    for dir in \
        "${SCRIPT_DIR}/usr.bin/${name}/${name}.exe" \
        "${SCRIPT_DIR}/bin/${name}/${name}.exe"; do
        if [ -f "$dir" ]; then
            echo "$dir"
            return
        fi
    done
    echo ""
}

# Build a list of available Rust binaries
RUST_AVAILABLE=()
for b in basename dirname echo false grep seq sleep true yes; do
    if [ -n "$(rust_bin "$b")" ]; then
        RUST_AVAILABLE+=("$b")
    fi
done

if [ ${#RUST_AVAILABLE[@]} -eq 0 ]; then
    echo -e "${RED}No Rust release binaries found!${NC}"
    echo -e "Run: cd rust && cargo build --release"
    echo -e "Looked in: ${RUST_TARGET}"
    exit 1
fi

echo -e "${GREEN}Rust binaries available: ${RUST_AVAILABLE[*]}${NC}"

# Check for C binaries (optional)
C_AVAILABLE=()
for b in "${RUST_AVAILABLE[@]}"; do
    cb=$(c_bin "$b")
    if [ -n "$cb" ]; then
        C_AVAILABLE+=("$b")
    fi
done

if [ ${#C_AVAILABLE[@]} -gt 0 ]; then
    echo -e "${GREEN}C binaries available for comparison: ${C_AVAILABLE[*]}${NC}"
    COMPARE_MODE="rust-vs-c"
else
    echo -e "${YELLOW}No C binaries found — Rust-only benchmarks (absolute times)${NC}"
    echo -e "${YELLOW}For C comparison, build MINIX first or run inside QEMU.${NC}"
    COMPARE_MODE="rust-only"
fi

# ------------------------------------------------------------------
# Create test data (if not exists)
# ------------------------------------------------------------------
echo -e "${YELLOW}[setup] Creating benchmark test data...${NC}"

GREP_FILE="${DATA_DIR}/grep-benchmark.txt"
if [ ! -f "$GREP_FILE" ] || [ "$(wc -l < "$GREP_FILE")" -lt 50000 ]; then
    echo "Generating grep test data (100K lines)..."
    {
        for i in $(seq 1 1000); do
            echo "the quick brown fox jumps over the lazy dog $i"
            echo "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG $i"
            echo "Lorem ipsum dolor sit amet consectetur adipiscing elit $i"
            echo "Pack my box with five dozen liquor jugs $i"
            echo "How vexingly quick daft zebras jump $i"
            echo "Sphinx of black quartz judge my vow $i"
            echo "The five boxing wizards jump quickly $i"
            echo "Fix problem solving git conflict rebase merge $i"
            echo "Rust cargo clippy fmt build test benchmark $i"
            echo "MINIX kernel driver server filesystem network $i"
        done
    } > "$GREP_FILE"
fi

# Note: additional test data can be placed in ${DATA_DIR}/

echo -e "${GREEN}Test data ready in ${DATA_DIR}/${NC}"
echo ""

# ------------------------------------------------------------------
# Hyperfine options
# ------------------------------------------------------------------
HYPERFINE_JSON="--export-json"
HYPERFINE_IGNORE="--ignore-failure"

if [ "$MODE" = "quick" ]; then
    HYPERFINE_OPTS="--warmup 1 --min-runs 3"
elif [ "$CI_MODE" = true ]; then
    HYPERFINE_OPTS="--warmup 1 --min-runs 5"
else
    HYPERFINE_OPTS="--warmup 3 --min-runs 10"
fi

# ------------------------------------------------------------------
# Define benchmarks
# Format: name:RUST_CMD | name:RUST_CMD:C_CMD
# ------------------------------------------------------------------
BENCHMARKS=()

add_benchmark() {
    local name="$1"
    local bin="$2"
    local args="$3"

    local rb=$(rust_bin "$bin")
    [ -z "$rb" ] && return

    local rust_cmd="${rb} ${args}"

    # Check if C version available for this utility
    local cb=$(c_bin "$bin")
    if [ -n "$cb" ]; then
        BENCHMARKS+=("${name}:${rust_cmd}:${cb} ${args}")
    else
        BENCHMARKS+=("${name}:${rust_cmd}")
    fi
}

# basename
add_benchmark "basename-path"     "basename" "/usr/share/dict/words"
add_benchmark "basename-suffix"   "basename" "/var/log/syslog.1.gz .gz"

# dirname
add_benchmark "dirname-path"      "dirname"  "/usr/share/dict/words"
add_benchmark "dirname-deep"      "dirname"  "/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p"

# echo
add_benchmark "echo-short"        "echo"     "Hello World from MINIX"
add_benchmark "echo-nflag"        "echo"     "-n No trailing newline"

# grep — various modes
if [ -n "$(rust_bin grep)" ]; then
    add_benchmark "grep-fixed"      "grep" "-F fox ${GREP_FILE}"
    add_benchmark "grep-regex"      "grep" "'jump.*quick' ${GREP_FILE}"
    add_benchmark "grep-icase"      "grep" "-i 'MINIX' ${GREP_FILE}"
    add_benchmark "grep-count"      "grep" "-c 'lorem' ${GREP_FILE}"
    add_benchmark "grep-invert"     "grep" "-v 'dog' ${GREP_FILE}"
fi

# seq
if [ -n "$(rust_bin seq)" ]; then
    add_benchmark "seq-small"       "seq"   "1 1000"
    add_benchmark "seq-large"       "seq"   "1 100000"
    add_benchmark "seq-float"       "seq"   "0.5 0.25 10.0"
fi

# sleep (short durations)
if [ -n "$(rust_bin sleep)" ]; then
    add_benchmark "sleep-10ms"      "sleep" "0.01"
    add_benchmark "sleep-100ms"     "sleep" "0.1"
fi

# yes (pipe to head to limit output)
if [ -n "$(rust_bin yes)" ]; then
    add_benchmark "yes-default"     "yes"   "| head -c 1M"
    add_benchmark "yes-custom"      "yes"   "'MINIX Rust port' | head -c 1M"
fi

# true/false (process creation overhead)
add_benchmark "true-exit"          "true"  ""
add_benchmark "false-exit"         "false" ""

# Apply filter
if [ -n "$FILTER" ]; then
    IFS=',' read -ra FILTER_UTILS <<< "$FILTER"
    FILTERED=()
    for entry in "${BENCHMARKS[@]}"; do
        name="${entry%%:*}"
        for util in "${FILTER_UTILS[@]}"; do
            # Match utility name prefix (e.g., "grep" matches "grep-fixed")
            if [[ "$name" == "$util"* ]]; then
                FILTERED+=("$entry")
                break
            fi
        done
    done
    BENCHMARKS=("${FILTERED[@]}")
fi

if [ ${#BENCHMARKS[@]} -eq 0 ]; then
    echo -e "${RED}No benchmarks matched the filter.${NC}"
    exit 1
fi

# ------------------------------------------------------------------
# Run benchmarks
# ------------------------------------------------------------------
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Rust vs C Performance Benchmarks${NC}"
echo -e "${BLUE}========================================${NC}"
echo "Mode: $MODE"
echo "Comparison: $COMPARE_MODE"
echo "Benchmark variants: ${#BENCHMARKS[@]}"
echo ""

TOTAL=${#BENCHMARKS[@]}
CURRENT=0

# Combined JSON
ALL_JSON="${RESULTS_DIR}/all-benchmarks.json"
echo "[" > "$ALL_JSON"
FIRST_ENTRY=true

for entry in "${BENCHMARKS[@]}"; do
    CURRENT=$((CURRENT + 1))

    # Parse entry format: "name:RUST_CMD" or "name:RUST_CMD:C_CMD"
    name="${entry%%:*}"
    rest="${entry#*:}"

    # Split on last ':'
    if [[ "$rest" == *:* ]]; then
        rust_cmd="${rest%%:*}"
        c_cmd="${rest#*:}"
    else
        rust_cmd="$rest"
        c_cmd=""
    fi

    echo -e "${YELLOW}[${CURRENT}/${TOTAL}] ${name}${NC}"

    JSON_OUT="${RESULTS_DIR}/${name}.json"

    # Build hyperfine command
    if [ -n "$c_cmd" ]; then
        # Rust vs C comparison
        if [ "$JSON_ONLY" = true ] || [ "$CI_MODE" = true ]; then
            hyperfine $HYPERFINE_OPTS \
                --export-json "$JSON_OUT" \
                --ignore-failure \
                "$rust_cmd" \
                "$c_cmd" \
                2>/dev/null || true
        else
            hyperfine $HYPERFINE_OPTS \
                --export-json "$JSON_OUT" \
                --ignore-failure \
                --style basic \
                "$rust_cmd" \
                "$c_cmd" \
                2>&1 || true
            echo ""
        fi
    else
        # Rust only (absolute time)
        if [ "$JSON_ONLY" = true ] || [ "$CI_MODE" = true ]; then
            hyperfine $HYPERFINE_OPTS \
                --export-json "$JSON_OUT" \
                --ignore-failure \
                "$rust_cmd" \
                2>/dev/null || true
        else
            hyperfine $HYPERFINE_OPTS \
                --export-json "$JSON_OUT" \
                --ignore-failure \
                --style basic \
                "$rust_cmd" \
                2>&1 || true
            echo ""
        fi
    fi

    # Append to combined JSON
    if [ -f "$JSON_OUT" ]; then
        if [ "$FIRST_ENTRY" = false ]; then
            echo "," >> "$ALL_JSON"
        fi
        cat "$JSON_OUT" >> "$ALL_JSON"
        FIRST_ENTRY=false
    fi
done

echo "]" >> "$ALL_JSON"

# ------------------------------------------------------------------
# Generate summary
# ------------------------------------------------------------------
echo -e "${YELLOW}Generating benchmark summary...${NC}"

SUMMARY="${RESULTS_DIR}/summary.md"
{
    echo "# Benchmark Results: Rust vs C Utilities"
    echo ""
    echo "**Date**: $(date)"
    echo "**Mode**: ${MODE}"
    echo "**Comparison**: ${COMPARE_MODE}"
    echo "**Hyperfine**: ${HYPERFINE_VER}"
    echo "**Rust**: $(rustc --version 2>/dev/null || echo '?')"
    echo "**Target**: $(rustc -vV 2>/dev/null | grep host || echo "${RUST_TARGET}")"
    echo ""
    echo "## Results"
    echo ""
} > "$SUMMARY"

if [ "$COMPARE_MODE" = "rust-vs-c" ]; then
    echo "| Benchmark | Rust (s) | C (s) | Speedup |" >> "$SUMMARY"
    echo "|-----------|----------|-------|---------|" >> "$SUMMARY"
else
    echo "| Benchmark | Rust (s) | Std Dev |" >> "$SUMMARY"
    echo "|-----------|----------|---------|" >> "$SUMMARY"
fi

for entry in "${BENCHMARKS[@]}"; do
    name="${entry%%:*}"
    JSON_OUT="${RESULTS_DIR}/${name}.json"

    if [ ! -f "$JSON_OUT" ]; then
        echo "| ${name} | ERROR | — | — |" >> "$SUMMARY"
        continue
    fi

    # Extract data from JSON
    # Use relative path for Python portability (Windows vs Unix paths)
    REL_JSON="benchmark-results/${name}.json"

    if [ "$COMPARE_MODE" = "rust-vs-c" ]; then
        python3 <<PYEOF 2>/dev/null >> "$SUMMARY" || echo "| ${name} | PARSE ERROR | — | — |" >> "$SUMMARY"
import json
try:
    with open('${REL_JSON}') as f:
        data = json.load(f)
except:
    with open(r'${JSON_OUT}') as f:
        data = json.load(f)
results = data.get('results', [])
if len(results) >= 2:
    r_mean = results[0]['mean']
    r_stddev = results[0]['stddev']
    c_mean = results[1]['mean']
    c_stddev = results[1]['stddev']
    ratio = c_mean / r_mean if r_mean > 0 else 0
    if ratio > 1.0:
        speedup = f'{ratio:.2f}x Rust'
    elif ratio < 1.0:
        speedup = f'{1.0/ratio:.2f}x C'
    else:
        speedup = '1.00x tie'
    print(f'| ${name} | {r_mean:.6f} ± {r_stddev:.6f} | {c_mean:.6f} ± {c_stddev:.6f} | {speedup} |')
else:
    print(f'| ${name} | ERROR | ERROR | — |')
PYEOF
    else
        python3 <<PYEOF 2>/dev/null >> "$SUMMARY" || echo "| ${name} | PARSE ERROR | — |" >> "$SUMMARY"
import json
try:
    with open('${REL_JSON}') as f:
        data = json.load(f)
except:
    with open(r'${JSON_OUT}') as f:
        data = json.load(f)
results = data.get('results', [])
if len(results) >= 1:
    r_mean = results[0]['mean']
    r_stddev = results[0]['stddev']
    print(f'| ${name} | {r_mean:.6f} | ±{r_stddev:.6f} |')
else:
    print(f'| ${name} | ERROR | — |')
PYEOF
    fi
done

# Add footer
{
    echo ""
    echo "## System"
    echo "- **CPU**: $(nproc 2>/dev/null || echo '?') cores"
    echo "- **RAM**: $(free -h 2>/dev/null | grep Mem | awk '{print $2}' || echo '?')"
    echo "- **Rustc**: $(rustc --version 2>/dev/null || echo '?')"
    echo ""
    echo "## Reproduce"
    echo '```bash'
    echo "cd rust && cargo build --release && cd .."
    echo "bash scripts/run_benchmarks.sh"
    echo '```'
} >> "$SUMMARY"

# Also create plain text
cp "$SUMMARY" "${RESULTS_DIR}/summary.txt"

# Generate a CSV for CI tracking
CSV_OUT="${RESULTS_DIR}/benchmarks.csv"
echo "benchmark,mean_seconds,stddev_seconds,comparison" > "$CSV_OUT"
for entry in "${BENCHMARKS[@]}"; do
    name="${entry%%:*}"
    JSON_OUT="${RESULTS_DIR}/${name}.json"
    if [ -f "$JSON_OUT" ]; then
        python3 -c "
import json
with open('${JSON_OUT}') as f:
    data = json.load(f)
results = data.get('results', [])
for i, r in enumerate(results):
    label = 'rust' if i == 0 else 'c'
    print(f'${name}-{label},{r[\"mean\"]},{r[\"stddev\"]},{label}')
" 2>/dev/null >> "$CSV_OUT" || true
    fi
done

echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}Benchmarks complete!${NC}"
echo -e "Summary: ${RESULTS_DIR}/summary.md"
echo -e "JSON:    ${ALL_JSON}"
echo -e "CSV:     ${CSV_OUT}"
echo -e "${BLUE}========================================${NC}"

# Print results to terminal
echo ""
echo -e "${CYAN}=== Results ===${NC}"
tail -n +4 "$SUMMARY" | head -40 | while IFS= read -r line; do
    if [[ "$line" == "|"* ]]; then
        echo "$line"
    fi
done
echo ""
