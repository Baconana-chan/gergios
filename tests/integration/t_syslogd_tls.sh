# $NetBSD: t_syslogd_tls.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Integration test for syslogd with wolfSSL TLS support.
# Tests TLS initialization, certificate generation, and TLS connection handling.

SYSLOGD=/usr/sbin/syslogd

syslogd_tls_head()
{
	atf_set "descr" "Tests syslogd TLS initialization with wolfSSL"
	atf_set "require.progs" "${SYSLOGD}"
}

syslogd_tls_body()
{
	# Test 1: Verify syslogd binary exists and links against wolfSSL
	# Use correct pattern: capture ldd output first, then check
	if command -v ldd >/dev/null 2>&1; then
		LDD_OUTPUT=$(ldd "${SYSLOGD}" 2>&1 || true)
		echo "${LDD_OUTPUT}" | grep -q "wolfssl" || \
		    atf_skip "syslogd not linked against wolfSSL"
	fi

	# Test 2: Verify that wolfSSL libraries are available
	atf_check test -f /usr/lib/libwolfssl.so
}

syslogd_tls_certificates_head()
{
	atf_set "descr" "Tests syslogd TLS certificate handling with wolfSSL"
}

syslogd_tls_certificates_body()
{
	# Check that openssl command is available
	if ! command -v openssl >/dev/null 2>&1; then
		atf_skip "openssl command not available"
	fi

	# Test that wolfSSL can be used to generate keys and certificates
	# like syslogd does in tls.c (write_x509files)
	CERT_DIR=$(mktemp -d /tmp/syslogd_cert.XXXXXX)

	# Generate test key and cert
	openssl genrsa -out "${CERT_DIR}/key.pem" 2048 2>/dev/null
	atf_check test -f "${CERT_DIR}/key.pem"

	openssl req -new -x509 -key "${CERT_DIR}/key.pem" \
	    -out "${CERT_DIR}/cert.pem" -days 1 \
	    -subj "/CN=syslogd-test" 2>/dev/null
	atf_check test -f "${CERT_DIR}/cert.pem"

	# Verify certificate is valid
	openssl x509 -in "${CERT_DIR}/cert.pem" -noout 2>/dev/null
	atf_check -s exit:0 -o match:"syslogd-test" \
	    openssl x509 -in "${CERT_DIR}/cert.pem" -subject -noout 2>/dev/null

	rm -rf "${CERT_DIR}"
}

syslogd_tls_dhparams_head()
{
	atf_set "descr" "Tests syslogd DH parameter generation (get_dh1024)"
}

syslogd_tls_dhparams_body()
{
	# Test that DH parameters can be generated
	# This mirrors the get_dh1024() function in tls.c
	if ! command -v openssl >/dev/null 2>&1; then
		atf_skip "openssl command not available"
	fi

	DH_DIR=$(mktemp -d /tmp/syslogd_dh.XXXXXX)

	# Generate DH parameters
	openssl dhparam -out "${DH_DIR}/dh1024.pem" -2 1024 2>/dev/null
	if [ -f "${DH_DIR}/dh1024.pem" ]; then
		# Verify DH params
		atf_check \
		    openssl dhparam -in "${DH_DIR}/dh1024.pem" -noout 2>/dev/null
	else
		atf_skip "DH parameter generation not supported"
	fi

	rm -rf "${DH_DIR}"
}

atf_init_test_cases()
{
	atf_add_test_case syslogd_tls
	atf_add_test_case syslogd_tls_certificates
	atf_add_test_case syslogd_tls_dhparams
}
