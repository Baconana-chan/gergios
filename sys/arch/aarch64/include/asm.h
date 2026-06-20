/*	$NetBSD$	*/

/*
 * Copyright (c) 2024 GergiOS.
 * All rights reserved.
 *
 * asm.h — ARM64 (AArch64) assembly macros and definitions.
 *
 * Provides macros for assembly language programming:
 *   ENTRY(name)      — Define a global function entry point
 *   ENTRY_NP(name)   — Define a global function (no profiling)
 *   END(name)        — End a function definition
 *   IMPORT(sym)      — Declare an external symbol
 *   _C_LABEL(x)      — Convert C identifier to asm label
 *   LABEL(name)      — Define a global label (not a function)
 *   WEAK_ALIAS       — Create a weak alias
 *   STRONG_ALIAS     — Create a strong alias
 *
 * Reference: ARM Architecture Reference Manual ARMv8-A (DDI 0487)
 */

#ifndef _AARCH64_ASM_H_
#define _AARCH64_ASM_H_

#define _C_LABEL(x)	x
#define _ASM_LABEL(x)	x

#ifndef _ALIGN_TEXT
#define _ALIGN_TEXT	.align 2
#endif

#ifndef _TEXT_SECTION
#define _TEXT_SECTION	.text
#endif

#define _ASM_TYPE_FUNCTION	%function
#define _ASM_TYPE_OBJECT	%object

#define _ENTRY(x) \
	_TEXT_SECTION; _ALIGN_TEXT; .globl x; .type x, _ASM_TYPE_FUNCTION; x:

#define _END(x)		.size x, .-x

/* Profiling prologue (no-op unless GPROF is defined) */
#ifdef GPROF
#define _PROF_PROLOGUE \
	stp	x29, x30, [sp, #-16]!; mov x29, sp; bl __mcount; \
	ldp	x29, x30, [sp], #16
#else
#define _PROF_PROLOGUE
#endif

#define	ENTRY(y)		_ENTRY(_C_LABEL(y)); _PROF_PROLOGUE
#define	ENTRY_NP(y)		_ENTRY(_C_LABEL(y))
#define	END(y)			_END(_C_LABEL(y))

#define	ASENTRY(y)		_ENTRY(_ASM_LABEL(y)); _PROF_PROLOGUE
#define	ASENTRY_NP(y)		_ENTRY(_ASM_LABEL(y))
#define	ASEND(y)		_END(_ASM_LABEL(y))

#define	ASMSTR			.asciz

/* Global label (not a function, no type info) */
#define _LABEL(x) \
	.globl x; x:
#define	LABEL(y)		_LABEL(_C_LABEL(y))

/* External symbol declaration */
#define IMPORT(sym)		.extern _C_LABEL(sym)

/* RCS ID string */
#define RCSID(x)		.pushsection ".ident"; .asciz x; .popsection

/* Weak and strong aliases */
#define	WEAK_ALIAS(alias, sym)	\
	.weak alias;		\
	alias = sym

#define STRONG_ALIAS(alias, sym)	\
	.globl alias;			\
	alias = sym

/* Warning references */
#ifdef __STDC__
#define	WARN_REFERENCES(sym, msg)		\
	.pushsection .gnu.warning. ## sym;	\
	.ascii msg;				\
	.popsection
#else
#define	WARN_REFERENCES(sym, msg)		\
	.pushsection .gnu.warning./**/sym;	\
	.ascii msg;				\
	.popsection
#endif /* __STDC__ */

/* Position-independent code (PIC) support */
#ifdef __PIC__
#define	PLT_SYM(x)	x
#else
#define	PLT_SYM(x)	x
#endif /* __PIC__ */

#endif /* !_AARCH64_ASM_H_ */
