#	$NetBSD: Makefile,v 1.1 2026-06-12 wolfssl Exp $
#	wolfSSL library build for Minix

.include <bsd.own.mk>

WOLFSSL_DIST=	${.CURDIR}/../dist
WOLFSSL_SRCDIR=	${WOLFSSL_DIST}/src
WOLFSSL_WOLFSSLDIR=	${WOLFSSL_DIST}/wolfssl
WOLFSSL_WOLFCRYPTDIR=	${WOLFSSL_DIST}/wolfcrypt

# Include wolfSSL configuration
.include "${.CURDIR}/../../Makefile.wolfssl"

# Include custom config.h
CPPFLAGS+=	-I${.CURDIR}/..

# Library name
LIB=	wolfssl

# wolfSSL core source files (from src/)
SRCS=	wolfio.c ssl.c tls.c record.c tls13.c
SRCS+=	internal.c keys.c
SRCS+=	ssl_api_cert.c ssl_api_crl_ocsp.c ssl_api_pk.c
SRCS+=	ssl_asn1.c ssl_bn.c ssl_certman.c ssl_crypto.c
SRCS+=	ssl_load.c ssl_misc.c ssl_p7p12.c ssl_sess.c ssl_sk.c
SRCS+=	x509.c x509_str.c
SRCS+=	bio.c conf.c crl.c ocsp.c pk.c pk_ec.c pk_rsa.c
SRCS+=	quic.c sniffer.c dtls.c dtls13.c

# wolfCrypt source files (from wolfcrypt/src/)
SRCS+=	aes.c sha.c sha256.c sha512.c md5.c
SRCS+=	dh.c rsa.c dsa.c ecc.c curve25519.c ed25519.c
SRCS+=	random.c
SRCS+=	hmac.c
SRCS+=	error.c
SRCS+=	wc_port.c
SRCS+=	sp_int.c
SRCS+=	asn.c
SRCS+=	pkcs7.c
SRCS+=	coding.c
SRCS+=	digest.c
SRCS+=	signature.c
SRCS+=	logging.c
SRCS+=	hash.c
SRCS+=	memory.c
SRCS+=	misc.c
SRCS+=	pwdbased.c
SRCS+=	evp.c evp_pk.c
SRCS+=	cryptocb.c
SRCS+=	cpuid.c
SRCS+=	integer.c
SRCS+=	chacha.c
SRCS+=	poly1305.c
SRCS+=	chacha20_poly1305.c
SRCS+=	kdf.c
SRCS+=	hpke.c
SRCS+=	arc4.c
SRCS+=	des3.c

# Additional wolfCrypt files
SRCS+=	tfm.c
SRCS+=	wolfmath.c
SRCS+=	wc_encrypt.c
SRCS+=	wc_dsp.c
SRCS+=	wolfevent.c
SRCS+=	wolfentropy.c
SRCS+=	rng_bank.c
SRCS+=	async.c
SRCS+=	compress.c

# Include wolfSSL source directories
.PATH:	${WOLFSSL_SRCDIR}
.PATH:	${WOLFSSL_DIST}/wolfcrypt/src

# Include directories
CPPFLAGS+=	-I${WOLFSSL_DIST}
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}
CPPFLAGS+=	-I${WOLFSSL_WOLFCRYPTDIR}
CPPFLAGS+=	-I${WOLFSSL_DIST}/wolfcrypt/src
CPPFLAGS+=	-I${WOLFSSL_WOLFSSLDIR}
CPPFLAGS+=	-I${WOLFSSL_WOLFSSLDIR}/openssl
CPPFLAGS+=	-I${WOLFSSL_WOLFSSLDIR}/wolfcrypt

# Build flags
CFLAGS+=	-fPIC
CFLAGS+=	-O2
CFLAGS+=	-Wall

# Library dependencies
LDADD+=	-lm
DPADD+=	${LIBM}

# Shared library version
SHLIB_MAJOR=	44
SHLIB_MINOR=	2
SHLIB_TEENY=	0

# Install headers
INCSDIR=	/usr/include/wolfssl
INCS=		${WOLFSSL_WOLFSSLDIR}/ssl.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfio.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/version.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/error-ssl.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/internal.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/callbacks.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/crl.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/ocsp.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/quic.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/sniffer.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/test.h

# Install wolfCrypt headers
INCSDIR=	/usr/include/wolfssl/wolfcrypt
INCS=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/aes.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/sha.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/sha256.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/sha512.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/md5.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/dh.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/rsa.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/dsa.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/ecc.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/curve25519.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/ed25519.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/random.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/hmac.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/error.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/wc_port.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/types.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/asn.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/pkcs7.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/coding.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/digest.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/signature.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/logging.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/hash.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/memory.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/misc.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/pwdbased.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/evp.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/cryptocb.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/cpuid.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/integer.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/chacha.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/poly1305.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/chacha20_poly1305.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/kdf.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/hpke.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/arc4.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/des3.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/tfm.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/wolfcrypt/wolfmath.h

# Install OpenSSL compatibility headers
INCSDIR=	/usr/include/wolfssl/openssl
INCS=		${WOLFSSL_WOLFSSLDIR}/openssl/ssl.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/x509.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/evp.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/crypto.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/dh.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/bn.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/err.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/rand.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/pem.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/sha.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/md5.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/rsa.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/dsa.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/ec.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/ecdh.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/ecdsa.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/asn1.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/bio.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/objects.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/opensslv.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/tls1.h
INCS+=		${WOLFSSL_WOLFSSLDIR}/openssl/opensslconf.h

# wolfSSL-specific install
.if defined(__MINIX)
CPPFLAGS+=	-DWOLFSSL_MINIX
.endif

.include <bsd.lib.mk>
