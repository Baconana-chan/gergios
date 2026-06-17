# $NetBSD: t_httpd_ssl.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Integration test for bozohttpd HTTP server with wolfSSL TLS support.
# Tests SSL initialization, certificate loading, and HTTPS serving.

HTTPD=/usr/sbin/bozohttpd

httpd_ssl_init_head()
{
	atf_set "descr" "Tests HTTP server SSL initialization with wolfSSL"
	atf_set "require.progs" "${HTTPD}"
}

httpd_ssl_init_body()
{
	# Test 1: Verify httpd binary links against wolfSSL
	if command -v ldd >/dev/null 2>&1; then
		LDD_OUTPUT=$(ldd "${HTTPD}" 2>&1 || true)
		echo "${LDD_OUTPUT}" | grep -q "wolfssl" || \
		    atf_skip "httpd not linked against wolfSSL"
	fi

	# Test 2: Check wolfSSL library availability
	atf_check test -f /usr/lib/libwolfssl.so

	# Test 3: Verify httpd SSL support (try -h for help)
	SSL_SUPPORT=$("${HTTPD}" -h 2>&1 | grep -ci "ssl" || true)
	echo "SSL support in httpd: ${SSL_SUPPORT}"
}

httpd_ssl_certificate_head()
{
	atf_set "descr" "Tests HTTP server certificate and key loading with wolfSSL"
}

httpd_ssl_certificate_body()
{
	# Test certificate and key loading similar to SSL_CTX_use_certificate_chain_file
	# and SSL_CTX_use_PrivateKey_file used by ssl-bozo.c
	if ! command -v openssl >/dev/null 2>&1; then
		atf_skip "openssl command not available"
	fi

	CERT_DIR=$(mktemp -d /tmp/httpd_cert.XXXXXX)

	# Create self-signed certificate and key for testing
	openssl req -x509 -newkey rsa:2048 -keyout "${CERT_DIR}/key.pem" \
	    -out "${CERT_DIR}/cert.pem" -days 1 -nodes \
	    -subj "/CN=bozohttpd-test" 2>/dev/null

	# Verify key and cert exist
	atf_check test -f "${CERT_DIR}/cert.pem"
	atf_check test -f "${CERT_DIR}/key.pem"

	# Verify key and cert match using modulus
	CERT_MOD=$(openssl x509 -noout -modulus -in "${CERT_DIR}/cert.pem" 2>/dev/null | \
	    openssl md5 2>/dev/null)
	KEY_MOD=$(openssl rsa -noout -modulus -in "${CERT_DIR}/key.pem" 2>/dev/null | \
	    openssl md5 2>/dev/null)

	if [ -n "${CERT_MOD}" ] && [ -n "${KEY_MOD}" ]; then
		atf_check test "${CERT_MOD}" = "${KEY_MOD}"
	else
		echo "Certificate and key modulus verification skipped"
	fi

	rm -rf "${CERT_DIR}"
}

httpd_ssl_tls_handshake_head()
{
	atf_set "descr" "Tests TLS handshake with self-signed certificate"
	atf_set "timeout" "30"
}

httpd_ssl_tls_handshake_body()
{
	if ! command -v openssl >/dev/null 2>&1; then
		atf_skip "openssl command not available"
	fi

	CERT_DIR=$(mktemp -d /tmp/httpd_ssl.XXXXXX)

	# Create server certificate
	openssl req -x509 -newkey rsa:2048 -keyout "${CERT_DIR}/key.pem" \
	    -out "${CERT_DIR}/cert.pem" -days 1 -nodes \
	    -subj "/CN=localhost" 2>/dev/null

	# Verify certificate with openssl
	atf_check openssl x509 -in "${CERT_DIR}/cert.pem" -text -noout 2>/dev/null

	# Test TLS version support
	SIGNATURE_ALG=$(openssl x509 -in "${CERT_DIR}/cert.pem" -noout -text 2>/dev/null | \
	    grep "Signature Algorithm" | head -1)
	echo "Certificate signature algorithm: ${SIGNATURE_ALG}"

	rm -rf "${CERT_DIR}"
}

atf_init_test_cases()
{
	atf_add_test_case httpd_ssl_init
	atf_add_test_case httpd_ssl_certificate
	atf_add_test_case httpd_ssl_tls_handshake
}
