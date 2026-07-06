# Makefile for lsmod — list loaded kernel modules

.include <bsd.own.mk>

PROG=   lsmod
SRCS=   lsmod.c
MAN=

BINDIR?= /usr/sbin

DPADD+= ${LIBSYS}
LDADD+= -lsys

.include <bsd.prog.mk>
