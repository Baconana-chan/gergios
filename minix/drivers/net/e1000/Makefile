# Makefile for the Intel Pro/1000 driver.
PROG=	e1000
SRCS=	e1000.c

FILES=${PROG}.conf
FILESNAME=${PROG}
FILESDIR= /etc/system.conf.d

DPADD+=	${LIBNETDRIVER} ${LIBSYS} ${LIBGERGIOSDRIVER}
LDADD+=	-lnetdriver -lsys -lgergios_driver

# Include path for libgergios_driver headers
CPPFLAGS+=	-I${.CURDIR}/../../../lib/libgergios_driver

WARNS?= 5

.include <minix.service.mk>
