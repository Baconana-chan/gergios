# Testing Guide — GergiOS

> Phase 9.6: CI/CD Hardening — Documentation
> See `planning/28_testing_framework_migration.md` for the full migration plan.

## Overview

GergiOS has a multi-language, multi-framework testing infrastructure:

| Language | Framework | Scope | Run command |
|----------|-----------|-------|-------------|
| Rust (unit) | `cargo test` | All crates | `cd rust && cargo test --workspace` |
| Rust (proptest) | `proptest` (TestRunner::run) | IPC, ext4, network, BT | `cd rust && cargo test --tests proptest_*` |
| Rust (fuzz) | `cargo-fuzz` | Message, TCP, UDP, DNS | `cd rust && cargo fuzz run <target>` |
| C/C++ (Catch2) | Catch2 v2.13.10 | FFI tests, wire-format | `cd build && ctest -L phase9` |
| C (coverage) | gcov/lcov | C code coverage | `./scripts/run_c_coverage.sh` |
| Integration | QEMU | Boot, FS, Net, BT | `./scripts/qemu_test_*.sh` |
| Benchmarks | hyperfine | Rust vs C performance | `./scripts/run_benchmarks.sh` |
| Security | Shell scripts | Capability, MAC, W^X | `./scripts/run_security_tests.sh` |

## Quick Start

### 1. Run all Rust tests

```bash
cd rust && cargo test --workspace
```

To see output for passing tests:

```bash
cd rust && cargo test --workspace -- --nocapture
```

### 2. Run property-based tests only

```bash
cd rust
cargo test -p minix-rs --test proptest_message
cargo test -p ext4-core --test proptest_extent --test proptest_dir
cargo test -p net-parse --test proptest_packet
cargo test -p minix-bt-stack --test proptest_sdp
```

Total: **36 proptests** across 5 crates.

### 3. Run Catch2 standalone tests

```bash
mkdir -p build && cd build
cmake .. -DMK_CATCH2=ON -DMKATF=OFF
cmake --build . --target tcp_socket tcp_error raw_icmp raw_ipv6 ipv6_addr \
  bpf_filter bpf_attach blocktest ds_test rmibtest safecopy_test
ctest -L phase9 --output-on-failure
```

Total: **129 Catch2 tests** across 8 test suites.

### 4. Run benchmarks

```bash
# Rust benchmarks (requires release build)
cd rust && cargo build --release
./scripts/run_benchmarks.sh --quick

# C benchmarks (requires C utilities)
./scripts/run_c_benchmarks.sh --quick

# Compare with baseline
./scripts/compare_benchmarks.sh --baseline bench-baseline --current benchmark-results
```

### 5. Run coverage

```bash
# Rust
cargo llvm-cov --manifest-path rust/Cargo.toml --workspace --lcov --output-path rust/lcov.info

# C (requires CMake build with coverage flags)
./scripts/run_c_coverage.sh --quick
```

### 6. Run QEMU smoke tests

```bash
# Build boot image first
./scripts/qemu_test_smoke.sh --timeout 120

# Filesystem, network, Bluetooth
./scripts/qemu_test_fs.sh --timeout 120
./scripts/qemu_test_net.sh --timeout 120
./scripts/qemu_test_bt.sh --timeout 120
```

## Test Architecture

### Rust Tests (`rust/`)

```
rust/
├── proptest-helpers/       # Shared test strategies (proptest)
├── minix-rs/tests/
│   └── proptest_message.rs # IPC message roundtrip
├── ext4-core/tests/
│   ├── proptest_extent.rs  # Extent tree proptests
│   └── proptest_dir.rs     # Directory operation proptests
├── net-parse/tests/
│   └── proptest_packet.rs  # TCP/UDP header proptests
├── minix-bt-stack/tests/
│   └── proptest_sdp.rs     # SDP encoding proptests
└── fuzz/                   # Fuzz targets (nightly only)
    ├── fuzz_minixrs_message
    ├── fuzz_netparse_tcp
    ├── fuzz_netparse_udp
    ├── fuzz_netparse_dns
    ├── fuzz_audiobuf_ringpos
    └── fuzz_procfspath_pid
```

### C/C++ Catch2 Tests (`tests/`)

```
tests/
├── tcp/                    # TCP wire format (migrated from test91)
├── raw/                    # RAW ICMP/IPv6 (migrated from test92)
├── ipv6/                   # IPv6 address/DAD (migrated from test93)
├── bpf/                    # BPF filter/attach (migrated from test94)
├── blocktest/              # Block device I/O + partition tables
├── ds/                     # Data Store key-value semantics
├── rmibtest/               # RMIB sysctl node tree
├── safecopy/               # Grant-based safecopy
├── ext4_ffi/               # ext4 C FFI integration
├── driver_ffi/             # AHCI/e1000/virtio-net register tests
└── bt_ffi/                 # Bluetooth IPC + SDP encoding
```

## Adding a New Test

### Adding a Rust test

1. Add the test to the relevant crate:
   ```rust
   #[cfg(test)]
   mod tests {
       #[test]
       fn my_new_test() {
           assert_eq!(2 + 2, 4);
       }
   }
   ```

2. For proptests, use `TestRunner::run()` API (not `proptest!` macro):
   ```rust
   #[test]
   fn my_proptest() {
       let mut runner = TestRunner::new(Config::default());
       runner.run(&(any::<u32>(),), |(val,)| {
           // property assertions
           Ok(())
       }).unwrap();
   }
   ```

3. Run: `cargo test -p <crate> my_new_test`

### Adding a Catch2 test

1. Create a `.cpp` file in the appropriate `tests/<component>/` directory.
2. Include the Catch2 header and use TEST_CASE macros:
   ```cpp
   #include "catch.hpp"

   TEST_CASE("my new feature", "[component][tag]") {
       REQUIRE(some_condition);
   }
   ```

3. Add the test target to the CMakeLists.txt.
4. Run: `cd build && cmake .. && cmake --build . --target <test_name> && ./<test_name>`

## CI Pipeline

See `docs/ci-pipeline.md` for the full CI/CD architecture.

| CI Job | Trigger | Approx Time | 
|--------|---------|-------------|
| `rust-build` | push/PR | ~5 min |
| `rust-sanitizers` | push/PR | ~10 min |
| `rust-fuzz` | schedule | ~30 min |
| `rust-coverage` | schedule | ~10 min |
| `build` (legacy) | push/PR | ~20 min |
| `qemu-smoke` | schedule | ~5 min |
| `rust-benchmarks` | schedule | ~10 min |
| `c-benchmarks` | schedule | ~10 min |
| `c-coverage` | push/PR | ~10 min |
| `static-analysis` | push/PR | ~5 min |
| `security-scan` | push/PR | ~10 min |

## Test Metrics Dashboard

The test dashboard is available at `dashboard/index.html` (generated by `scripts/generate_dashboard.sh`).

```bash
# Generate dashboard from CI artifacts
./scripts/generate_dashboard.sh --artifact-dir ci-artifacts/ --output-dir dashboard/

# Deploy to gh-pages
./scripts/generate_dashboard.sh --gh-pages /path/to/gh-pages/repo/
```

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `cargo test` fails with linker error | Run `cargo clean` then `cargo test` |
| Catch2 tests not found | Check `MK_CATCH2=ON` in CMake configure |
| QEMU test timeout | Increase `--timeout` or check host resources |
| Benchmark baseline missing | Run benchmarks once to create baseline |
| gcov/lcov not found | `sudo apt install lcov` |
