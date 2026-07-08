#include <sys/capability.h>
#include <minix/syslib.h>
#include <errno.h>

int cap_set_proc(cap_t caps)
{
	int r;

	r = sys_capctl(SELF, CAP_OP_SET, caps);
	if (r != 0) {
		errno = r > 0 ? r : -r;
		return -1;
	}

	return 0;
}
