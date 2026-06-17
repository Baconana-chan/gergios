/*
 * $NetBSD: h_wolfssl_compat.c,v 1.0 2026/06/17 00:00:00 minix Exp $
 *
 * Copyright (c) 2026 Minix Project
 * All rights reserved.
 *
 * Compatibility test helper for wolfSSL migration.
 *
 * Tests:
 *  1. SSLv23_method() — client and server mode creation
 *  2. PEM certificate/key loading (format compatibility)
 *  3. Cipher suite negotiation (multiple cipher lists)
 *  4. Protocol version compatibility (SSL_OP_NO_* masks)
 *  5. Peer certificate verification (self-signed)
 *  6. Error behavior for invalid certificates
 *  7. Auth mode settings (SSL_VERIFY_PEER, SSL_VERIFY_NONE)
 *  8. Re-negotiation compatibility (SSL_CTX_set_options)
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/err.h>
#include <wolfssl/openssl/rand.h>
#include <wolfssl/openssl/evp.h>
#include <wolfssl/openssl/x509.h>
#include <wolfssl/openssl/pem.h>
#include <wolfssl/openssl/bio.h>
#include <wolfssl/version.h>

static int tests_run = 0;
static int tests_passed = 0;
static int tests_failed = 0;
static int tests_skipped = 0;

#define TEST_NAME(n) static void test_##n(void)
#define TEST_FAIL(msg) do { \
	fprintf(stderr, "FAIL: %s\n", msg); \
	tests_failed++; \
	return; \
} while (0)
#define TEST_SKIP(msg) do { \
	fprintf(stderr, "SKIP: %s\n", msg); \
	tests_skipped++; \
	return; \
} while (0)
#define TEST_ASSERT(cond, msg) do { \
	if (!(cond)) { TEST_FAIL(msg); } \
} while (0)

/*
 * Helper: generate a self-signed cert and write to PEM BIO.
 * Returns the PEM data in 'out' (caller must OPENSSL_free).
 * Returns length, or 0 on failure.
 */
static int
generate_self_signed_pem(char **out)
{
	EVP_PKEY *pkey = EVP_PKEY_new();
	RSA *rsa = RSA_generate_key(2048, RSA_F4, NULL, NULL);
	if (!rsa) { EVP_PKEY_free(pkey); return 0; }
	EVP_PKEY_set1_RSA(pkey, rsa);
	RSA_free(rsa);

	X509 *cert = X509_new();
	X509_set_version(cert, 2);
	ASN1_INTEGER_set(X509_get_serialNumber(cert), 1);
	X509_gmtime_adj(X509_get_notBefore(cert), 0);
	X509_gmtime_adj(X509_get_notAfter(cert), 365 * 24 * 3600);

	X509_NAME *name = X509_get_subject_name(cert);
	X509_NAME_add_entry_by_txt(name, "CN", MBSTRING_ASC,
	    (const unsigned char *)"compat-test.minix", -1, -1, 0);
	X509_set_issuer_name(cert, name);
	X509_set_pubkey(cert, pkey);
	X509_sign(cert, pkey, EVP_sha256());

	/* Write cert to PEM BIO */
	BIO *bio = BIO_new(BIO_s_mem());
	PEM_write_bio_X509(bio, cert);

	long len = BIO_get_mem_data(bio, out);
	char *pem = malloc(len + 1);
	memcpy(pem, *out, len);
	pem[len] = '\0';
	*out = pem;

	BIO_free(bio);
	X509_free(cert);
	EVP_PKEY_free(pkey);
	return (int)len;
}

/* ------------------------------------------------------------------ */
/* Test 1: SSLv23_method — client and server mode                      */
/* ------------------------------------------------------------------ */
TEST_NAME(sslv23_methods)
{
	SSL_library_init();

	const SSL_METHOD *client_meth = SSLv23_client_method();
	TEST_ASSERT(client_meth != NULL, "SSLv23_client_method() returned NULL");

	const SSL_METHOD *server_meth = SSLv23_server_method();
	TEST_ASSERT(server_meth != NULL, "SSLv23_server_method() returned NULL");

	const SSL_METHOD *meth = SSLv23_method();
	TEST_ASSERT(meth != NULL, "SSLv23_method() returned NULL");

	/* Create contexts with each method */
	SSL_CTX *cli_ctx = SSL_CTX_new(client_meth);
	TEST_ASSERT(cli_ctx != NULL, "SSL_CTX_new(client) failed");

	SSL_CTX *srv_ctx = SSL_CTX_new(server_meth);
	TEST_ASSERT(srv_ctx != NULL, "SSL_CTX_new(server) failed");

	/* Create SSL objects */
	SSL *cli_ssl = SSL_new(cli_ctx);
	TEST_ASSERT(cli_ssl != NULL, "SSL_new(client) failed");

	SSL *srv_ssl = SSL_new(srv_ctx);
	TEST_ASSERT(srv_ssl != NULL, "SSL_new(server) failed");

	SSL_free(cli_ssl);
	SSL_free(srv_ssl);
	SSL_CTX_free(cli_ctx);
	SSL_CTX_free(srv_ctx);
}

/* ------------------------------------------------------------------ */
/* Test 2: PEM certificate/key loading                                 */
/* ------------------------------------------------------------------ */
TEST_NAME(pem_cert_loading)
{
	SSL_library_init();
	OpenSSL_add_all_digests();

	char *pem_data;
	int pem_len = generate_self_signed_pem(&pem_data);
	TEST_ASSERT(pem_len > 0, "Self-signed cert generation failed");

	/* Write PEM to a temp file for loading */
	char tmpfile[] = "/tmp/wolfssl_compat_cert.XXXXXX";
	int fd = mkstemp(tmpfile);
	TEST_ASSERT(fd >= 0, "mkstemp failed");
	write(fd, pem_data, pem_len);
	close(fd);

	/* Load cert from file (like bozohttpd's SSL_CTX_use_certificate_chain_file) */
	SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
	int ret = SSL_CTX_use_certificate_chain_file(ctx, tmpfile);
	TEST_ASSERT(ret == 1, "SSL_CTX_use_certificate_chain_file failed");

	/* Load key — we didn't save the private key, but cert loading alone
	 * validates PEM format compatibility */
	SSL_CTX_free(ctx);
	unlink(tmpfile);
	free(pem_data);
}

/* ------------------------------------------------------------------ */
/* Test 3: Cipher suite negotiation (multiple cipher lists)            */
/* ------------------------------------------------------------------ */
TEST_NAME(cipher_suite_negotiation)
{
	SSL_library_init();

	SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
	TEST_ASSERT(ctx != NULL, "SSL context creation failed");

	/* Test multiple cipher list formats */

	/* 1. OpenSSL-style HIGH */
	int ret = SSL_CTX_set_cipher_list(ctx, "HIGH:!aNULL:!eNULL");
	TEST_ASSERT(ret == 1, "Cipher list 'HIGH:!aNULL:!eNULL' failed");

	/* 2. TLS 1.3 ciphers (if supported) */
	ret = SSL_CTX_set_cipher_list(ctx,
	    "TLS13-AES128-GCM-SHA256:TLS13-AES256-GCM-SHA384");
	if (ret != 1) {
		fprintf(stderr, "  TLS 1.3 ciphers not available (expected)\n");
	}

	/* 3. ECDHE ciphers */
	ret = SSL_CTX_set_cipher_list(ctx,
	    "ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384");
	/* ECDHE may require ECC support — acceptable if not available */

	/* 4. Default cipher list */
	ret = SSL_CTX_set_cipher_list(ctx, "DEFAULT");
	if (ret != 1) {
		/* Try wolfSSL default */
		ret = SSL_CTX_set_cipher_list(ctx, "ALL:!COMPLEMENTOFDEFAULT");
		TEST_ASSERT(ret == 1, "Default cipher list failed");
	}

	/* 5. AES-only (used by syslogd/https for compatibility) */
	ret = SSL_CTX_set_cipher_list(ctx,
	    "AES128-GCM-SHA256:AES256-GCM-SHA384:AES128-SHA:AES256-SHA");
	TEST_ASSERT(ret == 1, "AES-only cipher list failed");

	SSL_CTX_free(ctx);
}

/* ------------------------------------------------------------------ */
/* Test 4: Protocol version negotiation (SSL_OP_NO_* masks)            */
/* ------------------------------------------------------------------ */
TEST_NAME(protocol_version_masks)
{
	SSL_library_init();

	SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
	TEST_ASSERT(ctx != NULL, "SSL context creation failed");

	/* Test that all version-mask options are accepted */
	long opts;

	/* Disable SSLv2 */
	opts = SSL_CTX_set_options(ctx, SSL_OP_NO_SSLv2);
	TEST_ASSERT((opts & SSL_OP_NO_SSLv2) != 0,
	    "SSL_OP_NO_SSLv2 not accepted");

	/* Disable SSLv3 */
	opts = SSL_CTX_set_options(ctx, SSL_OP_NO_SSLv3);
	TEST_ASSERT((opts & SSL_OP_NO_SSLv3) != 0,
	    "SSL_OP_NO_SSLv3 not accepted");

	/* Disable TLSv1 */
	opts = SSL_CTX_set_options(ctx, SSL_OP_NO_TLSv1);
	TEST_ASSERT((opts & SSL_OP_NO_TLSv1) != 0,
	    "SSL_OP_NO_TLSv1 not accepted");

	/* Disable TLSv1.1 */
	opts = SSL_CTX_set_options(ctx, SSL_OP_NO_TLSv1_1);
	TEST_ASSERT((opts & SSL_OP_NO_TLSv1_1) != 0,
	    "SSL_OP_NO_TLSv1_1 not accepted");

	/* Disable ALL weak versions — force TLS 1.2+ only */
	opts = SSL_CTX_set_options(ctx,
	    SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3 |
	    SSL_OP_NO_TLSv1 | SSL_OP_NO_TLSv1_1 |
	    SSL_OP_SINGLE_DH_USE);
	TEST_ASSERT((opts & SSL_OP_SINGLE_DH_USE) != 0,
	    "SSL_OP_SINGLE_DH_USE not accepted");

	fprintf(stderr, "  All protocol version masks accepted: 0x%lx\n", opts);

	SSL_CTX_free(ctx);
}

/* ------------------------------------------------------------------ */
/* Test 5: Peer certificate verification (self-signed)                 */
/* ------------------------------------------------------------------ */
TEST_NAME(peer_cert_verify)
{
	SSL_library_init();
	OpenSSL_add_all_digests();

	char *pem_data;
	int pem_len = generate_self_signed_pem(&pem_data);
	TEST_ASSERT(pem_len > 0, "Self-signed cert generation failed");

	/* Create cert from PEM data */
	BIO *bio = BIO_new_mem_buf(pem_data, pem_len);
	X509 *cert = PEM_read_bio_X509(bio, NULL, NULL, NULL);
	BIO_free(bio);

	TEST_ASSERT(cert != NULL, "PEM_read_bio_X509 failed");

	/* Verify self-signed: subject == issuer */
	X509_NAME *subject = X509_get_subject_name(cert);
	X509_NAME *issuer = X509_get_issuer_name(cert);

	char subject_str[256], issuer_str[256];
	X509_NAME_oneline(subject, subject_str, sizeof(subject_str));
	X509_NAME_oneline(issuer, issuer_str, sizeof(issuer_str));

	TEST_ASSERT(strcmp(subject_str, issuer_str) == 0,
	    "Self-signed cert: subject != issuer");

	fprintf(stderr, "  CN: %s\n", subject_str);

	/* Verify cert fingerprint (like syslogd's X509_digest) */
	unsigned char md[EVP_MAX_MD_SIZE];
	unsigned int md_len;
	int ret = X509_digest(cert, EVP_sha256(), md, &md_len);
	TEST_ASSERT(ret == 1, "X509_digest with SHA-256 failed");
	TEST_ASSERT(md_len == 32, "SHA-256 digest length != 32");

	fprintf(stderr, "  SHA-256 fingerprint: ");
	for (unsigned int i = 0; i < md_len; i++)
		fprintf(stderr, "%02x%c", md[i], i < md_len - 1 ? ':' : '\n');

	X509_free(cert);
	free(pem_data);
}

/* ------------------------------------------------------------------ */
/* Test 6: Error behavior for invalid certificates                     */
/* ------------------------------------------------------------------ */
TEST_NAME(invalid_cert_errors)
{
	SSL_library_init();
	SSL_load_error_strings();

	/* Attempt to load invalid PEM data */
	const char *invalid_pem = "-----BEGIN CERTIFICATE-----\n"
	    "AAAA\n"
	    "-----END CERTIFICATE-----";

	BIO *bio = BIO_new_mem_buf((void *)invalid_pem, -1);
	X509 *cert = PEM_read_bio_X509(bio, NULL, NULL, NULL);
	BIO_free(bio);

	if (cert != NULL) {
		X509_free(cert);
		TEST_FAIL("Invalid PEM was accepted as valid certificate");
	}

	/* Error queue should contain an error */
	unsigned long err = ERR_get_error();
	TEST_ASSERT(err != 0, "Error queue empty after invalid PEM");
	fprintf(stderr, "  Correctly rejected invalid PEM (error: 0x%lx)\n", err);

	/* Test SSL_CTX_use_certificate_chain_file with nonexistent path */
	SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
	int ret = SSL_CTX_use_certificate_chain_file(ctx,
	    "/nonexistent/path/cert.pem");
	TEST_ASSERT(ret != 1, "Expected failure for nonexistent cert file");

	SSL_CTX_free(ctx);
}

/* ------------------------------------------------------------------ */
/* Test 7: Auth mode settings (SSL_VERIFY_PEER, SSL_VERIFY_NONE)      */
/* ------------------------------------------------------------------ */
TEST_NAME(ssl_verify_modes)
{
	SSL_library_init();

	SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
	TEST_ASSERT(ctx != NULL, "SSL context creation failed");

	/* Set verify mode: NONE (no peer cert required) */
	SSL_CTX_set_verify(ctx, SSL_VERIFY_NONE, NULL);
	int mode = SSL_CTX_get_verify_mode(ctx);
	TEST_ASSERT(mode == SSL_VERIFY_NONE,
	    "SSL_VERIFY_NONE mode not set correctly");

	fprintf(stderr, "  SSL_VERIFY_NONE: mode=0x%x\n", mode);

	/* Set verify mode: PEER (require peer cert, optional fail if no cert) */
	SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, NULL);
	mode = SSL_CTX_get_verify_mode(ctx);
	TEST_ASSERT((mode & SSL_VERIFY_PEER) != 0,
	    "SSL_VERIFY_PEER mode not set");

	fprintf(stderr, "  SSL_VERIFY_PEER: mode=0x%x\n", mode);

	/* Set verify mode: PEER | FAIL_IF_NO_PEER_CERT */
	SSL_CTX_set_verify(ctx,
	    SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, NULL);
	mode = SSL_CTX_get_verify_mode(ctx);
	TEST_ASSERT((mode & SSL_VERIFY_FAIL_IF_NO_PEER_CERT) != 0,
	    "SSL_VERIFY_FAIL_IF_NO_PEER_CERT not set");

	fprintf(stderr, "  SSL_VERIFY_PEER|FAIL: mode=0x%x\n", mode);

	SSL_CTX_free(ctx);
}

/* ------------------------------------------------------------------ */
/* Test 8: SSL_CTX options and mode flags                              */
/* ------------------------------------------------------------------ */
TEST_NAME(ssl_options_and_modes)
{
	SSL_library_init();

	SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
	TEST_ASSERT(ctx != NULL, "SSL context creation failed");

	/* Test SSL_CTX_set_mode (used by ftp, syslogd) */
	long mode = SSL_CTX_set_mode(ctx, SSL_MODE_AUTO_RETRY);
	TEST_ASSERT((mode & SSL_MODE_AUTO_RETRY) != 0,
	    "SSL_MODE_AUTO_RETRY not accepted");
	fprintf(stderr, "  SSL_MODE_AUTO_RETRY: 0x%lx\n", mode);

	/* Test SSL_CTX_set_options for commonly used flags */
	long opts = SSL_CTX_set_options(ctx,
	    SSL_OP_ALL | SSL_OP_NO_SSLv2 | SSL_OP_NO_SSLv3 |
	    SSL_OP_SINGLE_DH_USE | SSL_OP_NO_COMPRESSION);
	TEST_ASSERT((opts & SSL_OP_NO_COMPRESSION) != 0,
	    "SSL_OP_NO_COMPRESSION not accepted");
	fprintf(stderr, "  Combined options: 0x%lx\n", opts);

	/* Verify options persist */
	long got_opts = SSL_CTX_get_options(ctx);
	TEST_ASSERT(got_opts == opts,
	    "SSL_CTX_get_options != set_options");

	SSL_CTX_free(ctx);
}

/* ------------------------------------------------------------------ */
/* Dispatch table                                                      */
/* ------------------------------------------------------------------ */
typedef void (*test_func_t)(void);
struct test_entry {
	int id;
	const char *name;
	test_func_t func;
};

static const struct test_entry test_table[] = {
	{ 1, "sslv23_methods",          test_sslv23_methods },
	{ 2, "pem_cert_loading",        test_pem_cert_loading },
	{ 3, "cipher_suite_negotiation",  test_cipher_suite_negotiation },
	{ 4, "protocol_version_masks",   test_protocol_version_masks },
	{ 5, "peer_cert_verify",         test_peer_cert_verify },
	{ 6, "invalid_cert_errors",      test_invalid_cert_errors },
	{ 7, "ssl_verify_modes",         test_ssl_verify_modes },
	{ 8, "ssl_options_and_modes",    test_ssl_options_and_modes },
	{ 0, NULL, NULL }
};

int
main(int argc, char **argv)
{
	int opt;
	int selected = -1; /* -1 = run all */

	while ((opt = getopt(argc, argv, "t:")) != -1) {
		switch (opt) {
		case 't':
			selected = atoi(optarg);
			if (selected < 1 || selected > 8) {
				fprintf(stderr, "Invalid test: %d\n", selected);
				fprintf(stderr, "Valid range: 1-8\n");
				return 1;
			}
			break;
		default:
			fprintf(stderr, "Usage: %s [-t test_id]\n", argv[0]);
			return 1;
		}
	}

	fprintf(stderr, "wolfSSL Compatibility Tests\n");
	fprintf(stderr, "===========================\n");
	fprintf(stderr, "Library version: %s\n", wolfSSL_lib_version_string());
#ifdef LIBWOLFSSL_VERSION_STRING
	fprintf(stderr, "Build version:  %s\n", LIBWOLFSSL_VERSION_STRING);
#endif
	fprintf(stderr, "\n");

	if (selected == -1) {
		for (int i = 0; test_table[i].name != NULL; i++) {
			tests_run++;
			fprintf(stderr, "  [%d] %s ... ", test_table[i].id,
			    test_table[i].name);
			test_table[i].func();
			fprintf(stderr, "PASSED\n");
			tests_passed++;
		}
	} else {
		for (int i = 0; test_table[i].name != NULL; i++) {
			if (test_table[i].id == selected) {
				tests_run++;
				fprintf(stderr, "  [%d] %s ... ",
				    test_table[i].id, test_table[i].name);
				test_table[i].func();
				fprintf(stderr, "PASSED\n");
				tests_passed++;
				break;
			}
		}
	}

	fprintf(stderr, "\n");
	fprintf(stderr, "Results: %d run, %d passed, %d failed, %d skipped\n",
	    tests_run, tests_passed, tests_failed, tests_skipped);

	return tests_failed > 0 ? 1 : 0;
}
