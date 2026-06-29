/*	x86_64 ptrace stub	*/

#ifndef _X86_64_PTRACE_H_
#define _X86_64_PTRACE_H_

#define PT_TRACE_ME	0
#define PT_READ_I	1
#define PT_READ_D	2
#define PT_WRITE_I	4
#define PT_WRITE_D	5
#define PT_CONTINUE	7
#define PT_KILL		8
#define PT_STEP		9
#define PT_ATTACH	10
#define PT_DETACH	11
#define PT_GETREGS	12
#define PT_SETREGS	13
#define PT_GETFPREGS	14
#define SETFPREGS	15

#endif /* _X86_64_PTRACE_H_ */
