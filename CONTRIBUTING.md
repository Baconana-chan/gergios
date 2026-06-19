# Contributing to GergiOS

Thank you for your interest in contributing to GergiOS! This document provides guidelines and instructions for contributing.

**Table of Contents:**
- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Contribution Areas](#contribution-areas)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Licensing](#licensing)
- [Questions?](#questions)

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to **gergios@proton.me**.

## Getting Started

### 1. Understand the Architecture

GergiOS is a microkernel OS with a layered architecture:

- **MINIX microkernel** — kernel, servers, drivers, filesystems, network stack
- **NetBSD userland** — POSIX (BSD) ABI layer: libc, libm, system headers, tools
- **GergiOS-native layer** — Rust utilities, Wayland GUI, new userland components

Start by reading:
- [ROADMAP.md](ROADMAP.md) — project direction and release plans
- `planning/10_netbsd_dependency_audit.md` — compatibility strategy (macOS model)
- `planning/03_migration_roadmap.md` — component-by-component modernization plan

### 2. Pick an Area

Areas where contributions are especially welcome:

| Area | Skill Set | Difficulty |
|------|-----------|------------|
| **Rust utilities** | Rust, POSIX APIs | 🟢 Easy |
| **Build system** | CMake, BSD Make | 🟡 Medium |
| **Kernel/drivers** | C, systems programming | 🔴 Hard |
| **Filesystems** | C, ext4 | 🔴 Hard |
| **Documentation** | Technical writing | 🟢 Easy |
| **Testing** | QEMU, shell scripting | 🟢 Easy |
| **GUI/Wayland** | Wayland, compositors | 🔴 Hard |

### 3. Find an Issue

Check the [issues tracker](https://github.com/gergios/gergios/issues) for:
- `good first issue` — small, well-scoped tasks
- `help wanted` — contributions needed
- `Rust` — porting tools from C to Rust

## Development Setup

### Requirements

- A Unix-like build environment (Linux recommended)
- CMake ≥ 3.20
- LLVM/Clang toolchain
- Rust toolchain (for Rust components)
- QEMU (for testing)
- NASM (for x86_64 assembly)

### Building

```bash
# CMake build (recommended for GergiOS components)
mkdir build && cd build
cmake .. -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-minix.cmake
ninja

# BSD Make build (for NetBSD compat layer)
./build.sh build

# Rust utilities
cd rust && cargo build --workspace
```

### Testing

```bash
# Run in QEMU
qemu-system-x86_64 -m 1G -cdrom gergios.iso

# Run Rust tests
cd rust && cargo test --workspace
```

## Contribution Areas

### Rust Ports

The most accessible way to contribute is porting userland tools from C to Rust.
Check `planning/10` §6 for the current audit. Simple tools to port:

```rust
// Example: rewriting tools in Rust
// Pattern: rust/<tool>/ + Cargo.toml + src/main.rs + Makefile
```

See existing tools in `rust/` for reference (`cat`, `ls`, `cp`, etc.).

### Crypto / wolfSSL

- Porting more components to use wolfSSL instead of legacy crypto
- Writing tests for the hcrypto layer

### Kernel & Drivers

- MINIX kernel development (C, systems programming)
- USB stack porting
- Driver framework design

### Wayland / GUI

- Wayland compositor for microkernel architecture
- NetSurf integration as WebView
- Framebuffer and input drivers

## Pull Request Process

1. **Fork** the repository and create your branch from `main`
2. **Discuss** significant changes via an issue first
3. **Follow** the coding standards below
4. **Test** your changes — ensure the build passes
5. **Write** commit messages in English, following [Conventional Commits](https://www.conventionalcommits.org/)
6. **Submit** a pull request with a clear description of changes

### Commit Message Format

```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`, `rust`
Scope: the component (e.g., `cat`, `kernel`, `build`, `crypto`)

Examples:
```
rust(cat): implement cat utility in Rust
fix(kernel): correct page table entry for DMA
docs(roadmap): add NetSurf WebView integration
```

## Coding Standards

### C

- Follow the existing style in the file you're modifying
- C17 (gnu17) with `-Wall -Wextra`
- No K&R-style function declarations
- Use `_Noreturn` for functions that don't return
- Use `_Static_assert` for compile-time assertions

### Rust

- Follow `rustfmt` conventions
- Use edition 2024
- Minimize `unsafe` — prefer safe abstractions
- Handle errors with `Result` or `Option` (avoid `unwrap()` where possible)
- Use `libc` crate for POSIX syscalls only when necessary

### Build System

- CMake for GergiOS-native components
- BSD Make for NetBSD compat layer
- Keep dual-build compatibility

## Licensing

All new contributions to this project will be licensed under **GPLv2 or later**.

By contributing, you agree that your contributions will be licensed under GPLv2+.
You must have the right to contribute the code under this license.

If you're contributing code originally written by someone else (e.g., porting
from another open-source project), ensure the license is compatible with GPLv2+.

## Questions?

- Open a [Discussion](https://github.com/gergios/gergios/discussions)
- Contact: **gergios@proton.me**
- Matrix: (coming soon)

Thank you for helping make GergiOS better! 🚀
