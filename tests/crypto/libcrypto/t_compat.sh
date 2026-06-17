# $NetBSD: t_compat.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Compatibility test suite for wolfSSL migration.
# Tests TLS client/server method creation, PEM cert loading,
# cipher negotiation, protocol version masks, cert verification,
# error handling, auth modes, and SSL options.

COMPAT_BIN=$(atf_get_srcdir)/h_wolfssl_compat

compat_sslv23_methods_head()
{
	atf_set "descr" "Verifies SSLv23_client_method, SSLv23_server_method, SSLv23_method"
}

compat_sslv23_methods_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 1
}

compat_pem_cert_loading_head()
{
	atf_set "descr" "Verifies PEM certificate loading via SSL_CTX_use_certificate_chain_file"
}

compat_pem_cert_loading_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 2
}

compat_cipher_suites_head()
{
	atf_set "descr" "Verifies multiple cipher suite formats (HIGH, AES, ECDHE, DEFAULT)"
}

compat_cipher_suites_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 3
}

compat_protocol_versions_head()
{
	atf_set "descr" "Verifies SSL_OP_NO_SSLv2/v3/TLSv1/TLSv1_1 version masks"
}

compat_protocol_versions_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 4
}

compat_peer_cert_head()
{
	atf_set "descr" "Verifies peer certificate verification and SHA-256 fingerprint"
}

compat_peer_cert_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 5
}

compat_invalid_cert_errors_head()
{
	atf_set "descr" "Verifies error handling for invalid PEM and nonexistent cert paths"
}

compat_invalid_cert_errors_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 6
}

compat_verify_modes_head()
{
	atf_set "descr" "Verifies SSL_VERIFY_NONE, SSL_VERIFY_PEER, SSL_VERIFY_FAIL_IF_NO_PEER_CERT"
}

compat_verify_modes_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 7
}

compat_options_modes_head()
{
	atf_set "descr" "Verifies SSL_CTX_set_mode (AUTO_RETRY) and SSL_CTX_set_options (ALL, NO_COMPRESSION)"
}

compat_options_modes_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}" -t 8
}

compat_all_head()
{
	atf_set "descr" "Runs all wolfSSL compatibility tests"
	atf_set "timeout" "120"
}

compat_all_body()
{
	if [ ! -x "${COMPAT_BIN}" ]; then
		atf_skip "h_wolfssl_compat binary not found"
	fi
	atf_check "${COMPAT_BIN}"
}

atf_init_test_cases()
{
	atf_add_test_case compat_sslv23_methods
	atf_add_test_case compat_pem_cert_loading
	atf_add_test_case compat_cipher_suites
	atf_add_test_case compat_protocol_versions
	atf_add_test_case compat_peer_cert
	atf_add_test_case compat_invalid_cert_errors
	atf_add_test_case compat_verify_modes
	atf_add_test_case compat_options_modes
	atf_add_test_case compat_all
}
