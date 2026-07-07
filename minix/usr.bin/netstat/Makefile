# Makefile for netstat — network statistics utility

.include <bsd.own.mk>

PROG=	netstat
SRCS=	netstat.c

DPADD+=	${LIBSYS}
LDADD+=	-lsys

WARNS?=	5

.include <bsd.prog.mk>
