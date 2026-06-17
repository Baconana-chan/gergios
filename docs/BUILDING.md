# Building MINIX with wolfSSL

This document describes how to build MINIX with wolfSSL as the TLS/crypto
library instead of OpenSSL.

## Prerequisites

- MINIX source tree with wolfSSL integrated (`crypto/external/gpl2/wolfssl/`)
- Build tools (GCC, make, etc.)
- wolfSSL v5.9.1-stable source (already included in the tree)

## Quick Start

To build MINIX with wolfSSL:

```bash
# From the MINIX root directory
make build
```

The build system automatically detects `MKCRYPTO != "no"` and uses wolfSSL
for all migrated components (syslogd, ftp, httpd, telnet, passwd, factor, BIND).

## Configuration

### wolfSSL Configuration

wolfSSL is configured through `crypto/Makefile.wolfssl` and
`crypto/external/gpl2/wolfssl/config.h`. Key configuration options:

**OpenSSL compatibility layer** (required for all migrated components):
```makefile
CPPFLAGS+= -DOPENSSL_EXTRA
CPPFLAGS+= -DOPENSSL_EXTRA_X509_SMALL
```

**Cryptographic algorithms** (enabled for MINIX):
```makefile
CPPFLAGS+= -DHAVE_AESGCM -DHAVE_CHACHA -DHAVE_POLY1305
CPPFLAGS+= -DHAVE_ECC -DHAVE_CURVE25519 -DHAVE_ED25519
CPPFLAGS+= -DHAVE_DH -DHAVE_RSA -DHAVE_DSA
CPPFLAGS+= -DHAVE_SHA -DHAVE_SHA256 -DHAVE_SHA512 -DHAVE_MD5
```

**Disabled features** (for size reduction):
```makefile
CPPFLAGS+= -DNO_MD4 -DNO_RC4 -DNO_PSK -DNO_HC128 -DNO_RABBIT
CPPFLAGS+= -DNO_DES3 -DNO_DSA -DNO_DH -DNO_OLD_TLS
```

### Migrated Component Makefiles

Each migrated component's Makefile includes:
1. `-lwolfssl` for linking
2. `${LIBWOLFSSL}` for dependency tracking
3. wolfSSL include paths for header resolution

Example (syslogd):
```makefile
.if ${MKCRYPTO} != "no"
LDADD+=\t-lwolfssl
DPADD+=\t${LIBWOLFSSL}
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl
.else
CPPFLAGS+=-DDISABLE_TLS -DDISABLE_SIGN
.endif
```

## Migrating a New Component

To migrate a new component from OpenSSL to wolfSSL:

### 1. Replace Headers

```c
// Before
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/x509.h>

// After
#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/err.h>
#include <wolfssl/openssl/x509.h>
```

### 2. Update Makefile

```makefile
.if ${MKCRYPTO} != "no"
LDADD+= -lwolfssl
DPADD+= ${LIBWOLFSSL}
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl
.endif
```

### 3. Handle API Differences

See `docs/wolfssl-usage-guide.md` for a complete list of API mappings
and known differences.

## Testing

### Unit Tests

```bash
# Run wolfSSL migration unit tests
cd tests/crypto/libcrypto
atf-run t_wolfssl

# Run security tests
atf-run t_security

# Run performance benchmarks
atf-run t_perf

# Run compatibility tests
atf-run t_compat
```

### Integration Tests

```bash
# Run integration tests for all migrated components
cd tests/integration
atf-run t_syslogd_tls
atf-run t_ftp_ssl
atf-run t_httpd_ssl
atf-run t_telnet_encrypt
atf-run t_bind_dnssec
atf-run t_cross_component
```

## Cross-Compilation

wolfSSL supports cross-compilation for MINIX target hardware:

```bash
# Configure wolfSSL for cross-compilation
cd crypto/external/gpl2/wolfssl/dist
./configure --host=i386-pc-minix \
  --enable-opensslextra \
  --enable-opensslall \
  --enable-fastmath \
  --enable-smallstack

# Build MINIX with wolfSSL
cd ../../../../..
make build
```

## Troubleshooting

### Missing wolfSSL headers

If you see errors like `wolfssl/openssl/ssl.h: No such file or directory`,
ensure the wolfSSL include paths are correctly set in the component's Makefile:

```makefile
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl
```

### Linking errors (undefined symbols)

If you see undefined symbol errors for OpenSSL functions, verify:
1. The component links with `-lwolfssl`
2. `OPENSSL_EXTRA` is defined in the wolfSSL configuration
3. The function is available in wolfSSL's OpenSSL compatibility layer

### DH parameter errors

Some components (like syslogd) use `DH_set0_pqg()` instead of direct
`dh->p` / `dh->g` struct member access. wolfSSL does not expose DH
struct members as writable pointers. See `usr.sbin/syslogd/tls.c` for
an example of the correct pattern.

## References

- [wolfSSL Manual](https://www.wolfssl.com/docs/)
- [wolfSSL OpenSSL Compatibility](https://www.wolfssl.com/docs/openssl/)
- [wolfSSL GitHub](https://github.com/wolfSSL/wolfssl)
- [Migration Plan](planning/06_openssl_to_wolfssl_migration.md)
