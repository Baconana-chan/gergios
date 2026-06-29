/* x86_64 stub — do_iopenable: I/O privilege not supported on x86_64. */

#include "kernel/system.h"
#include <minix/endpoint.h>

#include "arch_proto.h"

int do_iopenable(struct proc * caller, message * m_ptr)
{
	return EPERM;
}
