# Documentation Index

This directory contains all project documentation. This index provides a quick reference to all available documents.

## i386 Deprecation

Documents related to the i386 (32-bit x86) architecture deprecation.

| Document | Description |
|----------|-------------|
| [Deprecation Announcement](i386-deprecation-announcement.md) | Official deprecation announcement and timeline |
| [Migration FAQ](i386-deprecation-faq.md) | Frequently asked questions about the deprecation |
| [Troubleshooting Guide](i386-migration-troubleshooting.md) | Common migration issues and solutions |
| [Codebase Audit](i386-codebase-audit.md) | Assessment of i386 dependencies across the codebase |
| [Migration Support Channels](migration-support-channels.md) | Support resources and migration checklist |

## wolfSSL Migration

Documents related to the OpenSSL 0.9.8 → wolfSSL 5.9.1 migration.

| Document | Phase | Description |
|----------|-------|-------------|
| [Migration Plan](../planning/06_openssl_to_wolfssl_migration.md) | All | Full migration plan with status and details |
| [Build Instructions](BUILDING.md) | 5.2 | How to build MINIX with wolfSSL |
| [API Usage Guide](wolfssl-usage-guide.md) | 5.2 | OpenSSL → wolfSSL API mappings and known differences |
| [Configuration Reference](wolfssl-configuration-reference.md) | 5.3 | All config.h defines explained |
| [Testing Guide](wolfssl-testing-guide.md) | 5.3 | How to run, write, and debug wolfSSL tests |
| [Security Audit](wolfssl-security-audit.md) | 4.3 | CVE comparison, protocol/cipher analysis |
| [Performance Report](wolfssl-performance-report.md) | 4.4 | Benchmark results and expected ranges |
| [Compatibility Report](wolfssl-compatibility-report.md) | 4.5 | API compatibility matrix and known limitations |

## Build System

| Document | Description |
|----------|-------------|
| [CMake Migration Guide](cmake-migration-guide.md) | BSD Make → CMake migration details |
| [Dual Build Guide](dual-build-guide.md) | Transition guide for dual build systems |
| [Infrastructure Setup](INFRASTRUCTURE_SETUP.md) | Development environment setup |

## External References

| Resource | Link |
|----------|------|
| wolfSSL Package | `crypto/external/gpl2/wolfssl/README` |
| Compatibility Layer | `crypto/external/gpl2/wolfssl/COMPATIBILITY.md` |
| wolfSSL Manual | https://www.wolfssl.com/docs/ |
| wolfSSL GitHub | https://github.com/wolfSSL/wolfssl |
| OpenSSL Compatibility | https://www.wolfssl.com/docs/openssl/ |

## Migrated Components (wolfSSL)

| Component | Location | Status |
|-----------|----------|--------|
| syslogd | `usr.sbin/syslogd/` | ✅ Complete |
| ftp | `usr.bin/ftp/` | ✅ Complete |
| httpd | `libexec/httpd/` | ✅ Complete |
| telnet | `lib/libtelnet/`, `usr.bin/telnet/`, `libexec/telnetd/` | ✅ Complete |
| passwd | `usr.bin/passwd/` | ✅ Complete |
| factor | `games/factor/` | ✅ Complete |
| BIND | `external/bsd/bind/` | ✅ Complete |

## Still Using OpenSSL (not yet migrated)

heimdal, netpgp, libsaslc, libevent, fetch, tcpdump, dhcp, pkg_install
