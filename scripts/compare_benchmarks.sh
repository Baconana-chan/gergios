#!/bin/bash
# compare_benchmarks.sh — benchmark regression detection
#
# Phase 9.6: CI/CD Hardening
# See planning/28_testing_framework_migration.md
#
# Compares current benchmark JSON results against a stored baseline
# and reports regressions > 10%.
#
# Usage:
#   ./scripts/compare_benchmarks.sh                          # Compare default paths
#   ./scripts/compare_benchmarks.sh --current <file>          # Current results JSON
#   ./scripts/compare_benchmarks.sh --baseline <file>         # Baseline JSON
#   ./scripts/compare_benchmarks.sh --threshold 0.05          # 5% regression threshold
#   ./scripts/compare_benchmarks.sh --ci                      # CI mode (exit 1 on regression)
#   ./scripts/compare_benchmarks.sh --export-md <file>        # Export PR comment

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/benchmark-results}"
BASELINE_DIR="${BASELINE_DIR:-$(pwd)/benchmark-baseline}"
THRESHOLD=0.10  # 10% default
CI_MODE=false
EXPORT_MD=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --current)    RESULTS_DIR="$2"; shift 2 ;;
        --baseline)   BASELINE_DIR="$2"; shift 2 ;;
        --threshold)  THRESHOLD="$2"; shift 2 ;;
        --ci)         CI_MODE=true; shift ;;
        --export-md)  EXPORT_MD="$2"; shift 2 ;;
        --help)
            echo "Usage: $0 [--current <dir>] [--baseline <dir>] [--threshold 0.10] [--ci] [--export-md <file>]"
            exit 0 ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

echo -e "${BLUE}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║   Benchmark Regression Detection — Phase 9.6 ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════╝${NC}"
echo "Current:  ${RESULTS_DIR}"
echo "Baseline: ${BASELINE_DIR}"
echo "Threshold: ${THRESHOLD} (${THRESHOLD}% change)"
echo "CI mode:  ${CI_MODE}"
echo ""

CURRENT_JSON="${RESULTS_DIR}/all-benchmarks.json"
BASELINE_JSON="${BASELINE_DIR}/all-benchmarks.json"

if [ ! -f "$CURRENT_JSON" ]; then
    echo -e "${RED}Error: Current results not found: ${CURRENT_JSON}${NC}"
    exit 1
fi
if [ ! -f "$BASELINE_JSON" ]; then
    echo -e "${YELLOW}No baseline found at ${BASELINE_JSON}${NC}"
    echo "Saving current results as baseline..."
    mkdir -p "$BASELINE_DIR"
    cp "$CURRENT_JSON" "$BASELINE_JSON"
    exit 0
fi

# ── Compare benchmarks using Python ─────────────────────────────────────────

REGRESSIONS=$(python3 -c "
import json, sys

with open('${CURRENT_JSON}') as f:
    current_data = json.load(f)
with open('${BASELINE_JSON}') as f:
    baseline_data = json.load(f)

# Extract results from current
current_results = {}
for entry in current_data if isinstance(current_data, list) else [current_data]:
    for r in entry.get('results', []):
        cmd = r.get('command', 'unknown')
        # Shorten command to benchmark name
        name = cmd.split('/')[-1].split()[0] if '/' in cmd else cmd[:40]
        current_results[name] = {
            'mean': r.get('mean', 0),
            'stddev': r.get('stddev', 0),
        }

# Extract results from baseline
baseline_results = {}
for entry in baseline_data if isinstance(baseline_data, list) else [baseline_data]:
    for r in entry.get('results', []):
        cmd = r.get('command', 'unknown')
        name = cmd.split('/')[-1].split()[0] if '/' in cmd else cmd[:40]
        baseline_results[name] = {
            'mean': r.get('mean', 0),
            'stddev': r.get('stddev', 0),
        }

# Compare
found = False
regressions = []
improvements = []

for name in sorted(set(list(current_results.keys()) + list(baseline_results.keys()))):
    cur = current_results.get(name)
    base = baseline_results.get(name)
    if not cur or not base:
        continue
    if cur['mean'] == 0 or base['mean'] == 0:
        continue
    found = True
    ratio = cur['mean'] / base['mean']
    change = (ratio - 1.0) * 100

    status = 'OK'
    if ratio > 1.0 + ${THRESHOLD}:
        status = 'REGRESSION'
        regressions.append((name, change, cur['mean'], base['mean']))
    elif ratio < 1.0 - ${THRESHOLD}:
        status = 'IMPROVEMENT'
        improvements.append((name, change, cur['mean'], base['mean']))
    else:
        status = 'OK'

    print(f'{status}: {name}: {change:+.2f}% (cur={cur[\"mean\"]:.6f}s base={base[\"mean\"]:.6f}s)')

if not found:
    print('WARNING: No matching benchmarks found between current and baseline')

# Exit with summary info
if len(regressions) > 0:
    print(f'REGRESSION_COUNT={len(regressions)}')
    print(f'REGRESSION_NAMES={\",\".join(r[0] for r in regressions)}')
if len(improvements) > 0:
    print(f'IMPROVEMENT_COUNT={len(improvements)}')
print(f'TOTAL_COMPARED={len(set(current_results.keys()) & set(baseline_results.keys()))}')
" 2>&1) || true

echo ""
echo "$REGRESSIONS"
echo ""

# ── Extract counts ──────────────────────────────────────────────────────────
REGRESSION_COUNT=$(echo "$REGRESSIONS" | grep "REGRESSION_COUNT=" | cut -d= -f2 || echo "0")
IMPROVEMENT_COUNT=$(echo "$REGRESSIONS" | grep "IMPROVEMENT_COUNT=" | cut -d= -f2 || echo "0")

# ── Generate PR comment (markdown) ──────────────────────────────────────────
if [ -n "$EXPORT_MD" ]; then
    {
        echo "## 📊 Benchmark Regression Report"
        echo ""
        echo "| Status | Count |"
        echo "|--------|-------|"
        echo "| ✅ OK | — |"
        echo "| 🔴 Regression | ${REGRESSION_COUNT} |"
        echo "| 🟢 Improvement | ${IMPROVEMENT_COUNT} |"
        echo ""
        echo "### Regressions (threshold: ${THRESHOLD}%)"
        echo ""
        echo "$REGRESSIONS" | grep "^REGRESSION:" | while IFS=: read -r status name rest; do
            echo "- 🔴 \`${name}\` ${rest}"
        done
        echo ""
        echo "### Improvements"
        echo ""
        echo "$REGRESSIONS" | grep "^IMPROVEMENT:" | while IFS=: read -r status name rest; do
            echo "- 🟢 \`${name}\` ${rest}"
        done
        echo ""
        echo "---"
        echo "_Generated by scripts/compare_benchmarks.sh_"
    } > "$EXPORT_MD"
    echo -e "${GREEN}PR comment exported: ${EXPORT_MD}${NC}"
fi

# ── CI mode: fail on regressions ─────────────────────────────────────────────
if [ "$CI_MODE" = true ] && [ "${REGRESSION_COUNT:-0}" -gt 0 ]; then
    echo -e "${RED}❌ ${REGRESSION_COUNT} regression(s) detected (threshold: ${THRESHOLD}%)${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Comparison complete.${NC}"
echo "Regressions: ${REGRESSION_COUNT:-0}, Improvements: ${IMPROVEMENT_COUNT:-0}"
