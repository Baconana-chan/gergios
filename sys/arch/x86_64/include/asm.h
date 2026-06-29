/*	x86_64 asm.h — assembly macros and definitions	*/

#ifndef _X86_64_ASM_H_
#define _X86_64_ASM_H_

#define _C_LABEL(x)	x
#define _ASM_LABEL(x)	x

#ifndef _ALIGN_TEXT
#define _ALIGN_TEXT	.align 16,0x90
#endif

#ifndef _TEXT_SECTION
#define _TEXT_SECTION	.text
#endif

#define _ASM_TYPE_FUNCTION	@function
#define _ASM_TYPE_OBJECT	@object

#define _ENTRY(x) \
	_TEXT_SECTION; _ALIGN_TEXT; .globl x; .type x, _ASM_TYPE_FUNCTION; x:

#define _END(x)		.size x, .-x

#ifdef GPROF
#define _PROF_PROLOGUE \
	pushq %rbp; movq %rsp, %rbp; call __mcount; popq %rbp
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

#define _LABEL(x) \
	.globl x; x:
#define	LABEL(y)		_LABEL(_C_LABEL(y))

#define IMPORT(sym)		.extern _C_LABEL(sym)

#define RCSID(x)		.pushsection .ident; .asciz x; .popsection

#define	WEAK_ALIAS(alias, sym)	\
	.weak alias;		\
	alias = sym

#define STRONG_ALIAS(alias, sym)	\
	.globl alias;			\
	alias = sym

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
#endif

#ifdef __PIC__
#define	PLT_SYM(x)	x
#else
#define	PLT_SYM(x)	x
#endif

#endif /* !_X86_64_ASM_H_ */
