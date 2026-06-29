/*	x86_64 trap.h — trap type values	*/

#ifndef _X86_64_TRAP_H_
#define _X86_64_TRAP_H_

#define	T_PRIVINFLT	 0	/* privileged instruction */
#define	T_BPTFLT	 1	/* breakpoint trap */
#define	T_ARITHTRAP	 2	/* arithmetic trap */
#define	T_ASTFLT	 3	/* asynchronous system trap */
#define	T_PROTFLT	 4	/* protection fault */
#define	T_TRCTRAP	 5	/* trace trap */
#define	T_PAGEFLT	 6	/* page fault */
#define	T_ALIGNFLT	 7	/* alignment fault */
#define	T_DIVIDE	 8	/* integer divide fault */
#define	T_NMI		 9	/* non-maskable interrupt */
#define	T_OFLOW		10	/* overflow trap */
#define	T_BOUND		11	/* bounds check fault */
#define	T_DNA		12	/* device not available fault */
#define	T_DOUBLEFLT	13	/* double fault */
#define	T_FPOPFLT	14	/* fp coprocessor operand fetch fault */
#define	T_TSSFLT	15	/* invalid tss fault */
#define	T_SEGNPFLT	16	/* segment not present fault */
#define	T_STKFLT	17	/* stack fault */
#define	T_MCA		18	/* machine check */
#define T_XMM		19	/* SSE FP exception */
#define T_RESERVED	20	/* reserved fault base */

#define	T_USER		0x100	/* Trap's coming from user mode */

#define TC_TSS		0x80000000
#define TC_FLAGMASK	(TC_TSS)

#endif /* !_X86_64_TRAP_H_ */
