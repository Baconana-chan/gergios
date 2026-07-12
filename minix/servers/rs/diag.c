/* Diagnostic collection and reporting for the Reincarnation Server (RS).
 *
 * Level 4: "Diagnostics & Analysis" — collects diagnostic data at crash time,
 * saves reports, and provides the foundation for failure analysis.
 */
#include "inc.h"
#include "diag.h"
#include <string.h>
#include <stdio.h>

/* ========================================================================= *
 * Ring buffer for diagnostic log entries.
 * ========================================================================= */
static struct rs_diag_log_entry diag_log[RS_DIAG_LOG_SIZE];
static int diag_log_head = 0;
static int diag_log_count = 0;

/* ========================================================================= *
 * Helpers
 * ========================================================================= */

/*
 * Try to get VM memory usage for a service endpoint.
 * Returns the memory size in bytes, or 0 on failure.
 *
 * Uses the VM_GETRUSAGE kernel call via sys_datacopy pattern.
 * If VM is not available (e.g., during early boot), returns 0.
 */
static uint64_t query_service_memory(endpoint_t ep)
{
    /* For Phase 4, we use a best-effort query to VM.
     * The standard MINIX interface is through sys_getinfo(GET_PROCTAB)
     * which gives us the kernel process table, but not detailed
     * memory usage per process.
     *
     * A more precise approach would be to send an IPC to VM:
     *   message m;
     *   m.m_type = VM_GETRUSAGE;
     *   m.VM_ENDPT = ep;
     *   ipc_sendrec(VM_PROC_NR, &m);
     *   if (m.m_type == OK) return m.VM_MEM_USAGE;
     *
     * For now, return 0 (unknown). This will be enhanced in
     * Phase 5 with dedicated VM_RS_FREE_MEM calls.
     */
    return 0;
}

/*
 * Try to get the system's free memory from VM.
 * Returns the free memory in bytes, or 0 on failure.
 */
static uint64_t query_free_memory(void)
{
    /* Query VM for free memory.
     * For Phase 4, we can query the kernel's memory info
     * via sys_getinfo (if available) or send an IPC to VM.
     *
     * For now, return 0 (unknown). Enhanced in Phase 5.
     */
    return 0;
}

/*
 * Get the total number of processes from the kernel.
 */
static int query_total_procs(void)
{
    /* Future: read from PM via sys_getproctab(). For now, return 0. */
    return 0;
}

/* ========================================================================= *
 * diag_init
 * ========================================================================= */
void diag_init(void)
{
    /* Initialize the diagnostic log ring buffer. */
    diag_log_head = 0;
    diag_log_count = 0;
    memset(diag_log, 0, sizeof(diag_log));

    if (rs_verbose)
        printf("RS: diagnostic subsystem initialized\n");
}

/* ========================================================================= *
 * collect_diagnostics
 * ========================================================================= */
int collect_diagnostics(struct rproc *rp, struct rs_diag_packet *dp)
{
/* Collect diagnostics for a service at crash/healthcheck-failure time.
 *
 * Fills in the diagnostic packet from:
 *   - rproc fields (flags, timestamps, restarts)
 *   - VM queries (memory usage, free memory)
 *
 * Returns OK on success.
 */
    struct rprocpub *rpub;
    clock_t now;

    if (!rp || !dp)
        return EINVAL;

    rpub = rp->r_pub;
    now = getticks();

    /* Clear the packet first. */
    memset(dp, 0, sizeof(*dp));

    /* Fill in identifiers. */
    dp->d_ep = rpub->endpoint;
    strlcpy(dp->d_label, rpub->label, RS_MAX_LABEL_LEN);

    /* Crash time. */
    dp->d_crash_time = now;

    /* Determine signal and exit status.
     * The signal manager callback sets RS_TERMINATED flag before
     * calling terminate_service. We infer the signal from context:
     *   - RS_TERMINATED + RS_HEALTHCHECK_FAIL = killed by RS
     *   - RS_TERMINATED + RS_EXITING = stopped by RS_DOWN
     *   - RS_TERMINATED alone = natural crash
     *   - No RS_TERMINATED = proactive restart or init failure
     */
    if (rp->r_flags & RS_TERMINATED) {
        if (rp->r_flags & RS_HEALTHCHECK_FAIL) {
            dp->d_signal = SIGKILL;
            dp->d_exit_status = 0;
        } else if (rp->r_flags & RS_EXITING) {
            dp->d_signal = SIGTERM;
            dp->d_exit_status = 0;
        } else {
            /* Natural crash. Future: record the actual signal
             * from the signal manager callback in rproc. */
            dp->d_signal = SIGKILL;
            dp->d_exit_status = 0;
        }
    } else {
        dp->d_signal = 0;
        dp->d_exit_status = 0;
    }

    /* Service resource usage. */
    dp->d_svc_res.dsr_mem_usage = query_service_memory(rpub->endpoint);
    dp->d_svc_res.dsr_service_uptime = now - rp->r_alive_tm;
    dp->d_svc_res.dsr_restarts = rp->r_restarts;
    dp->d_svc_res.dsr_signal = dp->d_signal;
    dp->d_svc_res.dsr_exit_status = dp->d_exit_status;

    /* System resource snapshot. */
    dp->d_sys_res.dsr_free_mem = query_free_memory();
    dp->d_sys_res.dsr_total_procs = query_total_procs();
    dp->d_sys_res.dsr_uptime = now;

    /* Run failure analysis on the collected data. */
    dp->d_reason = analyze_failure(dp);

    /* Generate a human-readable recommendation. */
    switch (dp->d_reason) {
    case FAIL_SEGFAULT:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' crashed with SIGSEGV. This may indicate a "
            "software bug. Check the stack trace (if available) and "
            "consider updating the service binary.",
            dp->d_label);
        break;

    case FAIL_NOMEM:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' appears to have crashed due to low memory. "
            "Free system memory was %llu bytes. Consider increasing "
            "available RAM or reducing the number of running services.",
            dp->d_label,
            (unsigned long long) dp->d_sys_res.dsr_free_mem);
        break;

    case FAIL_TIMEOUT:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' stopped responding. This may indicate a "
            "deadlock or infinite loop. Check if the service is "
            "waiting for a blocked dependency.",
            dp->d_label);
        break;

    case FAIL_DEP_DIED:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' failed because a critical dependency is dead. "
            "Check which services reported failure and restart them "
            "in the correct order.",
            dp->d_label);
        break;

    case FAIL_RESOURCE_EXHAUSTION:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' may have exhausted system resources. "
            "Consider increasing resource limits or checking for "
            "resource leaks in the code.",
            dp->d_label);
        break;

    case FAIL_KILLED:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' was intentionally stopped by RS. "
            "No action required if this was expected.",
            dp->d_label);
        break;

    case FAIL_INIT_FAILURE:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' failed during initialization. "
            "Check the service binary and configuration for errors.",
            dp->d_label);
        break;

    default:
        snprintf(dp->d_recommendation, RS_DIAG_RECOMMEND_LEN,
            "Service '%s' failed for an unknown reason. "
            "Check system logs for more details.",
            dp->d_label);
        break;
    }

    if (rs_verbose)
        printf("RS: diagnostics collected for %s: reason=%d, signal=%d, "
            "uptime=%lu, restarts=%d\n",
            dp->d_label, dp->d_reason, dp->d_signal,
            (unsigned long) dp->d_svc_res.dsr_service_uptime,
            dp->d_svc_res.dsr_restarts);

    return OK;
}

/* ========================================================================= *
 * save_diag_report
 * ========================================================================= */
void save_diag_report(const struct rs_diag_packet *dp)
{
/* Save a diagnostic report into the ring buffer for later retrieval.
 * Also prints a summary to the console.
 */
    struct rs_diag_log_entry *entry;

    if (!dp)
        return;

    /* Print a summary to the console. */
    printf("RS: DIAG %s (ep=%d) signal=%d reason=%s uptime=%lu "
        "restarts=%d\n",
        dp->d_label, dp->d_ep, dp->d_signal,
        fail_reason_to_string(dp->d_reason),
        (unsigned long) dp->d_svc_res.dsr_service_uptime,
        dp->d_svc_res.dsr_restarts);

    if (dp->d_sys_res.dsr_free_mem > 0) {
        printf("RS:   system: free_mem=%llu bytes, total_procs=%d\n",
            (unsigned long long) dp->d_sys_res.dsr_free_mem,
            dp->d_sys_res.dsr_total_procs);
    }

    if (dp->d_svc_res.dsr_mem_usage > 0) {
        printf("RS:   service: mem_usage=%llu bytes\n",
            (unsigned long long) dp->d_svc_res.dsr_mem_usage);
    }

    /* Add to ring buffer. */
    entry = &diag_log[diag_log_head];
    memset(entry, 0, sizeof(*entry));
    entry->dle_timestamp = dp->d_crash_time;
    entry->dle_endpoint = dp->d_ep;
    entry->dle_signal = dp->d_signal;
    entry->dle_reason = dp->d_reason;
    entry->dle_restarts = dp->d_svc_res.dsr_restarts;
    entry->dle_used = 1;

    /* Advance head. */
    diag_log_head = (diag_log_head + 1) % RS_DIAG_LOG_SIZE;
    if (diag_log_count < RS_DIAG_LOG_SIZE)
        diag_log_count++;
}

/* ========================================================================= *
 * save_diag_report_to_disk
 * ========================================================================= */
void save_diag_report_to_disk(const struct rs_diag_packet *dp)
{
/* Best-effort attempt to save the diagnostic report to disk.
 *
 * This writes to /var/log/rs/crash/<label>.<timestamp>.log.
 * Note: if VFS is the crashed service, this will silently fail.
 * Uses direct file I/O.
 */
    char path[RS_DIAG_DUMP_PATH_LEN];
    char buf[2048];
    int fd, n;
    clock_t now;

    if (!dp)
        return;

    now = getticks();
    snprintf(path, sizeof(path), "/var/log/rs/crash/%s.%lu.log",
        dp->d_label, (unsigned long) now);

    fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) {
        if (rs_verbose)
            printf("RS: cannot write diagnostic report to %s: %d\n",
                path, errno);
        return;
    }

    n = snprintf(buf, sizeof(buf),
        "=== RS Diagnostic Report ===\n"
        "Service:      %s\n"
        "Endpoint:     %d\n"
        "Crash time:   %lu (ticks)\n"
        "Signal:       %s (%d)\n"
        "Exit status:  %d\n"
        "Uptime:       %lu (ticks)\n"
        "Restarts:     %d\n"
        "Reason:       %s\n"
        "Mem usage:    %llu bytes\n"
        "Free mem:     %llu bytes\n"
        "Total procs:  %d\n"
        "Recommend:    %s\n"
        "=== End of Report ===\n",
        dp->d_label, dp->d_ep,
        (unsigned long) dp->d_crash_time,
        signal_num_to_string(dp->d_signal), dp->d_signal,
        dp->d_exit_status,
        (unsigned long) dp->d_svc_res.dsr_service_uptime,
        dp->d_svc_res.dsr_restarts,
        fail_reason_to_string(dp->d_reason),
        (unsigned long long) dp->d_svc_res.dsr_mem_usage,
        (unsigned long long) dp->d_sys_res.dsr_free_mem,
        dp->d_sys_res.dsr_total_procs,
        dp->d_recommendation);

    if (n > 0) {
        write(fd, buf, (size_t) n);
    }

    close(fd);

    if (rs_verbose)
        printf("RS: diagnostic report saved to %s\n", path);
}

/* ========================================================================= *
 * get_diag_log
 * ========================================================================= */
const struct rs_diag_log_entry *get_diag_log(int *count)
{
    if (count)
        *count = diag_log_count;
    return diag_log;
}

/* ========================================================================= *
 * do_diag_report
 * ========================================================================= */
int do_diag_report(message *m_ptr)
{
/* Handle RS_DIAG_REPORT request: copy the diagnostic log to user space.
 *
 * The caller provides a buffer via m_rs_req.addr / m_rs_req.len (safecopy).
 * The buffer should be large enough to hold 'count' struct rs_diag_log_entry
 * entries. On success, returns the number of entries copied.
 */
    int count;
    size_t copy_size;

    if (!m_ptr)
        return EINVAL;

    /* Determine how many entries to copy. */
    count = diag_log_count;
    if (count == 0) {
        m_ptr->DIAG_COUNT = 0;
        return OK;
    }

    /* Validate the caller's buffer. */
    if (m_ptr->m_rs_req.len < (int)sizeof(struct rs_diag_log_entry)) {
        return EINVAL;
    }

    copy_size = (size_t)count * sizeof(struct rs_diag_log_entry);
    if (copy_size > (size_t)m_ptr->m_rs_req.len) {
        copy_size = (size_t)m_ptr->m_rs_req.len;
        count = (int)(copy_size / sizeof(struct rs_diag_log_entry));
    }

    /* Copy from the ring buffer into the caller's space. */
    {
        int s = sys_datacopy(SELF, (vir_bytes) diag_log,
            m_ptr->m_source, (vir_bytes) m_ptr->m_rs_req.addr,
            copy_size);
        if (s != OK)
            return s;
    }

    m_ptr->DIAG_COUNT = count;
    return OK;
}

/* ========================================================================= *
 * clear_diag_log
 * ========================================================================= */
void clear_diag_log(void)
{
    diag_log_head = 0;
    diag_log_count = 0;
    memset(diag_log, 0, sizeof(diag_log));

    if (rs_verbose)
        printf("RS: diagnostic log cleared\n");
}
