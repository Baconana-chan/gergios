# wolfSSL Usage Guide for MINIX Developers

## Introduction

wolfSSL is an embedded SSL/TLS library that replaces OpenSSL 0.9.8 in
MINIX. It provides an OpenSSL compatibility layer (`wolfssl/openssl/*.h`)
that allows most existing OpenSSL code to compile with minimal changes.

This guide covers:
- Common API mappings between OpenSSL and wolfSSL
- Patterns used in migrated MINIX components
- Known differences and workarounds
- Best practices for new code

## API Compatibility

### SSL/TLS Initialization

**OpenSSL:**
```c
SSL_library_init();
SSL_load_error_strings();
OpenSSL_add_all_digests();
```

**wolfSSL:**
```c
SSL_library_init();           /* Same API via compat layer */
SSL_load_error_strings();     /* Same API via compat layer */
/* OpenSSL_add_all_digests()  — NO-OP, digests registered by default */
```

Used by: `syslogd`, `ftp`, `httpd`, `BIND`

### SSL Context Creation

**OpenSSL:**
```c
const SSL_METHOD *meth = SSLv23_method();
SSL_CTX *ctx = SSL_CTX_new(meth);
```

**wolfSSL:** (identical API via compat layer)
```c
const SSL_METHOD *meth = SSLv23_method();         /* Fully supported */
const SSL_METHOD *meth = SSLv23_client_method();   /* Fully supported */
const SSL_METHOD *meth = SSLv23_server_method();   /* Fully supported */
SSL_CTX *ctx = SSL_CTX_new(meth);
```

Used by: `syslogd`, `ftp`, `httpd`, `BIND`

### SSL Connection

**OpenSSL:**
```c
SSL *ssl = SSL_new(ctx);
SSL_set_fd(ssl, fd);
SSL_connect(ssl);    /* Client */
SSL_accept(ssl);     /* Server */
SSL_read(ssl, buf, len);
SSL_write(ssl, buf, len);
SSL_shutdown(ssl);
SSL_free(ssl);
SSL_CTX_free(ctx);
```

**wolfSSL:** (identical API via compat layer)

Used by: `ftp` (client), `httpd` (server)

### Certificate Loading

**OpenSSL:**
```c
SSL_CTX_use_certificate_chain_file(ctx, "cert.pem");
SSL_CTX_use_PrivateKey_file(ctx, "key.pem", SSL_FILETYPE_PEM);
SSL_CTX_check_private_key(ctx);
```

**wolfSSL:** (identical API via compat layer)

Used by: `httpd` (`ssl-bozo.c`)

### EVP Digest Operations

**OpenSSL:**
```c
EVP_MD_CTX *mdctx = EVP_MD_CTX_new();
EVP_DigestInit_ex(mdctx, EVP_sha256(), NULL);
EVP_DigestUpdate(mdctx, data, len);
EVP_DigestFinal_ex(mdctx, digest, &digest_len);
EVP_MD_CTX_free(mdctx);
```

**wolfSSL:**
```c
/* Note: Use EVP_MD_CTX_new()/_free() instead of _create()/_destroy() */
EVP_MD_CTX *mdctx = EVP_MD_CTX_new();        /* wolfSSL canonical */
EVP_MD_CTX *mdctx = EVP_MD_CTX_create();     /* Also available */
```

Used by: `syslogd` (`sign.c`), `BIND`

### BIGNUM Operations

**OpenSSL:**
```c
BIGNUM *bn = BN_new();
BN_CTX *ctx = BN_CTX_new();
BN_set_word(bn, 100);
BN_mod_exp(result, base, exp, mod, ctx);
BN_free(bn);
BN_CTX_free(ctx);
```

**wolfSSL:** (identical API via compat layer)

Used by: `telnet` (`pk.c`), `factor`, `BIND`

### X509 Certificate Handling

**OpenSSL:**
```c
X509_NAME *name = X509_get_subject_name(cert);
X509_NAME_oneline(name, buf, sizeof(buf));
X509_digest(cert, EVP_sha256(), md, &md_len);
```

**wolfSSL:** (identical API via compat layer)

Used by: `syslogd`, `httpd`, `ftp`

## Known API Differences

### 1. DH Parameter Setup

**OpenSSL (direct struct access — NOT SUPPORTED):**
```c
DH *dh = DH_new();
dh->p = BN_bin2bn(p_data, p_len, NULL);   /* ERROR: dh->p not writable */
dh->g = BN_bin2bn(g_data, g_len, NULL);   /* ERROR: dh->g not writable */
```

**wolfSSL (use setter function):**
```c
DH *dh = DH_new();
BIGNUM *bn_p = BN_bin2bn(p_data, p_len, NULL);
BIGNUM *bn_g = BN_bin2bn(g_data, g_len, NULL);
DH_set0_pqg(dh, bn_p, NULL, bn_g);  /* Takes ownership of bn_p, bn_g */
/* Don't BN_free(bn_p) or BN_free(bn_g) — DH_set0_pqg owns them now */
```

See: `usr.sbin/syslogd/tls.c` (`get_dh1024()`)

### 2. X509_STORE_CTX Member Access

**OpenSSL (direct struct access — NOT SUPPORTED):**
```c
X509 *cert = ctx->current_cert;           /* ERROR: struct member */
int err = ctx->error;                      /* ERROR: struct member */
int depth = ctx->error_depth;              /* ERROR: struct member */
```

**wolfSSL (use getter functions):**
```c
X509 *cert = X509_STORE_CTX_get_current_cert(ctx);
int err = X509_STORE_CTX_get_error(ctx);
int depth = X509_STORE_CTX_get_error_depth(ctx);
```

See: `usr.sbin/syslogd/tls.c` (`check_peer_cert()`)

### 3. EVP_PKEY Type Access

**OpenSSL (direct struct access — NOT SUPPORTED):**
```c
int type = pkey->type;                     /* ERROR: struct member */
```

**wolfSSL (use getter function):**
```c
int type = EVP_PKEY_id(pkey);
```

See: `usr.sbin/syslogd/sign.c`

### 4. DSA Private Key Access

**OpenSSL (direct struct access — NOT SUPPORTED):**
```c
const BIGNUM *priv = key->pkey.dsa->priv_key;  /* ERROR: deep struct */
```

**wolfSSL (check key exists):**
```c
assert(key != NULL);  /* Can't access internal DSA members */
```

See: `usr.sbin/syslogd/sign.c`

### 5. SSL_set_rfd / SSL_set_wfd

**OpenSSL:**
```c
SSL_set_rfd(ssl, fd);   /* Set read fd */
SSL_set_wfd(ssl, fd);   /* Set write fd */
```

**wolfSSL:** (both map to `SSL_set_fd`)
```c
SSL_set_rfd(ssl, fd);   /* Maps to SSL_set_fd(ssl, fd) */
SSL_set_wfd(ssl, fd);   /* Maps to SSL_set_fd(ssl, fd) */
```

This is safe when both fds refer to the same socket (as in httpd).

See: `libexec/httpd/ssl-bozo.c`

### 6. ERR_remove_state

**OpenSSL:**
```c
ERR_remove_state(0);     /* Deprecated in 1.1.0 */
```

**wolfSSL:**
```c
ERR_clear_error();       /* Use instead */
```

See: `external/bsd/bind/dist/lib/isc/timer.c`, `task.c`

### 7. ENGINE API

**OpenSSL:**
```c
ENGINE *e = ENGINE_by_id("dynamic");
ENGINE_init(e);
```

**wolfSSL:** ENGINE API is **not supported**. Code using ENGINE must be
guarded with `#if defined(USE_ENGINE)`.

```c
#if defined(USE_ENGINE)
/* ENGINE code — disabled for wolfSSL */
#endif
```

See: `external/bsd/bind/dist/lib/dns/openssl_link.c`

### 8. X509_print_fp

**OpenSSL:**
```c
X509_print_fp(fp, cert);  /* Text representation */
```

**wolfSSL:** Not available. Use `PEM_write_X509` instead.

See: `usr.sbin/syslogd/tls.c`

## Testing Patterns

### Writing a C Helper Test

```c
#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/err.h>

int main() {
    SSL_library_init();
    SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
    if (ctx == NULL) return 1;
    SSL *ssl = SSL_new(ctx);
    if (ssl == NULL) { SSL_CTX_free(ctx); return 1; }
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    return 0;
}
```

Compile with:
```bash
gcc -o test test.c -I/path/to/wolfssl -lwolfssl
```

### Writing an ATF Test Case

```sh
my_test_head() {
    atf_set "descr" "Tests wolfSSL feature X"
}

my_test_body() {
    if [ ! -x "${HELPER_BIN}" ]; then
        atf_skip "helper binary not found"
    fi
    atf_check "${HELPER_BIN}" -t N
}
```

## Performance Tips

1. **Use fast math**: `FAST_MATH` enables optimized big-number arithmetic
   (already configured for MINIX)

2. **Small stack**: `SMALL_STACK` reduces per-operation stack usage
   (already configured for MINIX)

3. **Disable unused features**: Each disabled feature reduces library size
   (MD4, RC4, PSK, etc. are already disabled for MINIX)

4. **Use ECDHE over DHE**: ECDHE key exchange is ~10x faster than DHE
   and provides equivalent security

5. **Enable hardware acceleration**: Add `--enable-intelasm` for AES-NI
   support on x86-64 targets

## Common Error Messages

### "wolfssl/openssl/ssl.h: No such file or directory"
**Fix**: Add wolfSSL include paths to the Makefile.

### "undefined reference to `SSL_library_init'"
**Fix**: Link with `-lwolfssl` instead of `-lssl`.

### "assignment to member 'p' of 'DH' from incompatible type"
**Fix**: Use `DH_set0_pqg()` instead of direct struct member assignment.

### "storage size of 'ctx' isn't known"
**Fix**: Include `<wolfssl/openssl/x509.h>` for X509 types, or
`<wolfssl/openssl/ssl.h>` for SSL types.

## References

- [Migration Plan](planning/06_openssl_to_wolfssl_migration.md)
- [Build Instructions](BUILDING.md)
- [Security Audit](docs/wolfssl-security-audit.md)
- [Performance Report](docs/wolfssl-performance-report.md)
- [Compatibility Report](docs/wolfssl-compatibility-report.md)
- wolfSSL Documentation: https://www.wolfssl.com/docs/
