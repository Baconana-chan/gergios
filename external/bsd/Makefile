#	$NetBSD: Makefile,v 1.54 2015/09/28 17:12:46 christos Exp $
#	$GergiOS: Cleaned up external/bsd/Makefile — pkgsrc-only packages removed

.include <bsd.own.mk>

# GergiOS core: only essential packages that are not easily replaceable via pkgsrc
SUBDIR=	dhcpcd \
	libarchive libevent libpcap

.if (${MKATF} != "no")
SUBDIR+= atf
.endif
.if (${MKCRYPTO} != "no")
SUBDIR+= pkg_install
.endif
.if (${MKBYACC} != "no")
SUBDIR+= byacc
.endif
.if (${MKBZIP2} != "no")
SUBDIR+= bzip2
.endif
.if (${MKFETCH} != "no")
SUBDIR+= fetch
.endif
.if (${MKFILE} != "no")
SUBDIR+= file
.endif
.if (${MKFLEX} != "no")
SUBDIR+= flex
.endif
.if (${MKLESS} != "no")
SUBDIR+= less
.endif
.if (${MKMDOCML} != "no")
SUBDIR+= mdocml
.endif
.if (${MKOPENRESOLV} != "no")
SUBDIR+= openresolv
.endif
.if (${MKTCPDUMP} != "no")
SUBDIR+= tcpdump
.endif
.if (${MKTMUX} != "no")
SUBDIR+= tmux
.endif
.if (${MKTOP} != "no")
SUBDIR+= top
.endif
.if (${MKBIND} != "no")
SUBDIR+= bind
.endif
.if (${MKDHCP} != "no")
SUBDIR+= dhcp
.endif
.if (${MKBLACKLIST} != "no")
SUBDIR+= blacklist
.endif
.if (${MKNVI} != "no")
SUBDIR+= nvi
.endif
.if (${MKCTWM} != "no")
SUBDIR+= ctwm
.endif
# IP Filter
.if (${MKIPFILTER} != "no")
SUBDIR+=ipf
.endif
.if (${MKISCSI} != "no")
SUBDIR+= iscsi 
.endif
.if (${MKKYUA} != "no")
SUBDIR+= lutok kyua-testers kyua-cli kyua-atf-compat
.endif
.if (${MKLDAP} != "no")
SUBDIR+= openldap
.endif
.if (${MKLIBCXX} != "no")
SUBDIR+= libc++
.endif
.if (${MKLLVM} != "no")
.if (${MKLLVMCMDS} != "no") && defined(__MINIX)
SUBDIR+= llvm
.endif # (${MKLLVMCMDS} != "no") && defined(__MINIX)
.endif
.if (${MKPCC} != "no")
SUBDIR+= pcc
.endif
.if (${MKDTRACE} != "no")
SUBDIR+= libproc
SUBDIR+= librtld_db
.endif

.include <bsd.subdir.mk>
