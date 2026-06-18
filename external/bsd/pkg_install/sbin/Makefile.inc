# $NetBSD: Makefile.inc,v 1.3 2010/11/05 09:09:01 he Exp $

.include <bsd.own.mk>

.include "${.PARSEDIR}/../Makefile.inc"

LIBINSTALL != cd ${.PARSEDIR}/../lib && ${PRINTOBJDIR}

BINDIR?=	/usr/sbin
CPPFLAGS+=	-DBINDIR='"${BINDIR}"'

DPADD+=	${LIBINSTALL}/libinstall.a
LDADD+=	-L${LIBINSTALL} -linstall -ltermcap

# wolfSSL migration: replace OpenSSL with wolfSSL
DPADD+=		${LIBFETCH} ${LIBWOLFSSL}
LDADD+=		-lfetch -lwolfssl

CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl

DPADD+=		${LIBARCHIVE}
LDADD+=		-larchive

DPADD+=		${LIBZ} ${LIBBZ2}
LDADD+=		-lz -lbz2

DPADD+=		${LIBLZMA}
LDADD+=		-llzma
