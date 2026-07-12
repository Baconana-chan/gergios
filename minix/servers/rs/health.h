/* Healthcheck framework for the Reincarnation Server (RS).
 *
 * Level 2: "Health monitoring" — RS not only waits for death notifications,
 * but actively checks the health of services.
 *
 * Types of healthchecks:
 *   HC_PING           — send a notification, check if service responds
 *   HC_HEARTBEAT      — check if heartbeat is timely
 *   HC_RESOURCES      — query VM for memory/descriptor usage
 *   HC_RESPONSE_TIME  — measure IPC response time
 *   HC_CUSTOM         — user-defined check function
 */
#ifndef RS_HEALTH_H
#define RS_HEALTH_H

#include <minix/endpoint.h>
#include <minix/type.h>

/* Maximum number of healthchecks per service. */
#define RS_HC_MAX_PER_SERVICE   8

/* Maximum name length for a healthcheck. */
#define RS_HC_NAME_LEN          32

/* Healthcheck types. */
enum healthcheck_type {
    HC_PING = 1,            /* service responds to IPC notification? */
    HC_HEARTBEAT,           /* is heartbeat timely? */
    HC_RESOURCES,           /* any memory/fd leak? */
    HC_RESPONSE_TIME,       /* IPC response time within bounds? */
    HC_CUSTOM,              /* user-registered check function */
};

/* Result of a healthcheck. */
enum healthcheck_result {
    HC_OK = 0,              /* service is healthy */
    HC_FAIL_TIMEOUT,        /* service didn't respond in time */
    HC_FAIL_CRASHED,        /* service appears dead */
    HC_FAIL_RESOURCES,      /* resource exhaustion detected */
    HC_FAIL_SLOW,           /* response time exceeded threshold */
    HC_FAIL_CUSTOM,         /* custom check failed */
};

/* A registered healthcheck. */
struct rs_healthcheck {
    int hc_type;                    /* enum healthcheck_type */
    endpoint_t hc_endpoint;         /* which service we're checking */
    clock_t hc_interval;            /* how often to check (ticks) */
    clock_t hc_timeout;             /* max response time (ticks) */
    clock_t hc_last_check;          /* timestamp of last check */
    clock_t hc_last_response;       /* timestamp of last response */
    int hc_consecutive_failures;    /* how many times in a row */
    char hc_name[RS_HC_NAME_LEN];  /* human-readable name */
};

/* Forward declaration. */
struct rproc;

/* IPC structure for RS_REGISTER_HEALTHCHECK / RS_UNREGISTER_HEALTHCHECK.
 * Passed via safecopy using m_rs_req.addr / m_rs_req.len.
 */
struct rs_hc_req {
    endpoint_t rsh_ep;              /* target service endpoint */
    int rsh_type;                   /* healthcheck type (enum healthcheck_type) */
    clock_t rsh_interval;           /* check interval (ticks) */
    clock_t rsh_timeout;            /* response timeout (ticks) */
    char rsh_name[RS_HC_NAME_LEN]; /* healthcheck name */
};

/*===========================================================================*
 *                    Function prototypes                                    *
 *===========================================================================*/

/* Register/unregister a healthcheck for a service. */
int do_register_healthcheck(message *m_ptr);
int do_unregister_healthcheck(message *m_ptr);

/* Run all healthchecks for a service (called from do_period). */
int check_service_health(struct rproc *rp, clock_t now);

/* Handle a failed healthcheck — log and trigger recovery. */
void handle_healthcheck_failure(struct rproc *rp,
    struct rs_healthcheck *hc, enum healthcheck_result result);

/* Count active healthchecks for a service. */
int healthcheck_count(struct rproc *rp);

#endif /* RS_HEALTH_H */
