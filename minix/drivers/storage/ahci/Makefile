# Makefile for the Advanced Host Controller Interface driver (AHCI)
PROG=	ahci
SRCS=	ahci.c

DPADD+=	${LIBBLOCKDRIVER} ${LIBSYS} ${LIBTIMERS} ${LIBMTHREAD} ${LIBGERGIOSDRIVER}
LDADD+=	-lblockdriver -lsys -ltimers -lmthread -lgergios_driver

# Include path for libgergios_driver headers
CPPFLAGS+=	-I${.CURDIR}/../../../lib/libgergios_driver

.include <minix.service.mk>
