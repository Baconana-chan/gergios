# CI/CD Pipeline Architecture — GergiOS

> Phase 9.6: CI/CD Hardening — Documentation
> See `planning/28_testing_framework_migration.md` for the full migration plan.

## Overview

The GergiOS CI/CD pipeline uses GitHub Actions with **11 jobs** across 3 trigger types:

- **push/PR** (every commit): Fast validation
- **schedule** (daily/weekly): Deep testing
- **workflow_dispatch** (manual): On-demand runs

## Pipeline Diagram

```
push/PR branch:
  ├─ rust-build ............. Build + test all Rust crates
  ├─ rust-sanitizers ....... ASan + UBSan (nightly)
  ├─ build ................. Legacy BSD Make (continue-on-error)
  ├─ static-analysis ....... clang-tidy + cppcheck
  ├─ security-scan ......... CodeQL + Safety
  └─ c-coverage ............ C gcov/lcov (CMake)

schedule / workflow_dispatch:
  ├─ rust-fuzz ............. 6 targets (600s each)
  ├─ rust-coverage ......... Rust llvm-cov → Codecov
  ├─ rust-benchmarks ....... Rust hyperfine → JSON
  ├─ c-benchmarks .......... C hyperfine → JSON
  └─ qemu-smoke ............ Matrix: smoke, fs, net, bt
```

## Job Details

### 1. `rust-build` — Rust Build & Test
- **Trigger**: push/PR
- **Time**: ~5 min
- **Steps**:
  1. Checkout code
  2. Install Rust stable + clippy + rustfmt
  3. Cache: `rust/` + `Cargo.lock`
  4. `cargo build --workspace --verbose`
  5. `cargo test --workspace --verbose`
  6. `cargo clippy --workspace -- -D warnings` (continue-on-error)
  7. Upload test results artifact (14 day retention)

### 2. `rust-sanitizers` — Sanitizer Tests
- **Trigger**: push/PR
- **Time**: ~10 min
- **Steps**:
  1. Install Rust nightly + rust-src
  2. `RUSTFLAGS="-Z sanitizer=address" cargo test --workspace`
  3. `RUSTFLAGS="-Z sanitizer=undefined" cargo test --workspace`
  4. Upload sanitizer logs

### 3. `rust-fuzz` — Fuzz Testing
- **Trigger**: schedule/dispatch
- **Time**: ~30 min (6 targets × 300s each)
- **Targets**: Message, TCP, UDP, DNS, AudioBuf, ProcFS
- **Upload**: Artifacts + logs (30 day retention)

### 4. `rust-coverage` — Rust Coverage
- **Trigger**: schedule/dispatch
- **Time**: ~10 min
- **Steps**:
  1. Install `cargo-llvm-cov`
  2. `cargo llvm-cov --workspace --lcov --output-path lcov.info`
  3. Upload to Codecov (flag: `rust`)
  4. Upload lcov.info artifact (30 day retention)

### 5. `build` — Legacy BSD Make
- **Trigger**: push/PR
- **Time**: ~20 min
- **Status**: 🟡 continue-on-error
- **Steps**:
  1. Install dependencies (gcc, make, lcov, qemu)
  2. `make do-tools`, `make do-lib`, `make do-build`
  3. Upload build artifacts (7 day retention)

### 6. `qemu-smoke` — QEMU Smoke Tests
- **Trigger**: schedule/dispatch
- **Matrix**: smoke | fs | net | bt
- **Time**: ~5 min per variant
- **Status**: 🟡 continue-on-error
- **Steps**:
  1. Install QEMU, gdisk, dosfstools, e2fsprogs, ovmf, Limine
  2. Build kernel via BSD Make
  3. Run smoke test script with 120s timeout
  4. Upload serial output + logs (14 day retention)

### 7. `rust-benchmarks` — Rust Benchmarks
- **Trigger**: schedule/dispatch
- **Time**: ~10 min
- **Steps**:
  1. Build Rust release binaries
  2. Install hyperfine
  3. `scripts/run_benchmarks.sh --ci`
  4. Upload results JSON + CSV (90 day retention)

### 8. `c-benchmarks` — C Benchmarks
- **Trigger**: schedule/dispatch
- **Time**: ~10 min (parallel to rust-benchmarks)
- **Status**: 🟡 continue-on-error
- **Steps**:
  1. Build C utilities via BSD Make
  2. Install hyperfine
  3. `scripts/run_c_benchmarks.sh --ci`
  4. Upload results JSON + CSV (90 day retention)

### 9. `c-coverage` — C Coverage
- **Trigger**: push/PR
- **Time**: ~10 min
- **Steps**:
  1. CMake configure with `--coverage -g -O0`
  2. Build 11 Catch2 test targets via Ninja
  3. `ctest -L phase9`
  4. `scripts/run_c_coverage.sh --html-only --ci`
  5. Upload to Codecov (flags: `c,unittests`)
  6. Upload HTML report + summary (30 day retention)

### 10. `static-analysis` — Static Analysis
- **Trigger**: push/PR
- **Time**: ~5 min
- **Status**: 🟡 continue-on-error
- **Steps**:
  1. Install clang-tidy + cppcheck
  2. `clang-tidy` on top 20 C/C++ files
  3. `cppcheck --enable=all --xml .`
  4. Upload cppcheck report

### 11. `security-scan` — Security Scanning
- **Trigger**: push/PR
- **Time**: ~10 min
- **Permissions**: security-events: write
- **Steps**:
  1. CodeQL Analysis (C/C++)
  2. `safety check` for Python dependencies
  3. Upload safety report

## Artifact Retention

| Artifact | Retention | 
|----------|-----------|
| Rust test results | 14 days |
| Sanitizer logs | 14 days |
| Fuzz artifacts | 30 days |
| Coverage reports | 30 days |
| Benchmark results | 90 days |
| Build artifacts | 7 days |
| QEMU serial logs | 14 days |

## Regression Detection

Benchmark regression detection runs via `scripts/compare_benchmarks.sh`:

```bash
# Compare current run against stored baseline
./scripts/compare_benchmarks.sh \
  --current benchmark-results/ \
  --baseline benchmark-baseline/ \
  --threshold 0.10 \
  --ci
```

- **Threshold**: >10% regression = failure in CI
- **Baseline**: Stored from previous successful run
- **Output**: PR comment markdown via `--export-md`

## Dashboard

The test results dashboard is generated from CI artifacts:

```bash
./scripts/generate_dashboard.sh \
  --artifact-dir ci-artifacts/ \
  --output-dir dashboard/
```

**Metrics displayed**:
- Total pass/fail/skip counts
- Pass rate bar
- Per-suite breakdown table
- Top 20 benchmarks by runtime
- Coverage summary

## Cache Strategy

| Cache | Key | Scope |
|-------|-----|-------|
| Rust crates | `hashFiles('rust/Cargo.lock')` | `rust/` workspace |
| Rust nightly | `hashFiles('rust/Cargo.lock')` | nightly toolchain |
| Rust fuzz | `hashFiles('rust/fuzz/Cargo.toml')` | fuzz workspace |
| Rust benchmarks | `hashFiles('rust/Cargo.lock')` | release build |

## Notifications

- PR comments for coverage changes (via `py-cov-action`)
- Benchmark regression comments (via `compare_benchmarks.sh --export-md`)
- Slack/Discord: TODO (requires webhook configuration)

## Future Improvements

- [ ] earm architecture build matrix
- [ ] ThreadSanitizer (TSan) for nightly
- [ ] Increase fuzz time to 600s per target
- [ ] Slack/Discord notifications
- [ ] Retry logic for QEMU flaky tests
- [ ] `continue-on-error` → fail-only for critical tests
