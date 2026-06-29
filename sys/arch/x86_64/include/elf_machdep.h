/*	$NetBSD: elf_machdep.h,v 1.1 2005/12/27 18:42:10 christos Exp $	*/

/* machine/elf_machdep.h — x86_64 ELF machine definitions.
 * Minimal stub for MINIX kernel compilation.
 */

#ifndef _MACHINE_ELF_MACHDEP_H_
#define _MACHINE_ELF_MACHDEP_H_

#define ARCH_ELFSIZE		32

#define ELF_MACHINE_OK(x)	((x) == EM_X86_64)

#define ELF_MACHDEP_ENDIANNESS	ELFDATA2LSB

/* Relocation types for x86_64 */
#define R_X86_64_NONE		0
#define R_X86_64_64		1
#define R_X86_64_PC32		2
#define R_X86_64_GOT32		3
#define R_X86_64_PLT32		4
#define R_X86_64_COPY		5
#define R_X86_64_GLOB_DAT	6
#define R_X86_64_JUMP_SLOT	7
#define R_X86_64_RELATIVE	8
#define R_X86_64_GOTPCREL	9
#define R_X86_64_32		10
#define R_X86_64_32S		11
#define R_X86_64_16		12
#define R_X86_64_PC16		13
#define R_X86_64_8		14
#define R_X86_64_PC8		15

#endif /* _MACHINE_ELF_MACHDEP_H_ */
