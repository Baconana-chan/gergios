# GergiOS

> **A modern microkernel operating system — built on MINIX 3, reimagined for the future.**

[![License: GPL v2+](https://img.shields.io/badge/License-GPLv2%2B-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
![Status](https://img.shields.io/badge/Status-Development-green)
![Architecture](https://img.shields.io/badge/Arch-x86__64%20%7C%20ARM64-lightgrey)

---

## What is GergiOS?

GergiOS is an open-source, microkernel-based operating system forked from **MINIX 3**. The project's mission is to modernize the MINIX codebase and evolve it into a practical, secure, general-purpose OS while preserving its microkernel architecture.

**MINIX** remains the kernel core — like Linux in Android, or XNU in macOS.
**GergiOS** is the product: the OS that users and developers interact with.

### Architecture

```
┌───────────────────────────────────────┐
│        GergiOS Native Applications    │
│  (Rust tooling, Wayland GUI, …)       │
├───────────────────────────────────────┤
│     POSIX (BSD) Userland / NetBSD ABI │
│  ┌─────────┬──────────┬─────────────┐ │
│  │ libc    │  userland│  build sys   │ │
│  │ libm    │  tools   │  (BSD Make)  │ │
│  │ sys/*.h │  (bin/,  │              │ │
│  │         │  usr.bin)│              │ │
│  └─────────┴──────────┴─────────────┘ │
├───────────────────────────────────────┤
│    MINIX Microkernel (kernel,         │
│     servers, drivers, fs, net)        │
└───────────────────────────────────────┘
          ↑ NetBSD syscall ABI
     (fork, exec, signals, IPC, …)
```

## Key Features

- **Microkernel architecture** — MINIX kernel in user space: servers, drivers, FS, network each run as isolated processes
- **x86_64 support** — fully migrated; legacy i386 removed
- **Modern crypto** — wolfSSL 5.9.1 replacing OpenSSL 0.9.8
- **Dual build system** — CMake for native components, BSD Make for NetBSD compat layer
- **Rust ecosystem** — growing set of userland tools rewritten in Rust (grep, cat, ls, cp, …)
- **C17 language standard** — modernized from C89
- **Wayland graphics stack** — in development
- **NetBSD ABI compatibility** — POSIX (BSD) userland, like macOS

## Project Status

GergiOS is in **active development**. The 1.0 "Aurora" release (target: Q3 2026) focuses on:

- ✅ Build system modernization (CMake)
- ✅ Crypto migration (OpenSSL → wolfSSL)
- ✅ x86_64 & i386 removal
- ✅ C17 + Rust infrastructure
- ✅ GergiOS branding
- ✅ pkgsrc integration foundations
- 🟡 ext4 filesystem foundation
- 🟡 Wayland / GUI stack (display server + NetSurf WebView)
- 🟡 Driver framework, USB, security design
- 🟡 IPv6, network stack evaluation
- 🟡 UEFI bootloader

See [ROADMAP.md](ROADMAP.md) for the full plan and [planning/](planning/) for detailed documents.

## Quick Start

### Building

```bash
# CMake build (recommended)
mkdir build && cd build
cmake .. -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-minix.cmake
ninja

# BSD Make build (legacy — for NetBSD compat layer)
./build.sh build
```

### Running

GergiOS currently runs on x86_64 hardware or QEMU:

```bash
# QEMU (requires a built disk image)
qemu-system-x86_64 -m 1G -cdrom gergios.iso
```

### Rust Utilities

```bash
cd rust
cargo build --workspace
```

Available Rust tools (34 and growing): `cat`, `chmod`, `cp`, `echo`, `grep`, `head`, `hostname`, `id`, `kill`, `ln`, `ls`, `mkdir`, `mv`, `printf`, `pwd`, `rm`, `rmdir`, `seq`, `sleep`, `sort`, `sync`, `tail`, `tee`, `touch`, `tr`, `tty`, `uname`, `uniq`, `wc`, `yes`, `basename`, `dirname`, `false`, `printenv`, `true` …

## Repository Structure

| Path | Description |
|------|-------------|
| `minix/` | MINIX microkernel (kernel, servers, drivers, FS, net) |
| `lib/` | NetBSD libraries (libc, libm, …) |
| `bin/`, `sbin/`, `usr.bin/` | NetBSD userland (C, BSD Make) |
| `rust/` | Rust workspace (native tools and libraries) |
| `crypto/` | wolfSSL, libhcrypto |
| `cmake/` | CMake build system |
| `share/mk/` | BSD Make infrastructure |
| `planning/` | Internal planning documents (in Russian) |
| `sys/` | Kernel headers and boot library |
| `external/` | Third-party packages (LLVM, GCC, …) |
| `etc/` | System configuration files |

## License

GergiOS is licensed under **GPLv2 or later**.

The original MINIX code from Vrije Universiteit was BSD-licensed. All newly written and modernized code is contributed under GPLv2+. See the [LICENSE](LICENSE) file for details.

## Community

- **Chat**: [Matrix](https://matrix.to/#/#gergios:matrix.org) (coming soon)
- **Issues**: [GitHub Issues](https://github.com/gergios/gergios/issues)
- **Discussions**: [GitHub Discussions](https://github.com/gergios/gergios/discussions)

Before contributing, please read [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [SECURITY.md](SECURITY.md).

## Related Projects

- [MINIX 3](https://www.minix3.org) — The original microkernel OS
- [NetSurf](https://www.netsurf-browser.org) — Lightweight browser engine (GPLv2)
- [wolfSSL](https://www.wolfssl.com) — Embedded TLS/crypto library
- [visurf](https://drewdevault.com/blog/visurf-announcement/) — Wayland-native NetSurf frontend
