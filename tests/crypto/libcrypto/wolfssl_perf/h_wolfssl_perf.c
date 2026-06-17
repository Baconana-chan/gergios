/*
 * $NetBSD: h_wolfssl_perf.c,v 1.0 2026/06/17 00:00:00 minix Exp $
 *
 * Copyright (c) 2026 Minix Project
 * All rights reserved.
 *
 * Performance benchmark helper for wolfSSL migration.
 *
 * Benchmarks:
 *  1. AES-128-GCM encryption throughput
 *  2. SHA-256 hashing throughput
 *  3. RSA-2048 sign/verify operations per second
 *  4. DH-2048 key generation time
 *  5. TLS context creation overhead
 *  6. Memory usage (peak RSS estimation via mallinfo / sbrk)
 *  7. Combined SSL handshake simulation
 */

#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <time.h>

#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/err.h>
#include <wolfssl/openssl/rand.h>
#include <wolfssl/openssl/evp.h>
#include <wolfssl/openssl/aes.h>
#include <wolfssl/openssl/sha.h>
#include <wolfssl/openssl/rsa.h>
#include <wolfssl/openssl/dh.h>
#include <wolfssl/openssl/bn.h>
#include <wolfssl/openssl/ec.h>
#include <wolfssl/openssl/ecdsa.h>
#include <wolfssl/version.h>

/* ------------------------------------------------------------------ */
/* Timing helpers                                                      */
/* ------------------------------------------------------------------ */

static double
now(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return (double)ts.tv_sec + (double)ts.tv_nsec / 1.0e9;
}

#define TIME_THIS(label, iters, code) do {			\
	double _t0 = now();					\
	for (int _i = 0; _i < (iters); _i++) { code; }		\
	double _t1 = now();					\
	double _elapsed = (_t1 - _t0);				\
	double _per_sec = (iters) / _elapsed;			\
	printf("  %-30s %8d ops in %7.3f s  [%10.1f ops/s]\n",	\
	    (label), (int)(iters), _elapsed, _per_sec);		\
} while (0)

/* ------------------------------------------------------------------ */
/* Benchmark 1: AES-128-GCM encryption                                 */
/* ------------------------------------------------------------------ */
static void
bench_aes_gcm(void)
{
	unsigned char key[16] = {0};
	unsigned char iv[12] = {0};
	unsigned char plain[4096], cipher[4096 + 16];
	memset(plain, 0xAB, sizeof(plain));

	EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
	EVP_EncryptInit_ex(ctx, EVP_aes_128_gcm(), NULL, NULL, NULL);

	const int ITERS = 10000;
	TIME_THIS("AES-128-GCM encrypt (4 KB)", ITERS, {
		EVP_EncryptInit_ex(ctx, NULL, NULL, key, iv);
		int outl = sizeof(cipher);
		EVP_EncryptUpdate(ctx, cipher, &outl, plain, sizeof(plain));
		int tmpl;
		EVP_EncryptFinal_ex(ctx, cipher + outl, &tmpl);
	});

	EVP_CIPHER_CTX_free(ctx);
}

/* ------------------------------------------------------------------ */
/* Benchmark 2: SHA-256 hashing                                        */
/* ------------------------------------------------------------------ */
static void
bench_sha256(void)
{
	unsigned char data[4096], digest[32];
	memset(data, 0xCD, sizeof(data));

	/* Reuse context to avoid allocator overhead in measurement */
	EVP_MD_CTX *mdctx = EVP_MD_CTX_new();
	EVP_DigestInit_ex(mdctx, EVP_sha256(), NULL);

	const int ITERS = 50000;
	TIME_THIS("SHA-256 hash (4 KB)", ITERS, {
		EVP_DigestInit_ex(mdctx, NULL, NULL, NULL);
		EVP_DigestUpdate(mdctx, data, sizeof(data));
		EVP_DigestFinal_ex(mdctx, digest, NULL);
	});

	EVP_MD_CTX_free(mdctx);
}

/* ------------------------------------------------------------------ */
/* Benchmark 3: RSA-2048 sign / verify                                 */
/* ------------------------------------------------------------------ */
static void
bench_rsa_sign_verify(void)
{
	RSA *rsa = RSA_generate_key(2048, RSA_F4, NULL, NULL);
	if (!rsa) {
		printf("  %-30s  RSA keygen failed, skipping\n",
		    "RSA-2048 sign+verify");
		return;
	}

	unsigned char hash[32], sig[256];
	unsigned int siglen = sizeof(sig);
	memset(hash, 0xAA, sizeof(hash));

	const int ITERS = 500;
	TIME_THIS("RSA-2048 sign (SHA-256)", ITERS, {
		RSA_sign(NID_sha256WithRSAEncryption, hash, sizeof(hash),
		    sig, &siglen, rsa);
	});

	TIME_THIS("RSA-2048 verify (SHA-256)", ITERS, {
		RSA_verify(NID_sha256WithRSAEncryption, hash, sizeof(hash),
		    sig, siglen, rsa);
	});

	RSA_free(rsa);
}

/* ------------------------------------------------------------------ */
/* Benchmark 4: DH-2048 key generation                                 */
/* ------------------------------------------------------------------ */
static void
bench_dh_keygen(void)
{
	DH *dh = DH_new();
	/* Use 2048-bit DH group (RFC 3526 group 14) */
	static const unsigned char dh2048_p[] = {
		0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xC9,0x0F,0xDA,0xA2,
		0x21,0x68,0xC2,0x34,0xC4,0xC6,0x62,0x8B,0x80,0xDC,0x1C,0xD1,
		0x29,0x02,0x4E,0x08,0x8A,0x67,0xCC,0x74,0x02,0x0B,0xBE,0xA6,
		0x3B,0x13,0x9B,0x22,0x51,0x4A,0x08,0x79,0x8E,0x34,0x04,0xDD,
		0xEF,0x95,0x19,0xB3,0xCD,0x3A,0x43,0x1B,0x30,0x2B,0x0A,0x6D,
		0xF2,0x5F,0x14,0x37,0x4F,0xE1,0x35,0x6D,0x6D,0x51,0xC2,0x45,
		0xE4,0x85,0xB5,0x76,0x62,0x5E,0x7E,0xC6,0xF4,0x4C,0x42,0xE9,
		0xA6,0x37,0xED,0x6B,0x0B,0xFF,0x5C,0xB6,0xF4,0x06,0xB7,0xED,
		0xEE,0x38,0x6B,0xFB,0x5A,0x89,0x9F,0xA5,0xAE,0x9F,0x24,0x11,
		0x7C,0x4B,0x1F,0xE6,0x49,0x28,0x66,0x51,0xEC,0xE6,0x53,0x81,
		0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF,0xFF };
	static const unsigned char dh2048_g[] = { 0x02 };

	BIGNUM *p = BN_bin2bn(dh2048_p, sizeof(dh2048_p), NULL);
	BIGNUM *g = BN_bin2bn(dh2048_g, sizeof(dh2048_g), NULL);
	DH_set0_pqg(dh, p, NULL, g); /* takes ownership */

	const int ITERS = 200;
	TIME_THIS("DH-2048 key generation", ITERS, {
		DH *d = DH_new();
		BIGNUM *bp = BN_dup(p);
		BIGNUM *bg = BN_dup(g);
		DH_set0_pqg(d, bp, NULL, bg);
		DH_generate_key(d);
		DH_free(d);
	});

	DH_free(dh);
}

/* ------------------------------------------------------------------ */
/* Benchmark 5: TLS context creation overhead                          */
/* ------------------------------------------------------------------ */
static void
bench_tls_context(void)
{
	SSL_library_init();
	SSL_load_error_strings();

	const int ITERS = 5000;
	TIME_THIS("SSL_CTX_new + SSL_new + SSL_free + SSL_CTX_free", ITERS, {
		SSL_CTX *ctx = SSL_CTX_new(SSLv23_method());
		if (ctx) {
			SSL *ssl = SSL_new(ctx);
			if (ssl) SSL_free(ssl);
			SSL_CTX_free(ctx);
		}
	});
}

/* ------------------------------------------------------------------ */
/* Benchmark 6: Memory usage estimate (via mallinfo / sbrk)            */
/* ------------------------------------------------------------------ */
#if defined(__linux__) || defined(__NetBSD__)
#include <malloc.h>
#endif

static long
get_allocated_bytes(void)
{
#if defined(__linux__) || defined(__NetBSD__)
#if defined(HAVE_MALLINFO)
	struct mallinfo mi = mallinfo();
	return (long)mi.uordblks;
#else
	/* Fallback: approximate via sbrk */
	static void *initial_brk = NULL;
	if (!initial_brk)
		initial_brk = sbrk(0);
	return (long)((char *)sbrk(0) - (char *)initial_brk);
#endif
#else
	return -1;
#endif
}

static void
bench_memory_usage(void)
{
	printf("  %-30s  ", "Memory baseline (estimated)");

	long before = get_allocated_bytes();
	if (before < 0) {
		printf("not available on this platform\n");
		return;
	}

	/* Allocate several SSL contexts + connections */
	SSL_CTX *ctxs[10];
	SSL *ssls[10];
	SSL_library_init();
	for (int i = 0; i < 10; i++) {
		ctxs[i] = SSL_CTX_new(SSLv23_method());
		ssls[i] = SSL_new(ctxs[i]);
	}
	long after = get_allocated_bytes();

	for (int i = 0; i < 10; i++) {
		SSL_free(ssls[i]);
		SSL_CTX_free(ctxs[i]);
	}

	long diff = after - before;
	printf("~%ld KB for 10 SSL connections\n",
	    diff > 0 ? diff / 1024 : 0);
}

/* ------------------------------------------------------------------ */
/* Benchmark 7: SSL handshake simulation (purely in-process)           */
/* ------------------------------------------------------------------ */
static void
bench_tls_handshake(void)
{
	SSL_library_init();

	const int ITERS = 200;
	TIME_THIS("TLS handshake (in-process simulation)", ITERS, {
		/* Server side */
		SSL_CTX *srv_ctx = SSL_CTX_new(SSLv23_server_method());
		SSL *srv_ssl = SSL_new(srv_ctx);

		/* Client side */
		SSL_CTX *cli_ctx = SSL_CTX_new(SSLv23_client_method());
		SSL *cli_ssl = SSL_new(cli_ctx);

		/* Set non-blocking to simulate handshake start */
		SSL_set_connect_state(cli_ssl);
		SSL_set_accept_state(srv_ssl);

		SSL_free(cli_ssl);
		SSL_free(srv_ssl);
		SSL_CTX_free(cli_ctx);
		SSL_CTX_free(srv_ctx);
	});
}

/* ------------------------------------------------------------------ */
/* Benchmark 8: ECDSA P-256 key generation                             */
/* ------------------------------------------------------------------ */
static void
bench_ecc_keygen(void)
{
	EC_KEY *ec = EC_KEY_new_by_curve_name(NID_X9_62_prime256v1);
	if (!ec) {
		printf("  %-30s  ECC not available, skipping\n",
		    "ECDSA P-256 key generation");
		return;
	}

	const int ITERS = 2000;
	TIME_THIS("ECDSA P-256 key generation", ITERS, {
		EC_KEY *k = EC_KEY_new_by_curve_name(NID_X9_62_prime256v1);
		if (k) {
			EC_KEY_generate_key(k);
			EC_KEY_free(k);
		}
	});

	EC_KEY_free(ec);
}

/* ------------------------------------------------------------------ */
/* Test dispatch                                                       */
/* ------------------------------------------------------------------ */
typedef void (*bench_func_t)(void);
struct bench_entry {
	int id;
	const char *name;
	bench_func_t func;
};

static const struct bench_entry bench_table[] = {
	{ 1, "AES-128-GCM encryption",   bench_aes_gcm },
	{ 2, "SHA-256 hashing",          bench_sha256 },
	{ 3, "RSA-2048 sign+verify",     bench_rsa_sign_verify },
	{ 4, "DH-2048 key generation",   bench_dh_keygen },
	{ 5, "TLS context allocation",   bench_tls_context },
	{ 6, "Memory usage",             bench_memory_usage },
	{ 7, "TLS handshake simulation", bench_tls_handshake },
	{ 8, "ECDSA P-256 key generation", bench_ecc_keygen },
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
				fprintf(stderr, "Invalid benchmark: %d\n",
				    selected);
				fprintf(stderr, "Valid range: 1-8\n");
				return 1;
			}
			break;
		default:
			fprintf(stderr, "Usage: %s [-t benchmark_id]\n", argv[0]);
			return 1;
		}
	}

	printf("wolfSSL Performance Benchmarks\n");
	printf("==============================\n");
	printf("Library version: %s\n", wolfSSL_lib_version_string());
	printf("\n");

	printf("Running benchmarks...\n");
	printf("\n");

	int run = 0, failed = 0;
	for (int i = 0; bench_table[i].name != NULL; i++) {
		if (selected != -1 && bench_table[i].id != selected)
			continue;
		run++;
		printf("--- Benchmark %d: %s ---\n",
		    bench_table[i].id, bench_table[i].name);
		bench_table[i].func();
		printf("\n");
	}

	printf("Benchmarks complete: %d run\n", run);
	return failed > 0 ? 1 : 0;
}
