#include <sys/capability.h>
#include <minix/syslib.h>
#include <errno.h>

int cap_get_proc(cap_t *caps)
{
	message m;
	int r;

	if (caps == NULL) {
		errno = EFAULT;
		return -1;
	}

	m.CAPCTL_ENDPT = SELF;
	m.CAPCTL_OP = CAP_OP_GET;
	m.CAPCTL_CAPS = 0;

	r = _kernel_call(SYS_CAPCTL, &m);
	if (r != 0) {
		errno = r > 0 ? r : -r;
		return -1;
	}

	*caps = m.CAPCTL_CAPS;
	return 0;
}
