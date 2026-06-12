PROG=	fetch
BINDIR=	/usr/bin
MAN=	fetch.1

CPPFLAGS+= -DHAVE_SYS_PARAM_H \
		   -DHAVE_SYS_IOCTL_H \
		   -DHAVE_SYS_SOCKET_H \
		   -DHAVE_SYS_STAT_H \
		   -DHAVE_SYS_TIME_H \
		   -DHAVE_ERR_H \
		   -DHAVE_STDINT_H \
		   -DHAVE_TERMIOS_H \
		   -DHAVE_SYSEXITS_H \
		   -DHAVE_UTIME_H

.include <bsd.own.mk>

DPADD+=	${LIBFETCH}
LDADD+=	-lfetch

.if ${MKCRYPTO} == "yes"
# wolfSSL migration: replace OpenSSL with wolfSSL
DPADD+=	${LIBWOLFSSL}
LDADD+=	-lwolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl
.endif

.include <bsd.prog.mk>
