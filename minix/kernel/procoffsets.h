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
 * struct segframe layout (offset from start of p_seg):
 *   p_cr3           0  (reg_t, 8 bytes)
 *   p_cr3_v         8  (u64_t*, 8 bytes)
 *   fpu_state      16  (char*, 8 bytes)
 *   p_kern_trap_style  24  (int, 4 bytes, padded to 8)
 *   total:          32
 *
 * sizeof(stackframe_s) = 160 (20 regs x 8 bytes)
 * P_CR3 = 160 + 0 = 160
 * P_KERN_TRAP_STYLE = 160 + 24 = 184 */
#define P_CR3			160
#define P_KERN_TRAP_STYLE	184

/* Segment selector constants (from archconst.h) — numeric values for
 * assembly files that may not have C preprocessor access.
 *   KERN_CS_INDEX = 1, KERN_DS_INDEX = 2
 *   USER_CS_INDEX = 3, USER_DS_INDEX = 4, LDT_INDEX = 5
 *   SEG_SELECTOR(i) = (i) * 8
 */
#define KERN_CS_SELECTOR	8
#define KERN_DS_SELECTOR	16
#define LDT_SELECTOR		40
