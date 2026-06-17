# wolfSSL Testing Guide for MINIX

This document describes how to run, interpret, and extend the wolfSSL
test suite in the MINIX build system.

## Test Suite Overview

The wolfSSL test suite is organized into five categories:

| Category | Location | Tests | Purpose |
|----------|----------|-------|---------|
| Unit | `tests/crypto/libcrypto/` | 9 test scripts | Verify individual API calls and features |
| Security | `tests/crypto/libcrypto/wolfssl_security/` | 10 tests | Verify security properties |
| Performance | `tests/crypto/libcrypto/wolfssl_perf/` | 7 benchmarks | Measure crypto operation throughput |
| Compatibility | `tests/crypto/libcrypto/wolfssl_compat/` | 8 tests | Verify OpenSSL API compatibility |
| Integration | `tests/integration/` | 18 test cases | Verify cross-component interaction |

### Test Framework: ATF (Automatic Test Framework)

MINIX uses the ATF (Automatic Test Framework) for all tests. Each test
script defines:
- `head()` — single-line test description
- `body()` — test logic (skip, check, cleanup)

---

## 1. Running Tests

### Run All wolfSSL Tests

```bash
# From the MINIX source root
cd tests/crypto/libcrypto

# Run all unit tests
atf-run t_wolfssl

# Run all security tests
atf-run t_security

# Run all compatibility tests
atf-run t_compat

# Run all performance benchmarks
atf-run t_perf

# Run all integration tests
cd tests/integration
atf-run t_syslogd_tls
atf-run t_ftp_ssl
atf-run t_httpd_ssl
atf-run t_telnet_encrypt
atf-run t_bind_dnssec
atf-run t_cross_component
```

### Run a Single Test Case

```bash
# Run just one test case (e.g., test 3 — EVP digest)
atf-run t_wolfssl:wolfssl_evp_digest

# Run just the TLS 1.2 security test
atf-run t_security:security_tls_12_support

# Run just the AES-GCM performance benchmark
atf-run t_perf:perf_aes_gcm
```

### Run All Tests (Full Suite)

```bash
# From tests/ directory
cd tests/crypto/libcrypto
for t in t_wolfssl t_security t_compat t_perf; do
    echo "=== $t ==="
    atf-run $t
done

# Integration tests
cd tests/integration
for t in t_syslogd_tls t_ftp_ssl t_httpd_ssl t_telnet_encrypt t_bind_dnssec t_cross_component; do
    echo "=== $t ==="
    atf-run $t
done
```

---

## 2. Test Descriptions & Expected Results

### 2.1 Unit Tests (`t_wolfssl`)

| Test Case | C Helper `-t N` | What It Tests | Expected Result |
|-----------|----------------|---------------|-----------------|
| `wolfssl_init` | 1 | `SSL_library_init`, `SSL_load_error_strings` | PASS |
| `wolfssl_ssl_context` | 2 | `SSL_CTX_new`, `SSLv23_method`, `SSL_new` | PASS |
| `wolfssl_evp_digest` | 3 | SHA-256 via EVP_DigestInit/Update/Final | PASS |
| `wolfssl_bn_ops` | 4 | BN_new, BN_add, BN_mul, BN_cmp, BN_mod | PASS |
| `wolfssl_rand` | 5 | RAND_status, RAND_bytes | PASS |
| `wolfssl_err` | 6 | ERR_get_error, ERR_error_string, ERR_clear_error | PASS |
| `wolfssl_dh` | 7 | DH_new, DH_set0_pqg, DH_size | PASS |
| `wolfssl_rsa` | 8 | RSA_new, RSA_generate_key_ex | PASS |
| `wolfssl_version` | 9 | wolfSSL version string | PASS |
| `wolfssl_version_compat` | 10 | OPENSSL_VERSION_NUMBER checks for BIND | PASS |
| `wolfssl_dsa_hmac` | 11-12 | DSA + HMAC operations | PASS |

### 2.2 Security Tests (`t_security`)

| Test Case | What It Tests | Security Property |
|-----------|--------------|-------------------|
| `security_tls_12_support` | TLS 1.2 via SSLv23_method + SSL_OP_NO_* | Protocol security |
| `security_modern_ciphers` | AES-GCM, ChaCha20-Poly1305 availability | Cipher strength |
| `security_certificate_validation` | Self-signed X.509 cert, SHA-256 signing | Certificate trust |
| `security_dh_parameter_strength` | DH >= 1024 bits | Key exchange strength |
| `security_prng_quality` | RAND_status, RAND_bytes, uniqueness | Randomness quality |
| `security_weak_protocols_disabled` | SSLv2/SSLv3 options disabled | Protocol hardening |
| `security_weak_ciphers_disabled` | RC4/MD5/NULL cipher rejection | Cipher hardening |
| `security_forward_secredy` | DHE/ECDHE cipher suites | Forward secrecy |
| `security_certificate_name_validation` | X509_NAME_oneline, subject/issuer | Certificate trust |
| `security_error_handling_safety` | ERR queue, no sensitive info leaks | Error safety |

### 2.3 Compatibility Tests (`t_compat`)

| Test Case | What It Tests | API Coverage |
|-----------|--------------|--------------|
| `compat_sslv23_methods` | SSLv23_method/client/server + SSL object creation | SSL context |
| `compat_pem_cert_loading` | Generate cert + write PEM + SSL_CTX_use_certificate_chain_file | Certificate |
| `compat_cipher_suite_negotiation` | 5 cipher list formats (HIGH, TLS13, ECDHE, DEFAULT, AES) | Cipher strings |
| `compat_protocol_version_masks` | SSL_OP_NO_SSLv2/v3/TLSv1/TLSv1_1 + SINGLE_DH_USE | Protocol options |
| `compat_peer_cert_verify` | X509_digest SHA-256, X509_NAME_oneline | Certificate verify |
| `compat_invalid_cert_errors` | Corrupt PEM, missing file errors | Error handling |
| `compat_ssl_verify_modes` | SSL_VERIFY_NONE/PEER/FAIL_IF_NO_PEER_CERT | Auth modes |
| `compat_ssl_options_and_modes` | AUTO_RETRY, ALL, NO_COMPRESSION, get_options | SSL_CTX options |

### 2.4 Performance Benchmarks (`t_perf`)

| Benchmark | Iterations | Measures | Expected Range (x86-64) |
|-----------|-----------|----------|----------------------|
| `perf_aes_gcm` | 10,000 | AES-128-GCM encrypt 4 KB | ~800-1200 MB/s |
| `perf_sha256` | 50,000 | SHA-256 hash 4 KB | ~400-700 MB/s |
| `perf_rsa_sign_verify` | 500 | RSA-2048 sign + verify | ~300-600 op/s sign |
| `perf_dh_keygen` | 200 | DH-2048 key generation | ~300-500 op/s |
| `perf_tls_context` | 5,000 | SSL_CTX + SSL creation | ~50,000+ op/s |
| `perf_memory_usage` | 10 | Memory per SSL connection | ~20-50 KB |
| `perf_tls_handshake` | 200 | Handshake state creation | ~10,000+ op/s |
| `perf_ecc_keygen` | 2,000 | ECDSA P-256 key generation | ~5,000+ op/s |

### 2.5 Integration Tests

| Test Script | Test Cases | Components |
|-------------|-----------|------------|
| `t_syslogd_tls` | syslogd TLS flag, cert generation, DH params | syslogd |
| `t_ftp_ssl` | FTP SSL init, SSL context | ftp |
| `t_httpd_ssl` | HTTPd SSL init, cert/key check, TLS handshake | httpd |
| `t_telnet_encrypt` | Telnet BN ops, DES operations | telnet |
| `t_bind_dnssec` | BIND named link, keygen, zone signing | BIND |
| `t_cross_component` | Cross-system BN, cert handling, concurrent usage | All |

---

## 3. Understanding Test Output

### ATF Success Output

```
t_wolfssl (1/1): 12 test cases
    wolfssl_init: passed
    wolfssl_ssl_context: passed
    ...
    wolfssl_dsa_hmac: passed
Result: PASS
```

### ATF Failure Output

```
t_wolfssl (1/1): 12 test cases
    wolfssl_dh: [0.1s] FAILED
    --- wolfssl_dh at line 67 ----
    ...
Result: FAIL
```

In case of failure:
1. Find the failing C helper test number from the ATF script
2. Run the helper directly: `./h_wolfssl_migrate -t N`
3. Check stderr for wolfSSL error details
4. Verify the Makefile links with `-lwolfssl`

### ATF Skip Output

```
t_bind_dnssec (1/1): 3 test cases
    bind_dnssec_init: skipped: named not found
```

Tests skip automatically when:
- The component binary is not built (BIND, ftp, httpd)
- wolfSSL is not linked (checked via ldd)
- Required tools are missing (openssl CLI, dnssec-keygen)

---

## 4. Writing New Tests

### Adding a C Helper Test

```c
#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/err.h>

static int test_new_feature(void) {
    SSL_library_init();
    SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
    if (ctx == NULL) {
        fprintf(stderr, "FAIL: SSL_CTX_new returned NULL\n");
        return 1;
    }
    SSL_CTX_free(ctx);
    return 0;
}

int main(int argc, char **argv) {
    int ch, test = 0;
    while ((ch = getopt(argc, argv, "t:")) != -1) {
        if (ch == 't') test = atoi(optarg);
    }
    if (test == 1) return test_new_feature();
    return test_new_feature(); /* run all if no -t flag */
}
```

### Adding an ATF Test Case

```sh
atf_test_case wolfssl_new_feature
wolfssl_new_feature_head() {
    atf_set "descr" "Tests new wolfSSL feature"
}
wolfssl_new_feature_body() {
    if [ ! -x "${HELPER}" ]; then
        atf_skip "helper not compiled"
    fi
    atf_check "${HELPER}" -t 1
}
```

Then add `atf_add_test_case wolfssl_new_feature` to the `atf_init_test_cases()`
function.

### Adding a Makefile Entry

```makefile
TESTS_SH+=      t_wolfssl       # add to tests/crypto/libcrypto/Makefile

# For new subdirectory
SUBDIR+=        wolfssl_newtest # add to parent Makefile
```

---

## 5. Debugging Test Failures

### Step 1: Run the C Helper Directly

```bash
# Build the helper
cd tests/crypto/libcrypto/wolfssl_security
make h_wolfssl_security

# Run specific test (e.g., test 4 — DH parameter strength)
./h_wolfssl_security -t 4
echo "Exit code: $?"
```

### Step 2: Enable wolfSSL Debug Output

```c
/* Add to your C file for debugging */
wolfSSL_Debugging_ON();
```

Or set environment variable:
```bash
export WOLFSSL_DEBUG=1
```

### Step 3: Check wolfSSL Error Queue

```c
/* After a failed operation */
unsigned long err = ERR_get_error();
char buf[256];
ERR_error_string_n(err, buf, sizeof(buf));
fprintf(stderr, "wolfSSL error: %s\n", buf);
```

### Step 4: Verify Library Linking

```bash
# Check that the binary links against wolfSSL
ldd /path/to/binary | grep wolfssl

# Should output: libwolfssl.so => /usr/lib/libwolfssl.so
```

---

## 6. Common Test Failures

### "SSL_CTX_new returned NULL"

**Cause**: wolfSSL library not properly initialized, or required feature disabled.

**Fix**:
1. Ensure `SSL_library_init()` is called before `SSL_CTX_new()`
2. Verify `OPENSSL_EXTRA` is defined in config.h
3. Check that no conflicting `NO_*` defines are set

### "RAND_status returned 0"

**Cause**: PRNG not properly seeded.

**Fix**:
1. Ensure `/dev/urandom` is available (or equivalent entropy source)
2. Check that `NO_DEV_RANDOM` is not preventing seeding
3. Call `RAND_poll()` before `RAND_status()`

### "DH_set0_pqg returned error"

**Cause**: BIGNUM parameters were freed before calling `DH_set0_pqg`.

**Fix**: `DH_set0_pqg()` takes ownership of BIGNUMs — do NOT free them
before the call. After success, do NOT free them at all.

### "BIND named: ENGINE not supported"

**Cause**: wolfSSL does not support the ENGINE API.

**Fix**: Ensure `USE_ENGINE` is defined to 0 in `dst_openssl.h`:
```c
#define USE_ENGINE 0
```

### "ATF test skipped: helper not compiled"

**Cause**: The C helper binary wasn't built because make dependencies didn't
trigger.

**Fix**: Build the helper explicitly:
```bash
cd tests/crypto/libcrypto/wolfssl_security
make
```

---

## 7. Test Coverage Matrix

| Component | Unit | Security | Compat | Perf | Integration |
|-----------|------|----------|--------|------|-------------|
| wolfSSL core | ✅ | ✅ | ✅ | ✅ | ✅ |
| syslogd | ✅ | ✅ | ✅ | ✅ | ✅ |
| ftp | ✅ | — | ✅ | — | ✅ |
| httpd | ✅ | ✅ | ✅ | — | ✅ |
| telnet | ✅ | — | — | — | ✅ |
| passwd | — ¹ | — | — | — | — |

¹ passwd migration was UI API replacement (`UI_UTIL_read_pw_string` → POSIX `termios`), not crypto API. No wolfSSL crypto tests needed.
| factor | ✅ | — | — | — | — |
| BIND | ✅ | ✅ | ✅ | ✅ | ✅ |

`—` = covered implicitly by wolfSSL core tests (same API surface)

---

## References

- [Build Instructions](BUILDING.md)
- [Configuration Reference](wolfssl-configuration-reference.md)
- [API Usage Guide](wolfssl-usage-guide.md)
- [Security Audit](wolfssl-security-audit.md)
- [Performance Report](wolfssl-performance-report.md)
- [Compatibility Report](wolfssl-compatibility-report.md)
- [Migration Plan](planning/06_openssl_to_wolfssl_migration.md)
- ATF Documentation: https://github.com/jmmv/atf
- wolfSSL Test Framework: https://www.wolfssl.com/docs/testing/
