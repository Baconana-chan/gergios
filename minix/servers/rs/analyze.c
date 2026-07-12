/* Failure analysis for the Reincarnation Server (RS).
 *
 * Level 4: "Diagnostics & Analysis" — classifies the failure cause
 * based on collected diagnostic data and provides human-readable
 * recommendations.
 */
#include "inc.h"
#include "diag.h"
#include <minix/endpoint.h>
#include <string.h>
#include <stdio.h>

/* ========================================================================= *
 * Signal name lookup table.
 * ========================================================================= */
static const struct {
    int   signo;
    const char *name;
} signal_names[] = {
    { SIGABRT,   "SIGABRT"   },
    { SIGALRM,   "SIGALRM"   },
    { SIGBUS,    "SIGBUS"    },
    { SIGCHLD,   "SIGCHLD"   },
    { SIGCONT,   "SIGCONT"   },
    { SIGFPE,    "SIGFPE"    },
    { SIGHUP,    "SIGHUP"    },
    { SIGILL,    "SIGILL"    },
    { SIGINT,    "SIGINT"    },
    { SIGKILL,   "SIGKILL"   },
    { SIGPIPE,   "SIGPIPE"   },
    { SIGQUIT,   "SIGQUIT"   },
    { SIGSEGV,   "SIGSEGV"   },
    { SIGSTOP,   "SIGSTOP"   },
    { SIGTERM,   "SIGTERM"   },
    { SIGTSTP,   "SIGTSTP"   },
    { SIGTTIN,   "SIGTTIN"   },
    { SIGTTOU,   "SIGTTOU"   },
    { SIGUSR1,   "SIGUSR1"   },
    { SIGUSR2,   "SIGUSR2"   },
    { SIGSYS,    "SIGSYS"    },
    { SIGTRAP,   "SIGTRAP"   },
    { SIGURG,    "SIGURG"    },
    { SIGXCPU,   "SIGXCPU"   },
    { SIGXFSZ,   "SIGXFSZ"   },
    { SIGVTALRM, "SIGVTALRM" },
    { SIGPROF,   "SIGPROF"   },
    { SIGIO,     "SIGIO"     },
    { SIGWINCH,  "SIGWINCH"  },
    { 0,         NULL        },  /* terminator */
};

/* ========================================================================= *
 * Fail reason name lookup table.
 * ========================================================================= */
static const struct {
    enum fail_reason reason;
    const char      *name;
} fail_reason_names[] = {
    { FAIL_UNKNOWN,               "unknown"                    },
    { FAIL_SEGFAULT,              "segfault"                   },
    { FAIL_NOMEM,                 "out of memory"              },
    { FAIL_TIMEOUT,               "timeout"                    },
    { FAIL_DEADLOCK,              "deadlock"                   },
    { FAIL_HW_ERROR,              "hardware error"             },
    { FAIL_DEP_DIED,              "dependency died"            },
    { FAIL_RESOURCE_EXHAUSTION,   "resource exhaustion"        },
    { FAIL_SOFTWARE_BUG,          "software bug"               },
    { FAIL_KILLED,                "killed by RS"               },
    { FAIL_INIT_FAILURE,          "initialization failure"     },
    { 0,                          NULL                         },  /* terminator */
};

/* ========================================================================= *
 * signal_num_to_string
 * ========================================================================= */
const char *signal_num_to_string(int signo)
{
    int i;

    for (i = 0; signal_names[i].name != NULL; i++) {
        if (signal_names[i].signo == signo)
            return signal_names[i].name;
    }

    return "SIGNAL_UNKNOWN";
}

/* ========================================================================= *
 * fail_reason_to_string
 * ========================================================================= */
const char *fail_reason_to_string(enum fail_reason reason)
{
    int i;

    for (i = 0; fail_reason_names[i].name != NULL; i++) {
        if (fail_reason_names[i].reason == reason)
            return fail_reason_names[i].name;
    }

    return "UNKNOWN";
}

/* ========================================================================= *
 * analyze_failure
 * ========================================================================= */
enum fail_reason analyze_failure(const struct rs_diag_packet *dp)
{
/* Analyze the failure cause based on the diagnostic packet data.
 *
 * Decision logic:
 *   SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGTRAP → FAIL_SEGFAULT
 *   SIGKILL (from healthcheck) → check uptime + memory → NOMEM or TIMEOUT
 *   SIGTERM → FAIL_KILLED
 *   Normal exit with non-zero code → check memory + restart frequency
 *   Signal 0 (proactive) → check dependencies
 *   RS_HEALTHCHECK_FAIL flag → already in the diagnostic
 *   RS_INITIALIZING flag → FAIL_INIT_FAILURE
 */
    if (!dp)
        return FAIL_UNKNOWN;

    /* Check for initialization failure: if the service died during init
     * (short uptime, RS_INITIALIZING was set at crash time), it's an
     * init failure. We can infer this from uptime and restarts. */
    if (dp->d_svc_res.dsr_service_uptime < RS_DIAG_MIN_UPTIME &&
        dp->d_svc_res.dsr_restarts == 0 &&
        dp->d_signal == 0) {
        return FAIL_INIT_FAILURE;
    }

    /* Crashes with memory-corruption signals → software bug. */
    switch (dp->d_signal) {
    case SIGSEGV:
    case SIGBUS:
    case SIGILL:
    case SIGFPE:
    case SIGTRAP:
        return FAIL_SEGFAULT;

    case SIGABRT:
        /* SIGABRT indicates assert() or abort() — also a software bug,
         * but a different class (intentional self-destruct vs crash). */
        return FAIL_SOFTWARE_BUG;

    case SIGPIPE:
        /* SIGPIPE: wrote to a broken pipe (e.g., peer died). */
        return FAIL_SOFTWARE_BUG;

    case SIGKILL:
        /* Killed by RS or externally. Check if the system was low on
         * memory at crash time — the OOM killer may have been invoked. */
        if (dp->d_sys_res.dsr_free_mem > 0 &&
            dp->d_sys_res.dsr_free_mem < RS_DIAG_LOW_MEM_THRESHOLD) {
            return FAIL_NOMEM;
        }
        /* If uptime was very short, it might be an init failure. */
        if (dp->d_svc_res.dsr_service_uptime < RS_DIAG_MIN_UPTIME) {
            return FAIL_INIT_FAILURE;
        }
        /* Otherwise, killed by healthcheck timeout. */
        return FAIL_TIMEOUT;

    case SIGTERM:
        return FAIL_KILLED;

    case SIGXCPU:
    case SIGXFSZ:
        return FAIL_RESOURCE_EXHAUSTION;
    }

    /* No signal, non-zero exit status. Check resource pressure. */
    if (dp->d_signal == 0 && dp->d_exit_status != 0) {
        if (dp->d_sys_res.dsr_free_mem > 0 &&
            dp->d_sys_res.dsr_free_mem < RS_DIAG_LOW_MEM_THRESHOLD) {
            return FAIL_NOMEM;
        }
        /* Frequent restarts with short uptime → likely bug or resource issue. */
        if (dp->d_svc_res.dsr_restarts >= RS_DIAG_FREQUENT_THRESHOLD &&
            dp->d_svc_res.dsr_service_uptime < RS_DIAG_MIN_UPTIME) {
            return FAIL_RESOURCE_EXHAUSTION;
        }
        return FAIL_SOFTWARE_BUG;
    }

    /* No signal, zero exit status (proactive restart, no crash data).
     * If we're here during a restart that was triggered by the
     * dependency subsystem, the diagnostic would have FAIL_DEP_DIED. */
    if (dp->d_signal == 0 && dp->d_exit_status == 0) {
        /* Proactive restart — not a crash. */
        return FAIL_UNKNOWN;
    }

    return FAIL_UNKNOWN;
}
