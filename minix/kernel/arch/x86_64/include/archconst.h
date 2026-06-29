#ifndef _X86_64_ACONST_H
#define _X86_64_ACONST_H 1

#include <machine/interrupt.h>
#include <machine/memory.h>

/* Constants for 64-bit protected/long mode. */

/* Table sizes. */
#define IDT_SIZE 256

/* GDT layout (64-bit: SYSCALL/SYSRET compliant) */
/* In long mode, GDT entries are still 8 bytes, but descriptors
 * use a different format. The L bit (bit 21 of segment flags)
 * marks a 64-bit code segment.
 */
#define KERN_CS_INDEX        1
#define KERN_DS_INDEX        2
#define USER_CS_INDEX        3
#define USER_DS_INDEX        4
#define LDT_INDEX            5
#define TSS_INDEX_FIRST      6
#define TSS_INDEX(cpu)       (TSS_INDEX_FIRST + (cpu))
#define GDT_SIZE             (TSS_INDEX(CONFIG_MAX_CPUS))

#define SEG_SELECTOR(i)          ((i)*8)
#define KERN_CS_SELECTOR SEG_SELECTOR(KERN_CS_INDEX)
#define KERN_DS_SELECTOR SEG_SELECTOR(KERN_DS_INDEX)
#define USER_CS_SELECTOR (SEG_SELECTOR(USER_CS_INDEX) | USER_PRIVILEGE)
#define USER_DS_SELECTOR (SEG_SELECTOR(USER_DS_INDEX) | USER_PRIVILEGE)
#define LDT_SELECTOR SEG_SELECTOR(LDT_INDEX)
#define TSS_SELECTOR(cpu) SEG_SELECTOR(TSS_INDEX(cpu))

#define DESC_SIZE	8

/* Privileges. */
#define INTR_PRIVILEGE       0
#define USER_PRIVILEGE       3
#define RPL_MASK             0x03

/* Exception vectors. */
#define DIVIDE_VECTOR        0
#define DEBUG_VECTOR         1
#define NMI_VECTOR           2
#define BREAKPOINT_VECTOR    3
#define OVERFLOW_VECTOR      4
#define BOUNDS_VECTOR        5
#define INVAL_OP_VECTOR      6
#define COPROC_NOT_VECTOR    7
#define DOUBLE_FAULT_VECTOR  8
#define COPROC_SEG_VECTOR    9
#define INVAL_TSS_VECTOR    10
#define SEG_NOT_VECTOR      11
#define STACK_FAULT_VECTOR  12
#define PROTECTION_VECTOR   13
#define PAGE_FAULT_VECTOR   14
#define COPROC_ERR_VECTOR   16
#define ALIGNMENT_CHECK_VECTOR  17
#define MACHINE_CHECK_VECTOR    18
#define SIMD_EXCEPTION_VECTOR   19

/* Selector bits. */
#define TI                0x04
#define RPL               0x03

/* Descriptor type flags (long mode). */
#define PRESENT           0x80
#define DPL               0x60
#define DPL_SHIFT            5
#define SEGMENT           0x10

/* Segment descriptor shift/max values (for sdesc) */
#define BASE_MIDDLE_SHIFT   16
#define BASE_HIGH_SHIFT     24
#define BYTE_GRAN_MAX   0xFFFFFL
#define PAGE_GRAN_SHIFT     12

/* Access-byte bits. */
#define EXECUTABLE        0x08
#define CONFORMING        0x04
#define EXPAND_DOWN       0x04
#define READABLE          0x02
#define WRITEABLE         0x02
#define TSS_BUSY          0x02
#define ACCESSED          0x01

/* Descriptor types. */
#define AVL_286_TSS          1
#define LDT                  2
#define BUSY_286_TSS         3
#define CALL_286_GATE        4
#define TASK_GATE            5
#define INT_GATE_TYPE       14  /* 64-bit interrupt gate: type=14, IST field */
#define TRAP_GATE_TYPE      15  /* 64-bit trap gate: type=15 */

/* TSS type (64-bit: TSS is 16 bytes, type=9 or 11 for busy) */
#define TSS_TYPE            9   /* 64-bit available TSS */

/* Granularity byte (long mode). */
#define GRANULARITY_SHIFT   16
#define GRANULAR            0x80
#define DEFAULT             0x40
#define LONG_MODE           0x20  /* L bit: 64-bit code segment */
#define AVL                 0x10
#define LIMIT_HIGH          0x0F

/* Program stack words and masks. */
#define INIT_PSW      0x0200
#define INIT_TASK_PSW 0x1200
#define TRACEBIT      0x0100
#define SETPSW(rp, new) \
	((rp)->p_reg.psw = ((rp)->p_reg.psw & ~0xCD5) | ((new) & 0xCD5))
#define IF_MASK 0x00000200
#define IOPL_MASK 0x003000
#define RF_MASK 0x00010000

/* User-settable RFLAGS bits (arithmetic flags + DF + AC + ID) */
/* User-settable RFLAGS bits: arithmetic flags (CF,PF,AF,ZF,SF,DF,OF)
 * + AC (bit 18) + ID (bit 21). Bits 22-31 are reserved on x86-64. */
#define X86_FLAGS_USER		0x240CD5

/* CPU vendor strings (same as i386). */
#define INTEL_CPUID_GEN_EBX	0x756e6547
#define INTEL_CPUID_GEN_EDX	0x49656e69
#define INTEL_CPUID_GEN_ECX	0x6c65746e
#define AMD_CPUID_GEN_EBX	0x68747541
#define AMD_CPUID_GEN_EDX	0x69746e65
#define AMD_CPUID_GEN_ECX	0x444d4163
#define CPU_VENDOR_INTEL	0
#define CPU_VENDOR_AMD		2
#define CPU_VENDOR_UNKNOWN	0xff

/* FPU context alignment. */
#define FPUALIGN		16

/* Poweroff 16-bit code address. */
#define BIOS_POWEROFF_ENTRY 0x1000

/* Kernel stack top reserved (proc ptr + cpu id). */
#define X86_STACK_TOP_RESERVED	(2 * sizeof(reg_t))

#define PG_ALLOCATEME ((phys_bytes)-1)

/* MSR addresses (x86_64 adds SYSCALL/SYSRET MSRs). */
#define INTEL_MSR_PERFMON_CRT0         0xc1
#define INTEL_MSR_SYSENTER_CS         0x174
#define INTEL_MSR_SYSENTER_ESP        0x175
#define INTEL_MSR_SYSENTER_EIP        0x176
#define INTEL_MSR_PERFMON_SEL0        0x186
#define INTEL_MSR_PERFMON_SEL0_ENABLE (1 << 22)

/* AMD/x86_64 SYSCALL/SYSRET MSRs (for long mode). */
#define AMD_EFER_SCE		(1L << 0)
#define AMD_MSR_EFER		0xC0000080
#define AMD_MSR_STAR		0xC0000081  /* SYSCALL target EIP for 32-bit */
#define AMD_MSR_LSTAR		0xC0000082  /* SYSCALL target RIP for 64-bit */
#define AMD_MSR_CSTAR		0xC0000083  /* SYSCALL target EIP for compat */
#define AMD_MSR_SF_MASK		0xC0000084  /* SYSCALL RFLAGS mask */

/* Trap styles recorded on kernel entry and exit. */
#define KTS_NONE	1
#define KTS_INT_HARD	2
#define KTS_INT_ORIG	3
#define KTS_INT_UM	4
#define KTS_FULLCONTEXT	5

/* x86_64 uses syscall/sysret (not sysenter/sysexit or int 0x80). */
#define KTS_SYSCALL	7

#endif /* _X86_64_ACONST_H */
