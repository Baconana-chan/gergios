/* Recovery strategies for the Reincarnation Server (RS).
 *
 * Level 5: "Proactive Recovery" — RS tries multiple recovery strategies
 * based on the analyzed failure cause before falling through to the
 * standard restart logic.
 *
 * Each crash cycle tries ONE strategy. If the service crashes again,
 * the next strategy in the plan is tried. If all strategies are
 * exhausted, RS surrenders and logs a final diagnostic report to disk.
 *
 * Recovery plan selection is based on the fail_reason from Level 4
 * (see diag.h). Each fail_reason has a purpose-built plan.
 */
#ifndef RS_STRATEGY_H
#define RS_STRATEGY_H

#include <minix/type.h>
#include "diag.h"  /* for enum fail_reason, struct rs_diag_packet */

/* ========================================================================= *
 * Constants
 * ========================================================================= */

/* Recovery strategy indices — ordered from least to most invasive. */
enum recovery_strategy {
    STRAT_RESTART = 1,              /* simple restart (existing behaviour) */
    STRAT_RESTART_DEPS,             /* restart dependencies first */
    STRAT_RESTART_CLEAN,            /* fresh binary, clean state */
    STRAT_RESTART_ISOLATE,          /* new endpoint, isolate from failures */
    STRAT_RESTART_MINIMAL,          /* reduced resource mode */
    STRAT_FREE_MEMORY,              /* ask VM to reclaim memory */
    STRAT_CLEAR_CACHE,              /* ask VM to clear disk cache */
    STRAT_SCHED_BOOST,              /* increase priority/quantum via scheduler */
    STRAT_USER_ALERT,               /* print surrender notice to console */
    STRAT_SURRENDER,                /* give up — no more restarts */
};

/* Return codes from execute_recovery_plan(). */
#define RS_RECOVERY_OK          0   /* strategy executed, continue normally */
#define RS_RECOVERY_SURRENDER   1   /* all strategies exhausted — stop restarts */
#define RS_RECOVERY_ERROR      -1   /* internal error */

/* Maximum strategies per plan. */
#define RS_MAX_STRATEGIES       12

/* Maximum recovery attempts across all crash cycles before surrender. */
#define RS_MAX_RECOVERY_ATTEMPTS   20

/* Maximum attempt log entries stored per-service. Must match the array size
 * in struct rs_recovery_data below. */
#define RS_MAX_ATTEMPT_LOG        20

/* ========================================================================= *
 * Recovery plan — one per fail_reason
 * ========================================================================= */
struct recovery_plan {
    enum recovery_strategy strategies[RS_MAX_STRATEGIES];
    int  num_strategies;             /* number of strategies in the plan */
    int  max_attempts;               /* max total attempts before surrender */
};

/* ========================================================================= *
 * Per-attempt log entry for surrender history
 * ========================================================================= */
#define RS_SURRENDER_DESC_LEN     48

struct rs_attempt_entry {
    enum recovery_strategy strategy;        /* which strategy was tried */
    int result;                              /* OK = success, !OK = fail */
    char desc[RS_SURRENDER_DESC_LEN];        /* human-readable description */
};

/* ========================================================================= *
 * Per-service recovery tracking (stored in struct rproc)
 * ========================================================================= */
struct rs_recovery_data {
    int rrd_attempts;                /* total attempts this plan */
    int rrd_current_strategy;        /* index into plan->strategies (next to try) */
    int rrd_surrendered;             /* 1 = surrendered, do not restart */
    struct rs_attempt_entry rrd_attempt_log[RS_MAX_ATTEMPT_LOG];  /* attempt history */
    int rrd_attempt_count;           /* number of entries in attempt_log */
};

/* ========================================================================= *
 * Function prototypes
 * ========================================================================= */

/* Get the recovery plan for a given failure reason. */
const struct recovery_plan *recovery_get_plan(enum fail_reason reason);

/* Execute the next recovery strategy for a service.
 *
 * Called from terminate_service() after diagnostics collection.
 * Tries ONE strategy per crash cycle (the next untried one).
 *
 * Returns:
 *   RS_RECOVERY_OK        — strategy executed, proceed with normal restart
 *   RS_RECOVERY_SURRENDER — all strategies exhausted, do NOT restart
 *   RS_RECOVERY_ERROR     — internal failure
 */
int execute_recovery_plan(struct rproc *rp, enum fail_reason reason,
    const struct rs_diag_packet *dp);

/* Reset recovery tracking for a service (e.g., after successful restart). */
void recovery_reset(struct rproc *rp);

#endif /* RS_STRATEGY_H */
