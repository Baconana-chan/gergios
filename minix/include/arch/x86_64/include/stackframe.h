#ifndef STACK_FRAME_H
#define STACK_FRAME_H

#include <sys/types.h>

typedef unsigned long reg_t;         /* machine register */
typedef reg_t segdesc_t;

/* The stack frame layout is determined by the software, but for efficiency
 * it is laid out so the assembly code to use it is as simple as possible.
 *
 * x86_64 stackframe_s layout matches the SAVE_GP_REGS macro in sconst.h
 * + the interrupt frame pushed by the CPU:
 *   - r15, r14, r13, r12, r11, r10, r9, r8,
 *   - di, si, fp, bx, dx, cx, retreg,
 *   - pc, cs, psw, sp, ss (from interrupt)
 */
struct stackframe_s {
	reg_t r15;
	reg_t r14;
	reg_t r13;
	reg_t r12;
	reg_t r11;
	reg_t r10;
	reg_t r9;
	reg_t r8;
	reg_t di;
	reg_t si;
	reg_t fp;
	reg_t bx;
	reg_t dx;
	reg_t cx;
	reg_t retreg;
	reg_t pc;
	reg_t cs;
	reg_t psw;
	reg_t sp;
	reg_t ss;
};

#endif /* #ifndef STACK_FRAME_H */
