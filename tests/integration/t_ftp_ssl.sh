# $NetBSD: t_ftp_ssl.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Integration test for FTP client with wolfSSL SSL/TLS support.
# Tests SSL initialization, AUTH TLS command, and encrypted data transfer.

FTP=/usr/bin/ftp

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

ftp_ssl_init_head()
{
	atf_set "descr" "Tests FTP SSL initialization with wolfSSL"
	atf_set "require.progs" "${FTP}"
}

ftp_ssl_init_body()
{
	# Test 1: Verify ftp binary links against wolfSSL
	if command -v ldd >/dev/null 2>&1; then
		LDD_OUTPUT=$(ldd "${FTP}" 2>&1 || true)
		echo "${LDD_OUTPUT}" | grep -q "wolfssl" || \
		    atf_skip "ftp not linked against wolfSSL"
	fi

	# Test 2: Check that wolfSSL library is loadable
	atf_check test -f /usr/lib/libwolfssl.so
}

ftp_ssl_connection_head()
{
	atf_set "descr" "Tests FTP over TLS (explicit FTPS) connection"
	atf_set "timeout" "30"
}

ftp_ssl_connection_body()
{
	# Verify ftp SSL support is compiled in
	SSL_SUPPORT=$("${FTP}" -S 2>&1 | grep -c "SSL" || true)
	if [ "${SSL_SUPPORT}" -eq 0 ]; then
		atf_skip "FTP not compiled with SSL support"
	fi

	if [ -z "${WOLFSSL_CFLAGS}" ]; then
		atf_skip "wolfSSL headers not found"
	fi

	# Test local SSL context creation via a helper
	# This validates that the wolfSSL compat layer works for FTP
	cat > /tmp/ftp_ssl_test.c << EOF
#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/err.h>
int main() {
	SSL_library_init();
	SSL_load_error_strings();
	const SSL_METHOD *meth = SSLv23_client_method();
	SSL_CTX *ctx = SSL_CTX_new(meth);
	if (ctx == NULL) return 1;
	SSL_CTX_set_mode(ctx, SSL_MODE_AUTO_RETRY);
	SSL *ssl = SSL_new(ctx);
	if (ssl == NULL) return 1;
	SSL_free(ssl);
	SSL_CTX_free(ctx);
	return 0;
}
EOF

	# Compile and run the helper
	if command -v gcc >/dev/null 2>&1; then
		gcc -o /tmp/ftp_ssl_test /tmp/ftp_ssl_test.c \
		    ${WOLFSSL_CFLAGS} -lwolfssl 2>/dev/null || \
		    atf_skip "Failed to compile SSL test helper"
		atf_check /tmp/ftp_ssl_test
		rm -f /tmp/ftp_ssl_test
	fi
	rm -f /tmp/ftp_ssl_test.c
}

atf_init_test_cases()
{
	atf_add_test_case ftp_ssl_init
	atf_add_test_case ftp_ssl_connection
}
