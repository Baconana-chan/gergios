
.include <bsd.own.mk>

DISTDIR=	${NETBSDSRCDIR}/minix/lib/liblwip/dist
SRCDIR=		${DISTDIR}/src

LIB=lwip

.PATH:		${SRCDIR}/core ${SRCDIR}/core/ipv4 ${SRCDIR}/core/ipv6
.include	"${.CURDIR}/core/Makefile.inc"

.PATH:		${SRCDIR}/netif
.include	"${.CURDIR}/netif/Makefile.inc"

# MINIX-specific lwIP support modules
.PATH:		${.CURDIR}
.PATH:		${NETBSDSRCDIR}/minix/lib/liblwip/wireguard
.PATH:		${NETBSDSRCDIR}/minix/lib/liblwip/wireguard/crypto/refc

SRCS += lwipsyncookie.c lwip_ratelimit.c lwip_tcp_md5.c lwip_ipsec.c
SRCS += lwip_dtls.c
SRCS += wireguard-platform.c
SRCS += wireguard.c wireguardif.c crypto.c
SRCS += blake2s.c chacha20.c chacha20poly1305.c poly1305-donna.c x25519.c

# wolfSSL include paths (for DTLS support)
WOLFSSLSRC=	${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=	-I${WOLFSSLSRC}
CPPFLAGS+=	-I${WOLFSSLSRC}/src
CPPFLAGS+=	-I${WOLFSSLSRC}/wolfcrypt
CPPFLAGS+=	-I${WOLFSSLSRC}/wolfssl

CPPFLAGS+=	-I${NETBSDSRCDIR}/minix/lib/liblwip/wireguard
CPPFLAGS+=	-I${NETBSDSRCDIR}/minix/lib/liblwip/wireguard/crypto/refc

CPPFLAGS+=	-D_MINIX_SYSTEM
CPPFLAGS+=	-I${SRCDIR}/include -I${.CURDIR}
CPPFLAGS+=	-Wno-empty-body

WARNS?= 5

.include <bsd.lib.mk>
