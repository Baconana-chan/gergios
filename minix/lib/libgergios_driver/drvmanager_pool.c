/* drvmanager_pool.c — Parallel Module Init Worker Pool Implementation
 *
 * Implements a fixed-size pthread worker pool that calls
 * drvmanager_init_module() for loaded .ko/.so modules in parallel.
 *
 * Architecture:
 *   - Job queue: circular buffer of module names (DRMGR_POOL_MAX_JOBS)
 *   - Worker threads: N threads, each waits on condvar for jobs
 *   - Submission: main thread writes module name to queue, signals condvar
 *   - Completion: worker writes result, sets done flag, signals completion
 *   - Sync: main thread waits on completion condvar until all done
 *
 * The job queue is a simple array with head/tail pointers. Each slot
 * stores the module name and a completion flag.  Workers process jobs
 * in FIFO order.
 *
 * Thread safety (verified by design):
 *   - g_pool.mutex protects head, tail, jobs[], done[] state
 *   - g_pool.cond signals workers when new jobs arrive
 *   - g_pool.done_cond signals sync waiter when any job completes
 *   - Each job slot indexed once (circular), no ABA problem since
 *     a slot is only reused after head passes it
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <pthread.h>
#include <unistd.h>

#include "drvmanager_pool.h"
#include "drvmanager.h"

/*===========================================================================*
 *		Pool state                                               *
 *===========================================================================*/

/* A single job in the queue */
struct pool_job {
	char name[DRVMANAGER_NAME_MAX];
	volatile int  done;	/* 1 = worker finished processing */
	int           result;	/* return value from init_module */
};

/* Global pool state */
static struct {
	/* Threads */
	pthread_t *threads;
	int        num_threads;
	volatile int running;	/* 1 = pool is active */

	/* Job queue (circular buffer) */
	struct pool_job jobs[DRMGR_POOL_MAX_JOBS];
	int head;		/* next slot to fill (producer index) */
	int tail;		/* next slot to consume (consumer index) */

	/* Completion tracking (used by sync() — avoids the race where
	 * tail advances on dequeue but completion is later) */
	int submitted_count;	/* total jobs submitted since last idle */
	int completed_count;	/* total jobs completed */

	/* Synchronisation primitives */
	pthread_mutex_t mutex;
	pthread_cond_t  job_cond;	/* signals workers: new job available */
	pthread_cond_t  done_cond;	/* signals sync waiter: job completed */
} g_pool;

/*===========================================================================*
 *		Worker thread function                                   *
 *===========================================================================*/

static void *worker_thread(void *arg)
{
	(void)arg;

	printf("pool: worker thread started (tid=%p)\n",
	    (void *)pthread_self());

	while (g_pool.running) {
		int idx;
		char job_name[DRVMANAGER_NAME_MAX];

		pthread_mutex_lock(&g_pool.mutex);

		/* Wait for a job while pool is running and queue is empty */
		while (g_pool.running && g_pool.head == g_pool.tail) {
			pthread_cond_wait(&g_pool.job_cond, &g_pool.mutex);
		}

		if (!g_pool.running) {
			pthread_mutex_unlock(&g_pool.mutex);
			break;
		}

		/* Dequeue the next job */
		idx = g_pool.tail % DRMGR_POOL_MAX_JOBS;
		g_pool.tail++;

		/* Copy name while holding the lock */
		strncpy(job_name, g_pool.jobs[idx].name,
		    sizeof(job_name) - 1);
		job_name[sizeof(job_name) - 1] = '\0';

		pthread_mutex_unlock(&g_pool.mutex);

		/* Process the job (call init_module) — NO LOCK HELD */
		printf("pool: worker: initialising '%s'...\n", job_name);
		int result = drvmanager_init_module(job_name);
		printf("pool: worker: init_module '%s' returned %d\n",
		    job_name, result);

		/* Store result and signal completion */
		pthread_mutex_lock(&g_pool.mutex);
		g_pool.jobs[idx].result = result;
		g_pool.jobs[idx].done   = 1;
		g_pool.completed_count++;
		/* Wake up sync() waiter */
		pthread_cond_signal(&g_pool.done_cond);
		pthread_mutex_unlock(&g_pool.mutex);
	}

	printf("pool: worker thread exiting (tid=%p)\n",
	    (void *)pthread_self());
	return NULL;
}

/*===========================================================================*
 *		Public API                                               *
 *===========================================================================*/

int drmgr_pool_init(int num_threads)
{
	int ret;

	/* Already running — no-op */
	if (g_pool.running)
		return 0;

	/* Auto-detect number of CPUs */
	if (num_threads <= 0) {
		long ncpu = sysconf(_SC_NPROCESSORS_ONLN);
		num_threads = (ncpu > 0) ? (int)ncpu : DRMGR_POOL_DEFAULT_THREADS;
		/* Clamp to reasonable range */
		if (num_threads < 1) num_threads = 1;
		if (num_threads > 16) num_threads = 16;
	}

	printf("pool: initialising with %d worker thread(s)\n", num_threads);

	/* Initialise synchronisation primitives */
	pthread_mutex_init(&g_pool.mutex, NULL);
	pthread_cond_init(&g_pool.job_cond, NULL);
	pthread_cond_init(&g_pool.done_cond, NULL);

	g_pool.head = 0;
	g_pool.tail = 0;
	g_pool.num_threads = num_threads;
	g_pool.running = 1;

	/* Allocate thread array */
	g_pool.threads = calloc(num_threads, sizeof(pthread_t));
	if (!g_pool.threads) {
		printf("pool: failed to allocate thread array\n");
		g_pool.running = 0;
		return -ENOMEM;
	}

	/* Create worker threads */
	for (int i = 0; i < num_threads; i++) {
		ret = pthread_create(&g_pool.threads[i], NULL,
		    worker_thread, NULL);
		if (ret != 0) {
			printf("pool: pthread_create(%d) failed: %d\n",
			    i, ret);
			/* We have a partial pool — still functional */
			g_pool.num_threads = i;
			break;
		}
	}

	printf("pool: ready with %d thread(s)\n", g_pool.num_threads);
	return 0;
}

int drmgr_pool_submit(const char *module_name)
{
	if (!module_name || !module_name[0])
		return -EINVAL;

	/* Verify module exists in registry */
	if (!drvmanager_find(module_name))
		return -ENOENT;

	pthread_mutex_lock(&g_pool.mutex);

	/* Check for queue full */
	int count = g_pool.head - g_pool.tail;
	if (count >= DRMGR_POOL_MAX_JOBS) {
		pthread_mutex_unlock(&g_pool.mutex);
		printf("pool: job queue full (%d pending)\n",
		    drmgr_pool_pending());
		return -EAGAIN;
	}

	/* Enqueue the job */
	int idx = g_pool.head % DRMGR_POOL_MAX_JOBS;
	strncpy(g_pool.jobs[idx].name, module_name,
	    sizeof(g_pool.jobs[idx].name) - 1);
	g_pool.jobs[idx].name[sizeof(g_pool.jobs[idx].name) - 1] = '\0';
	g_pool.jobs[idx].done = 0;
	g_pool.jobs[idx].result = 0;
	g_pool.head++;
	g_pool.submitted_count++;

	/* Wake one worker */
	pthread_cond_signal(&g_pool.job_cond);

	pthread_mutex_unlock(&g_pool.mutex);

	printf("pool: submitted '%s' for async init (head=%d, tail=%d)\n",
	    module_name, g_pool.head, g_pool.tail);
	return 0;
}

void drmgr_pool_sync(void)
{
	pthread_mutex_lock(&g_pool.mutex);

	/* Snapshot the submission count — workers may advance tail
	 * (on dequeue) before completing, so head!=tail is NOT a
	 * reliable indicator of in-flight work.  Use the counter. */
	int wait_for = g_pool.submitted_count;

	/* Wait until all submitted jobs have been completed.
	 * Workers increment completed_count AFTER setting done=1,
	 * so by the time completed_count >= wait_for, all jobs
	 * are truly finished. */
	while (g_pool.completed_count < wait_for) {
		pthread_cond_wait(&g_pool.done_cond, &g_pool.mutex);
	}

	/* Reset counters for the next batch */
	g_pool.submitted_count = 0;
	g_pool.completed_count = 0;

	pthread_mutex_unlock(&g_pool.mutex);

	printf("pool: sync complete (%d job(s) finished)\n", wait_for);
}

int drmgr_pool_wait(const char *module_name)
{
	if (!module_name) return -EINVAL;

	pthread_mutex_lock(&g_pool.mutex);

	/* Scan the queue for this module */
	for (int i = g_pool.tail; i < g_pool.head; i++) {
		int idx = i % DRMGR_POOL_MAX_JOBS;
		if (strcmp(g_pool.jobs[idx].name, module_name) != 0)
			continue;

		/* Found it — wait if not done */
		while (!g_pool.jobs[idx].done) {
			pthread_cond_wait(&g_pool.done_cond, &g_pool.mutex);
		}

		pthread_mutex_unlock(&g_pool.mutex);
		return g_pool.jobs[idx].result;
	}

	pthread_mutex_unlock(&g_pool.mutex);

	/* Module not found in queue — might already be done or never submitted */
	printf("pool: wait: '%s' not in pending queue\n", module_name);
	return -ENOENT;
}

int drmgr_pool_pending(void)
{
	int count;

	pthread_mutex_lock(&g_pool.mutex);
	count = g_pool.head - g_pool.tail;
	pthread_mutex_unlock(&g_pool.mutex);

	return count > 0 ? count : 0;
}

int drmgr_pool_thread_count(void)
{
	return g_pool.num_threads;
}

void drmgr_pool_destroy(void)
{
	if (!g_pool.running)
		return;

	printf("pool: shutting down...\n");

	/* Wait for all pending jobs to complete */
	drmgr_pool_sync();

	/* Signal all workers to exit */
	pthread_mutex_lock(&g_pool.mutex);
	g_pool.running = 0;
	pthread_cond_broadcast(&g_pool.job_cond);
	pthread_mutex_unlock(&g_pool.mutex);

	/* Join all worker threads */
	for (int i = 0; i < g_pool.num_threads; i++) {
		if (g_pool.threads[i]) {
			pthread_join(g_pool.threads[i], NULL);
		}
	}

	/* Cleanup */
	free(g_pool.threads);
	g_pool.threads = NULL;
	pthread_mutex_destroy(&g_pool.mutex);
	pthread_cond_destroy(&g_pool.job_cond);
	pthread_cond_destroy(&g_pool.done_cond);

	g_pool.num_threads = 0;
	g_pool.head = 0;
	g_pool.tail = 0;

	printf("pool: shut down\n");
}
