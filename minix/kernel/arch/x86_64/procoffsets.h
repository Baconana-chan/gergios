/*
 * procoffsets.h — x86_64 struct offset definitions for assembly
 *
 * GENERATED MANUALLY from procoffsets.cf struct definitions.
 * struct stackframe_s layout (arch/x86_64/include/stackframe.h):
 *   r15     0
 *   r14     8
 *   r13     16
 *   r12     24
 *   r11     32
 *   r10     40
 *   r9      48
 *   r8      56
 *   di      64
 *   si      72
 *   fp      80
 *   bx      88
 *   dx      96
 *   cx      104
 *   retreg  112
 *   pc      120
 *   cs      128
 *   psw     136
 *   sp      144
 *   ss      152
 *   total:  160 bytes (20 x 8)
 *
 * These registers are at the beginning of struct proc (p_reg).
 */

/* General-purpose register offsets (within p_reg (stackframe_s)) */
#define R15REG   0
#define R14REG   8
#define R13REG   16
#define R12REG   24
#define R11REG   32
#define R10REG   40
#define R9REG    48
#define R8REG    56
#define DIREG    64
#define SIREG    72
#define BPREG    80
#define BXREG    88
#define DXREG    96
#define CXREG    104
#define AXREG    112

/* Saved exception state (from interrupt/exception frame) */
#define PCREG    120     /* RIP */
#define CSREG    128     /* CS */
#define PSWREG   136     /* RFLAGS */
#define SPREG    144     /* RSP (user stack, from CPL change) */
#define SSREG    152     /* SS (user stack segment, from CPL change) */

/* Process field offsets (beyond p_reg / stackframe_s)
 * P_KERN_TRAP_STYLE = offsetof(struct proc, p_seg.p_kern_trap_style)
 *   sizeof(stackframe_s) = 160
 *   p_cr3  (8) + p_cr3_v (8) + fpu_state (8) + p_kern_trap_style (4) = 28
 *   but padded to 32 (multiple of 8)
 *   P_KERN_TRAP_STYLE = 160 + 24 = 184 */
#define P_KERN_TRAP_STYLE	184
