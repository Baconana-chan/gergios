# wolfSSL Migration Documentation Index

This directory contains all documentation related to the OpenSSL 0.9.8 →
wolfSSL 5.9.1 migration in MINIX.

## Quick Reference

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

## External References

| Resource | Link |
|----------|------|
| wolfSSL Package | `crypto/external/gpl2/wolfssl/README` |
| Compatibility Layer | `crypto/external/gpl2/wolfssl/COMPATIBILITY.md` |
| wolfSSL Manual | https://www.wolfssl.com/docs/ |
| wolfSSL GitHub | https://github.com/wolfSSL/wolfssl |
| OpenSSL Compatibility | https://www.wolfssl.com/docs/openssl/ |

## Migrated Components

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
