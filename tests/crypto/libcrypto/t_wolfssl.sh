# $NetBSD: t_wolfssl.sh,v 1.0 2026/06/17 00:00:00 minix Exp $
#
# Copyright (c) 2026 Minix Project
# All rights reserved.
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions
# are met:
# 1. Redistributions of source code must retain the above copyright
#    notice, this list of conditions and the following disclaimer.
# 2. Redistributions in binary form must reproduce the above copyright
#    notice, this list of conditions and the following disclaimer in the
#    documentation and/or other materials provided with the distribution.
#
# THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR
# IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
# OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
# IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT,
# INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
# NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
# DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
# THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY OR TORT
# (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
# THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#

#
# t_wolfssl.sh
# ATF test script for wolfSSL OpenSSL compatibility layer migration.
# Each test case runs a specific sub-test via command-line argument.
#

atf_test_case migrate_init
migrate_init_head()
{
	atf_set "descr" "Tests wolfSSL library initialization (syslogd, ftp, httpd, BIND)"
}
migrate_init_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 1
}

atf_test_case migrate_ssl_context
migrate_ssl_context_head()
{
	atf_set "descr" "Tests SSL context creation and configuration (syslogd, ftp, httpd)"
}
migrate_ssl_context_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 2
}

atf_test_case migrate_evp
migrate_evp_head()
{
	atf_set "descr" "Tests EVP digest operations (syslogd sign.c, BIND)"
}
migrate_evp_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 3
}

atf_test_case migrate_bn
migrate_bn_head()
{
	atf_set "descr" "Tests BIGNUM operations (telnet pk.c, factor, BIND)"
}
migrate_bn_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 4
}

atf_test_case migrate_rand
migrate_rand_head()
{
	atf_set "descr" "Tests random number generation (syslogd, BIND)"
}
migrate_rand_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 5
}

atf_test_case migrate_error
migrate_error_head()
{
	atf_set "descr" "Tests error handling (all components)"
}
migrate_error_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 6
}

atf_test_case migrate_dh
migrate_dh_head()
{
	atf_set "descr" "Tests DH parameter handling (syslogd tls.c)"
}
migrate_dh_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 7
}

atf_test_case migrate_rsa
migrate_rsa_head()
{
	atf_set "descr" "Tests RSA operations (BIND opensslrsa_link.c)"
	atf_set "timeout" "300"
}
migrate_rsa_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 8
}

atf_test_case migrate_version
migrate_version_head()
{
	atf_set "descr" "Tests wolfSSL version information (named main.c)"
}
migrate_version_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 9
}

atf_test_case migrate_hmac
migrate_hmac_head()
{
	atf_set "descr" "Tests HMAC operations (BIND ISC headers)"
}
migrate_hmac_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate" 12
}

atf_test_case migrate_all
migrate_all_head()
{
	atf_set "descr" "Runs all wolfSSL migration tests in sequence"
	atf_set "timeout" "600"
}
migrate_all_body()
{
	atf_check -o ignore -e ignore "$(atf_get_srcdir)/h_wolfssl_migrate"
}

atf_init_test_cases()
{
	atf_add_test_case migrate_init
	atf_add_test_case migrate_ssl_context
	atf_add_test_case migrate_evp
	atf_add_test_case migrate_bn
	atf_add_test_case migrate_rand
	atf_add_test_case migrate_error
	atf_add_test_case migrate_dh
	atf_add_test_case migrate_rsa
	atf_add_test_case migrate_version
	atf_add_test_case migrate_hmac
	atf_add_test_case migrate_all
}
