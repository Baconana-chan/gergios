# wolfSSL Migration — Infrastructure Verification Checklist
# =========================================================
# Run this checklist after a full MINIX build with MKCRYPTO=yes.
# Each check should PASS before considering the migration complete.
#
# Usage:
#   export DESTDIR=/path/to/destdir   # e.g., /usr/pkg or /usr/release
#   sh releasetools/verify-wolfssl.sh

# Default DESTDIR if not set
: "${DESTDIR:=/usr}"

if [ ! -d "${DESTDIR}/usr/lib" ] && [ ! -d "${DESTDIR}/lib" ]; then
    echo "ERROR: DESTDIR=${DESTDIR} does not contain usr/lib or lib"
    echo "Set DESTDIR to the root of the MINIX installation or build output."
    exit 1
fi

# Helper to find actual library path (DESTDIR may have different structures)
find_lib() {
    for d in "${DESTDIR}/usr/lib" "${DESTDIR}/lib" "${DESTDIR}"; do
        if [ -f "$d/$1" ]; then
            echo "$d/$1"
            return 0
        fi
    done
    return 1
}

## 1. Library Verification

check_library() {
    local lib="$1"
    if [ -f "${DESTDIR}/usr/lib/${lib}" ]; then
        echo "PASS: ${lib} found"
        ls -la "${DESTDIR}/usr/lib/${lib}"
    else
        echo "FAIL: ${lib} NOT found"
    fi
}

check_headers() {
    local header="$1"
    if [ -f "${DESTDIR}/usr/include/${header}" ]; then
        echo "PASS: ${header} found"
    else
        echo "FAIL: ${header} NOT found"
    fi
}

echo "=== Library Files ==="
# Try both .so and .a; at least one should exist
libso=$(find_lib "libwolfssl.so")
liba=$(find_lib "libwolfssl.a")
if [ -n "$libso" ] || [ -n "$liba" ]; then
    echo "PASS: libwolfssl found"
    [ -n "$libso" ] && ls -la "$libso"
    [ -n "$liba" ]  && ls -la "$liba"
else
    echo "FAIL: libwolfssl NOT found in ${DESTDIR}"
fi

echo ""
echo "=== Headers ==="
check_headers "wolfssl/ssl.h"
check_headers "wolfssl/openssl/ssl.h"
check_headers "wolfssl/openssl/evp.h"
check_headers "wolfssl/openssl/bn.h"
check_headers "wolfssl/openssl/dh.h"
check_headers "wolfssl/openssl/dsa.h"
check_headers "wolfssl/openssl/rsa.h"
check_headers "wolfssl/openssl/err.h"
check_headers "wolfssl/openssl/x509.h"
check_headers "wolfssl/openssl/pem.h"
check_headers "wolfssl/openssl/rand.h"

## 2. Component Binary Verification

echo ""
echo "=== Component Binary Linking ==="

check_linking() {
    local bin="$1"
    local path
    
    # Try common locations
    for p in "${DESTDIR}/usr/sbin/${bin}" \
             "${DESTDIR}/usr/bin/${bin}" \
             "${DESTDIR}/usr/libexec/${bin}" \
             "${DESTDIR}/usr/games/${bin}" \
             "${DESTDIR}/sbin/${bin}" \
             "${DESTDIR}/bin/${bin}"; do
        if [ -f "$p" ]; then
            path="$p"
            break
        fi
    done
    
    if [ -z "$path" ]; then
        echo "SKIP: ${bin} not found (not built or not applicable)"
        return
    fi
    
    # Try ldd; if not available, use readelf or objdump as fallback
    if command -v ldd >/dev/null 2>&1; then
        if ldd "$path" 2>/dev/null | grep -q wolfssl; then
            echo "PASS: ${bin} @ ${path} — linked with libwolfssl"
        else
            echo "FAIL: ${bin} @ ${path} — NOT linked with wolfSSL"
        fi
    elif command -v readelf >/dev/null 2>&1; then
        if readelf -d "$path" 2>/dev/null | grep -q wolfssl; then
            echo "PASS: ${bin} @ ${path} — linked with libwolfssl"
        else
            echo "FAIL: ${bin} @ ${path} — NOT linked with wolfSSL"
        fi
    else
        echo "WARN: ${bin} — neither ldd nor readelf available"
    fi
}

check_linking "syslogd"
check_linking "ftp"
check_linking "httpd"
check_linking "telnet"
check_linking "telnetd"
check_linking "named"
check_linking "factor"

## 3. Test Infrastructure Verification

echo ""
echo "=== Test Infrastructure ==="

check_test() {
    local test_script="$1"
    local test_dir="$2"
    
    if [ -f "${test_dir}/${test_script}.sh" ]; then
        echo "PASS: ${test_script}.sh found in ${test_dir}"
    else
        echo "FAIL: ${test_script}.sh NOT found in ${test_dir}"
    fi
}

# Unit tests
check_test "t_wolfssl" "tests/crypto/libcrypto"
check_test "t_security" "tests/crypto/libcrypto"
check_test "t_perf" "tests/crypto/libcrypto"
check_test "t_compat" "tests/crypto/libcrypto"

# Helper binaries
check_helper() {
    local helper="$1"
    local dir="$2"
    
    if [ -f "${dir}/${helper}" ] || [ -f "${dir}/${helper}.c" ]; then
        echo "PASS: ${helper} source/ binary found in ${dir}"
    else
        echo "FAIL: ${helper} NOT found in ${dir}"
    fi
}

check_helper "h_wolfssl_migrate.c" "tests/crypto/libcrypto"
check_helper "h_wolfssl_security.c" "tests/crypto/libcrypto/wolfssl_security"
check_helper "h_wolfssl_perf.c" "tests/crypto/libcrypto/wolfssl_perf"
check_helper "h_wolfssl_compat.c" "tests/crypto/libcrypto/wolfssl_compat"

# Integration tests
check_test "t_syslogd_tls" "tests/integration"
check_test "t_ftp_ssl" "tests/integration"
check_test "t_httpd_ssl" "tests/integration"
check_test "t_telnet_encrypt" "tests/integration"
check_test "t_bind_dnssec" "tests/integration"
check_test "t_cross_component" "tests/integration"

## 4. Documentation Verification

echo ""
echo "=== Documentation ==="

check_doc() {
    local doc="$1"
    if [ -f "$doc" ]; then
        echo "PASS: ${doc} found"
    else
        echo "FAIL: ${doc} NOT found"
    fi
}

check_doc "docs/BUILDING.md"
check_doc "docs/wolfssl-usage-guide.md"
check_doc "docs/wolfssl-configuration-reference.md"
check_doc "docs/wolfssl-testing-guide.md"
check_doc "docs/wolfssl-security-audit.md"
check_doc "docs/wolfssl-performance-report.md"
check_doc "docs/wolfssl-compatibility-report.md"
check_doc "docs/README.md"
check_doc "crypto/external/gpl2/wolfssl/COMPATIBILITY.md"
check_doc "crypto/external/gpl2/wolfssl/README"
check_doc "planning/06_openssl_to_wolfssl_migration.md"
check_doc "releasetools/wolfssl-build.conf"
check_doc "releasetools/verify-wolfssl.sh"

## 5. Config Consistency Check

echo ""
echo "=== Configuration Consistency ==="

# Check config.h for conflicts
if grep -q "NO_DH" crypto/external/gpl2/wolfssl/config.h && \
   grep -q "HAVE_DH" crypto/external/gpl2/wolfssl/config.h; then
    echo "WARN: config.h has HAVE_DH + NO_DH (NO_* takes precedence!)"
else
    echo "PASS: config.h DH config consistent"
fi

if grep -q "NO_DSA" crypto/external/gpl2/wolfssl/config.h && \
   grep -q "HAVE_DSA" crypto/external/gpl2/wolfssl/config.h; then
    echo "WARN: config.h has HAVE_DSA + NO_DSA (NO_* takes precedence!)"
else
    echo "PASS: config.h DSA config consistent"
fi

## 6. Summary

echo ""
echo "=== Summary ==="
echo "Check the results above. All items should show PASS."
echo "FAIL items indicate missing or broken infrastructure."
echo "SKIP items indicate optional components that may not be built."
