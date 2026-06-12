#	$NetBSD: Makefile,v 1.1 2026-06-12 wolfssl Exp $
#	wolfSSL library build for Minix

.include <bsd.own.mk>

WOLFSSL_DIST=	${.CURDIR}/../dist
WOLFSSL_SRCDIR=	${WOLFSSL_DIST}/src

# Include wolfSSL configuration
.include "${.CURDIR}/../../Makefile.wolfssl"

# Library name
LIB=	wolfssl

# wolfSSL core source files
SRCS=	wolfio.c ssl.c tls.c record.c tls13.c

# wolfCrypt source files
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
SRCS+=	socket.c
SRCS+=	tls13.c

# Additional wolfCrypt files
SRCS+=	arc4.c
SRCS+=	des3.c
SRCS+=	poly1305.c
SRCS+=	chacha20.c
SRCS+=	chacha20_poly1305.c

# Include wolfSSL source directories
.PATH:	${WOLFSSL_SRCDIR}
.PATH:	${WOLFSSL_SRCDIR}/wolfcrypt/src

# Include directories
CPPFLAGS+=	-I${WOLFSSL_DIST}
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfcrypt
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfcrypt/src
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfssl
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfssl/openssl

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
INCS=	wolfssl/ssl.h wolfssl/wolfcrypt/wc_port.h
INCSDIR=/usr/include

# Install all wolfSSL headers
INCSDIR=	/usr/include/wolfssl
INCS+=		wolfssl/ssl.h wolfssl/wolfcrypt/wc_port.h wolfssl/wolfcrypt/types.h
INCSDIR=	/usr/include/wolfssl/openssl
INCS+=		wolfssl/openssl/ssl.h wolfssl/openssl/x509.h wolfssl/openssl/evp.h
INCS+=		wolfssl/openssl/crypto.h wolfssl/openssl/dh.h wolfssl/openssl/bn.h
INCS+=		wolfssl/openssl/err.h wolfssl/openssl/rand.h wolfssl/openssl/pem.h

# wolfSSL-specific install
.if defined(__MINIX)
CPPFLAGS+=	-DWOLFSSL_MINIX
.endif

.include <bsd.lib.mk>
