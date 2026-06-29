/* x86_64 stub — do_readbios: BIOS read not supported on x86_64. */

#include "kernel/system.h"

int do_readbios(struct proc * caller, message * m_ptr)
{
	return EPERM;
}
