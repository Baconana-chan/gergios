/* Healthcheck framework for the Reincarnation Server (RS).
 *
 * Level 2: "Health monitoring" — actively checks service health
 * via registered healthchecks and triggers recovery on failure.
 */
#include "inc.h"
#include "health.h"
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <string.h>
#include <stdio.h>

/*===========================================================================*
 *                    Helper: find healthcheck slot                          *
 *===========================================================================*/
static struct rs_healthcheck *find_hc(struct rproc *rp, const char *name)
{
/* Find a healthcheck by name for the given service. */
    int i;
    if (!rp || !rp->r_healthchecks)
        return NULL;
    for (i = 0; i < RS_HC_MAX_PER_SERVICE; i++) {
        struct rs_healthcheck *hc = &rp->r_healthchecks[i];
        if (hc->hc_type != 0 && strcmp(hc->hc_name, name) == 0)
            return hc;
    }
    return NULL;
}

/*===========================================================================*
 *                    Helper: find free healthcheck slot                     *
 *===========================================================================*/
static struct rs_healthcheck *find_free_hc(struct rproc *rp)
{
/* Find an unused healthcheck slot for the given service. */
    int i;
    if (!rp || !rp->r_healthchecks)
        return NULL;
    for (i = 0; i < RS_HC_MAX_PER_SERVICE; i++) {
        if (rp->r_healthchecks[i].hc_type == 0)
            return &rp->r_healthchecks[i];
    }
    return NULL;
}

/*===========================================================================*
 *                do_register_healthcheck                                    *
 *===========================================================================*/
int do_register_healthcheck(message *m_ptr)
{
/* Register a healthcheck for a service.
 *
 * The caller passes a struct rs_hc_req via m_rs_req.addr / m_rs_req.len
 * (safecopy), following the same pattern as RS_UP.
 */
    int who_p, s;
    struct rproc *rp, *target_rp;
    struct rs_healthcheck *hc;
    struct rs_hc_req hc_req;
    int slot;

    /* Lookup caller. */
    if (rs_isokendpt(m_ptr->m_source, &who_p) != OK) {
        return EINVAL;
    }
    rp = rproc_ptr[who_p];
    if (!rp || !(rp->r_flags & RS_IN_USE)) {
        return ESRCH;
    }

    /* Copy the request structure from caller space. */
    if (m_ptr->m_rs_req.len < sizeof(struct rs_hc_req)) {
        return EINVAL;
    }
    s = sys_datacopy(m_ptr->m_source,
        (vir_bytes) m_ptr->m_rs_req.addr,
        SELF, (vir_bytes) &hc_req, sizeof(hc_req));
    if (s != OK) {
        return s;
    }

    /* Validate healthcheck type. */
    if (hc_req.rsh_type < HC_PING || hc_req.rsh_type > HC_CUSTOM) {
        return EINVAL;
    }

    /* Validate target endpoint. */
    if (hc_req.rsh_ep == ANY || hc_req.rsh_ep == NONE) {
        return EINVAL;
    }
    if (rs_isokendpt(hc_req.rsh_ep, &slot) != OK) {
        return EINVAL;
    }
    target_rp = rproc_ptr[slot];
    if (!target_rp || !(target_rp->r_flags & RS_IN_USE)) {
        return ESRCH;
    }

    /* Check permission: caller must be root or able to control target. */
    s = check_call_permission(m_ptr->m_source, 0, target_rp);
    if (s != OK) {
        return s;
    }

    /* Make sure name is null-terminated. */
    hc_req.rsh_name[RS_HC_NAME_LEN - 1] = '\0';
    if (hc_req.rsh_name[0] == '\0') {
        return EINVAL;
    }

    /* Allocate healthchecks array if not yet present. */
    if (!target_rp->r_healthchecks) {
        target_rp->r_healthchecks = (struct rs_healthcheck *)
            malloc(sizeof(struct rs_healthcheck) * RS_HC_MAX_PER_SERVICE);
        if (!target_rp->r_healthchecks) {
            return ENOMEM;
        }
        memset(target_rp->r_healthchecks, 0,
            sizeof(struct rs_healthcheck) * RS_HC_MAX_PER_SERVICE);
    }

    /* Check for duplicate name. */
    if (find_hc(target_rp, hc_req.rsh_name) != NULL) {
        return EEXIST;
    }

    /* Find free slot. */
    hc = find_free_hc(target_rp);
    if (!hc) {
        return ENOMEM;  /* no slots available */
    }

    /* Fill in the healthcheck. */
    hc->hc_type = hc_req.rsh_type;
    hc->hc_endpoint = hc_req.rsh_ep;
    hc->hc_interval = (hc_req.rsh_interval > 0) ? hc_req.rsh_interval : RS_DELTA_T;
    hc->hc_timeout = (hc_req.rsh_timeout > 0) ? hc_req.rsh_timeout : 2 * hc->hc_interval;
    hc->hc_last_check = 0;
    hc->hc_last_response = getticks();
    hc->hc_consecutive_failures = 0;
    strlcpy(hc->hc_name, hc_req.rsh_name, RS_HC_NAME_LEN);

    if (rs_verbose)
        printf("RS: healthcheck '%s' registered for %s (type=%d, interval=%ld)\n",
            hc->hc_name, srv_to_string(target_rp), hc_req.rsh_type,
            hc->hc_interval);

    return OK;
}

/*===========================================================================*
 *                do_unregister_healthcheck                                  *
 *===========================================================================*/
int do_unregister_healthcheck(message *m_ptr)
{
/* Unregister a healthcheck by name.
 *
 * The caller passes a struct rs_hc_req via m_rs_req.addr / m_rs_req.len
 * (safecopy), same as REGISTER. Only rsh_ep and rsh_name are used.
 */
    int who_p, slot, s;
    struct rproc *rp, *target_rp;
    struct rs_healthcheck *hc;
    struct rs_hc_req hc_req;

    /* Lookup caller. */
    if (rs_isokendpt(m_ptr->m_source, &who_p) != OK) {
        return EINVAL;
    }
    rp = rproc_ptr[who_p];
    if (!rp || !(rp->r_flags & RS_IN_USE)) {
        return ESRCH;
    }

    /* Copy the request structure from caller space. */
    if (m_ptr->m_rs_req.len < sizeof(struct rs_hc_req)) {
        return EINVAL;
    }
    s = sys_datacopy(m_ptr->m_source,
        (vir_bytes) m_ptr->m_rs_req.addr,
        SELF, (vir_bytes) &hc_req, sizeof(hc_req));
    if (s != OK) {
        return s;
    }
    hc_req.rsh_name[RS_HC_NAME_LEN - 1] = '\0';

    /* Validate target. */
    if (rs_isokendpt(hc_req.rsh_ep, &slot) != OK) {
        return EINVAL;
    }
    target_rp = rproc_ptr[slot];
    if (!target_rp || !(target_rp->r_flags & RS_IN_USE)) {
        return ESRCH;
    }

    /* Check permission. */
    s = check_call_permission(m_ptr->m_source, 0, target_rp);
    if (s != OK) {
        return s;
    }

    /* Find and clear the healthcheck. */
    hc = find_hc(target_rp, hc_req.rsh_name);
    if (!hc) {
        return ESRCH;
    }
    memset(hc, 0, sizeof(struct rs_healthcheck));

    if (rs_verbose)
        printf("RS: healthcheck '%s' removed for %s\n",
            hc_req.rsh_name, srv_to_string(target_rp));

    return OK;
}

/*===========================================================================*
 *                healthcheck_count                                          *
 *===========================================================================*/
int healthcheck_count(struct rproc *rp)
{
/* Count active healthchecks for a service. */
    int i, count = 0;
    if (!rp || !rp->r_healthchecks)
        return 0;
    for (i = 0; i < RS_HC_MAX_PER_SERVICE; i++) {
        if (rp->r_healthchecks[i].hc_type != 0)
            count++;
    }
    return count;
}

/*===========================================================================*
 *                check_service_health                                       *
 *===========================================================================*/
int check_service_health(struct rproc *rp, clock_t now)
{
/* Run all registered healthchecks for a service.
 * Returns 0 if all healthy, -1 if a check failed.
 */
    int i;
    enum healthcheck_result result;

    if (!rp || !rp->r_healthchecks)
        return 0;

    for (i = 0; i < RS_HC_MAX_PER_SERVICE; i++) {
        struct rs_healthcheck *hc = &rp->r_healthchecks[i];
        if (hc->hc_type == 0)
            continue;

        /* Check if interval has expired. */
        if (now - hc->hc_last_check < hc->hc_interval)
            continue;

        hc->hc_last_check = now;

        switch (hc->hc_type) {
        case HC_PING:
            /* Send a notification to the service. If it's alive,
             * it doesn't need to respond — we just check later
             * whether it crashed. For now, we check by seeing if
             * the service's alive_tm is recent enough.
             */
            if (now - rp->r_alive_tm > hc->hc_timeout) {
                result = HC_FAIL_TIMEOUT;
            } else {
                result = HC_OK;
            }
            break;

        case HC_HEARTBEAT:
            /* Check if heartbeat is timely (same as existing RS logic). */
            if (rp->r_period > 0 && rp->r_alive_tm < rp->r_check_tm &&
                now - rp->r_alive_tm > hc->hc_timeout) {
                result = HC_FAIL_TIMEOUT;
            } else {
                result = HC_OK;
            }
            break;

        case HC_RESOURCES:
            /* For now, just check if the service is still alive.
             * Future: query VM for memory usage.
             */
            if (now - rp->r_alive_tm > hc->hc_timeout) {
                result = HC_FAIL_TIMEOUT;
            } else {
                result = HC_OK;
            }
            break;

        case HC_RESPONSE_TIME:
            /* Check if the service's last response was timely.
             * Uses the last_response timestamp updated when the
             * service responds to IPC.
             */
            if (now - hc->hc_last_response > hc->hc_timeout) {
                result = HC_FAIL_SLOW;
            } else {
                result = HC_OK;
            }
            break;

        case HC_CUSTOM:
            /* Custom checks need a registered callback.
             * For now, treat as always OK (placeholder for Level 4+).
             */
            result = HC_OK;
            break;

        default:
            result = HC_OK;
            break;
        }

        if (result != HC_OK) {
            hc->hc_consecutive_failures++;
            handle_healthcheck_failure(rp, hc, result);
            return -1;  /* stop at first failure */
        } else {
            hc->hc_consecutive_failures = 0;
        }
    }

    return 0;
}

/*===========================================================================*
 *                handle_healthcheck_failure                                 *
 *===========================================================================*/
void handle_healthcheck_failure(struct rproc *rp,
    struct rs_healthcheck *hc, enum healthcheck_result result)
{
/* Handle a failed healthcheck — log it and trigger recovery. */
    const char *reason;

    switch (result) {
    case HC_FAIL_TIMEOUT:
        reason = "timeout (no response)";
        break;
    case HC_FAIL_CRASHED:
        reason = "crashed";
        break;
    case HC_FAIL_RESOURCES:
        reason = "resource exhaustion";
        break;
    case HC_FAIL_SLOW:
        reason = "response too slow";
        break;
    case HC_FAIL_CUSTOM:
        reason = "custom check failed";
        break;
    default:
        reason = "unknown";
        break;
    }

    printf("RS: %s FAILED healthcheck '%s' (%s), %d consecutive failure(s)\n",
        srv_to_string(rp), hc->hc_name, reason, hc->hc_consecutive_failures);

    /* After 3 consecutive failures, trigger a restart. */
    if (hc->hc_consecutive_failures >= 3) {
        printf("RS: %s restarting after %d healthcheck failures\n",
            srv_to_string(rp), hc->hc_consecutive_failures);

        /* Set flag to prevent the heartbeat loop from also reacting. */
        rp->r_flags |= RS_HEALTHCHECK_FAIL;

        if (rp->r_flags & RS_TERMINATED || rp->r_flags & RS_EXITING) {
            /* Service is already dead — clear flag and restart directly. */
            rp->r_flags &= ~RS_HEALTHCHECK_FAIL;
            restart_service(rp);
        } else {
            /* If the service is still alive (e.g. slow response), force a
             * crash to trigger the normal restart via terminate_service().
             */
            crash_service(rp);
        }
    }
}
