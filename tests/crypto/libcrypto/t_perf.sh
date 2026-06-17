# $NetBSD: t_perf.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Performance benchmark test suite for wolfSSL migration.

PERF_BIN=$(atf_get_srcdir)/h_wolfssl_perf

perf_aes_gcm_head()
{
	atf_set "descr" "Benchmarks AES-128-GCM encryption throughput"
	atf_set "timeout" "120"
}

perf_aes_gcm_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
	fi
	atf_check "${PERF_BIN}" -t 1
}

perf_sha256_head()
{
	atf_set "descr" "Benchmarks SHA-256 hashing throughput"
	atf_set "timeout" "120"
}

perf_sha256_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
	fi
	atf_check "${PERF_BIN}" -t 2
}

perf_rsa_head()
{
	atf_set "descr" "Benchmarks RSA-2048 sign and verify operations"
	atf_set "timeout" "120"
}

perf_rsa_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
	fi
	atf_check "${PERF_BIN}" -t 3
}

perf_dh_head()
{
	atf_set "descr" "Benchmarks DH-2048 key generation time"
	atf_set "timeout" "120"
}

perf_dh_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
	fi
	atf_check "${PERF_BIN}" -t 4
}

perf_tls_context_head()
{
	atf_set "descr" "Benchmarks SSL_CTX_new and SSL_new allocation time"
	atf_set "timeout" "60"
}

perf_tls_context_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
	fi
	atf_check "${PERF_BIN}" -t 5
}

perf_memory_head()
{
	atf_set "descr" "Estimates memory usage for SSL connections"
	atf_set "timeout" "30"
}

perf_memory_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
	fi
	atf_check "${PERF_BIN}" -t 6
}perf_handshake_head()
{
	atf_set "descr" "Benchmarks TLS handshake simulation (in-process)"
	atf_set "timeout" "60"
}

perf_handshake_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
fi
	atf_check "${PERF_BIN}" -t 7
}

perf_ecc_keygen_head()
{
	atf_set "descr" "Benchmarks ECDSA P-256 key generation"
	atf_set "timeout" "120"
}

perf_ecc_keygen_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
fi
	atf_check "${PERF_BIN}" -t 8
}

perf_all_head()
{
	atf_set "descr" "Runs all wolfSSL performance benchmarks"
	atf_set "timeout" "600"
}

perf_all_body()
{
	if [ ! -x "${PERF_BIN}" ]; then
		atf_skip "h_wolfssl_perf binary not found"
fi
	atf_check "${PERF_BIN}"
}

atf_init_test_cases()
{
	atf_add_test_case perf_aes_gcm
	atf_add_test_case perf_sha256
	atf_add_test_case perf_rsa
	atf_add_test_case perf_dh
	atf_add_test_case perf_tls_context
	atf_add_test_case perf_memory
	atf_add_test_case perf_handshake
	atf_add_test_case perf_ecc_keygen
	atf_add_test_case perf_all
}
