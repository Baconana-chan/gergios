/* _cpuid() - interface to cpuid instruction - x86_64 version */

/* void _cpuid(u32_t *eax, u32_t *ebx, u32_t *ecx, u32_t *edx); */
/* 0 for OK, nonzero for unsupported */

#include <machine/asm.h>

/* x86_64 System V ABI register args:
 *   RDI = eax_ptr  (1st arg)
 *   RSI = ebx_ptr  (2nd arg)
 *   RDX = ecx_ptr  (3rd arg)
 *   RCX = edx_ptr  (4th arg)
 *
 * CPUID uses EAX (leaf), EBX, ECX (subleaf), EDX.
 * RCX is both a pointer arg AND a CPUID input register,
 * so we must save all pointers before loading values.
 *
 * R8-R11 are caller-saved (no need to push/pop them).
 */

ENTRY(_cpuid)
	/* Save callee-saved registers we'll use */
	push	%rbx
	push	%rbp

	/* Save all 4 pointer args in R8-R11 (caller-saved) */
	mov	%rdi, %r8	/* R8 = eax_ptr */
	mov	%rsi, %r9	/* R9 = ebx_ptr */
	mov	%rdx, %r10	/* R10 = ecx_ptr */
	mov	%rcx, %r11	/* R11 = edx_ptr */

	/* Load CPUID inputs from saved pointers */
	mov	(%r8), %rax	/* EAX = *eax_ptr (CPUID leaf) */
	mov	(%r9), %rbx	/* EBX = *ebx_ptr */
	mov	(%r10), %rcx	/* ECX = *ecx_ptr (CPUID subleaf) */

	cpuid

	/* Store results back through saved pointers */
	mov	%rax, (%r8)	/* *eax_ptr = EAX */
	mov	%rbx, (%r9)	/* *ebx_ptr = EBX */
	mov	%rcx, (%r10)	/* *ecx_ptr = ECX */
	mov	%rdx, (%r11)	/* *edx_ptr = EDX */

	/* Restore callee-saved registers */
	pop	%rbp
	pop	%rbx

	ret
