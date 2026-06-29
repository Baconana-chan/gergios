/*	$NetBSD: math.h,v 1.9 2012/03/15 11:10:55 skrll Exp $	*/

/*
 * Machine-dependent math.h for bare-metal x86_64-elf builds.
 *
 * Provides the arch-specific macros needed by <math.h>.
 * On bare-metal, floating-point is typically not used in kernel
 * context, so we provide minimal stubs that avoid compilation errors.
 *
 * NOTE: __isinf, __isnan, __signbit are NOT defined here — Clang
 * provides these as builtins via __has_builtin.
 * __fpclassifyf/d/l are declared as functions (not macros) because
 * using __builtin_fpclassify in a macro definition causes Clang
 * error: 'cannot redeclare builtin function __builtin_fpclassify'.
 */

#ifndef _MACHINE_MATH_H_
#define _MACHINE_MATH_H_

/* x86_64 uses SSE2 for floating-point, so float == double precision.
 * __FLT_EVAL_METHOD__ is defined by the compiler.
 * If not set by -fno-fp-eval-method, default to 0 (no extra precision). */
#ifndef __FLT_EVAL_METHOD__
#define __FLT_EVAL_METHOD__ 0
#endif

/* x86_64 supports long double (80-bit extended precision) */
#define __HAVE_LONG_DOUBLE

/* x86_64 supports NAN */
#define __HAVE_NANF

/* x86_64 FPU exception/infinity support */
#define __INFINITY	__builtin_inff()

/* Classification functions — declared but not defined.
 * These are normally provided by libc. On bare-metal, they're stubs
 * that should never be called (kernel code doesn't use fpclassify).
 * Clang's __builtin_fpclassify cannot be used in a macro expansion
 * without causing 'cannot redeclare builtin' errors. */
int	__fpclassifyf(float);
int	__fpclassifyd(double);
int	__fpclassifyl(long double);

#endif /* _MACHINE_MATH_H_ */
