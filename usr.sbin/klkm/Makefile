# Makefile for KLKM — GergiOS Driver Manager daemon + CLI tools

.include <bsd.own.mk>

PROG=   klkm
SRCS=   klkm.c

MAN=

BINDIR?= /usr/sbin

# Symlinks for CLI tools
LINKS+= ${BINDIR}/klkm ${BINDIR}/modprobe
LINKS+= ${BINDIR}/klkm ${BINDIR}/insmod
LINKS+= ${BINDIR}/klkm ${BINDIR}/rmmod

# Link against libgergios_driver for driver manager + modprobe + elf loader
LDADD+= -lgergios_driver

# Library search path for libgergios_driver
LDFLAGS+= -L${NETBSDSRCDIR}/minix/lib/libgergios_driver

# Standard system library
LDADD+= -lsys

# Include paths for minix headers
CPPFLAGS+= -I${NETBSDSRCDIR}/minix
CPPFLAGS+= -I${NETBSDSRCDIR}/minix/include

# Include paths for libgergios_driver headers
CPPFLAGS+= -I${NETBSDSRCDIR}/minix/lib/libgergios_driver

.include <bsd.prog.mk>
