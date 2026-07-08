/* The kernel call implemented in this file:
 *   m_type:	SYS_CAPCTL
 *
 * The parameters for this kernel call are:
 *   CAPCTL_ENDPT	(process endpoint of target)
 *   CAPCTL_OP		(capability operation: CAP_OP_*)
 *   CAPCTL_CAPS		(capability mask)
 *
 * Capability operations:
 *   CAP_OP_GET       — get effective capabilities of target
 *   CAP_OP_SET       — set effective capabilities (can only drop, not raise)
 *   CAP_OP_BOUND_GET — get bounding set of target
 *   CAP_OP_BOUND_SET — set bounding set (requires CAP_SYS_ADMIN on caller)
 *   CAP_OP_LIST      — list all defined capability names
 *
 * Capability model:
 *   - cap_effective: currently active capabilities
 *   - cap_permitted: superset that effective can draw from
 *   - cap_bound:     immutable bounding set (set once, can't be raised)
 *
 * Inheritance:
 *   fork():   child inherits all three sets from parent
 *   exec():   effective = permitted = bound (reset to bounding set)
 *   setuid(): NO automatic elevation (root does not auto-gain caps)
 */

#include "kernel/system.h"
#include <minix/capability.h>
#include <string.h>

#if USE_CAPCTL

/*===========================================================================*
 *				do_capctl				     *
 *===========================================================================*/
int do_capctl(struct proc * caller, message * m_ptr)
{
/* Handle sys_capctl(). Manage process capabilities.
 * Only system processes can call this; caller must have appropriate caps
 * to modify target's capabilities.
 */
  struct proc *rp;
  proc_nr_t proc_nr;
  int r;
  uint64_t caps;

  /* Only system processes may issue capability calls. */
  if (!(priv(caller)->s_flags & SYS_PROC))
	return(EPERM);

  /* Determine target process. */
  if (m_ptr->CAPCTL_ENDPT == SELF)
	okendpt(caller->p_endpoint, &proc_nr);
  else if (!isokendpt(m_ptr->CAPCTL_ENDPT, &proc_nr))
	return(EINVAL);
  rp = proc_addr(proc_nr);

  switch (m_ptr->CAPCTL_OP)
  {
  case CAP_OP_GET:
	/* Return effective capabilities of target. */
	m_ptr->CAPCTL_CAPS = priv(rp)->s_cap_effective;
	return(OK);

  case CAP_OP_SET:
	/* Set effective capabilities of target.
	 * The new mask must be a subset of the permitted set.
	 * Caller must be RS (SYS_ADMIN) or the target itself.
	 * This operation can only DROP capabilities (never raise them above
	 * the bounding set).
	 */
	caps = m_ptr->CAPCTL_CAPS;

	/* Validate: new caps must be subset of permitted and bound. */
	if ((caps & ~priv(rp)->s_cap_permitted) != 0)
		return(EPERM);
	if ((caps & ~priv(rp)->s_cap_bound) != 0)
		return(EPERM);

	/* Only target itself or RS (SYS_ADMIN) can change caps. */
	if (caller != rp &&
	    !(priv(caller)->s_cap_effective & CAP_SYS_ADMIN))
		return(EPERM);

	priv(rp)->s_cap_effective = caps;
	return(OK);

  case CAP_OP_BOUND_GET:
	/* Return bounding set of target. */
	m_ptr->CAPCTL_CAPS = priv(rp)->s_cap_bound;
	return(OK);

  case CAP_OP_BOUND_SET:
	/* Set bounding set of target.
	 * Requires CAP_SYS_ADMIN on caller.
	 * Can only restrict the bounding set (never expand it).
	 */
	if (!(priv(caller)->s_cap_effective & CAP_SYS_ADMIN))
		return(EPERM);

	caps = m_ptr->CAPCTL_CAPS;

	/* Can only restrict bounding set, never expand. */
	if ((priv(rp)->s_cap_bound & ~caps) != 0) {
		/* Some bits would be removed - that's ok (restricting). */
	} else if (caps != priv(rp)->s_cap_bound) {
		/* Trying to add bits - not allowed. */
		return(EPERM);
	}

	/* Apply restriction: new bound = old bound ∩ requested. */
	priv(rp)->s_cap_bound &= caps;
	/* Also restrict permitted and effective to new bound. */
	priv(rp)->s_cap_permitted &= priv(rp)->s_cap_bound;
	priv(rp)->s_cap_effective &= priv(rp)->s_cap_bound;
	return(OK);

  case CAP_OP_LIST:
	/* List all defined capabilities.
	 * Returns the full set mask in caps field. Caller can interpret.
	 */
	m_ptr->CAPCTL_CAPS = CAP_FULL;
	return(OK);

  default:
	printf("do_capctl: bad operation %d\n",
		m_ptr->CAPCTL_OP);
	return(EINVAL);
  }
}

#endif /* USE_CAPCTL */
