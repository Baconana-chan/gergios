# $NetBSD: t_security.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Security test suite for wolfSSL migration.
# Tests TLS 1.2, cipher suites, certificate validation,
# DH parameter strength, PRNG, and known vulnerability checks.

SECURITY_BIN=$(atf_get_srcdir)/h_wolfssl_security

security_tls_12_head()
{
	atf_set "descr" "Verifies wolfSSL supports TLS 1.2"
}

security_tls_12_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 1
}

security_ciphers_head()
{
	atf_set "descr" "Verifies modern cipher suite availability (AES-GCM, ChaCha20-Poly1305)"
}

security_ciphers_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 2
}

security_certificate_validation_head()
{
	atf_set "descr" "Verifies X509 certificate generation and basic chain validation"
}

security_certificate_validation_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 3
}

security_dh_strength_head()
{
	atf_set "descr" "Verifies DH parameter minimum strength (>= 1024 bits)"
}

security_dh_strength_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 4
}

security_prng_head()
{
	atf_set "descr" "Verifies PRNG initialization and random byte quality"
}

security_prng_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 5
}

security_no_weak_protocols_head()
{
	atf_set "descr" "Verifies weak protocols (SSLv2, SSLv3) are not available"
}

security_no_weak_protocols_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 6
}

security_no_weak_ciphers_head()
{
	atf_set "descr" "Verifies weak ciphers (RC4, MD4, NULL) are not available"
}

security_no_weak_ciphers_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 7
}

security_forward_secrecy_head()
{
	atf_set "descr" "Verifies forward secrecy support (DHE/ECDHE cipher suites)"
}

security_forward_secrecy_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 8
}

security_cert_names_head()
{
	atf_set "descr" "Verifies certificate subject/issuer name extraction and validation"
}

security_cert_names_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 9
}

security_error_handling_head()
{
	atf_set "descr" "Verifies error handling does not leak sensitive information"
}

security_error_handling_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}" -t 10
}

security_all_head()
{
	atf_set "descr" "Runs all wolfSSL security tests"
	atf_set "timeout" "120"
}

security_all_body()
{
	if [ ! -x "${SECURITY_BIN}" ]; then
		atf_skip "h_wolfssl_security binary not found"
	fi
	atf_check "${SECURITY_BIN}"
}

atf_init_test_cases()
{
	atf_add_test_case security_tls_12
	atf_add_test_case security_ciphers
	atf_add_test_case security_certificate_validation
	atf_add_test_case security_dh_strength
	atf_add_test_case security_prng
	atf_add_test_case security_no_weak_protocols
	atf_add_test_case security_no_weak_ciphers
	atf_add_test_case security_forward_secrecy
	atf_add_test_case security_cert_names
	atf_add_test_case security_error_handling
	atf_add_test_case security_all
}
