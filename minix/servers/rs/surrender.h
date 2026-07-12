/* Surrender framework for the Reincarnation Server (RS).
 *
 * Level 6 Phase 1: "Anis — Console surrender"
 *
 * Anis is the name given to the RS's surrender personality — the one
 * who, after exhausting all recovery strategies, produces a beautiful
 * diagnostic report in the console and gracefully gives up.
 *
 * Key components:
 *   - rs_attempt_entry: per-attempt log stored in r_recovery
 *   - surrender_log_attempt(): record each strategy attempt
 *   - surrender_render(): the beautiful surrender box
 */
#ifndef RS_SURRENDER_H
#define RS_SURRENDER_H

#include "strategy.h"  /* for enum recovery_strategy, struct rs_recovery_data */
#include "diag.h"      /* for enum fail_reason, struct rs_diag_packet */

/* ========================================================================= *
 * Constants
 * ========================================================================= */

/* RS_MAX_ATTEMPT_LOG and RS_SURRENDER_DESC_LEN are defined in strategy.h. */

/* Width of the surrender box (characters). Must fit in 80-col console. */
#define RS_SURRENDER_BOX_WIDTH    76

/* Indent for box content from the left edge. */
#define RS_SURRENDER_BOX_INDENT   2

/* ANSI escape codes for console output.
 * These work on serial/QEMU consoles and most VT100-compatible terminals. */
#define ANSI_RED     "\x1B[1;31m"
#define ANSI_GREEN   "\x1B[1;32m"
#define ANSI_YELLOW  "\x1B[1;33m"
#define ANSI_BLUE    "\x1B[1;34m"
#define ANSI_CYAN    "\x1B[1;36m"
#define ANSI_BOLD    "\x1B[1m"
#define ANSI_RESET   "\x1B[0m"

/* CP437 box drawing characters used in VGA text mode console.
 * These are the single-line box drawing set. */
#define BOX_TL       "\xDA"       /* ┌ upper-left corner */
#define BOX_TR       "\xBF"       /* ┐ upper-right corner */
#define BOX_BL       "\xC0"       /* └ lower-left corner */
#define BOX_BR       "\xD9"       /* ┘ lower-right corner */
#define BOX_HORIZ    "\xC4"       /* ─ horizontal line */
#define BOX_VERT     "\xB3"       /* │ vertical line */
#define BOX_CROSS    "\xC5"       /* ┼ cross */
#define BOX_TEE_DOWN "\xC2"       /* ┬ tee pointing down */
#define BOX_TEE_UP   "\xC1"       /* ┴ tee pointing up */

/* ASCII fallback characters for non-VGA consoles. */
#define ASC_TL       "+"
#define ASC_TR       "+"
#define ASC_BL       "+"
#define ASC_BR       "+"
#define ASC_HORIZ    "-"
#define ASC_VERT     "|"

/* Output mode — auto-detected or forced. */
enum rs_surrender_output {
    RS_SURRENDER_AUTO   = 0,      /* auto-detect from kinfo */
    RS_SURRENDER_CP437  = 1,      /* force CP437 box drawing */
    RS_SURRENDER_ASCII  = 2,      /* force ASCII fallback */
};

/* struct rs_attempt_entry is defined in strategy.h (included above). */

/* ========================================================================= *
 * Function prototypes
 * ========================================================================= */

/* Set surrender output mode.
 * Normally auto-detected; call this to force a specific mode. */
void surrender_set_output(enum rs_surrender_output mode);

/* Log an attempt entry.
 * Called from execute_recovery_plan() after each attempt. */
void surrender_log_attempt(struct rproc *rp, enum recovery_strategy strategy,
    int result, const char *desc);

/* Render the full surrender box to the console.
 * Called when all recovery strategies are exhausted.
 * Sets rp->r_flags |= RS_EXITING via the plan. */
void surrender_render(struct rproc *rp, enum fail_reason reason,
    const struct rs_diag_packet *dp);

/* Render a compact surrender notification (used when RS itself crashes
 * and cannot render the full box). */
void surrender_notify(const char *service_label, enum fail_reason reason,
    int signal);

#endif /* RS_SURRENDER_H */
