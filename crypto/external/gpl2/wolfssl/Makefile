#	$NetBSD: Makefile,v 1.1 2026-06-12 wolfssl Exp $
#	wolfSSL integration for Minix

.include <bsd.own.mk>

WOLFSSL_DIST=	${.CURDIR}/dist
WOLFSSL_SRCDIR=	${WOLFSSL_DIST}/src

# Include wolfSSL configuration
.include "${.CURDIR}/../../Makefile.wolfssl"

# Library directories
SUBDIR= lib

.if (${MKCRYPTO} != "no")
SUBDIR+= lib
.endif

# Build wolfSSL library
LIB=	wolfssl

# Source files from wolfSSL
SRCS=	wolfio.c ssl.c tls.c record.c tls13.c

# Include wolfSSL source directories
.PATH:	${WOLFSSL_SRCDIR}
.PATH:	${WOLFSSL_SRCDIR}/wolfcrypt/src

# Additional source files
SRCS+=	aes.c sha.c sha256.c sha512.c md5.c
SRCS+=	dh.c rsa.c dsa.c ecc.c
SRCS+=	random.c
SRCS+=	hmac.c
SRCS+=	error.c
SRCS+=	wc_port.c

# wolfCrypt source files
SRCS+=	sp_int.c

# Include directories
CPPFLAGS+=	-I${WOLFSSL_DIST}
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfcrypt
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfcrypt/src

# wolfSSL OpenSSL compatibility layer
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfssl
CPPFLAGS+=	-I${WOLFSSL_SRCDIR}/wolfssl/openssl

# Build flags
CFLAGS+=	-fPIC
CFLAGS+=	-O2

# Library dependencies
LDADD+=	-lm
DPADD+=	${LIBM}

# Install headers
INCS=	wolfssl/ssl.h wolfssl/wolfcrypt/wc_port.h
INCSDIR=/usr/include

# wolfSSL-specific install
.if defined(__MINIX)
CPPFLAGS+=	-DWOLFSSL_MINIX
.endif

.include <bsd.lib.mk>
