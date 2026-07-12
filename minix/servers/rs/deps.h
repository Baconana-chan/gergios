/* Dependency graph for the Reincarnation Server (RS).
 *
 * Level 3: "Dependency-aware recovery" — RS knows which services depend
 * on which others, and restores them in the correct order.
 *
 * Key concepts:
 *   - Critical dependency: A cannot work without B (e.g., VFS → block driver)
 *   - Non-critical: A works in degraded mode without B (e.g., VFS → FS driver)
 *   - Cascade restart: if A depends on B and B is dead, restart B first,
 *     wait for its init, then restart A.
 */
#ifndef RS_DEPS_H
#define RS_DEPS_H

#include <minix/endpoint.h>
#include <minix/type.h>

/* Maximum dependencies per service. */
#define RS_DEP_MAX_PER_SERVICE   16

/* Maximum name length for a dependency reason. */
#define RS_DEP_REASON_LEN        64

/* Dependency flags. */
#define RS_DEP_CRITICAL    0x01   /* service cannot work without this dep */
#define RS_DEP_REGISTERED  0x02   /* registered via IPC (not built-in) */

/* A dependency: service A depends on service B. */
struct rs_dep {
    endpoint_t rs_service;          /* who depends (A) */
    endpoint_t rs_depends_on;       /* on whom (B) */
    int rs_flags;                   /* RS_DEP_CRITICAL, RS_DEP_REGISTERED */
    int rs_restart_priority;        /* lower = restart first (0 = deps first) */
    char rs_reason[RS_DEP_REASON_LEN]; /* why ("provides block I/O") */
    clock_t rs_established;         /* when this dep was registered */
};

/* Status of a dependency at crash time. */
struct rs_dep_status {
    endpoint_t ds_service;
    endpoint_t ds_depends_on;
    int ds_is_alive;                /* dependency is running? */
    int ds_is_healthy;              /* dependency passed healthcheck? */
    clock_t ds_last_alive;          /* when dependency was last seen alive */
};

/* IPC structure for RS_REGISTER_DEP / RS_UNREGISTER_DEP.
 * Passed via safecopy using m_rs_req.addr / m_rs_req.len.
 */
struct rs_dep_req {
    endpoint_t rsr_service;         /* who depends (usually caller) */
    endpoint_t rsr_depends_on;      /* on whom */
    int rsr_flags;                  /* RS_DEP_CRITICAL */
    int rsr_restart_priority;       /* restart order hint */
    char rsr_reason[RS_DEP_REASON_LEN]; /* human-readable reason */
};

/* Forward declaration. */
struct rproc;

/*===========================================================================*
 *                    Function prototypes                                    *
 *===========================================================================*/

/* Register/unregister a dependency. */
int do_register_dep(message *m_ptr);
int do_unregister_dep(message *m_ptr);

/* Initialize built-in dependency table (called at boot). */
void deps_init_table(void);

/* Check if a service has any dead critical dependencies.
 * Returns number of dead critical deps (0 = all alive).
 */
int check_dependencies(struct rproc *rp);

/* Cascade restart: restart all dead critical dependencies first,
 * then restart the service itself.
 * Returns OK on success, error code on failure.
 */
int cascade_restart(struct rproc *rp);

/* Count dependencies for a service. */
int dep_count(struct rproc *rp);

/* Find a dependency by target endpoint. */
struct rs_dep *dep_find(struct rproc *rp, endpoint_t target);

#endif /* RS_DEPS_H */
