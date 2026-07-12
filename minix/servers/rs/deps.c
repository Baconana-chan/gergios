/* Dependency graph implementation for the Reincarnation Server (RS).
 *
 * Level 3: "Dependency-aware recovery" — cascade restart logic.
 *
 * Built-in dependency table (deps_init_table): defines known dependencies
 * between core system services, loaded at boot. Additional dependencies
 * can be registered at runtime via RS_REGISTER_DEP IPC.
 */
#include "inc.h"
#include "deps.h"
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <string.h>
#include <stdio.h>

/*===========================================================================*
 *                Built-in dependency table                                  *
 *===========================================================================*
 * Known dependencies between MINIX system services.
 * These are loaded at boot into per-service r_deps arrays.
 *
 * restart_priority: lower number = restart first.
 *   0 = this dep must be started before everything else
 *   1 = block drivers (storage backend)
 *   2 = filesystem servers
 *   3 = VFS and higher-level services
 */
static const struct rs_dep builtin_deps[] = {
    /* VFS → block driver (critical: VFS can't work without block I/O).
     * NOTE: MEM_PROC_NR is used as a placeholder for "the block device
     * driver" since MINIX block drivers are started dynamically (AHCI,
     * virtio-blk, etc.) and don't have fixed endpoint numbers.
     * A future enhancement should make this extensible. */
    { VFS_PROC_NR,  MEM_PROC_NR,  .rs_flags = RS_DEP_CRITICAL,
      .rs_restart_priority = 1, .rs_reason = "provides block I/O" },

    /* MFS → block driver (critical) */
    { MFS_PROC_NR,  MEM_PROC_NR,  .rs_flags = RS_DEP_CRITICAL,
      .rs_restart_priority = 1, .rs_reason = "provides block I/O" },

    /* All services → PM (critical: process management) */
    { ANY,          PM_PROC_NR,   .rs_flags = RS_DEP_CRITICAL,
      .rs_restart_priority = 0, .rs_reason = "process management" },

    /* All services → VM (critical: memory management) */
    { ANY,          VM_PROC_NR,   .rs_flags = RS_DEP_CRITICAL,
      .rs_restart_priority = 0, .rs_reason = "memory management" },

    /* All services → RS (critical: service management) */
    { ANY,          RS_PROC_NR,   .rs_flags = RS_DEP_CRITICAL,
      .rs_restart_priority = 0, .rs_reason = "service management" },

    /* VFS → MFS (non-critical: only if /usr is mounted) */
    { VFS_PROC_NR,  MFS_PROC_NR,  .rs_flags = 0,
      .rs_restart_priority = 2, .rs_reason = "root filesystem" },

    /* Terminator. */
    { NONE, NONE, .rs_flags = 0, .rs_restart_priority = 0, .rs_reason = "" },
};

/*===========================================================================*
 *                    Helper: find free dep slot                             *
 *===========================================================================*/
static struct rs_dep *dep_find_free(struct rproc *rp)
{
    int i;
    if (!rp || !rp->r_deps)
        return NULL;
    for (i = 0; i < RS_DEP_MAX_PER_SERVICE; i++) {
        if (rp->r_deps[i].rs_service == NONE ||
            rp->r_deps[i].rs_service == 0)
            return &rp->r_deps[i];
    }
    return NULL;
}

/*===========================================================================*
 *                    dep_find — find dep by target                          *
 *===========================================================================*/
struct rs_dep *dep_find(struct rproc *rp, endpoint_t target)
{
    int i;
    if (!rp || !rp->r_deps)
        return NULL;
    for (i = 0; i < RS_DEP_MAX_PER_SERVICE; i++) {
        if ((rp->r_deps[i].rs_flags & (RS_DEP_CRITICAL | RS_DEP_REGISTERED))
            && rp->r_deps[i].rs_depends_on == target)
            return &rp->r_deps[i];
    }
    return NULL;
}

/*===========================================================================*
 *                    dep_count                                              *
 *===========================================================================*/
int dep_count(struct rproc *rp)
{
    int i, count = 0;
    if (!rp || !rp->r_deps)
        return 0;
    for (i = 0; i < RS_DEP_MAX_PER_SERVICE; i++) {
        if (rp->r_deps[i].rs_service != NONE &&
            rp->r_deps[i].rs_service != 0)
            count++;
    }
    return count;
}

/*===========================================================================*
 *                    deps_init_table — load built-in deps                   *
 *===========================================================================*/
void deps_init_table(void)
{
/* Load the built-in dependency table into per-service r_deps arrays.
 * Called once at boot time during sef_cb_init_fresh.
 */
    int i, slot;
    const struct rs_dep *bdep;
    struct rproc *rp;
    struct rs_dep *dep;

    for (i = 0; builtin_deps[i].rs_service != NONE; i++) {
        bdep = &builtin_deps[i];

        /* "ANY" means all services — iterate all slots. */
        if (bdep->rs_service == ANY) {
            for (slot = 0; slot < NR_SYS_PROCS; slot++) {
                rp = &rproc[slot];
                if (!(rp->r_flags & RS_IN_USE))
                    continue;

                /* Allocate deps array if needed. */
                if (!rp->r_deps) {
                    rp->r_deps = malloc(
                        sizeof(struct rs_dep) * RS_DEP_MAX_PER_SERVICE);
                    if (!rp->r_deps) {
                        printf("RS: deps_init_table: ENOMEM for %s\n",
                            rp->r_pub->label);
                        continue;
                    }
                    memset(rp->r_deps, 0,
                        sizeof(struct rs_dep) * RS_DEP_MAX_PER_SERVICE);
                }

                dep = dep_find_free(rp);
                if (!dep) {
                    printf("RS: deps_init_table: no dep slot for %s\n",
                        rp->r_pub->label);
                    continue;
                }
                *dep = *bdep;
                dep->rs_service = rp->r_pub->endpoint;
            }
        } else {
            /* Single service. */
            if (rs_isokendpt(bdep->rs_service, &slot) != OK)
                continue;
            rp = rproc_ptr[slot];
            if (!rp || !(rp->r_flags & RS_IN_USE))
                continue;

            if (!rp->r_deps) {
                rp->r_deps = malloc(
                    sizeof(struct rs_dep) * RS_DEP_MAX_PER_SERVICE);
                if (!rp->r_deps) {
                    printf("RS: deps_init_table: ENOMEM for %s\n",
                        rp->r_pub->label);
                    continue;
                }
                memset(rp->r_deps, 0,
                    sizeof(struct rs_dep) * RS_DEP_MAX_PER_SERVICE);
            }

            dep = dep_find_free(rp);
            if (!dep) {
                printf("RS: deps_init_table: no dep slot for %s\n",
                    rp->r_pub->label);
                continue;
            }
            *dep = *bdep;
        }
    }

    if (rs_verbose)
        printf("RS: dependency table initialized\n");
}

/*===========================================================================*
 *                    do_register_dep                                        *
 *===========================================================================*/
int do_register_dep(message *m_ptr)
{
/* Register a dependency via IPC.
 *
 * The caller passes a struct rs_dep_req via m_rs_req.addr / m_rs_req.len
 * (safecopy, same pattern as RS_UP).
 */
    int who_p, slot, s;
    struct rproc *rp, *dep_rp;
    struct rs_dep_req dep_req;
    struct rs_dep *dep;

    /* Copy the request structure (no caller lookup needed — we use
     * dep_req.rsr_service as the target). */
    if (m_ptr->m_rs_req.len < sizeof(struct rs_dep_req))
        return EINVAL;
    s = sys_datacopy(m_ptr->m_source,
        (vir_bytes) m_ptr->m_rs_req.addr,
        SELF, (vir_bytes) &dep_req, sizeof(dep_req));
    if (s != OK)
        return s;
    dep_req.rsr_reason[RS_DEP_REASON_LEN - 1] = '\0';

    /* Validate service endpoint. */
    if (dep_req.rsr_service == ANY || dep_req.rsr_service == NONE)
        return EINVAL;
    if (rs_isokendpt(dep_req.rsr_service, &slot) != OK)
        return EINVAL;
    rp = rproc_ptr[slot];
    if (!rp || !(rp->r_flags & RS_IN_USE))
        return ESRCH;

    /* Validate dependency endpoint. */
    if (dep_req.rsr_depends_on == NONE)
        return EINVAL;
    if (rs_isokendpt(dep_req.rsr_depends_on, &slot) != OK)
        return EINVAL;
    dep_rp = rproc_ptr[slot];
    if (!dep_rp || !(dep_rp->r_flags & RS_IN_USE))
        return ESRCH;

    /* Check permission: caller must be root or able to control target. */
    s = check_call_permission(m_ptr->m_source, 0, rp);
    if (s != OK)
        return s;

    /* Check for duplicate. */
    if (dep_find(rp, dep_req.rsr_depends_on) != NULL)
        return EEXIST;

    /* Allocate deps array if needed. */
    if (!rp->r_deps) {
        rp->r_deps = malloc(sizeof(struct rs_dep) * RS_DEP_MAX_PER_SERVICE);
        if (!rp->r_deps)
            return ENOMEM;
        memset(rp->r_deps, 0,
            sizeof(struct rs_dep) * RS_DEP_MAX_PER_SERVICE);
    }

    /* Find free slot. */
    dep = dep_find_free(rp);
    if (!dep)
        return ENOMEM;

    /* Fill in the dependency. */
    dep->rs_service = dep_req.rsr_service;
    dep->rs_depends_on = dep_req.rsr_depends_on;
    dep->rs_flags = (dep_req.rsr_flags & RS_DEP_CRITICAL) |
                    RS_DEP_REGISTERED;
    dep->rs_restart_priority = dep_req.rsr_restart_priority;
    dep->rs_established = getticks();
    strlcpy(dep->rs_reason, dep_req.rsr_reason, RS_DEP_REASON_LEN);

    if (rs_verbose)
        printf("RS: dep %s -> %s (%s)%s\n",
            rp->r_pub->label, dep_rp->r_pub->label,
            dep->rs_reason,
            (dep->rs_flags & RS_DEP_CRITICAL) ? " [CRITICAL]" : "");

    return OK;
}

/*===========================================================================*
 *                    do_unregister_dep                                      *
 *===========================================================================*/
int do_unregister_dep(message *m_ptr)
{
/* Unregister a dependency. Uses the same struct rs_dep_req safecopy pattern.
 * Only rsr_service and rsr_depends_on are used.
 */
    int who_p, slot, s;
    struct rproc *rp;
    struct rs_dep_req dep_req;
    struct rs_dep *dep;

    /* Lookup caller. */
    if (rs_isokendpt(m_ptr->m_source, &who_p) != OK)
        return EINVAL;
    rp = rproc_ptr[who_p];
    if (!rp || !(rp->r_flags & RS_IN_USE))
        return ESRCH;

    /* Copy the request structure. */
    if (m_ptr->m_rs_req.len < sizeof(struct rs_dep_req))
        return EINVAL;
    s = sys_datacopy(m_ptr->m_source,
        (vir_bytes) m_ptr->m_rs_req.addr,
        SELF, (vir_bytes) &dep_req, sizeof(dep_req));
    if (s != OK)
        return s;

    /* Validate service endpoint. */
    if (rs_isokendpt(dep_req.rsr_service, &slot) != OK)
        return EINVAL;
    rp = rproc_ptr[slot];
    if (!rp || !(rp->r_flags & RS_IN_USE))
        return ESRCH;

    /* Check permission. */
    s = check_call_permission(m_ptr->m_source, 0, rp);
    if (s != OK)
        return s;

    /* Find and clear the dependency. */
    dep = dep_find(rp, dep_req.rsr_depends_on);
    if (!dep)
        return ESRCH;
    memset(dep, 0, sizeof(struct rs_dep));

    if (rs_verbose)
        printf("RS: dep %d -> %d removed\n",
            dep_req.rsr_service, dep_req.rsr_depends_on);

    return OK;
}

/*===========================================================================*
 *                    check_dependencies                                    *
 *===========================================================================*/
int check_dependencies(struct rproc *rp)
{
/* Check if all critical dependencies of a service are alive.
 * Returns the number of dead critical deps (0 = all good).
 */
    int i, dead = 0;
    int slot;
    struct rproc *dep_rp;

    if (!rp || !rp->r_deps)
        return 0;

    for (i = 0; i < RS_DEP_MAX_PER_SERVICE; i++) {
        struct rs_dep *dep = &rp->r_deps[i];
        if (dep->rs_service == NONE || dep->rs_service == 0)
            continue;
        if (!(dep->rs_flags & RS_DEP_CRITICAL))
            continue;

        /* Lookup the dependency service. */
        if (rs_isokendpt(dep->rs_depends_on, &slot) != OK) {
            dead++;
            continue;
        }
        dep_rp = rproc_ptr[slot];
        if (!dep_rp || !(dep_rp->r_flags & RS_ACTIVE) ||
            (dep_rp->r_flags & RS_TERMINATED)) {
            dead++;
            if (rs_verbose)
                printf("RS: %s has dead dep %d (%s)\n",
                    srv_to_string(rp), dep->rs_depends_on,
                    dep->rs_reason);
        }
    }

    return dead;
}

/*===========================================================================*
 *                    cascade_restart                                        *
 *===========================================================================*/
int cascade_restart(struct rproc *rp)
{
/* Cascade restart: restart all dead critical dependencies first,
 * then restart the service itself.
 *
 * Algorithm:
 *   1. Iterate all critical deps
 *   2. For each dead critical dep, restart it first
 *   3. Wait briefly for dependency to init (passive — RS will
 *      get the init ready message asynchronously)
 *   4. Then restart the original service
 *
 * Returns OK on success.
 */
    int i, slot, ret = OK;
    struct rproc *dep_rp;

    if (!rp || !rp->r_deps)
        return restart_service(rp), OK;

    if (rs_verbose)
        printf("RS: cascade restart for %s\n", srv_to_string(rp));

    /* Phase 1: restart dead critical dependencies first. */
    for (i = 0; i < RS_DEP_MAX_PER_SERVICE; i++) {
        struct rs_dep *dep = &rp->r_deps[i];
        if (dep->rs_service == NONE || dep->rs_service == 0)
            continue;
        if (!(dep->rs_flags & RS_DEP_CRITICAL))
            continue;

        /* Is this dependency dead? */
        if (rs_isokendpt(dep->rs_depends_on, &slot) != OK) {
            continue;  /* endpoint unknown, can't help it */
        }
        dep_rp = rproc_ptr[slot];
        if (!dep_rp || !(dep_rp->r_flags & RS_ACTIVE) ||
            (dep_rp->r_flags & RS_TERMINATED)) {
            /* Dependency is dead — restart it first. */
            if (rs_verbose)
                printf("RS:   restarting dependency %d (%s) first\n",
                    dep->rs_depends_on, dep->rs_reason);

            if (dep_rp && (dep_rp->r_flags & RS_IN_USE)) {
                /* Attempt restart. restart_service handles
                 * backoff, cloning, etc.
                 */
                restart_service(dep_rp);
            }
        }
    }

    /* Phase 2: restart the service itself. */
    if (rp->r_flags & RS_TERMINATED) {
        if (rs_verbose)
            printf("RS:   now restarting %s\n", srv_to_string(rp));
        restart_service(rp);
    } else {
        /* Service is still alive but we're doing a proactive restart.
         * Mark it for healthcheck-style restart.
         */
        rp->r_flags |= RS_HEALTHCHECK_FAIL;
        crash_service(rp);
    }

    return ret;
}

/*===========================================================================*
 *                    dep_collect_status — for diagnostics (Level 4 prep)   *
 *===========================================================================*/
int dep_collect_status(struct rproc *rp, struct rs_dep_status *statuses,
    int max_statuses)
{
/* Collect the status of all dependencies for diagnostic purposes.
 * Returns number of deps filled.
 */
    int i, slot, count = 0;
    struct rproc *dep_rp;

    if (!rp || !rp->r_deps || !statuses || max_statuses <= 0)
        return 0;

    for (i = 0; i < RS_DEP_MAX_PER_SERVICE && count < max_statuses; i++) {
        struct rs_dep *dep = &rp->r_deps[i];
        if (dep->rs_service == NONE || dep->rs_service == 0)
            continue;

        statuses[count].ds_service = dep->rs_service;
        statuses[count].ds_depends_on = dep->rs_depends_on;

        if (rs_isokendpt(dep->rs_depends_on, &slot) != OK) {
            statuses[count].ds_is_alive = 0;
            statuses[count].ds_is_healthy = 0;
            statuses[count].ds_last_alive = 0;
        } else {
            dep_rp = rproc_ptr[slot];
            statuses[count].ds_is_alive =
                (dep_rp && (dep_rp->r_flags & RS_ACTIVE) &&
                 !(dep_rp->r_flags & RS_TERMINATED)) ? 1 : 0;
            statuses[count].ds_is_healthy =
                statuses[count].ds_is_alive &&
                !(dep_rp->r_flags & RS_HEALTHCHECK_FAIL);
            statuses[count].ds_last_alive =
                dep_rp ? dep_rp->r_alive_tm : 0;
        }
        count++;
    }

    return count;
}
