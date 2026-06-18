# Benchmark Results: Rust vs C Utilities

**Date**: Thu Jun 18 22:47:15     2026
**Mode**: quick
**Comparison**: rust-only
**Hyperfine**: hyperfine 1.20.0
**Rust**: rustc 1.95.0 (59807616e 2026-04-14)
**Target**: host: x86_64-pc-windows-msvc

## Results

| Benchmark | Rust (s) | Std Dev |
|-----------|----------|---------|
| true-exit | 0.035371 | ±0.002076 |
| false-exit | 0.038919 | ±0.015078 |

## System
- **CPU**: 8 cores
- **RAM**: ?
- **Rustc**: rustc 1.95.0 (59807616e 2026-04-14)

## Reproduce
```bash
cd rust && cargo build --release && cd ..
bash scripts/run_benchmarks.sh
```
