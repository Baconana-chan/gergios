# wolfSSL Configuration Reference for MINIX

This document provides a detailed reference for every configuration option
used in the MINIX wolfSSL integration.

## Overview

wolfSSL configuration is split across two files:

| File | Purpose |
|------|---------|
| `crypto/external/gpl2/wolfssl/config.h` | Compile-time feature defines (C preprocessor) |
| `crypto/Makefile.wolfssl` | Build system integration flags (Makefile `CPPFLAGS+=`) |

Both files must remain consistent — `config.h` is the authoritative source,
and `Makefile.wolfssl` mirrors the relevant flags for build system visibility.

---

## 1. OpenSSL Compatibility Layer

These are **required** for all migrated MINIX components. Without them,
code using OpenSSL API calls will not compile.

| Define | File | Purpose |
|--------|------|---------|
| `OPENSSL_EXTRA` | Both | Enables wolfSSL's OpenSSL compatibility layer. Provides macro mappings for SSL_CTX_new, SSL_new, SSL_read, SSL_write, etc. |
| `OPENSSL_EXTRA_X509_SMALL` | Both | Enables X509 certificate API with smaller memory footprint. Provides X509_get_subject_name, X509_get_issuer_name, X509_digest, etc. |
| `OPENSSL_ALL` | config.h | Enables all available OpenSSL compatibility features including EVP, BIO, PKCS7, PKCS12. Used by BIND for full API coverage. |
| `WOLFSSL_OPENSSL_COMPATIBLE` | config.h | Sets wolfSSL to use OpenSSL-compatible error codes and behavior. |

**Dependencies**: All other feature groups depend on `OPENSSL_EXTRA`.

**Migration impact**: Without these, all 7 migrated components (syslogd, ftp,
httpd, telnet, passwd, factor, BIND) would fail to compile.

---

## 2. Cryptographic Algorithms

### Symmetric Ciphers

| Define | File | Used By | Notes |
|--------|------|---------|-------|
| `HAVE_AESGCM` | Both | syslogd, httpd, BIND | AES-GCM authenticated encryption. Required for TLS 1.2. |
| `HAVE_CHACHA` | Both | syslogd, httpd | ChaCha20 stream cipher. Used with Poly1305 for ChaCha20-Poly1305. |
| `HAVE_POLY1305` | Both | syslogd, httpd | Poly1305 MAC. Used with ChaCha20 for modern AEAD cipher suite. |
| `NO_DES3` | Both | — | **Disabled** — 3DES is weak (Sweet32 attack). MINIX explicitly disables it. |
| `NO_RC4` | Both | — | **Disabled** — RC4 is broken. MINIX explicitly disables it. |

### Asymmetric Ciphers

| Define | File | Used By | Notes |
|--------|------|---------|-------|
| `HAVE_RSA` | Both | syslogd, httpd, BIND | RSA sign/verify/encrypt. Required for X.509 certificates. |
| `HAVE_DH` | Both | syslogd (tls.c) | Diffie-Hellman key exchange. Used for DHE cipher suites in syslogd. |
| `HAVE_DSA` | Both | BIND | Digital Signature Algorithm. Used by BIND for DNSSEC. |
| `HAVE_ECC` | Both | BIND, httpd | Elliptic Curve Cryptography. Required for ECDHE and ECDSA. |
| `HAVE_CURVE25519` | Both | — | Curve25519 (X25519) key exchange. Modern, fast alternative to ECDHE with P-256. |
| `HAVE_ED25519` | Both | — | Ed25519 signatures. Modern, fast alternative to ECDSA. |

### Hash Functions

| Define | File | Used By | Notes |
|--------|------|---------|-------|
| `HAVE_SHA` | Both | All | SHA-1 (160-bit). Still needed for legacy certificate support. |
| `HAVE_SHA256` | Both | All | SHA-256 (256-bit). Required for TLS 1.2 and modern certificates. |
| `HAVE_SHA512` | Both | BIND | SHA-512 (512-bit). Used by BIND for DNSSEC. |
| `HAVE_MD5` | Both | BIND | MD5 (128-bit). Still used by BIND for some DNSSEC operations. |
| `NO_MD4` | Both | — | **Disabled** — MD4 is cryptographically broken. Not used by any component. |
| `HAVE_BLAKE2B` | config.h | — | BLAKE2b hash. Optional, used for post-quantum compatibility. |
| `HAVE_BLAKE2S` | config.h | — | BLAKE2s hash. Optional, smaller variant of BLAKE2. |
| `HAVE_SHA3` | config.h | — | SHA-3. Optional, modern hash standard. |

### Key Derivation

| Define | File | Used By | Notes |
|--------|------|---------|-------|
| `HAVE_HMAC` | config.h | BIND | HMAC (Hash-based Message Authentication Code). Used by BIND ISC headers. |
| `HAVE_KDF` | config.h | — | Key Derivation Function framework. |
| `HAVE_HKDF` | config.h | — | HMAC-based Key Derivation Function. Required for TLS 1.3. |
| `HAVE_HPKE` | config.h | — | Hybrid Public Key Encryption. |

---

## 3. TLS/DTLS Protocol Support

| Define | File | Purpose |
|--------|------|---------|
| `WOLFSSL_TLS13` | config.h | Enables TLS 1.3 support. Provides modern handshake, 0-RTT, and AEAD-only ciphers. |
| `WOLFSSL_DTLS` | config.h | Enables DTLS (Datagram TLS) for UDP-based TLS. |
| `WOLFSSL_DTLS13` | config.h | Enables DTLS 1.3 for UDP with TLS 1.3 features. |
| `WOLFSSL_NO_OLD_TLS` | Both | Disables TLS 1.0 and 1.1. MINIX only supports TLS 1.2+. |

### Disabled Protocol Versions

| Define | File | Purpose |
|--------|------|---------|
| `NO_SSLV2` | config.h | **Disabled** — SSLv2 is completely broken. Not supported by any client. |
| `NO_SSLV3` | config.h | **Disabled** — SSLv3 is vulnerable to POODLE attack. |
| `NO_TLSV1` | config.h | **Disabled** — TLS 1.0 is deprecated (BEAST, POODLE-TLS). |
| `NO_TLSV1_1` | config.h | **Disabled** — TLS 1.1 is deprecated. |
| `NO_OLD_TLS` | Both | Disables all TLS versions before 1.2. |

### Why old protocol versions are disabled

| Vulnerability | Affected Version | CVE | Impact |
|--------------|-----------------|-----|--------|
| POODLE | SSLv3 | CVE-2014-3566 | Plaintext recovery |
| BEAST | TLS 1.0 | CVE-2011-3389 | plaintext recovery |
| Lucky 13 | TLS 1.0/1.1 | CVE-2013-0169 | Padding oracle |
| RC4 biases | TLS 1.0/1.1 | Multiple | Plaintext recovery |

---

## 4. Performance Optimizations

| Define | File | Purpose | Trade-off |
|--------|------|---------|-----------|
| `FAST_MATH` | Both | Uses assembly-optimized big-number arithmetic. 2-5x faster RSA/DH/ECC operations. | Increases code size by ~20KB. |
| `SMALL_STACK` | Both | Minimizes per-operation stack usage. Important for embedded systems with limited stack. | Slightly slower for some operations. |
| `TFM_TIMING_RESISTANT` | config.h | Constant-time big-number operations to prevent timing side-channel attacks. | Slightly slower math. |
| `ECC_TIMING_RESISTANT` | config.h | Constant-time ECC operations to prevent timing attacks on ECDSA/ECDHE. | Slightly slower ECC. |
| `SINGLE_THREADED` | config.h | Disables internal locking. Safe because MINIX typically runs each service in its own process. | **Removed from config.h** — was causing multi-core bug. |

### Performance Impact Estimates

| Optimization | RSA-2048 sign | ECDSA P-256 sign | DH-2048 keygen |
|-------------|--------------|------------------|----------------|
| Default | ~300 ops/s | ~5000 ops/s | ~300 ops/s |
| +FAST_MATH | ~600 ops/s | ~8000 ops/s | ~500 ops/s |
| +SMALL_STACK | ~550 ops/s | ~7500 ops/s | ~450 ops/s |
| +Timing resistant | ~500 ops/s | ~7000 ops/s | ~400 ops/s |

---

## 5. Memory Management

| Define | File | Purpose |
|--------|------|---------|
| `WOLFSSL_MALLOC` | config.h | Use wolfSSL's internal malloc wrapper for tracking. |
| `WOLFSSL_FREE` | config.h | Use wolfSSL's internal free wrapper for tracking. |
| `WOLFSSL_CALLOC` | config.h | Use wolfSSL's internal calloc wrapper for tracking. |
| `WOLFSSL_REALLOC` | config.h | Use wolfSSL's internal realloc wrapper for tracking. |
| `WOLFSSL_STATIC_MEMORY` | config.h | Use static memory pools instead of heap allocation. Reduces fragmentation. |

### Memory Footprint

| Configuration | Library Size | Per-SSL Connection |
|--------------|-------------|-------------------|
| Minimal (no features) | ~60 KB | ~12 KB |
| MINIX config (all features) | ~300 KB | ~30 KB |
| OpenSSL 0.9.8 equivalent | ~2 MB | ~60 KB |

---

## 6. Certificate Handling

| Define | File | Purpose | Used By |
|--------|------|---------|---------|
| `WOLFSSL_CERT_GEN` | config.h | Certificate generation (X.509 signing). | syslogd (write_x509files) |
| `WOLFSSL_CERT_REQ` | config.h | Certificate Signing Request (CSR) support. | — |
| `WOLFSSL_CERT_EXT` | config.h | X.509 v3 certificate extension support. | syslogd (SAN, key usage) |
| `WOLFSSL_CERTIFICATE_PARSING` | config.h | Parse X.509 certificates from DER/PEM. | All components |
| `WOLFSSL_KEY_GEN` | config.h | Cryptographic key generation (RSA, DSA, EC). | syslogd, BIND |
| `HAVE_X509` | config.h | X.509 certificate object support. | All components |
| `HAVE_X509_EXT` | config.h | X.509 extension parsing. | syslogd |
| `HAVE_X509_VERIFY` | config.h | Certificate chain verification. | syslogd, httpd |
| `HAVE_OCSP` | config.h | Online Certificate Status Protocol (real-time revocation). | All |
| `HAVE_CRL` | config.h | Certificate Revocation Lists. | — |
| `HAVE_CRL_MONITOR` | config.h | CRL file monitoring for auto-updates. | — |
| `SESSION_CERTS` | config.h | Store peer certificates in session. | All |
| `SESSION_INDEX` | config.h | Session indexing for faster lookups. | — |
| `HAVE_SESSION_TICKET` | config.h | TLS session ticket support (RFC 5077). | All |
| `HAVE_CERT_COMPRESSION` | config.h | Certificate compression (RFC 8879). | — |

---

## 7. I/O and Transport

| Define | File | Purpose |
|--------|------|---------|
| `WOLFSSL_DTLS` | config.h | DTLS support for UDP-based TLS. |
| `WOLFSSL_DTLS13` | config.h | DTLS 1.3 support. |
| `WOLFSSL_IO` | config.h | I/O callbacks for custom transport. |
| `WOLFSSL_NTP` | config.h | NTP-related I/O support. |
| `HAVE_BIO` | config.h | BIO abstraction layer (memory, file, socket). Used by PEM loading. |
| `WOLFSSL_NO_FILESYSTEM` | config.h | **Disables** file system operations. Relevant for MINIX's embedded use. |
| `NO_DEV_RANDOM` | config.h | **Disables** /dev/random dependency. MINIX uses its own entropy source. |
| `NO_FILESYSTEM` | config.h | **Disables** all file system access in crypto operations. |

### File System Constraints

Because `WOLFSSL_NO_FILESYSTEM` and `NO_FILESYSTEM` are defined:
- Certificate loading via `SSL_CTX_use_certificate_chain_file` reads from memory buffers, not disk
- OCSP responses must be provided in-memory, not from files
- CRL files are not automatically loaded from disk
- Random seeding does not use `/dev/random` or `/dev/urandom`

---

## 8. MINIX-Specific Defines

| Define | File | Purpose |
|--------|------|---------|
| `WOLFSSL_MINIX` | Both | Custom identifier for MINIX platform. Used for conditional compilation. |
| `WOLFSSL_NO_SCTP` | Both | **Disables** SCTP transport support (equivalent to OpenSSL's `OPENSSL_NO_SCTP`). |
| `WOLFSSL_SMALL_STACK` | Both | Reduces stack usage. Critical for MINIX's limited kernel stack. |
| `WOLFSSL_NO_OLD_TLS` | Both | Disables TLS 1.0/1.1. Only TLS 1.2+ available. |

---

## 9. Post-Quantum Cryptography (Optional)

| Define | File | Purpose | Status |
|--------|------|---------|--------|
| `HAVE_PQC` | config.h | Post-Quantum Cryptography framework. | Optional |
| `HAVE_KYBER` | config.h | ML-KEM (formerly Kyber) key encapsulation. | Optional |
| `HAVE_DILITHIUM` | config.h | ML-DSA (formerly Dilithium) digital signatures. | Optional |

These are **optional** and can be disabled to reduce library size if not needed.
Currently not required by any migrated MINIX component, but useful for
future-proofing.

---

## 10. BIND-Specific Configuration

When building BIND with `WOLFSSL_BIND`, the following OpenSSL version
compatibility settings apply:

| Define | Value | Effect |
|--------|-------|--------|
| `OPENSSL_VERSION_NUMBER` | `0x10100003L` | Reports as OpenSSL 1.1.0c. BIND version checks resolve to: `> 0x00908000L` ✅, `< 0x10100000L` ❌ (enters legacy paths). |
| `WOLFSSL_BIND` | defined | Sets `OPENSSL_VERSION_NUMBER` to 1.1.0-compatible value. |
| `USE_ENGINE` | 0 | ENGINE API **disabled** — BIND's USE_ENGINE code paths are compiled out. |

### Known Configuration Conflict

⚠️ **`config.h` has conflicting defines**:

| Enabled | Disabled | Effect |
|---------|----------|--------|
| `HAVE_DH` | `NO_DH` | DH **disabled** — `NO_*` takes precedence |
| `HAVE_DSA` | `NO_DSA` | DSA **disabled** — `NO_*` takes precedence |

wolfSSL's `NO_*` defines take precedence over `HAVE_*` defines. This means:
- **BIND**: DSA and DH key operations will fail at runtime despite `HAVE_DSA`/`HAVE_DH` being defined
- **syslogd**: DH parameter generation in `tls.c` (`get_dh1024()`) will fail
- **Resolution**: Remove `NO_DH` and `NO_DSA` from config.h if BIND or syslogd DH/DSA functionality is required

---

## 11. Quick Configuration Reference

### Minimum Configuration (for smallest library size)

```c
#define OPENSSL_EXTRA              /* Required for compat layer */
#define HAVE_SHA256                /* Required for TLS */
#define HAVE_RSA                   /* Required for certs */
#define NO_MD4                     /* Disable unused */
#define NO_RC4                     /* Disable weak */
#define NO_PSK                     /* Disable unused */
#define NO_HC128                   /* Disable unused */
#define NO_RABBIT                  /* Disable unused */
#define NO_DES3                    /* Disable weak */
#define FAST_MATH                  /* Performance */
#define SMALL_STACK                /* Embedded-friendly */
```

### Full MINIX Configuration (all migrated components)

See `crypto/external/gpl2/wolfssl/config.h` for the complete configuration
used in MINIX.

---

## 12. Configuration Validation

To verify that the configuration is correct:

```bash
# Check that OPENSSL_EXTRA is enabled
grep -r "OPENSSL_EXTRA" crypto/external/gpl2/wolfssl/config.h

# Check that weak protocols are disabled
grep "NO_SSLV" crypto/external/gpl2/wolfssl/config.h

# Check that required algorithms are enabled
for algo in AESGCM CHACHA POLY1305 ECC RSA SHA256; do
    grep "HAVE_$algo" crypto/external/gpl2/wolfssl/config.h
done
```

### Common Configuration Mistakes

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| `SSL_library_init` undefined | `OPENSSL_EXTRA` not defined | Add `-DOPENSSL_EXTRA` to CPPFLAGS |
| `X509_get_subject_name` undefined | `OPENSSL_EXTRA_X509_SMALL` not defined | Add to CPPFLAGS |
| `EVP_sha256` returns NULL | SHA-256 not enabled | Add `HAVE_SHA256` |
| TLS 1.3 handshake fails | `WOLFSSL_TLS13` not defined | Add to config.h |
| DH key exchange fails | `NO_DH` is defined | Remove `NO_DH` |
| Session resumption fails | `HAVE_SESSION_TICKET` not defined | Add to config.h |

---

## References

- [wolfSSL Build Instructions](BUILDING.md)
- [wolfSSL API Usage Guide](wolfssl-usage-guide.md)
- [wolfSSL Security Audit](wolfssl-security-audit.md)
- [wolfSSL Performance Report](wolfssl-performance-report.md)
- [wolfSSL Compatibility Report](wolfssl-compatibility-report.md)
- [Migration Plan](planning/06_openssl_to_wolfssl_migration.md)
- [wolfSSL Documentation](https://www.wolfssl.com/docs/)
- [wolfSSL GitHub](https://github.com/wolfSSL/wolfssl)
