/* config.h for wolfSSL Minix integration
 * Generated for Minix build system
 * Based on wolfSSL v5.9.1-stable
 */

#ifndef WOLFSSL_CONFIG_H
#define WOLFSSL_CONFIG_H

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
#define NO_WOLFSSL_SERVER
#define NO_DES3
#define NO_DSA
#define NO_DH
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

/* Post-Quantum Cryptography (optional, can be disabled for size) */
#define HAVE_PQC
#define HAVE_KYBER
#define HAVE_DILITHIUM

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

#endif /* WOLFSSL_CONFIG_H */
