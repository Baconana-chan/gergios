#include "syslib.h"

int sys_capctl(endpoint_t proc_ep, int op, uint64_t caps)
{
  message m;

  m.CAPCTL_ENDPT = proc_ep;
  m.CAPCTL_OP = op;
  m.CAPCTL_CAPS = caps;

  return _kernel_call(SYS_CAPCTL, &m);
}
