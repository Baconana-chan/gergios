# Makefile for arch-dependent TTY code (AArch64)
.include <bsd.own.mk>

HERE=${.CURDIR}/arch/${MACHINE_ARCH}
.PATH:  ${HERE}

SRCS += pl011.c
