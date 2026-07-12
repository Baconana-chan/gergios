/* Recovery strategies for the Reincarnation Server (RS).
 *
 * Level 5: "Proactive Recovery" — RS tries purpose-built recovery
 * strategies based on the failure cause before falling through to
 * the standard restart logic.
 *
 * Design:
 *   Each crash cycle tries ONE untried strategy. If the service crashes
 *   again, the next strategy in the plan is attempted. This allows the
 *   system to test progressively more invasive mitigations without
 *   busy-looping on the same failing approach.
 *
 *   Plans are defined per fail_reason. Most plans end with
 *   STRAT_USER_ALERT + STRAT_SURRENDER, which prints a diagnostic
 *   summary to the console and prevents further automatic restarts.
 */
#include "inc.h"
#include "strategy.h"

/* ========================================================================= *
 * Static recovery plan tables
 * ========================================================================= */

/* FAIL_UNKNOWN — no clear cause, try standard approaches. */
static const struct recovery_plan plan_unknown = {
    .strategies = {
        STRAT_RESTART,
        STRAT_RESTART_DEPS,
        STRAT_RESTART_CLEAN,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 5,
    .max_attempts = 5,
};

/* FAIL_SEGFAULT — likely a software bug, isolate and try clean slate. */
static const struct recovery_plan plan_segfault = {
    .strategies = {
        STRAT_RESTART_ISOLATE,
        STRAT_RESTART_DEPS,
        STRAT_RESTART_CLEAN,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 5,
    .max_attempts = 5,
};

/* FAIL_NOMEM — out of memory: free resources, then restart. */
static const struct recovery_plan plan_nomem = {
    .strategies = {
        STRAT_FREE_MEMORY,
        STRAT_CLEAR_CACHE,
        STRAT_RESTART_CLEAN,
        STRAT_RESTART_MINIMAL,
        STRAT_SCHED_BOOST,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 7,
    .max_attempts = 10,
};

/* FAIL_TIMEOUT — service hung, try more aggressive restart. */
static const struct recovery_plan plan_timeout = {
    .strategies = {
        STRAT_RESTART,
        STRAT_RESTART_DEPS,
        STRAT_RESTART_CLEAN,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 5,
    .max_attempts = 5,
};

/* FAIL_HW_ERROR — hardware error (SIGBUS), try fallback first. */
static const struct recovery_plan plan_hw = {
    .strategies = {
        STRAT_RESTART,
        STRAT_RESTART_MINIMAL,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 4,
    .max_attempts = 3,
};

/* FAIL_DEP_DIED — dependency problem already handled by Level 3 cascade. */
static const struct recovery_plan plan_dep_died = {
    .strategies = {
        STRAT_RESTART_DEPS,
        STRAT_RESTART,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 4,
    .max_attempts = 5,
};

/* FAIL_RESOURCE_EXHAUSTION — leaked FDs or IPC queues. */
static const struct recovery_plan plan_resource = {
    .strategies = {
        STRAT_FREE_MEMORY,
        STRAT_RESTART_CLEAN,
        STRAT_RESTART_MINIMAL,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 5,
    .max_attempts = 5,
};

/* FAIL_SOFTWARE_BUG — assert/abort, treat like segfault. */
static const struct recovery_plan plan_software = {
    .strategies = {
        STRAT_RESTART_ISOLATE,
        STRAT_RESTART_CLEAN,
        STRAT_RESTART_DEPS,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 5,
    .max_attempts = 5,
};

/* FAIL_KILLED — explicitly killed, just restart. */
static const struct recovery_plan plan_killed = {
    .strategies = {
        STRAT_RESTART,
        STRAT_RESTART_CLEAN,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 4,
    .max_attempts = 3,
};

/* FAIL_INIT_FAILURE — initialization failed, try clean restart. */
static const struct recovery_plan plan_init = {
    .strategies = {
        STRAT_RESTART_CLEAN,
        STRAT_RESTART_MINIMAL,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 4,
    .max_attempts = 5,
};

/* FAIL_DEADLOCK — peer services stuck, restart everything. */
static const struct recovery_plan plan_deadlock = {
    .strategies = {
        STRAT_RESTART_DEPS,
        STRAT_RESTART_CLEAN,
        STRAT_USER_ALERT,
        STRAT_SURRENDER,
    },
    .num_strategies = 4,
    .max_attempts = 5,
};

/* ========================================================================= *
 * Plan selection
 * ========================================================================= */
const struct recovery_plan *recovery_get_plan(enum fail_reason reason)
{
    switch (reason) {
    case FAIL_SEGFAULT:             return &plan_segfault;
    case FAIL_NOMEM:                return &plan_nomem;
    case FAIL_TIMEOUT:              return &plan_timeout;
    case FAIL_DEADLOCK:             return &plan_deadlock;
    case FAIL_HW_ERROR:             return &plan_hw;
    case FAIL_DEP_DIED:             return &plan_dep_died;
    case FAIL_RESOURCE_EXHAUSTION:  return &plan_resource;
    case FAIL_SOFTWARE_BUG:         return &plan_software;
    case FAIL_KILLED:               return &plan_killed;
    case FAIL_INIT_FAILURE:         return &plan_init;
    case FAIL_UNKNOWN:
    default:                        return &plan_unknown;
    }
}

/* ========================================================================= *
 * Strategy-specific helpers
 *
 * Each attempt_*() function returns OK if the strategy can proceed
 * (i.e., we should continue with this plan), or an error code if the
 * strategy is not applicable.
 * ========================================================================= */

/* STRAT_RESTART — nothing special, just proceed with normal restart. */
static int attempt_restart(struct rproc *rp)
{
    if (rs_verbose)
        printf("RS: recovery strategy STRAT_RESTART for %s\n",
            srv_to_string(rp));
    return OK;
}

/* STRAT_RESTART_DEPS — trigger cascade restart of critical deps. */
static int attempt_restart_deps(struct rproc *rp)
{
    if (rs_verbose)
        printf("RS: recovery strategy STRAT_RESTART_DEPS for %s\n",
            srv_to_string(rp));

    /* cascade_restart() internally checks dependencies and handles the
     * case where no deps are dead (Phase 2 just restarts the service). */
    rp->r_flags |= RS_DEP_FAIL;
    cascade_restart(rp);
    return OK;
}

/* STRAT_RESTART_CLEAN — reset restarts counter for a clean slate backoff. */
static int attempt_restart_clean(struct rproc *rp)
{
    if (rs_verbose)
        printf("RS: recovery strategy STRAT_RESTART_CLEAN for %s\n",
            srv_to_string(rp));

    /* Reset the backoff so the next restart has no delay.
     * The service will start fresh. */
    rp->r_backoff = 0;
    rp->r_restarts = 0;
    return OK;
}

/* STRAT_RESTART_ISOLATE — force a new endpoint via reincarnation. */
static int attempt_restart_isolate(struct rproc *rp)
{
    if (rs_verbose)
        printf("RS: recovery strategy STRAT_RESTART_ISOLATE for %s\n",
            srv_to_string(rp));

    /* Set the reincarnate flag so the service gets a new endpoint.
     * This is handled by the existing code in terminate_service(). */
    rp->r_flags |= RS_REINCARNATE;
    rp->r_backoff = 0;
    return OK;
}

/* STRAT_RESTART_MINIMAL — reduce resource consumption on next restart. */
static int attempt_restart_minimal(struct rproc *rp)
{
    int new_priority;
    int new_quantum;

    if (rs_verbose)
        printf("RS: recovery strategy STRAT_RESTART_MINIMAL for %s\n",
            srv_to_string(rp));

    /* Reduce resource consumption for the next restart:
     *   - Lower priority (higher numeric value = less CPU)
     *   - Shorter quantum (less CPU time per slice)
     * These values will be applied when sched_init_proc() is called
     * during the next restart. */
    new_priority = rp->r_priority + 5;
    if (new_priority > 19)
        new_priority = 19;

    new_quantum = rp->r_quantum / 2;
    if (new_quantum < 1)
        new_quantum = 1;

    rp->r_backoff = 0;
    rp->r_priority = new_priority;
    rp->r_quantum = new_quantum;

    if (rs_verbose)
        printf("RS: %s minimal mode: priority %d -> %d, quantum %d -> %d\n",
            srv_to_string(rp),
            rp->r_priority - 5, rp->r_priority,
            rp->r_quantum * 2, rp->r_quantum);

    return OK;
}

/* STRAT_FREE_MEMORY — reclaim cached pages via VM to free memory. */
static int attempt_free_memory(struct rproc *rp)
{
    struct vm_stats_info stats;
    int s;
    struct rproc *rp2;

    if (rs_verbose)
        printf("RS: recovery strategy STRAT_FREE_MEMORY for %s\n",
            srv_to_string(rp));

    /* Query current memory state to check if there's actually pressure. */
    s = vm_info_stats(&stats);
    if (s != OK) {
        if (rs_verbose)
            printf("RS: vm_info_stats failed: %d\n", s);
        return s;
    }

    if (rs_verbose)
        printf("RS: memory before reclaim: free=%lu cached=%lu total=%lu pages\n",
            stats.vsi_free, stats.vsi_cached, stats.vsi_total);

    /* If there are cached pages backed by block devices, clear them.
     * This is the most effective way to reclaim memory without killing
     * processes: iterate all active services with a dev_nr (block
     * drivers) and tell VM to release their cached disk blocks. */
    for (rp2 = BEG_RPROC_ADDR; rp2 < END_RPROC_ADDR; rp2++) {
        if (!(rp2->r_flags & RS_ACTIVE))
            continue;

        if (rp2->r_pub->dev_nr > 0) {
            if (rs_verbose)
                printf("RS: clearing VM cache for %s (dev %d)\n",
                    rp2->r_pub->label, rp2->r_pub->dev_nr);

            s = vm_clear_cache(rp2->r_pub->dev_nr);
            if (s != OK && rs_verbose)
                printf("RS: vm_clear_cache(%d) returned %d\n",
                    rp2->r_pub->dev_nr, s);
        }
    }

    /* Re-query memory state to see what we reclaimed. */
    s = vm_info_stats(&stats);
    if (s == OK && rs_verbose)
        printf("RS: memory after reclaim: free=%lu cached=%lu\n",
            stats.vsi_free, stats.vsi_cached);

    return OK;
}

/* STRAT_CLEAR_CACHE — ask VM to clear disk cache for all block devices. */
static int attempt_clear_cache(struct rproc *rp)
{
    int s;
    struct rproc *rp2;

    if (rs_verbose)
        printf("RS: recovery strategy STRAT_CLEAR_CACHE for %s\n",
            srv_to_string(rp));

    /* Clear cached blocks for all active block drivers.
     * This is equivalent to "drop_caches" on Linux. */
    for (rp2 = BEG_RPROC_ADDR; rp2 < END_RPROC_ADDR; rp2++) {
        if (!(rp2->r_flags & RS_ACTIVE))
            continue;

        if (rp2->r_pub->dev_nr > 0) {
            if (rs_verbose)
                printf("RS: clearing cache for %s (dev %d)\n",
                    rp2->r_pub->label, rp2->r_pub->dev_nr);

            s = vm_clear_cache(rp2->r_pub->dev_nr);
            if (s != OK && rs_verbose)
                printf("RS: vm_clear_cache(%d) returned %d\n",
                    rp2->r_pub->dev_nr, s);
        }
    }

    return OK;
}

/* STRAT_SCHED_BOOST — increase priority/quantum via scheduler IPC. */
static int attempt_sched_boost(struct rproc *rp)
{
    int new_priority = rp->r_priority;
    int new_quantum = rp->r_quantum;

    if (rs_verbose)
        printf("RS: recovery strategy STRAT_SCHED_BOOST for %s\n",
            srv_to_string(rp));

    /* Increase priority (lower numeric value = higher priority).
     * Clamp to MIN_USER_Q so we don't interfere with kernel tasks. */
    new_priority -= 3;
    if (new_priority < 1)
        new_priority = 1;

    /* Increase quantum for longer time slices. */
    new_quantum *= 2;
    if (new_quantum > 1000)
        new_quantum = 1000;

    if (rs_verbose)
        printf("RS: %s boost: priority %d -> %d, quantum %d -> %d\n",
            srv_to_string(rp), rp->r_priority, new_priority,
            rp->r_quantum, new_quantum);

    /* Apply via scheduler IPC (handles both KERNEL and user-space sched).
     * This directly updates the running process's scheduling parameters
     * without requiring a restart. */
    return sched_boost(rp->r_scheduler, rp->r_pub->endpoint,
        new_priority, new_quantum);
}

/* STRAT_USER_ALERT — print a diagnostic surrender notice via Anis. */
static int attempt_user_alert(struct rproc *rp, enum fail_reason reason,
    const struct rs_diag_packet *dp)
{
    surrender_log_attempt(rp, STRAT_USER_ALERT, OK,
        "User alert (diagnostic notice)");

    /* Log surrender details to console (compact format). */
    printf("RS: Proactive Recovery failed for %s\n", srv_to_string(rp));
    printf("RS:   Reason: %s, signal: %s (%d), attempts: %d\n",
        fail_reason_to_string(reason),
        signal_num_to_string(dp->d_signal), dp->d_signal,
        rp->r_recovery.rrd_attempts);

    return OK;
}

/* STRAT_SURRENDER — render the Anis surrender box and stop restarts. */
static int attempt_surrender(struct rproc *rp, enum fail_reason reason,
    const struct rs_diag_packet *dp)
{
    /* Render the beautiful surrender box. */
    surrender_render(rp, reason, dp);

    /* Mark the service so the existing logic won't restart it. */
    rp->r_flags |= RS_EXITING;

    /* Log surrender to the audit system. */
    {
        message audit_msg;

        memset(&audit_msg, 0, sizeof(audit_msg));
        audit_msg.AUDIT_OP = AUDIT_OP_LOG;
        audit_msg.AUDIT_LOG_TYPE = AUDIT_PRIV_CHANGE;
        audit_msg.AUDIT_LOG_RESULT = 0;
        audit_msg.AUDIT_LOG_SUBJECT = rp->r_pub->endpoint;
        _kernel_call(SYS_AUDIT, &audit_msg);
    }

    return OK;
}

/* ========================================================================= *
 * execute_recovery_plan — try the next untried strategy
 * ========================================================================= */
int execute_recovery_plan(struct rproc *rp, enum fail_reason reason,
    const struct rs_diag_packet *dp)
{
    const struct recovery_plan *plan;
    struct rs_recovery_data *rd;
    enum recovery_strategy strategy;
    int r;

    rd = &rp->r_recovery;
    plan = recovery_get_plan(reason);

    /* If already surrendered, prevent further restarts. */
    if (rd->rrd_surrendered) {
        if (rs_verbose)
            printf("RS: %s already surrendered, no more recovery\n",
                srv_to_string(rp));
        return RS_RECOVERY_SURRENDER;
    }

    /* Bounds check — if past the end, surrender. */
    if (rd->rrd_current_strategy >= plan->num_strategies ||
        rd->rrd_attempts >= plan->max_attempts) {
        rd->rrd_surrendered = 1;
        printf("RS: %s exhausted all %d recovery strategies after %d attempts.\n",
            srv_to_string(rp), plan->num_strategies, rd->rrd_attempts);
        attempt_user_alert(rp, reason, dp);
        attempt_surrender(rp);
        return RS_RECOVERY_SURRENDER;
    }

    /* Get the next strategy to try. */
    strategy = plan->strategies[rd->rrd_current_strategy];
    rd->rrd_attempts++;

    if (rs_verbose)
        printf("RS: recovery attempt %d/%d, strategy %d for %s\n",
            rd->rrd_attempts, plan->num_strategies,
            (int)strategy, srv_to_string(rp));

    /* Execute the strategy. */
    switch (strategy) {
    case STRAT_RESTART:
        r = attempt_restart(rp);
        break;
    case STRAT_RESTART_DEPS:
        r = attempt_restart_deps(rp);
        break;
    case STRAT_RESTART_CLEAN:
        r = attempt_restart_clean(rp);
        break;
    case STRAT_RESTART_ISOLATE:
        r = attempt_restart_isolate(rp);
        break;
    case STRAT_RESTART_MINIMAL:
        r = attempt_restart_minimal(rp);
        break;
    case STRAT_FREE_MEMORY:
        r = attempt_free_memory(rp);
        break;
    case STRAT_CLEAR_CACHE:
        r = attempt_clear_cache(rp);
        break;
    case STRAT_SCHED_BOOST:
        r = attempt_sched_boost(rp);
        break;
    case STRAT_USER_ALERT:
        r = attempt_user_alert(rp, reason, dp);
        break;
    case STRAT_SURRENDER:
        r = attempt_surrender(rp, reason, dp);
        rd->rrd_surrendered = 1;
        break;
    default:
        printf("RS: unknown recovery strategy %d\n", (int)strategy);
        r = EINVAL;
        break;
    }

    /* Log this attempt to the surrender attempt history. */
    if (strategy != STRAT_USER_ALERT && strategy != STRAT_SURRENDER) {
        int log_result;

        if (r == OK) {
            log_result = OK;
        } else {
            log_result = r; /* errno value = fail */
        }

        /* Pass NULL so surrender_log_attempt() auto-fills the name
         * from its internal strategy_name() function. */
        surrender_log_attempt(rp, strategy, log_result, NULL);
    }

    /* Advance to the next strategy for the next crash cycle. */
    rd->rrd_current_strategy++;

    if (r != OK) {
        if (rs_verbose)
            printf("RS: recovery strategy %d failed for %s (%d)\n",
                (int)strategy, srv_to_string(rp), r);
    }

    /* Check surrender after the strategy executed. */
    if (rd->rrd_surrendered)
        return RS_RECOVERY_SURRENDER;

    return RS_RECOVERY_OK;
}

/* ========================================================================= *
 * recovery_reset — clear recovery tracking after successful restart
 * ========================================================================= */
void recovery_reset(struct rproc *rp)
{
    memset(&rp->r_recovery, 0, sizeof(rp->r_recovery));
    if (rs_verbose)
        printf("RS: recovery tracking reset for %s\n", srv_to_string(rp));
}
