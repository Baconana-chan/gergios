# $NetBSD: Makefile,v 1.0 2026/06/17 00:00:00 minix Exp $

.include <bsd.own.mk>

.if ${MKCRYPTO} != "no" && ${MKATF} != "no"

TESTSDIR=	${TESTSBASE}/integration

TESTS_SH=	t_syslogd_tls
TESTS_SH+=	t_ftp_ssl
TESTS_SH+=	t_httpd_ssl
TESTS_SH+=	t_telnet_encrypt
TESTS_SH+=	t_bind_dnssec
TESTS_SH+=	t_cross_component

BINDIR=		${TESTSDIR}

.include <bsd.test.mk>

.else
NOPROG=
.include <bsd.prog.mk>
.endif
