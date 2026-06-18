# $NetBSD: Makefile.inc,v 1.6 2011/06/11 18:03:17 christos Exp $
#
# *** DEPRECATED ***
# OpenSSL has been replaced by wolfSSL/libhcrypto throughout the MINIX tree.
# These tests are retained for reference only and are not built by default.
# To re-enable: update to use wolfSSL paths and -lwolfssl.
#

.include <bsd.own.mk>
.include "../Makefile.inc"
TESTSDIR=	${TESTSBASE}/crypto/libcrypto
ATFFILE=	no

# NOTE: This test suite was for OpenSSL which has been removed.
# These tests are NOT included in the build (removed from TESTS_SUBDIRS).
# To re-enable: update to use wolfSSL paths and -lwolfssl.
