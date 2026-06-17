#	$NetBSD: Makefile,v 1.12 2013/02/12 20:31:14 christos Exp $
#
# OpenSSL is being replaced by wolfSSL on MINIX.
# See crypto/external/gpl2/wolfssl/ for the replacement.
# openssl remains for heimdal, netpgp, libsaslc which
# have not yet been migrated.
#

.include <bsd.own.mk>

.if defined(__MINIX)
SUBDIR+= openssl .WAIT heimdal netpgp .WAIT libsaslc
.else
SUBDIR+= openssl .WAIT heimdal netpgp .WAIT openssh libsaslc
.endif # defined(__MINIX)

.include <bsd.subdir.mk>
