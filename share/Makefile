#	$NetBSD: Makefile,v 1.33 2014/08/08 09:34:10 apb Exp $
#	from @(#)Makefile	8.1 (Berkeley) 6/5/93

# Missing:  ms

.include <bsd.own.mk>
#__MINIX: dict doc examples me \
#         tabset tmac wscons xml

.if ${MKSHARE} != "no" || \
	make(clean) || make(cleandir) || make(distclean) || make(obj)
.if defined(__MINIX)
# MINIX minimal: только mk/ (BSD Make инфраструктура);
# man, misc, terminfo, locale, i18n — data files из pkgsrc
SUBDIR=	mk
.if ${MKSHARE} != "no"
SUBDIR+=	legal
.endif
.else
SUBDIR=	legal man misc mk \
	terminfo
.if ${MKNLS} != "no"
SUBDIR+=i18n locale nls
.endif
.endif
.endif

# Speedup stubs for some subtrees that don't need to run these rules
includes-examples includes-man:
	@true

.include <bsd.subdir.mk>
