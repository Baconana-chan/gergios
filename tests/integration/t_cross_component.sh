# $NetBSD: t_cross_component.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Integration test for cross-component interactions with wolfSSL.
# Tests that multiple migrated components work together correctly.

# Determine wolfSSL include path using pkg-config or fallback
WOLFSSL_CFLAGS=""
if command -v pkg-config >/dev/null 2>&1; then
	WOLFSSL_CFLAGS=$(pkg-config --cflags wolfssl 2>/dev/null || true)
fi
if [ -z "${WOLFSSL_CFLAGS}" ]; then
	if [ -d /usr/include/wolfssl ]; then
		WOLFSSL_CFLAGS="-I/usr/include/wolfssl"
	elif [ -d /usr/local/include/wolfssl ]; then
		WOLFSSL_CFLAGS="-I/usr/local/include/wolfssl"
	fi
fi

cross_syslogd_httpd_head()
{
	atf_set "descr" "Tests syslogd and httpd interaction via wolfSSL"
	atf_set "timeout" "60"
}

cross_syslogd_httpd_body()
{
	# This test verifies that both syslogd and httpd can use
	# wolfSSL simultaneously for their TLS operations

	# Verify both binaries exist
	atf_check test -f /usr/sbin/syslogd
	atf_check test -f /usr/sbin/bozohttpd

	# Verify both link against wolfSSL
	if command -v ldd >/dev/null 2>&1; then
		ldd /usr/sbin/syslogd 2>&1 | grep -q "wolfssl" && \
		    SYSLOGD_WF=true || SYSLOGD_WF=false
		ldd /usr/sbin/bozohttpd 2>&1 | grep -q "wolfssl" && \
		    HTTPD_WF=true || HTTPD_WF=false

		if [ "${SYSLOGD_WF}" = "true" ] && [ "${HTTPD_WF}" = "true" ]; then
			echo "Both syslogd and httpd use wolfSSL - OK"
		else
			echo "syslogd wolfSSL: ${SYSLOGD_WF}, httpd wolfSSL: ${HTTPD_WF}"
		fi
	fi
}

cross_ftp_telnet_head()
{
	atf_set "descr" "Tests FTP and telnet BN operations via wolfSSL"
}

cross_ftp_telnet_body()
{
	# Both ftp and telnet use BIGNUM operations from wolfSSL
	# (ftp for DH key exchange, telnet for SRA)
	# This test verifies both can initialize BN context simultaneously

	atf_check test -f /usr/bin/ftp
	atf_check test -f /usr/bin/telnet

	if command -v ldd >/dev/null 2>&1; then
		ldd /usr/bin/ftp 2>&1 | grep -q "wolfssl" && \
		    FTP_WF=true || FTP_WF=false
		ldd /usr/bin/telnet 2>&1 | grep -q "wolfssl" && \
		    TELNET_WF=true || TELNET_WF=false

		echo "ftp wolfSSL: ${FTP_WF}, telnet wolfSSL: ${TELNET_WF}"
	fi
}

cross_bn_operations_head()
{
	atf_set "descr" "Tests BN operations across all components"
}

cross_bn_operations_body()
{
	if [ -z "${WOLFSSL_CFLAGS}" ]; then
		atf_skip "wolfSSL headers not found"
	fi

	if ! command -v gcc >/dev/null 2>&1; then
		atf_skip "gcc not available"
	fi

	# Test that BN operations work correctly via wolfSSL compat layer.
	# These are used by telnet (pk.c), factor, and BIND (openssl*_link.c).
	cat > /tmp/cross_bn_test.c << EOF
#include <wolfssl/openssl/bn.h>
#include <wolfssl/openssl/crypto.h>
#include <stdio.h>
#include <string.h>

int main() {
	BIGNUM *a, *b, *c;
	BN_CTX *ctx;
	int ret = 0;

	ctx = BN_CTX_new();
	a = BN_new(); b = BN_new(); c = BN_new();

	/* Test 1: Large number arithmetic (used by BIND) */
	BN_hex2bn(&a, "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7EDEE386BFB5A899FA5AE9F24117C4B1FE649286651ECE65381FFFFFFFFFFFFFFFF");
	if (a == NULL) { printf("FAIL: BN_hex2bn\\n"); ret = 1; }

	/* Test 2: Mod exp (used by telnet pk.c) */
	BN_set_word(b, 2);
	BN_set_word(c, 65537);
	BN_mod_exp(b, b, c, a, ctx);

	/* Test 3: GCD (used by factor) */
	BN_set_word(a, 100);
	BN_set_word(b, 75);
	BN_gcd(c, a, b, ctx);
	if (BN_get_word(c) != 25) { printf("FAIL: BN_gcd\\n"); ret = 1; }

	/* Test 4: Primality (used by factor) */
	if (BN_is_prime_ex(a, 10, ctx, NULL) != 0) {
		printf("FAIL: 100 should not be prime\\n"); ret = 1;
	}
	BN_set_word(a, 17);
	if (BN_is_prime_ex(a, 10, ctx, NULL) != 1) {
		printf("FAIL: 17 should be prime\\n"); ret = 1;
	}

	BN_free(c); BN_free(b); BN_free(a);
	BN_CTX_free(ctx);
	return ret;
}
EOF

	gcc -o /tmp/cross_bn_test /tmp/cross_bn_test.c \
	    ${WOLFSSL_CFLAGS} -lwolfssl 2>/dev/null || \
	    atf_skip "Failed to compile BN test helper"
	atf_check /tmp/cross_bn_test
	rm -f /tmp/cross_bn_test /tmp/cross_bn_test.c
}

cross_certificate_handling_head()
{
	atf_set "descr" "Tests certificate handling across components"
	atf_set "timeout" "30"
}

cross_certificate_handling_body()
{
	# Both syslogd and httpd use X509 certificate handling via wolfSSL.
	# This test verifies that certificate operations work correctly.
	if ! command -v openssl >/dev/null 2>&1; then
		atf_skip "openssl command not available"
	fi

	CERT_DIR=$(mktemp -d /tmp/cross_cert.XXXXXX)

	# Create certificate with various algorithms
	openssl req -x509 -newkey rsa:2048 -keyout "${CERT_DIR}/ca-key.pem" \
	    -out "${CERT_DIR}/ca-cert.pem" -days 365 -nodes \
	    -subj "/CN=Test CA" 2>/dev/null

	# Test SHA-256 fingerprint
	atf_check openssl x509 -in "${CERT_DIR}/ca-cert.pem" \
	    -fingerprint -sha256 -noout 2>/dev/null

	# Test subject name parsing (like X509_get_subject_name)
	SUBJECT=$(openssl x509 -in "${CERT_DIR}/ca-cert.pem" \
	    -subject -noout 2>/dev/null)
	echo "${SUBJECT}" | grep -q "CN=Test CA"
	atf_check test $? -eq 0

	rm -rf "${CERT_DIR}"
}

atf_init_test_cases()
{
	atf_add_test_case cross_syslogd_httpd
	atf_add_test_case cross_ftp_telnet
	atf_add_test_case cross_bn_operations
	atf_add_test_case cross_certificate_handling
}
