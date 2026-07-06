/* drvmanager_pool.h — Parallel Module Init Worker Pool
 *
 * Provides a fixed-size pthread worker pool for calling init_module()
 * on loaded .ko/.so modules in parallel on SMP systems.
 *
 * Usage:
 *   // One-time init at server startup
 *   drmgr_pool_init(0);  // 0 = auto-detect CPU count
 *
 *   // After ELF loading + dep resolution (sequential):
 *   // module is in LOADING state
 *   drmgr_pool_submit("e1000e");    // init_module in background thread
 *   drmgr_pool_submit("ahci");      // parallel init
 *
 *   // Wait for all to complete before continuing
 *   drmgr_pool_sync();
 *
 *   // Cleanup at shutdown
 *   drmgr_pool_destroy();
 *
 * Thread safety:
 *   - Module registry accessed by worker threads ONLY via
 *     drvmanager_init_module() — main-thread-only fields are not touched
 *   - Job queue protected by mutex + condition variable
 *   - Worker threads are reentrant-safe for init_module()
 */

#ifndef _GERGIOS_DRVMANAGER_POOL_H
#define _GERGIOS_DRVMANAGER_POOL_H

#include <stddef.h>

/*===========================================================================*
 *		Constants                                                *
 *===========================================================================*/

/* Maximum number of pending async init jobs */
#define DRMGR_POOL_MAX_JOBS		64

/* Default number of worker threads (when auto-detect fails) */
#define DRMGR_POOL_DEFAULT_THREADS	4

/*===========================================================================*
 *		Public API                                               *
 *===========================================================================*/

/* Initialise the worker pool.
 *
 * Creates @num_threads worker threads that wait for init_module jobs.
 * If @num_threads is 0, auto-detects the number of online CPUs
 * (via sysconf(_SC_NPROCESSORS_ONLN)) and uses that many threads,
 * clamped to [1, 16].
 *
 * Safe to call multiple times (subsequent calls are no-ops if
 * the pool is already running).
 *
 * @param num_threads  Number of worker threads, or 0 for auto-detect
 * @return 0 on success, negative errno on failure */
int drmgr_pool_init(int num_threads);

/* Submit a module for async init_module() execution.
 *
 * The module MUST already be loaded (ELF .ko parsed, .so dlopen'd,
 * dependencies resolved) and in the LOADING state.
 *
 * The worker thread will call drvmanager_init_module() which
 * transitions the module to LOADED or FAILED.
 *
 * @param module_name  Name of the module to initialise
 * @return 0 on success (job queued), negative errno on failure:
 *   -EAGAIN  job queue full, try again later
 *   -EINVAL  NULL or empty name
 *   -ENOENT  module not found in registry */
int drmgr_pool_submit(const char *module_name);

/* Wait for all pending async init jobs to complete.
 *
 * Blocks the calling thread until every submitted job has been
 * processed by a worker thread.  Does NOT drain or cancel jobs.
 *
 * Safe to call multiple times (idempotent when no jobs pending). */
void drmgr_pool_sync(void);

/* Wait for a specific module's init to complete.
 *
 * If the module is currently being initialised by a worker thread,
 * this blocks until it finishes.  If the module was never submitted
 * or is already done, returns immediately.
 *
 * @param module_name  Name of the module to wait for
 * @return 0 on success, -ENOENT if module not found */
int drmgr_pool_wait(const char *module_name);

/* Get the number of pending (incomplete) jobs. */
int drmgr_pool_pending(void);

/* Get the number of worker threads in the pool. */
int drmgr_pool_thread_count(void);

/* Destroy the worker pool.
 *
 * Waits for all pending jobs to complete, signals all workers to
 * exit, joins threads, and frees resources.  After this call, the
 * pool can be re-initialised with drmgr_pool_init().
 *
 * Safe to call when pool is not initialised (no-op). */
void drmgr_pool_destroy(void);

#endif /* _GERGIOS_DRVMANAGER_POOL_H */
