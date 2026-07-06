#	depmod Makefile — GergiOS Module Dependency Generator
#
# Generates modules.dep, modules.alias, modules.symbols from .modinfo.

PROG=	depmod
# MAN=	depmod.8  (man page not yet written)

DPADD+=	${LIBGERGIOS_DRIVER}
LDADD+=	-lgergios_driver

.include <bsd.prog.mk>
