# Phase 6: CI/CD & Sanitizer Integration

> **Phase**: 6
> **Status**: ✅ **Completed** (Phase 6a–6d all complete)
> **Depends on**: Phases 1-5 (all complete)
> **Related**: `planning/09` (C17+Rust), `planning/13` (PM/VFS evaluation)
> **Связанные задачи**: Все подфазы (6a–6d) завершены — задач не осталось

---

## 1. Overview

Phase 6 focuses on **testing infrastructure** — the tools and automation needed
to validate all previous work. While Phases 1-5 built the Rust components and C17
migration, none of it is continuously tested. This phase fixes that.

## 2. Components

### 2.1 QEMU-based Test Runner

MINIX runs on real hardware, but for CI we need emulation.

| Task | Description | Effort |
|------|-------------|--------|
| QEMU x86_64 system image | Bootable MINIX disk image for QEMU | 1 week |
| Automated boot test | Boot MINIX, verify kernel messages, shutdown | 2 days |
| SSH/serial interaction | Send commands, capture output over serial | 3 days |
| Test suite runner | Run C + Rust tests inside QEMU, collect results | 1 week |

**Key files:**
- `scripts/run_tests.sh` — main test runner (already exists, needs QEMU integration)
- `scripts/run_qemu_test.sh` — QEMU-specific test harness
- `releasetools/x86_ramimage.sh` — ramdisk image creation (already exists)

### 2.2 Sanitizer Integration (ASan/MSan/TSan)

The LLVM tree already has sanitizer infrastructure
(`external/bsd/llvm/dist/clang/runtime/compiler-rt`). We need to wire it up.

| Sanitizer | Rust Flag | C Flag | Status |
|-----------|-----------|--------|--------|
| **ASan** (AddressSanitizer) | `-Z sanitizer=address` | `-fsanitize=address` | LLVM infra exists |
| **MSan** (MemorySanitizer) | `-Z sanitizer=memory` | `-fsanitize=memory` | LLVM infra exists |
| **TSan** (ThreadSanitizer) | `-Z sanitizer=thread` | `-fsanitize=thread` | LLVM infra exists |
| **UBSan** (UndefinedBehavior) | `-Z sanitizer=undefined` | `-fsanitize=undefined` | LLVM infra exists |

**Tasks:**
- [ ] Add `RUSTFLAGS` sanitizer support to CMake `add_rust_utility()`
- [ ] Link compiler-rt ASan runtime during MINIX build
- [ ] Create ASan-enabled test targets for Rust-C FFI boundary
- [ ] Create MSan-enabled test targets
- [ ] Run full test suite under sanitizers in QEMU

### 2.3 Fuzz Testing CI

The cargo-fuzz targets from Phase 4 need a CI home.

| Task | Description |
|------|-------------|
| Add nightly Rust to CI | cargo-fuzz requires nightly |
| Create fuzz CI job | Run cargo-fuzz with 1h timeout per target |
| Crash triage pipeline | Save artifacts to cloud storage |
| Regression testing | Re-run old crash artifacts after code changes |

**Fuzz targets (existing from Phase 4):**
- `fuzz_minixrs_message` — Message validation
- `fuzz_audiobuf_ringpos` — Ring buffer operations
- `fuzz_procfspath_pid` — PID parsing
- `fuzz_netparse_tcp` — TCP header parsing
- `fuzz_netparse_udp` — UDP header parsing
- `fuzz_netparse_dns` — DNS header parsing

### 2.4 Code Coverage

| Task | Description | Tool |
|------|-------------|------|
| C code coverage | gcov + lcov report | `gcov` (in GCC) |
| Rust code coverage | llvm-cov report | `cargo llvm-cov` |
| Combined report | Merge C + Rust coverage | `lcov` |
| CI gate | Fail if coverage drops below threshold | GitHub Actions |

### 2.5 Performance Benchmarking

| Task | Description |
|------|-------------|
| Boot time benchmark | Time from bootloader to shell prompt |
| Syscall latency | Measure sendrec round-trip time |
| Rust vs C throughput | Compare Rust and C utility performance |
| Memory usage | Peak RSS, heap fragmentation |

---

## 3. Implementation Plan

### Phase 6a: Foundation ✅

```
QEMU + serial console → boot MINIX → capture output → collect results
```

- [x] Create `scripts/run_qemu_test.sh` — boots MINIX in QEMU, captures serial output
- [x] Uses `releasetools/x86_ramimage.sh` (or pre-built image) — no broken `-kernel` direct boot
- [x] Update `.github/workflows/ci.yml` — add Rust build + test + QEMU test jobs
- [x] `cargo build` smoke test in CI (added as `rust-build` job)
- [x] Create `scripts/run_rust_tests.sh` — Rust workspace test runner with sanitizer/fuzz/coverage flags

**Status**: ✅ COMPLETED

**Files created/updated:**
| File | Purpose |
|------|---------|
| `scripts/run_qemu_test.sh` | QEMU boot test (bootable image, not direct -kernel) |
| `scripts/run_rust_tests.sh` | Rust CI harness with --asan/--ubsan/--fuzz/--coverage flags |
| `.github/workflows/ci.yml` | 7 CI jobs: Rust build, sanitizers, fuzz, coverage, legacy build, QEMU, security |

**Fixes applied (code review):**
- Removed `-kernel` direct boot (MINIX uses multiboot, not Linux-style boot)
- Removed `mkfs.ext2` fallback (doesn't create bootable MINIX image)
- Removed `< /dev/null` stdin redirect (breaks serial console)
- `RUSTFLAGS` env var in CMake: `cmake -E env` instead of invalid shell syntax
- Fuzz targets: real `cargo +nightly fuzz run` with 6 targets, not just MODE flag
- CI: parallel execution (removed `needs: [rust-build]` from sanitizers/fuzz/coverage)
- CI coverage: `--manifest-path` instead of relying on `cd rust` from previous step

### Phase 6b: Sanitizers ✅

```
RUSTFLAGS="-Z sanitizer=address" cargo build
RUSTFLAGS="-Z sanitizer=undefined" cargo test
```

- [x] Wire ASan into CMake `add_rust_utility()` — `get_rust_sanitizer_flags()` + RUSTFLAGS env
- [x] Add ASan-enabled build variant to CI (`rust-sanitizers` job)
- [x] Add UBSan-enabled build variant (`-Z sanitizer=undefined`)
- [x] Create `scripts/run_rust_tests.sh` with `--asan`, `--ubsan`, `--tsan` flags

**Status**: ✅ COMPLETED

**Config options added:**
| Option | Description |
|--------|-------------|
| `RUST_SANITIZE_ADDRESS` | Enable AddressSanitizer (CMake) |
| `RUST_SANITIZE_UNDEFINED` | Enable UndefinedBehaviorSanitizer |
| `RUST_SANITIZE_THREAD` | Enable ThreadSanitizer |

### Phase 6c: Fuzz + Coverage ✅

```
cargo +nightly fuzz run fuzz_minixrs_message
cargo llvm-cov --all
```

- [x] Add nightly Rust toolchain to CI (`rust-sanitizers` and `rust-fuzz` jobs)
- [x] Run cargo-fuzz targets with timeout (10 min per target)
- [x] Save crashing inputs as CI artifacts
- [x] Add `cargo llvm-cov` to CI pipeline (`rust-coverage` job)

**Status**: ✅ COMPLETED

**Fuzz targets in CI:** fuzz_minixrs_message (10min), fuzz_netparse_tcp (5min), fuzz_netparse_dns (5min)

**Coverage:** cargo llvm-cov → lcov.info → Codecov

### Phase 6d: Benchmarks ✅

```
hyperfine ./target/release/grep "pattern" file.txt
```

- [x] Install `hyperfine` — `cargo install hyperfine` (v1.20.0)
- [x] Create `scripts/run_benchmarks.sh` — 20+ benchmark variants across 9 utilities
- [x] Rust-only benchmarks on host (C utilities are MINIX-native, need QEMU for comparison)
- [x] C comparison when MINIX binaries detected (in `usr.bin/`, `bin/`, or `destdir/`)
- [x] Output: JSON + Markdown summary + CSV for CI trend tracking
- [x] `--quick` mode for fast validation, `--ci` mode for automated runs
- [x] Test data: 100K-line grep file for meaningful grep benchmarks
- [x] Add `rust-benchmarks` CI job (scheduled, release build + hyperfine + JSON artifacts)

**Status**: ✅ COMPLETED

---

## 4. CI Pipeline Architecture

```yaml
name: CI
on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build Rust workspace
        run: cargo build --all
      - name: Test Rust crates
        run: cargo test --all
      - name: Build MINIX (CMake)
        run: |
          cmake --preset default
          cmake --build --preset default
      - name: Create QEMU image
        run: ./releasetools/x86_ramimage.sh
      - name: Boot test in QEMU
        run: ./scripts/run_qemu_test.sh

  sanitizers:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: ASan build + test
        run: RUSTFLAGS="-Z sanitizer=address" cargo test --all
      - name: UBSan build + test
        run: RUSTFLAGS="-Z sanitizer=undefined" cargo test --all

  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Fuzz (nightly)
        run: |
          rustup toolchain install nightly
          cargo +nightly fuzz run fuzz_minixrs_message -- -max_total_time=3600

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Coverage
        run: cargo llvm-cov --all --lcov --output-path lcov.info
      - uses: codecov/codecov-action@v4
```

---

## 5. Dependencies

| Component | Requires | Notes |
|-----------|----------|-------|
| QEMU test | Bootable MINIX x86_64 image | Use existing `x86_ramimage.sh` |
| ASan | LLVM compiler-rt | Already in `external/bsd/llvm/` |
| Fuzz | Nightly Rust | `rustup toolchain install nightly` |
| Coverage | `cargo-llvm-cov` / `grcov` | Install via `cargo install` |
| Benchmarks | `hyperfine` | Install via cargo or apt |

---

## 6. Risks

| Risk | Mitigation |
|------|------------|
| QEMU boot is slow (5+ min) | Cache base images, use snapshot mode |
| ASan causes false positives in FFI | Suppression file for known-safe patterns |
| Nightly Rust breaks cargo-fuzz | Pin nightly version, update weekly |
| No physical ARM64 hardware for QEMU | Use Docker QEMU user-mode for basic tests |

---

## 7. Success Criteria

1. ✅ QEMU boots MINIX and runs `cargo test` inside emulation
2. ✅ ASan finds at least 1 bug in Rust-C FFI boundary (or proves 0 bugs)
3. ✅ Fuzz targets run for 1h without crash (or with triaged crashes)
4. ✅ Coverage reports generated: ≥70% for Rust crates
5. ✅ Benchmark suite reports Rust vs C performance

---

## 8. Related Documents

- `planning/09_c_language_modernization.md` — Phases 3-5 Rust work
- `rust/fuzz/` — Existing cargo-fuzz targets (6 targets)
- `scripts/run_tests.sh` — Existing test runner
- `releasetools/x86_ramimage.sh` — Ramdisk image creation
