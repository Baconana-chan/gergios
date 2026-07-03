/* config.h for wolfSSL Minix integration
 * Generated for Minix build system
 * Based on wolfSSL v5.9.1-stable
 */

#ifndef WOLFSSL_CONFIG_H
#define WOLFSSL_CONFIG_H

/* ============================================================================
 * Bare-metal overrides (x86_64-elf / aarch64-elf targets without libc).
 * These are defined here so wolfSSL compiles without a full POSIX toolchain.
 * When using a full MINIX cross-toolchain with libc, these may be
 * overridden by the toolchain's system headers.
 * ========================================================================== */

/* Socket/network overrides for bare-metal builds.
 * - Define WOLFSSL_NO_SOCK to skip socket-dependent code paths
 * - Define WOLFSSL_IP4/6 directly (not as AF_INET) to avoid
 *   requiring <sys/socket.h> in IP address parsing code.
 * - Define WOLFSSL_SOCKADDR_IN6 as void* since struct sockaddr_in6
 *   is not available on bare-metal ELF targets.
 * - Define XINET_PTON as a stub to avoid inet_pton() calls.
 *
 * Note: AF_INET/AF_INET6 are intentionally NOT defined here.
 * On MINIX (NetBSD-derived), sys/sys/socket.h defines them
 * (AF_INET=2, AF_INET6=24). When sys/sys/socket.h is not
 * available (pure bare-metal), WOLFSSL_NO_SOCK prevents
 * compilation of code paths that reference them directly. */
#ifndef WOLFSSL_NO_SOCK
#define WOLFSSL_NO_SOCK
#endif
#ifndef WOLFSSL_IP4
#define WOLFSSL_IP4  2
#endif
#ifndef WOLFSSL_IP6
#define WOLFSSL_IP6  24  /* NetBSD uses 24 for AF_INET6 */
#endif
#ifndef WOLFSSL_SOCKADDR_IN6
/* Cannot use void* here because C treats 'void* a, b;' as 'void *a; void b;'
 * which is invalid. Use a struct typedef instead. */
#define WOLFSSL_SOCKADDR_IN6 wolfSSL_SockAddrIn6
#endif
#ifndef WOLFSSL_SOCKADDR_IN6_STUB_DEFINED
#define WOLFSSL_SOCKADDR_IN6_STUB_DEFINED
typedef struct {
    unsigned char addr[16];  /* IPv6 address (128 bits) */
    unsigned int scope_id;   /* scope ID */
} wolfSSL_SockAddrIn6;
#endif

/* EmbedReceive/EmbedSend stub declarations.
 * These are normally defined in wolfssl/wolfio.h inside
 * #if defined(USE_WOLFSSL_IO), but settings.h auto-defines
 * WOLFSSL_USER_IO which prevents USE_WOLFSSL_IO from being
 * auto-defined. Define them here so internal.c can compile.
 * The actual implementations are in wolfSSL's src/io.c.
 *
 * NOTE: We must NOT use WOLFSSL_LOCAL or WOLFSSL* types here
 * because config.h is force-included BEFORE wolfSSL headers,
 * so those types/macros are not yet defined. Use struct forward
 * declaration and plain extern instead. */
struct WOLFSSL;  /* forward decl, compatible with typedef later */

#ifndef CONFIG_H_EMBED_RECEIVE
#define CONFIG_H_EMBED_RECEIVE
#define EmbedReceive wolfSSL_EmbedReceive
#endif
#ifndef CONFIG_H_EMBED_SEND
#define CONFIG_H_EMBED_SEND
#define EmbedSend wolfSSL_EmbedSend
#endif
#ifndef CONFIG_H_EMBED_RECEIVE_FROM
#define CONFIG_H_EMBED_RECEIVE_FROM
#define EmbedReceiveFrom wolfSSL_EmbedReceiveFrom
#endif
#ifndef CONFIG_H_EMBED_SEND_TO
#define CONFIG_H_EMBED_SEND_TO
#define EmbedSendTo wolfSSL_EmbedSendTo
#endif

/* Function declarations (extern by default for functions in C) */
extern int wolfSSL_EmbedReceive(struct WOLFSSL* ssl, char* buf, int sz, void* ctx);
extern int wolfSSL_EmbedSend(struct WOLFSSL* ssl, char* buf, int sz, void* ctx);
extern int wolfSSL_EmbedReceiveFrom(struct WOLFSSL* ssl, char* buf, int sz, void* ctx);
extern int wolfSSL_EmbedSendTo(struct WOLFSSL* ssl, char* buf, int sz, void* ctx);


/* XINET_PTON macro — wolfSSL wraps inet_pton() with this.
 * On bare-metal, we can't call inet_pton(). Define as returning 0
 * (parse failure) since IP constraint parsing in certificates
 * is non-critical and MINIX servers rarely use IP-based constraints.
 * Note: XINET_PTON may also be defined in wolfio.h; we define it here
 * first so the wolfio.h #ifndef XINET_PTON sees our version. */
#ifndef XINET_PTON
#define XINET_PTON(af, src, dst)  ((void)(af), (void)(src), (void)(dst), 0)
#endif

/* Minix-specific configuration */
#define WOLFSSL_MINIX
#define WOLFSSL_NO_SCTP
#define WOLFSSL_SMALL_STACK
#define WOLFSSL_NO_OLD_TLS

/* OpenSSL compatibility layer - CRITICAL for Minix migration */
#define OPENSSL_EXTRA
#define OPENSSL_EXTRA_X509_SMALL
#define OPENSSL_ALL
#define WOLFSSL_OPENSSL_COMPATIBLE

/* Cryptographic algorithms - Required for Minix compatibility */
#define HAVE_AESGCM
#define HAVE_CHACHA
#define HAVE_POLY1305
#define HAVE_ECC
#define HAVE_CURVE25519
#define HAVE_ED25519
#define HAVE_DH
#define HAVE_RSA
#define HAVE_DSA
#define HAVE_SHA
#define HAVE_SHA256
#define HAVE_SHA512
#define HAVE_MD5
#define HAVE_HMAC
#define HAVE_PKCS7
#define HAVE_ASN
#define HAVE_CODING
#define HAVE_EVP
#define HAVE_PKCS12

/* TLS versions */
#define WOLFSSL_TLS13
#define WOLFSSL_DTLS
#define WOLFSSL_DTLS13
#define NO_SSLV2
#define NO_SSLV3
#define NO_TLSV1
#define NO_TLSV1_1

/* Disabled features for size reduction */
#define NO_MD4
#define NO_RC4
#define NO_PSK
#define NO_HC128
#define NO_RABBIT
#define NO_WOLFSSL_CLIENT
/* Note: WOLFSSL_SERVER is intentionally left enabled for MINIX.
 * MINIX components (syslogd, etc.) act as TLS servers.
 * NO_WOLFSSL_SERVER is NOT defined here. */
/* #define NO_WOLFSSL_SERVER */
#define NO_DES3
/* Note: NO_DSA and NO_DH intentionally NOT defined here.
 * HAVE_DSA and HAVE_DH are defined above for BIND and syslogd.
 * If DH/DSA are not needed, add -DNO_DH -DNO_DSA to CPPFLAGS. */
#define NO_OLD_TLS

/* Performance optimizations */
#define FAST_MATH
#define SMALL_STACK
/* Note: SINGLE_THREADED removed to avoid multi-core processing bug */
#define TFM_TIMING_RESISTANT
#define ECC_TIMING_RESISTANT

/* Memory management */
#define WOLFSSL_MALLOC
#define WOLFSSL_FREE
#define WOLFSSL_CALLOC
#define WOLFSSL_REALLOC
#define WOLFSSL_STATIC_MEMORY

/* File system */
#define WOLFSSL_NO_FILESYSTEM
#define NO_WRITE_TEMP_KEY

/* Threading */
#define SINGLE_THREADED
#define WOLFSSL_NO_THREADS

/* Error handling */
#define WOLFSSL_ERROR_CODE_OPENSSL
#define DEBUG_WOLFSSL_VERBOSE

/* Certificate handling */
#define WOLFSSL_CERT_GEN
#define WOLFSSL_CERT_REQ
#define WOLFSSL_CERT_EXT
#define WOLFSSL_CERTIFICATE_PARSING
#define WOLFSSL_KEY_GEN

/* ASN.1 */
#define WOLFSSL_ASN_TEMPLATE
#define HAVE_OID_ENCODING
#define HAVE_OID_DECODING

/* Key exchange */
#define HAVE_ECC
#define HAVE_CURVE25519
#define HAVE_ED25519
#define HAVE_ECDH
#define HAVE_ECDSA

/* Random number generation */
#define HAVE_HASHDRBG
#define WOLFSSL_GENSEED_FORTEST
#define NO_DEV_RANDOM
#define NO_FILESYSTEM

/* I/O */
#define WOLFSSL_DTLS
#define WOLFSSL_DTLS13
#define WOLFSSL_IO
#define WOLFSSL_NTP

/* Additional features for Minix compatibility */
#define HAVE_BIO
#define HAVE_CONF
#define HAVE_OCSP
#define HAVE_CRL
#define HAVE_PKCS8
#define HAVE_PKCS12
#define HAVE_X509
#define HAVE_X509_EXT
#define HAVE_X509_VERIFY

/* Post-Quantum Cryptography — disabled for bare-metal builds.
 * The wolfSSL dist version has internal consistency issues with
 * Dilithium/ML-DSA key size constants in ssl_load.c.
 * MINIX doesn't need PQC. Enable when wolfSSL is upgraded. */
/* #define HAVE_PQC */
/* #define HAVE_KYBER */
/* #define HAVE_DILITHIUM */

/* Additional cryptographic algorithms */
#define HAVE_BLAKE2B
#define HAVE_BLAKE2S
#define HAVE_SHA3
#define HAVE_SIPHASH

/* KDF and HPKE */
#define HAVE_KDF
#define HAVE_HKDF
#define HAVE_HPKE

/* Certificate compression */
#define HAVE_CERT_COMPRESSION

/* Session handling */
#define SESSION_CERTS
#define SESSION_INDEX
#define HAVE_SESSION_TICKET

/* Certificate verification */
#define HAVE_OCSP
#define HAVE_CRL
#define HAVE_CRL_MONITOR

/* Additional OpenSSL compatibility */
#define HAVE_OPENSSL_COMPATIBLE
#define HAVE_OPENSSL_COMPATIBLE_NAMES

/* Build configuration */
#define WOLFSSL_LIB
#define WOLFSSL_SSL
#define WOLFSSL_CRYPTO

/* TLS 1.3 requires explicit DH key size configuration.
 * Use FFDHE_2048 (minimum for TLS 1.3 compliance).
 * Also need WC_RSA_PSS for TLS 1.3 RSA signature schemes. */
#define HAVE_FFDHE_2048
#define WC_RSA_PSS

#endif /* WOLFSSL_CONFIG_H */
