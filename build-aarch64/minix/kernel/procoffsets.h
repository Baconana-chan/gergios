/*
 * procoffsets.h — AArch64 struct offset definitions for assembly
 *
 * GENERATED MANUALLY from procoffsets.cf struct definitions.
 * struct stackframe_s layout (archtypes.h):
 *   gpr[31]  0..240    (31 × 8 bytes)
 *   sp_el0   248       (1 × 8)
 *   elr_el1  256       (1 × 8)
 *   spsr_el1 264       (1 × 8)
 *   total:   272 bytes
 *
 * struct segframe layout (archtypes.h):
 *   p_ttbr    0       (1 × 8)
 *   p_ttbr_v  8       (1 × 8)
 *   tcr_el1   16      (1 × 8)
 *
 * struct proc layout (proc.h):
 *   p_reg     0..272   (stackframe_s)
 *   p_seg     272..288 (segframe)
 *   p_nr      288      (proc_nr_t = int32_t, 4 bytes)
 *   ... (more fields follow)
 *
 * P_TTBR = offsetof(struct proc, p_seg.ttbr0_el1) = 272
 */

/* General-purpose register offsets (within stackframe_s.gpr[]) */
#define REG0    0
#define REG1    8
#define REG2    16
#define REG3    24
#define REG4    32
#define REG5    40
#define REG6    48
#define REG7    56
#define REG8    64
#define REG9    72
#define REG10   80
#define REG11   88
#define REG12   96
#define REG13   104
#define REG14   112
#define REG15   120
#define REG16   128
#define REG17   136
#define REG18   144
#define REG19   152
#define REG20   160
#define REG21   168
#define REG22   176
#define REG23   184
#define REG24   192
#define REG25   200
#define REG26   208
#define REG27   216
#define REG28   224
#define REG29   232
#define REG30   240

/* Named aliases (x86-style compatibility) — guarded to avoid
 * redefinition warnings when sconst.h also defines them. */
#ifndef AXREG
#define AXREG   0       /* x0 */
#define BXREG   8       /* x1 */
#define CXREG   16      /* x2 */
#define DXREG   24      /* x3 */
#define SIREG   32      /* x4 */
#define DIREG   40      /* x5 */
#define BPREG   232     /* x29 = FP */
#define FPREG   232     /* x29 = FP */
#define LRREG   240     /* x30 = LR */
#endif

/* Saved exception state (after GPRs) */
#define SPREG   248     /* SP_EL0 (user stack pointer) */
#define PCREG   256     /* ELR_EL1 (exception return address) */
#define PSREG   264     /* SPSR_EL1 (saved processor state) */

/* Page table base register (within proc.p_seg) */
#define P_TTBR  272     /* p_seg.p_ttbr offset from proc base */
