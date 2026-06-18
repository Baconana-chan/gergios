# $NetBSD: Makefile for libhcrypto (heimdal's own crypto library)
# Created for wolfSSL migration Phase 3

.include <bsd.own.mk>
.include <${.CURDIR}/../../Makefile.inc>

HEIMDIST?=	${HEIMBASE}/dist

.PATH:	${HEIMDIST}/lib/hcrypto
.PATH:	${HEIMDIST}/lib/hcrypto/libtommath

LIB=		hcrypto

# Build flags
CPPFLAGS+=	-DBUILD_HCRYPTO_LIB -DUSE_HCRYPTO_LTM=1
CPPFLAGS+=	-I${HEIMDIST}/lib/hcrypto
CPPFLAGS+=	-I${HEIMDIST}/lib/hcrypto/libtommath
CWARNFLAGS.clang+=	-Wno-error=absolute-value -Wno-error=unused-variable
CWARNFLAGS.clang+=	-Wno-error=unused-function -Wno-error=tautological-compare

# Library dependencies
LIBDPLIBS+=	roken	${HEIMBASE}/lib/libroken

# LibTomMath sources (big number math library)
LTM_SRCS=				\
	libtommath/bncore.c		\
	libtommath/bn_mp_init.c		\
	libtommath/bn_mp_clear.c	\
	libtommath/bn_mp_exch.c		\
	libtommath/bn_mp_grow.c		\
	libtommath/bn_mp_shrink.c	\
	libtommath/bn_mp_clamp.c	\
	libtommath/bn_mp_zero.c		\
	libtommath/bn_mp_zero_multi.c	\
	libtommath/bn_mp_set.c		\
	libtommath/bn_mp_set_int.c	\
	libtommath/bn_mp_init_size.c	\
	libtommath/bn_mp_copy.c		\
	libtommath/bn_mp_init_copy.c	\
	libtommath/bn_mp_abs.c		\
	libtommath/bn_mp_neg.c		\
	libtommath/bn_mp_cmp_mag.c	\
	libtommath/bn_mp_cmp.c		\
	libtommath/bn_mp_cmp_d.c	\
	libtommath/bn_mp_rshd.c		\
	libtommath/bn_mp_lshd.c		\
	libtommath/bn_mp_mod_2d.c	\
	libtommath/bn_mp_div_2d.c	\
	libtommath/bn_mp_mul_2d.c	\
	libtommath/bn_mp_div_2.c	\
	libtommath/bn_mp_mul_2.c	\
	libtommath/bn_s_mp_add.c	\
	libtommath/bn_s_mp_sub.c	\
	libtommath/bn_fast_s_mp_mul_digs.c \
	libtommath/bn_s_mp_mul_digs.c	\
	libtommath/bn_fast_s_mp_mul_high_digs.c \
	libtommath/bn_s_mp_mul_high_digs.c \
	libtommath/bn_fast_s_mp_sqr.c	\
	libtommath/bn_s_mp_sqr.c	\
	libtommath/bn_mp_add.c		\
	libtommath/bn_mp_sub.c		\
	libtommath/bn_mp_karatsuba_mul.c \
	libtommath/bn_mp_mul.c		\
	libtommath/bn_mp_karatsuba_sqr.c \
	libtommath/bn_mp_sqr.c		\
	libtommath/bn_mp_div.c		\
	libtommath/bn_mp_mod.c		\
	libtommath/bn_mp_add_d.c	\
	libtommath/bn_mp_sub_d.c	\
	libtommath/bn_mp_mul_d.c	\
	libtommath/bn_mp_div_d.c	\
	libtommath/bn_mp_mod_d.c	\
	libtommath/bn_mp_expt_d.c	\
	libtommath/bn_mp_addmod.c	\
	libtommath/bn_mp_submod.c	\
	libtommath/bn_mp_mulmod.c	\
	libtommath/bn_mp_sqrmod.c	\
	libtommath/bn_mp_gcd.c		\
	libtommath/bn_mp_lcm.c		\
	libtommath/bn_fast_mp_invmod.c	\
	libtommath/bn_mp_invmod.c	\
	libtommath/bn_mp_reduce.c	\
	libtommath/bn_mp_montgomery_setup.c \
	libtommath/bn_fast_mp_montgomery_reduce.c \
	libtommath/bn_mp_montgomery_reduce.c \
	libtommath/bn_mp_exptmod_fast.c \
	libtommath/bn_mp_exptmod.c	\
	libtommath/bn_mp_2expt.c	\
	libtommath/bn_mp_n_root.c	\
	libtommath/bn_mp_jacobi.c	\
	libtommath/bn_reverse.c		\
	libtommath/bn_mp_count_bits.c	\
	libtommath/bn_mp_read_unsigned_bin.c \
	libtommath/bn_mp_read_signed_bin.c \
	libtommath/bn_mp_to_unsigned_bin.c \
	libtommath/bn_mp_to_signed_bin.c \
	libtommath/bn_mp_unsigned_bin_size.c \
	libtommath/bn_mp_signed_bin_size.c \
	libtommath/bn_mp_xor.c		\
	libtommath/bn_mp_and.c		\
	libtommath/bn_mp_or.c		\
	libtommath/bn_mp_rand.c		\
	libtommath/bn_mp_montgomery_calc_normalization.c \
	libtommath/bn_mp_prime_is_divisible.c \
	libtommath/bn_prime_tab.c	\
	libtommath/bn_mp_prime_fermat.c \
	libtommath/bn_mp_prime_miller_rabin.c \
	libtommath/bn_mp_prime_is_prime.c \
	libtommath/bn_mp_prime_next_prime.c \
	libtommath/bn_mp_find_prime.c	\
	libtommath/bn_mp_isprime.c	\
	libtommath/bn_mp_dr_reduce.c	\
	libtommath/bn_mp_dr_is_modulus.c \
	libtommath/bn_mp_dr_setup.c	\
	libtommath/bn_mp_reduce_setup.c \
	libtommath/bn_mp_toom_mul.c	\
	libtommath/bn_mp_toom_sqr.c	\
	libtommath/bn_mp_div_3.c	\
	libtommath/bn_s_mp_exptmod.c	\
	libtommath/bn_mp_reduce_2k.c	\
	libtommath/bn_mp_reduce_is_2k.c \
	libtommath/bn_mp_reduce_2k_setup.c \
	libtommath/bn_mp_reduce_2k_l.c	\
	libtommath/bn_mp_reduce_is_2k_l.c \
	libtommath/bn_mp_reduce_2k_setup_l.c \
	libtommath/bn_mp_radix_smap.c	\
	libtommath/bn_mp_read_radix.c	\
	libtommath/bn_mp_toradix.c	\
	libtommath/bn_mp_radix_size.c	\
	libtommath/bn_mp_fread.c	\
	libtommath/bn_mp_fwrite.c	\
	libtommath/bn_mp_cnt_lsb.c	\
	libtommath/bn_error.c		\
	libtommath/bn_mp_init_multi.c	\
	libtommath/bn_mp_clear_multi.c	\
	libtommath/bn_mp_exteuclid.c	\
	libtommath/bn_mp_toradix_n.c	\
	libtommath/bn_mp_prime_random_ex.c \
	libtommath/bn_mp_get_int.c	\
	libtommath/bn_mp_sqrt.c		\
	libtommath/bn_mp_is_square.c	\
	libtommath/bn_mp_init_set.c	\
	libtommath/bn_mp_init_set_int.c \
	libtommath/bn_mp_invmod_slow.c	\
	libtommath/bn_mp_prime_rabin_miller_trials.c \
	libtommath/bn_mp_to_signed_bin_n.c \
	libtommath/bn_mp_to_unsigned_bin_n.c

# hcrypto core source files (ec.c excluded — not in original Makefile.am)
HCRYPTO_SRCS=				\
	aes.c				\
	bn.c				\
	camellia.c			\
	camellia-ntt.c			\
	common.c			\
	des.c				\
	dh.c				\
	dh-ltm.c			\
	dsa.c				\
	doxygen.c			\
	engine.c			\
	evp.c				\
	evp-hcrypto.c			\
	evp-cc.c			\
	hmac.c				\
	md2.c				\
	md4.c				\
	md5.c				\
	pkcs5.c				\
	pkcs12.c			\
	rand-egd.c			\
	rand-fortuna.c			\
	rand-timer.c			\
	rand-unix.c			\
	rand.c				\
	rc2.c				\
	rc4.c				\
	rijndael-alg-fst.c		\
	rnd_keys.c			\
	rsa.c				\
	rsa-gmp.c			\
	rsa-ltm.c			\
	sha.c				\
	sha256.c			\
	sha512.c			\
	ui.c				\
	validate.c

SRCS=	${LTM_SRCS} ${HCRYPTO_SRCS}

# Install public hcrypto headers
INCSDIR=	/usr/include/hcrypto
INCS=		${HEIMDIST}/lib/hcrypto/aes.h		\
		${HEIMDIST}/lib/hcrypto/bn.h		\
		${HEIMDIST}/lib/hcrypto/des.h		\
		${HEIMDIST}/lib/hcrypto/dh.h		\
		${HEIMDIST}/lib/hcrypto/dsa.h		\
		${HEIMDIST}/lib/hcrypto/ec.h		\
		${HEIMDIST}/lib/hcrypto/ecdh.h		\
		${HEIMDIST}/lib/hcrypto/ecdsa.h		\
		${HEIMDIST}/lib/hcrypto/engine.h	\
		${HEIMDIST}/lib/hcrypto/evp.h		\
		${HEIMDIST}/lib/hcrypto/evp-hcrypto.h	\
		${HEIMDIST}/lib/hcrypto/evp-cc.h	\
		${HEIMDIST}/lib/hcrypto/hmac.h		\
		${HEIMDIST}/lib/hcrypto/md2.h		\
		${HEIMDIST}/lib/hcrypto/md4.h		\
		${HEIMDIST}/lib/hcrypto/md5.h		\
		${HEIMDIST}/lib/hcrypto/pkcs12.h	\
		${HEIMDIST}/lib/hcrypto/rand.h		\
		${HEIMDIST}/lib/hcrypto/rc2.h		\
		${HEIMDIST}/lib/hcrypto/rc4.h		\
		${HEIMDIST}/lib/hcrypto/rsa.h		\
		${HEIMDIST}/lib/hcrypto/sha.h		\
		${HEIMDIST}/lib/hcrypto/ui.h

.include <bsd.lib.mk>
