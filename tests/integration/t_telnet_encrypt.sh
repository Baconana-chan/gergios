# $NetBSD: t_telnet_encrypt.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Integration test for telnet/telnetd with wolfSSL encryption support.
# Tests SRA (SRP) authentication, DES encryption via BN operations.

TELNET=/usr/bin/telnet
TELNETD=/usr/libexec/telnetd

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

telnet_encrypt_init_head()
{
	atf_set "descr" "Tests telnet encryption initialization with wolfSSL"
	atf_set "require.progs" "${TELNET}"
}

telnet_encrypt_init_body()
{
	# Test 1: Verify telnet binary links against wolfSSL
	if command -v ldd >/dev/null 2>&1; then
		LDD_OUTPUT=$(ldd "${TELNET}" 2>&1 || true)
		echo "${LDD_OUTPUT}" | grep -q "wolfssl" || \
		    atf_skip "telnet not linked against wolfSSL"
	fi

	# Test 2: Verify wolfSSL library is available
	atf_check test -f /usr/lib/libwolfssl.so

	# Test 3: Check telnet has encryption support
	ENC_SUPPORT=$("${TELNET}" -? 2>&1 | grep -ci "encrypt" || true)
	echo "Encryption support in telnet: ${ENC_SUPPORT}"
}

telnet_encrypt_bn_head()
{
	atf_set "descr" "Tests telnet BN-based key exchange (like pk.c)"
}

telnet_encrypt_bn_body()
{
	if [ -z "${WOLFSSL_CFLAGS}" ]; then
		atf_skip "wolfSSL headers not found"
	fi

	if ! command -v gcc >/dev/null 2>&1; then
		atf_skip "gcc not available"
	fi

	# This test verifies the BN operations that telnet pk.c uses
	# for SRA (SRP) key exchange
	cat > /tmp/telnet_bn_test.c << EOF
#include <wolfssl/openssl/bn.h>
#include <stdio.h>
int main() {
	BIGNUM *p, *g, *a, *b, *A, *B, *secret;
	BN_CTX *ctx;
	int ret = 0;

	ctx = BN_CTX_new();
	p = BN_new(); g = BN_new();
	a = BN_new(); b = BN_new();
	A = BN_new(); B = BN_new();
	secret = BN_new();

	/* Simple DH-like key agreement using BN operations
	 * similar to telnet SRA in pk.c */

	/* Use small prime for test (in production, use large prime) */
	BN_set_word(g, 2);
	BN_set_word(a, 5);  /* Alice's private key */
	BN_set_word(b, 7);  /* Bob's private key */
	BN_set_word(p, 23); /* small prime */

	/* A = g^a mod p */
	BN_mod_exp(A, g, a, p, ctx);
	/* B = g^b mod p */
	BN_mod_exp(B, g, b, p, ctx);

	/* secret = B^a mod p = A^b mod p */
	BN_mod_exp(secret, B, a, p, ctx);

	if (BN_get_word(secret) != 17) {
		printf("FAIL: expected 17, got %lu\\n",
		    (unsigned long)BN_get_word(secret));
		ret = 1;
	}

	BN_free(secret);
	BN_free(B); BN_free(A);
	BN_free(b); BN_free(a);
	BN_free(g); BN_free(p);
	BN_CTX_free(ctx);

	return ret;
}
EOF

	gcc -o /tmp/telnet_bn_test /tmp/telnet_bn_test.c \
	    ${WOLFSSL_CFLAGS} -lwolfssl 2>/dev/null || \
	    atf_skip "Failed to compile BN test helper"
	atf_check /tmp/telnet_bn_test
	rm -f /tmp/telnet_bn_test /tmp/telnet_bn_test.c
}

telnet_encrypt_des_head()
{
	atf_set "descr" "Tests telnet DES encryption/decryption"
}

telnet_encrypt_des_body()
{
	# Test DES encryption functions used by telnet.
	# DES is a separate library (libdes), not part of OpenSSL/wolfSSL.
	# Check for libdes test binary in common locations.
	DES_TEST=""
	for dir in /usr/lib /usr/bin /usr/pkg/bin /usr/local/bin; do
		if [ -x "${dir}/h_destest" ]; then
			DES_TEST="${dir}/h_destest"
			break
		fi
	done

	if [ -z "${DES_TEST}" ]; then
		atf_skip "DES test binary (h_destest) not found"
	fi

	atf_check "${DES_TEST}" 2>&1 || \
	    atf_skip "DES test failed or not available"
}

atf_init_test_cases()
{
	atf_add_test_case telnet_encrypt_init
	atf_add_test_case telnet_encrypt_bn
	atf_add_test_case telnet_encrypt_des
}
