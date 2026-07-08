/*	sys/capability.h - Userland capability API (libcap).
 *
 *	Provides POSIX-like capability query and control for user processes.
 *	Underlying implementation uses SYS_CAPCTL kernel call.
 *
 *	Functions:
 *	  cap_get_proc(cap_t *caps)     - get effective capabilities
 *	  cap_set_proc(cap_t caps)      - set effective capabilities
 *	  cap_get_bound(cap_t *caps)    - get bounding set
 */

#ifndef _SYS_CAPABILITY_H_
#define _SYS_CAPABILITY_H_

#include <sys/types.h>
#include <minix/capability.h>

/* Userland capability type — opaque 64-bit mask. */
typedef uint64_t cap_t;

/* Retrieve the effective capability set of the calling process.
 * On success, stores the capability mask in *caps and returns 0.
 * On error, returns -1 and sets errno.
 */
int cap_get_proc(cap_t *caps);

/* Set the effective capability set of the calling process.
 * The new mask must be a subset of the permitted ∩ bounding sets.
 * Returns 0 on success, -1 on error with errno set.
 */
int cap_set_proc(cap_t caps);

/* Retrieve the bounding set of the calling process.
 * The bounding set is the immutable limit beyond which capabilities
 * can never be raised (set once at process creation, can only be
 * narrowed via SYS_CAPCTL CAP_OP_BOUND_SET).
 * Returns 0 on success, -1 on error with errno set.
 */
int cap_get_bound(cap_t *caps);

#endif /* _SYS_CAPABILITY_H_ */
