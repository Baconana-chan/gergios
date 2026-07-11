#!/bin/bash
# run_c_benchmarks.sh — C utility performance benchmarks
#
# Phase 9.5: C Coverage & Benchmarks
# See planning/28_testing_framework_migration.md
#
# Benchmarks C utilities (grep, diff, etc.) using hyperfine.
# Can be combined with scripts/run_benchmarks.sh for Rust vs C comparison.
#
# Usage:
#   ./scripts/run_c_benchmarks.sh                          # Full run
#   ./scripts/run_c_benchmarks.sh --quick                  # Quick run
#   ./scripts/run_c_benchmarks.sh --utility grep,seq       # Filter by name
#   ./scripts/run_c_benchmarks.sh --ci                     # CI mode (JSON)
#
# Requirements:
#   - hyperfine (cargo install hyperfine)
#   - C utilities built (via make or already on PATH)
#   - MINIX destdir or sysroot for cross-compiled binaries

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/benchmark-results-c}"
DATA_DIR="${SCRIPT_DIR}/benchmark-data"
MODE="full"
FILTER=""
CI_MODE=false

mkdir -p "$RESULTS_DIR"
mkdir -p "$DATA_DIR"

# ── Parse arguments ──────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)    MODE="quick"; shift ;;
        --utility)  FILTER="$2"; shift 2 ;;
        --ci)       CI_MODE=true; shift ;;
        --help)
            echo "Usage: $0 [--quick] [--utility grep,seq] [--ci]"
            echo "Benchmarks C utilities. Results in benchmark-results-c/"
            exit 0
            ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

# ── Prerequisites ────────────────────────────────────────────────────────────

if ! command -v hyperfine &>/dev/null; then
    echo -e "${RED}Error: hyperfine not found. Install: cargo install hyperfine${NC}"
    exit 1
fi

HYPERFINE_VER=$(hyperfine --version 2>&1 | head -1)
echo -e "${BLUE}Using ${HYPERFINE_VER}${NC}"

# ── Detect C binaries ────────────────────────────────────────────────────────

# Priority: PATH > destdir > MINIX-specific locations
c_bin() {
    local name="$1"
    # Check PATH first (system utilities)
    local sys_path
    sys_path=$(command -v "$name" 2>/dev/null || true)
    if [ -n "$sys_path" ]; then
        # Use system binary as baseline
        echo "$sys_path"
        return
    fi
    # Check destdir
    for dir in \
        "${SCRIPT_DIR}/destdir/usr/bin/${name}" \
        "${SCRIPT_DIR}/destdir/bin/${name}" \
        "${SCRIPT_DIR}/usr.bin/${name}/${name}" \
        "${SCRIPT_DIR}/bin/${name}/${name}"; do
        if [ -f "$dir" ] && [ -x "$dir" ]; then
            echo "$dir"
            return
        fi
    done
    echo ""
}

# Also define Rust binary detection for comparison
rust_bin() {
    local name="$1"
    local target="${SCRIPT_DIR}/rust/target/release"
    if [ -f "${target}/${name}.exe" ]; then
        echo "${target}/${name}.exe"
    elif [ -f "${target}/${name}" ]; then
        echo "${target}/${name}"
    fi
    echo ""
}

# ── Create test data ─────────────────────────────────────────────────────────

echo -e "${YELLOW}[setup] Creating benchmark test data...${NC}"

GREP_FILE="${DATA_DIR}/grep-benchmark.txt"
if [ ! -f "$GREP_FILE" ] || [ "$(wc -l < "$GREP_FILE")" -lt 50000 ]; then
    echo "Generating grep test data (100K lines)..."
    for i in $(seq 1 1000); do
        echo "the quick brown fox jumps over the lazy dog $i"
        echo "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG $i"
        echo "Lorem ipsum dolor sit amet consectetur adipiscing elit $i"
        echo "Pack my box with five dozen liquor jugs $i"
        echo "Rust cargo clippy fmt build test benchmark $i"
        echo "MINIX kernel driver server filesystem network $i"
    done > "$GREP_FILE"
fi

SEQ_FILE="${DATA_DIR}/seq-large.txt"
if [ ! -f "$SEQ_FILE" ]; then
    seq 1 100000 > "$SEQ_FILE"
fi

echo -e "${GREEN}Test data ready.${NC}"
echo ""

# ── Define benchmarks ────────────────────────────────────────────────────────

declare -a BENCHMARKS=()

add_benchmark() {
    local name="$1"
    local bin="$2"
    local args="$3"
    local label="${4:-c}"

    local cb
    cb=$(c_bin "$bin")
    [ -z "$cb" ] && return

    # Check for Rust version
    local rb
    rb=$(rust_bin "$bin")

    if [ -n "$rb" ]; then
        BENCHMARKS+=("${name}:${cb} ${args}:${rb} ${args}:${label}")
    else
        BENCHMARKS+=("${name}:${cb} ${args}::${label}")
    fi
}

# basename
add_benchmark "basename-path"     "basename" "/usr/share/dict/words"
add_benchmark "basename-suffix"   "basename" "/var/log/syslog.1.gz .gz"

# dirname
add_benchmark "dirname-path"      "dirname"  "/usr/share/dict/words"
add_benchmark "dirname-deep"      "dirname"  "/a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p"

# echo
add_benchmark "echo-short"        "echo"     "Hello World"
add_benchmark "echo-nflag"        "echo"     "-n No trailing newline"

# grep
add_benchmark "grep-fixed"        "grep" "-F fox ${GREP_FILE}"
add_benchmark "grep-regex"        "grep" "'jump.*quick' ${GREP_FILE}"
add_benchmark "grep-icase"        "grep" "-i 'MINIX' ${GREP_FILE}"
add_benchmark "grep-count"        "grep" "-c 'lorem' ${GREP_FILE}"

# seq
add_benchmark "seq-small"         "seq"   "1 1000"
add_benchmark "seq-large"         "seq"   "1 100000"
add_benchmark "seq-float"         "seq"   "0.5 0.25 10.0"

# sleep (short)
if command -v sleep &>/dev/null; then
    add_benchmark "sleep-10ms"    "sleep" "0.01"
    add_benchmark "sleep-100ms"   "sleep" "0.1"
fi

# true/false (process creation overhead)
add_benchmark "true-exit"    "true"  ""
add_benchmark "false-exit"   "false" ""

# Apply filter
if [ -n "$FILTER" ]; then
    IFS=',' read -ra FILTER_UTILS <<< "$FILTER"
    FILTERED=()
    for entry in "${BENCHMARKS[@]}"; do
        name="${entry%%:*}"
        for util in "${FILTER_UTILS[@]}"; do
            if [[ "$name" == "$util"* ]]; then
                FILTERED+=("$entry")
                break
            fi
        done
    done
    BENCHMARKS=("${FILTERED[@]}")
fi

if [ ${#BENCHMARKS[@]} -eq 0 ]; then
    echo -e "${RED}No C binaries found matching filter.${NC}"
    exit 1
fi

# ── Hyperfine options ───────────────────────────────────────────────────────

if [ "$MODE" = "quick" ]; then
    HF_OPTS="--warmup 1 --min-runs 3"
elif [ "$CI_MODE" = true ]; then
    HF_OPTS="--warmup 1 --min-runs 5"
else
    HF_OPTS="--warmup 3 --min-runs 10"
fi

# ── Run benchmarks ───────────────────────────────────────────────────────────

echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   C Performance Benchmarks — Phase 9.5      ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════╝${NC}"
echo "Mode:            $MODE"
echo "C benchmarks:    ${#BENCHMARKS[@]}"
echo ""

TOTAL=${#BENCHMARKS[@]}
CURRENT=0

ALL_JSON="${RESULTS_DIR}/all-benchmarks.json"
echo "[" > "$ALL_JSON"
FIRST=true

for entry in "${BENCHMARKS[@]}"; do
    CURRENT=$((CURRENT + 1))

    IFS=':' read -r name c_cmd r_cmd label <<< "$entry"
    echo -e "${YELLOW}[${CURRENT}/${TOTAL}] ${name}${NC}"

    JSON_OUT="${RESULTS_DIR}/${name}.json"

    if [ -n "$r_cmd" ]; then
        # C vs Rust comparison
        hyperfine $HF_OPTS \
            --export-json "$JSON_OUT" \
            --ignore-failure \
            --style basic \
            "$c_cmd" \
            "$r_cmd" \
            2>&1 || true
    else
        # C only (absolute times)
        hyperfine $HF_OPTS \
            --export-json "$JSON_OUT" \
            --ignore-failure \
            --style basic \
            "$c_cmd" \
            2>&1 || true
    fi

    # Append to combined JSON
    if [ -f "$JSON_OUT" ]; then
        if [ "$FIRST" = false ]; then echo "," >> "$ALL_JSON"; fi
        cat "$JSON_OUT" >> "$ALL_JSON"
        FIRST=false
    fi
done

echo "]" >> "$ALL_JSON"

# ── Generate summary ─────────────────────────────────────────────────────────

echo -e "${YELLOW}Generating summary...${NC}"

SUMMARY="${RESULTS_DIR}/summary.md"
{
    echo "# C Benchmark Results"
    echo ""
    echo "**Date**: $(date)"
    echo "**Mode**: ${MODE}"
    echo "**Hyperfine**: ${HYPERFINE_VER}"
    echo ""
    echo "## Results"
    echo ""
    echo "| Benchmark | C (s) | Rust (s) | Speedup |"
    echo "|-----------|-------|----------|---------|"
} > "$SUMMARY"

for entry in "${BENCHMARKS[@]}"; do
    IFS=':' read -r name c_cmd r_cmd label <<< "$entry"
    JSON_OUT="${RESULTS_DIR}/${name}.json"

    if [ ! -f "$JSON_OUT" ]; then
        echo "| ${name} | ERROR | — | — |" >> "$SUMMARY"
        continue
    fi

    python3 -c "
import json
with open('${JSON_OUT}') as f:
    data = json.load(f)
results = data.get('results', [])
if len(results) >= 2:
    r1 = results[0]['mean']
    r2 = results[1]['mean']
    ratio = r2 / r1 if r1 > 0 else 0
    if ratio > 1.0:
        spd = f'{ratio:.2f}x Rust'
    elif ratio < 1.0:
        spd = f'{1.0/ratio:.2f}x C'
    else:
        spd = 'tie'
    print(f'| ${name} | {r1:.6f} | {r2:.6f} | {spd} |')
elif len(results) >= 1:
    r1 = results[0]['mean']
    print(f'| ${name} | {r1:.6f} | — | (C only) |')
else:
    print(f'| ${name} | ERROR | — | — |')
" >> "$SUMMARY" 2>/dev/null || echo "| ${name} | PARSE_ERROR | — | — |" >> "$SUMMARY"
done

{
    echo ""
    echo "## System"
    echo "- **CPU**: $(nproc 2>/dev/null || echo '?') cores"
    echo "- **Date**: $(date)"
} >> "$SUMMARY"

# CSV for CI tracking
CSV_OUT="${RESULTS_DIR}/benchmarks.csv"
echo "benchmark,mean_seconds,stddev_seconds,implementation" > "$CSV_OUT"
for entry in "${BENCHMARKS[@]}"; do
    IFS=':' read -r name c_cmd r_cmd label <<< "$entry"
    JSON_OUT="${RESULTS_DIR}/${name}.json"
    if [ -f "$JSON_OUT" ]; then
        python3 -c "
import json
with open('${JSON_OUT}') as f:
    data = json.load(f)
results = data.get('results', [])
labels = ['c', 'rust']
for i, r in enumerate(results):
    l = labels[i] if i < len(labels) else 'unknown'
    print(f'${name}-{l},{r[\"mean\"]},{r[\"stddev\"]},{l}')
" >> "$CSV_OUT" 2>/dev/null || true
    fi
done

echo ""
echo -e "${GREEN}✅ C benchmarks complete.${NC}"
echo -e "Summary: ${SUMMARY}"
echo -e "CSV:     ${CSV_OUT}"
echo ""

# Print results
echo -e "${CYAN}=== Results ===${NC}"
tail -n +5 "$SUMMARY" | head -30
echo ""
