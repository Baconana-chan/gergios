/* Diagnostic framework for the Reincarnation Server (RS).
 *
 * Level 4: "Diagnostics & Analysis" — RS collects full diagnostic data
 * before each restart and analyzes the failure cause.
 *
 * Key components:
 *   - struct rs_diag_packet: inline diagnostic data collected at crash time
 *   - struct rs_diag_log_entry: ring buffer entry for log preservation
 *   - enum fail_reason: classification of the failure cause
 *   - collect_diagnostics(): snapshot service state before restart
 *   - save_diag_report(): preserve diagnostic data in the ring buffer
 *   - analyze_failure(): classify the failure cause based on collected data
 */
#ifndef RS_DIAG_H
#define RS_DIAG_H

#include <minix/endpoint.h>
#include <minix/type.h>

/* ========================================================================= *
 * Constants
 * ========================================================================= */

/* Maximum number of diagnostic log entries in the ring buffer. */
#define RS_DIAG_LOG_SIZE        64

/* Maximum path length for crash dump files. */
#define RS_DIAG_DUMP_PATH_LEN   128

/* Maximum recommendation string length. */
#define RS_DIAG_RECOMMEND_LEN   256

/* Number of consecutive failures threshold for "frequent crash" analysis. */
#define RS_DIAG_FREQUENT_THRESHOLD  5

/* Message field aliases for RS_DIAG_REPORT reply. */
#define DIAG_COUNT  m1_i3  /* number of diagnostic log entries returned */

/* Minimum uptime (in ticks) to distinguish "early crash" from "mature crash". */
#define RS_DIAG_MIN_UPTIME      (system_hz * 5)  /* 5 seconds */

/* Low memory threshold in bytes (below this → potential OOM). */
#define RS_DIAG_LOW_MEM_THRESHOLD   (4 * 1024 * 1024)  /* 4 MB */

/* Stack trace capture maximum depth.
 * sys_diagctl_stacktrace() captures up to ~10 frames by default. */
#define RS_DIAG_MAX_STACK_FRAMES    16

/* ========================================================================= *
 * Failure cause classification
 * ========================================================================= */
enum fail_reason {
    FAIL_UNKNOWN = 0,               /* cannot determine cause */
    FAIL_SEGFAULT,                  /* SIGSEGV — likely software bug */
    FAIL_NOMEM,                     /* out of memory at crash time */
    FAIL_TIMEOUT,                   /* heartbeat/healthcheck timeout */
    FAIL_DEADLOCK,                  /* no heartbeat + peer services stuck */
    FAIL_HW_ERROR,                  /* hardware error (SIGBUS, etc.) */
    FAIL_DEP_DIED,                  /* critical dependency died */
    FAIL_RESOURCE_EXHAUSTION,       /* file descriptors, IPC queues, etc. */
    FAIL_SOFTWARE_BUG,              /* non-segfault software error (assert, abort) */
    FAIL_KILLED,                    /* explicitly killed by RS/ admin */
    FAIL_INIT_FAILURE,              /* failed during initialization */
};

/* ========================================================================= *
 * Diagnostic packet — collected at crash time for each service
 * ========================================================================= */

/* System resource snapshot at crash time. */
struct rs_diag_system_resources {
    uint64_t dsr_free_mem;          /* free memory in bytes (from VM) */
    int      dsr_total_procs;       /* total processes in the system */
    clock_t  dsr_uptime;            /* system uptime at crash (ticks) */
};

/* Resource usage of the failing service. */
struct rs_diag_service_resources {
    uint64_t dsr_mem_usage;         /* service memory usage in bytes */
    clock_t  dsr_service_uptime;    /* how long the service ran (ticks) */
    int      dsr_restarts;          /* total restarts so far */
    int      dsr_signal;            /* termination signal (0 if normal exit) */
    int      dsr_exit_status;       /* exit status (if normal exit) */
};

/* The main diagnostic packet. */
struct rs_diag_packet {
    /* Identifiers. */
    endpoint_t d_ep;                /* which service crashed */
    char       d_label[RS_MAX_LABEL_LEN];  /* service label */

    /* Crash details. */
    clock_t    d_crash_time;        /* when the crash was detected (ticks) */
    int        d_signal;            /* termination signal (SIGSEGV, etc.) */
    int        d_exit_status;       /* exit code (if normal exit) */

    /* Resource data. */
    struct rs_diag_service_resources  d_svc_res;
    struct rs_diag_system_resources   d_sys_res;

    /* Analysis. */
    enum fail_reason d_reason;      /* classified failure cause */
    char       d_recommendation[RS_DIAG_RECOMMEND_LEN];  /* human advice */
};

/* ========================================================================= *
 * Diagnostic log entry — one per crash/restart event, kept in a ring buffer
 * ========================================================================= */
struct rs_diag_log_entry {
    clock_t    dle_timestamp;       /* when the event occurred */
    endpoint_t dle_endpoint;        /* which service */
    int        dle_signal;          /* termination signal */
    enum fail_reason dle_reason;    /* classified cause */
    int        dle_restarts;        /* restarts after this crash */
    int        dle_used;            /* 1 = valid entry */
};

/* ========================================================================= *
 * Forward declarations
 * ========================================================================= */
struct rproc;

/* ========================================================================= *
 * Function prototypes
 * ========================================================================= */

/* Collect diagnostics for a service at crash/healthcheck-failure time.
 * Fills in the diagnostic packet from rproc data, VM queries, etc.
 * Returns OK on success. */
int collect_diagnostics(struct rproc *rp, struct rs_diag_packet *dp);

/* Save a diagnostic report into the ring buffer for later retrieval.
 * Also prints a summary to the console. */
void save_diag_report(const struct rs_diag_packet *dp);

/* Save a diagnostic report to /var/log/rs/crash/<label>.<timestamp>.log.
 * Uses sys_datacopy and direct VFS write (best-effort, may fail). */
void save_diag_report_to_disk(const struct rs_diag_packet *dp);

/* Analyze the failure cause based on the diagnostic packet data.
 * Returns the classified fail_reason. */
enum fail_reason analyze_failure(const struct rs_diag_packet *dp);

/* Convert a fail_reason to a human-readable string. */
const char *fail_reason_to_string(enum fail_reason reason);

/* Convert a signal number to a human-readable name. */
const char *signal_num_to_string(int signo);

/* Retrieve the diagnostic log ring buffer — returns pointer to buffer
 * and sets *count to the number of valid entries. */
const struct rs_diag_log_entry *get_diag_log(int *count);

/* Clear the diagnostic log ring buffer. */
void clear_diag_log(void);

/* Initialize the diagnostic subsystem (called at boot). */
void diag_init(void);

#endif /* RS_DIAG_H */
