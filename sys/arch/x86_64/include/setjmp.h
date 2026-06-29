/*	x86_64 setjmp.h — setjmp/longjmp buffer layout	*/

#ifndef _X86_64_SETJMP_H_
#define _X86_64_SETJMP_H_

/* x86_64 setjmp buffer layout (jmp_buf):
 * Callee-saved: rbx, rbp, r12-r15 (6 registers)
 * Return address: saved on stack, slot for RIP
 * Stack pointer: RSP
 *
 * Word layout (each entry is 8 bytes / 1 long):
 *   word   0: _JB_MAGIC
 *   word   1: rbx
 *   word   2: rbp
 *   word   3: r12
 *   word   4: r13
 *   word   5: r14
 *   word   6: r15
 *   word   7: rsp
 *   word   8: return address (rip)
 *   word   9-12: signal mask (if _JB_MAGIC_SETJMP)
 */

#define	_JBLEN	32	/* size, in longs, of a jmp_buf */

#define _JB_MAGIC__SETJMP	0x4a8f5000
#define _JB_MAGIC_SETJMP	0x4a8f5001

#define _JB_MAGIC		 0
#define _JB_RBX			 1
#define _JB_RBP			 2
#define _JB_R12			 3
#define _JB_R13			 4
#define _JB_R14			 5
#define _JB_R15			 6
#define _JB_RSP			 7
#define _JB_RIP			 8
#define _JB_SIGMASK		 9

#endif /* _X86_64_SETJMP_H_ */
