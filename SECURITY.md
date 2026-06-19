# Security Policy

## Supported Versions

GergiOS is under active development and does not yet have a stable release.
During the development phase, security updates are provided for the latest
commit on the `main` branch.

| Version | Supported |
|---------|-----------|
| main (unstable) | ✅ Active development |
| < 1.0 | ❌ Pre-release |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue
in GergiOS, please report it privately before disclosing it publicly.

**Do NOT report security vulnerabilities via public GitHub issues.**

Instead, send a detailed report to **gergios@proton.me**.

### What to include

- Type of vulnerability (e.g., buffer overflow, privilege escalation, …)
- Affected component(s) and version(s)
- Steps to reproduce
- Potential impact
- Suggested fix (if known)

### What to expect

- **Acknowledgment** within 48 hours of your report
- **Assessment** within 5 business days (validity, severity, impact)
- **Fix timeline** — we will work on a fix based on severity
- **Disclosure** — once a fix is released, we will coordinate public disclosure

## Security Model

GergiOS inherits the MINIX 3 microkernel security model:

- **Minimal TCB** — the kernel is small (~8,000 LOC), most services run in user space
- **Isolated drivers** — device drivers are separate processes, protected by hardware
- **IPC-based** — all communication between components goes through message passing
- **Capability-aware** — capability-based security is being designed

### Current Limitations (pre-1.0)

- No mandatory access control (MAC) yet — planned for 1.1
- No formal verification — planned for future releases
- Limited ASLR — under development
- No signed updates — planned for 1.1+

## Security-Related Configuration

See `planning/03_migration_roadmap.md` §6 for the security roadmap.
Key security features planned:

- Capability-based access control (1.0 design, 1.1 implementation)
- MAC framework (SELinux/AppArmor equivalent)
- Memory-safe IPC via Rust validation layer
- Driver sandboxing improvements
- Boot security (signed kernel, measured boot)

## Known Security Issues

Tracked via [GitHub Issues](https://github.com/gergios/gergios/issues) with the `security` label.

## Third-Party Components

| Component | Version | Security Notes |
|-----------|---------|----------------|
| wolfSSL | 5.9.1 | Actively maintained, FIPS-ready |
| MINIX kernel | 3.4.0 | Pre-release security hardening |
| NetBSD libc | ~2015 | Legacy — being modernized |
| NetSurf | latest | GPLv2, custom rendering engine |

---

Last updated: 2026-06-19
