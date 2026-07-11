#!/bin/bash
# Enable recursive globbing for JUnit XML discovery
shopt -s globstar

# generate_dashboard.sh — HTML test results dashboard
#
# Phase 9.6: CI/CD Hardening
# See planning/28_testing_framework_migration.md
#
# Reads CI artifact files (JUnit XML, benchmark JSON, coverage reports)
# and generates a static HTML dashboard showing test history, pass/fail
# trends, and benchmark comparisons.
#
# Usage:
#   ./scripts/generate_dashboard.sh                                    # Generate from default paths
#   ./scripts/generate_dashboard.sh --artifact-dir <dir>               # CI artifacts directory
#   ./scripts/generate_dashboard.sh --output-dir <dir>                 # Output directory
#   ./scripts/generate_dashboard.sh --gh-pages <dir>                   # Deploy to gh-pages

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ARTIFACT_DIR="${ARTIFACT_DIR:-${SCRIPT_DIR}/ci-artifacts}"
OUTPUT_DIR="${OUTPUT_DIR:-${SCRIPT_DIR}/dashboard}"
GH_PAGES_DIR=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --artifact-dir) ARTIFACT_DIR="$2"; shift 2 ;;
        --output-dir)   OUTPUT_DIR="$2"; shift 2 ;;
        --gh-pages)     GH_PAGES_DIR="$2"; shift 2 ;;
        --help)
            echo "Usage: $0 [--artifact-dir <dir>] [--output-dir <dir>] [--gh-pages <dir>]"
            exit 0 ;;
        *) echo "Unknown: $1"; exit 1 ;;
    esac
done

mkdir -p "$OUTPUT_DIR"

echo "Generating test dashboard..."
echo "  Artifacts: ${ARTIFACT_DIR}"
echo "  Output:    ${OUTPUT_DIR}"

# ── Collect test results from JUnit XML files ───────────────────────────────
declare -a TEST_RUNS=()
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0

for xml in "${ARTIFACT_DIR}"/**/junit-*.xml; do
    [ -f "$xml" ] || continue
    name=$(basename "$xml" .xml)
    pass=$(grep -oP 'tests="\K[0-9]+' "$xml" 2>/dev/null || echo "0")
    failures=$(grep -oP 'failures="\K[0-9]+' "$xml" 2>/dev/null || echo "0")
    skipped=$(grep -oP 'skipped="\K[0-9]+' "$xml" 2>/dev/null || echo "0")
    TEST_RUNS+=("$name:$pass:$failures:$skipped")
    TOTAL_PASS=$((TOTAL_PASS + pass - failures - skipped))
    TOTAL_FAIL=$((TOTAL_FAIL + failures))
    TOTAL_SKIP=$((TOTAL_SKIP + skipped))
done

# ── Collect benchmark data ───────────────────────────────────────────────────
BENCHMARK_DATA=""
BENCHMARK_JSON="${ARTIFACT_DIR}/benchmark-results/all-benchmarks.json"
if [ -f "$BENCHMARK_JSON" ]; then
    BENCHMARK_DATA=$(python3 -c "
import json
with open('${BENCHMARK_JSON}') as f:
    data = json.load(f)
results = []
for entry in data if isinstance(data, list) else [data]:
    for r in entry.get('results', []):
        cmd = r.get('command', 'unknown')[:50]
        results.append((cmd, r.get('mean', 0), r.get('stddev', 0)))
# Print as HTML table rows
for cmd, mean, stddev in sorted(results, key=lambda x: x[1], reverse=True)[:20]:
    print(f'<tr><td>{cmd}</td><td>{mean:.4f}s</td><td>±{stddev:.4f}s</td></tr>')
" 2>/dev/null) || true
fi

# ── Collect coverage data ────────────────────────────────────────────────────
COVERAGE_DATA=""
COVERAGE_SUMMARY="${ARTIFACT_DIR}/coverage/c/coverage-summary.txt"
if [ -f "$COVERAGE_SUMMARY" ]; then
    COVERAGE_DATA=$(grep -E "lines.*%" "$COVERAGE_SUMMARY" | head -5 || echo "N/A")
fi

# ── Generate HTML ────────────────────────────────────────────────────────────

cat > "${OUTPUT_DIR}/index.html" << 'HTMLHEAD'
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>GergiOS Test Dashboard</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; 
         background: #0d1117; color: #c9d1d9; padding: 20px; }
  .container { max-width: 1200px; margin: 0 auto; }
  h1 { color: #58a6ff; margin-bottom: 20px; }
  h2 { color: #8b949e; margin: 20px 0 10px; padding-bottom: 5px; border-bottom: 1px solid #30363d; }
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 15px; margin-bottom: 20px; }
  .card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 15px; }
  .card h3 { font-size: 14px; color: #8b949e; margin-bottom: 5px; }
  .card .value { font-size: 28px; font-weight: 600; }
  .pass .value { color: #3fb950; }
  .fail .value { color: #f85149; }
  .skip .value { color: #d29922; }
  table { width: 100%; border-collapse: collapse; margin: 10px 0; }
  th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid #30363d; }
  th { color: #8b949e; font-weight: 500; }
  .bar { height: 20px; border-radius: 10px; background: #21262d; overflow: hidden; margin: 5px 0; }
  .bar-fill { height: 100%; border-radius: 10px; transition: width 0.5s; }
  .bar-pass { background: #3fb950; }
  .bar-fail { background: #f85149; }
  .footer { text-align: center; color: #484f58; padding: 20px 0; font-size: 12px; }
  .badge { display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 12px; font-weight: 500; }
  .badge-pass { background: #3fb95030; color: #3fb950; }
  .badge-fail { background: #f8514930; color: #f85149; }
  .badge-skip { background: #d2992230; color: #d29922; }
</style>
</head>
<body>
<div class="container">
HTMLHEAD

# Header with date
echo "<h1>📊 GergiOS Test Dashboard</h1>"
echo "<p style='color: #8b949e;'>Last updated: $(date '+%Y-%m-%d %H:%M:%S')</p>"

# Summary stats
{
    echo '<div class="stats">'
    echo "<div class='card pass'><h3>✅ Tests Passed</h3><div class='value'>${TOTAL_PASS:-0}</div></div>"
    echo "<div class='card fail'><h3>❌ Tests Failed</h3><div class='value'>${TOTAL_FAIL:-0}</div></div>"
    echo "<div class='card skip'><h3>⏩ Skipped</h3><div class='value'>${TOTAL_SKIP:-0}</div></div>"
    echo "<div class='card'><h3>🧪 Test Runs</h3><div class='value'>${#TEST_RUNS[@]}</div></div>"
    echo '</div>'
}

# Pass/Fail bar
if [ $((TOTAL_PASS + TOTAL_FAIL)) -gt 0 ]; then
    PASS_PCT=$((TOTAL_PASS * 100 / (TOTAL_PASS + TOTAL_FAIL)))
    echo "<h2>Overall Pass Rate</h2>"
    echo "<div class='bar'><div class='bar-fill bar-pass' style='width:${PASS_PCT}%'></div></div>"
    echo "<p style='color: #8b949e;'>${PASS_PCT}% passed (${TOTAL_PASS}/${TOTAL_PASS + TOTAL_FAIL})</p>"
fi

# Test runs table
if [ ${#TEST_RUNS[@]} -gt 0 ]; then
    echo "<h2>Test Runs (Last 30 days)</h2>"
    echo "<table><tr><th>Suite</th><th>Passed</th><th>Failed</th><th>Skipped</th><th>Status</th></tr>"
    for run in "${TEST_RUNS[@]}"; do
        IFS=':' read -r name pass failures skipped <<< "$run"
        status="pass"
        [ "$failures" -gt 0 ] && status="fail"
        echo "<tr><td>${name}</td><td>${pass}</td><td>${failures}</td><td>${skipped}</td>"
        echo "<td><span class='badge badge-${status}'>${status}</span></td></tr>"
    done
    echo "</table>"
fi

# Benchmarks table
if [ -n "$BENCHMARK_DATA" ]; then
    echo "<h2>🚀 Top 20 Benchmarks (by runtime)</h2>"
    echo "<table><tr><th>Benchmark</th><th>Mean</th><th>Std Dev</th></tr>"
    echo "$BENCHMARK_DATA"
    echo "</table>"
fi

# Coverage section
if [ -n "$COVERAGE_DATA" ]; then
    echo "<h2>📈 Coverage</h2>"
    echo "<pre style='background: #161b22; padding: 10px; border-radius: 8px; color: #8b949e;'>"
    echo "$COVERAGE_DATA"
    echo "</pre>"
fi

# Footer
echo "<div class='footer'>GergiOS Test Dashboard · Generated by scripts/generate_dashboard.sh</div>"
echo "</div></body></html>"

echo -e "\nDashboard generated: ${OUTPUT_DIR}/index.html"

# ── Copy to gh-pages if requested ───────────────────────────────────────────
if [ -n "$GH_PAGES_DIR" ]; then
    mkdir -p "$GH_PAGES_DIR"
    cp -r "${OUTPUT_DIR}"/* "$GH_PAGES_DIR"
    echo "Deployed to: ${GH_PAGES_DIR}/"
fi
