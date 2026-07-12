/* Surrender framework for the Reincarnation Server (RS).
 *
 * Level 6 Phase 1: "Anis — Console surrender"
 *
 * Anis produces a beautiful, human-readable diagnostic report in the
 * console when all recovery strategies have been exhausted. The report
 * uses CP437 box drawing characters for VGA console compatibility and
 * ANSI escape codes for color on serial/QEMU terminals.
 *
 * Design:
 *   - surrender_log_attempt() is called from execute_recovery_plan()
 *     after each strategy attempt to build the attempt history.
 *   - surrender_render() is called when all strategies are exhausted.
 *     It prints a formatted box with service info, cause, attempt
 *     history, system state, and a personal message from Anis.
 *   - Output mode is auto-detected: CP437 for VGA text mode, ASCII
 *     for serial consoles or when ANSI is not available.
 */
#include "inc.h"
#include "surrender.h"
#include <string.h>
#include <stdio.h>

/* ========================================================================= *
 * Static state
 * ========================================================================= */

/* Surrender output mode. */
static int surrender_mode = RS_SURRENDER_AUTO;

/* ========================================================================= *
 * Helpers
 * ========================================================================= */

/*
 * Format ticks into a human-readable time string (e.g., "2h 34m").
 * Returns pointer to a static buffer.
 */
static const char *format_time(clock_t ticks)
{
    static char buf[24];
    unsigned long total_secs;
    unsigned long hours, mins, secs;

    if (ticks <= 0) {
        snprintf(buf, sizeof(buf), "0s");
        return buf;
    }

    total_secs = (unsigned long)(ticks / system_hz);
    hours = total_secs / 3600;
    mins = (total_secs % 3600) / 60;
    secs = total_secs % 60;

    if (hours > 0) {
        snprintf(buf, sizeof(buf), "%luh %lum %lus", hours, mins, secs);
    } else if (mins > 0) {
        snprintf(buf, sizeof(buf), "%lum %lus", mins, secs);
    } else {
        snprintf(buf, sizeof(buf), "%lus", secs);
    }

    return buf;
}

/*
 * Format the attempt result as a colored string.
 * Returns pointer to a static buffer with ANSI codes.
 */
static const char *format_result(int result)
{
    static char buf[24];

    if (result == OK) {
        snprintf(buf, sizeof(buf), "%sOK%s", ANSI_GREEN, ANSI_RESET);
    } else if (result == EAGAIN) {
        snprintf(buf, sizeof(buf), "%sSKIP%s", ANSI_YELLOW, ANSI_RESET);
    } else {
        snprintf(buf, sizeof(buf), "%sFAIL%s", ANSI_RED, ANSI_RESET);
    }

    return buf;
}

/*
 * Format the attempt result as a plain string (no ANSI).
 */
static const char *format_result_plain(int result)
{
    static char buf[12];

    if (result == OK) {
        snprintf(buf, sizeof(buf), "OK");
    } else if (result == EAGAIN) {
        snprintf(buf, sizeof(buf), "SKIP");
    } else {
        snprintf(buf, sizeof(buf), "FAIL");
    }

    return buf;
}

/*
 * Get the strategy name as a short human-readable string.
 */
static const char *strategy_name(enum recovery_strategy s)
{
    switch (s) {
    case STRAT_RESTART:           return "Restart";
    case STRAT_RESTART_DEPS:      return "Restart with deps";
    case STRAT_RESTART_CLEAN:     return "Restart clean";
    case STRAT_RESTART_ISOLATE:   return "Restart isolated";
    case STRAT_RESTART_MINIMAL:   return "Restart minimal";
    case STRAT_FREE_MEMORY:       return "Free memory";
    case STRAT_CLEAR_CACHE:       return "Clear cache";
    case STRAT_SCHED_BOOST:       return "Boost priority";
    case STRAT_USER_ALERT:        return "User alert";
    case STRAT_SURRENDER:         return "Surrender";
    default:                      return "Unknown";
    }
}

/*
 * Auto-detect output mode.
 * Returns RS_SURRENDER_CP437 on VGA text mode, RS_SURRENDER_ASCII otherwise.
 *
 * Detection heuristic:
 *   - Default to CP437 (VGA text mode), which works on the primary
 *     MINIX console. Call surrender_set_output(RS_SURRENDER_ASCII)
 *     explicitly for serial-only environments.
 *   - The kinfo.do_serial_debug flag is NOT directly accessible from
 *     user-space RS; use explicit mode setting for serial consoles.
 */
static int detect_output_mode(void)
{
    return RS_SURRENDER_CP437;
}

/*
 * Return the box-drawing character for a given role.
 * Uses CP437 if mode is CP437, ASCII otherwise.
 */
static const char *box_char(int mode, const char *cp437, const char *ascii)
{
    return (mode == RS_SURRENDER_CP437) ? cp437 : ascii;
}

/*
 * Print a horizontal line for the box border.
 * len: total width of the box including corners.
 * mode: output mode (CP437 or ASCII).
 * is_top: 1 for top line, 0 for bottom line.
 */
static void print_horizontal(int mode, int is_top)
{
    int i;

    if (is_top) {
        printf("%s", box_char(mode, BOX_TL, ASC_TL));
    } else {
        printf("%s", box_char(mode, BOX_BL, ASC_BL));
    }

    for (i = 0; i < RS_SURRENDER_BOX_WIDTH - 2; i++) {
        printf("%s", box_char(mode, BOX_HORIZ, ASC_HORIZ));
    }

    if (is_top) {
        printf("%s\n", box_char(mode, BOX_TR, ASC_TR));
    } else {
        printf("%s\n", box_char(mode, BOX_BR, ASC_BR));
    }
}

/*
 * Print a box line with content.
 * content: the text to print inside the box (NULL for empty line).
 * mode: output mode.
 */
static void print_box_line(int mode, const char *content)
{
    int content_len, padding, i;

    printf("%s ", box_char(mode, BOX_VERT, ASC_VERT));

    if (content == NULL || content[0] == '\0') {
        /* Empty line. */
        for (i = 0; i < RS_SURRENDER_BOX_WIDTH - 4; i++) {
            printf(" ");
        }
    } else {
        /* Content line. Calculate padding. */
        content_len = (int)strlen(content);
        if (content_len > RS_SURRENDER_BOX_WIDTH - 4) {
            content_len = RS_SURRENDER_BOX_WIDTH - 4;
        }
        printf("%s", content);

        /* Right padding. */
        padding = RS_SURRENDER_BOX_WIDTH - 4 - content_len;
        for (i = 0; i < padding; i++) {
            printf(" ");
        }
    }

    printf(" %s\n", box_char(mode, BOX_VERT, ASC_VERT));
}

/*
 * Print a section header line inside the box.
 */
static void print_box_section(int mode, const char *title)
{
    char buf[RS_SURRENDER_BOX_WIDTH];
    int title_len, dashes_left, dashes_right, i, pos;

    title_len = (int)strlen(title);
    dashes_left = (RS_SURRENDER_BOX_WIDTH - 4 - title_len) / 2 - 1;
    dashes_right = RS_SURRENDER_BOX_WIDTH - 4 - title_len - dashes_left - 2;

    pos = 0;
    buf[pos++] = ' ';
    buf[pos++] = ' ';
    for (i = 0; i < dashes_left; i++) buf[pos++] = '-';
    buf[pos++] = ' ';
    buf[pos++] = ' ';
    memcpy(buf + pos, title, (size_t)title_len);
    pos += title_len;
    buf[pos++] = ' ';
    buf[pos++] = ' ';
    for (i = 0; i < dashes_right; i++) buf[pos++] = '-';
    buf[pos] = '\0';

    print_box_line(mode, buf);
}

/* ========================================================================= *
 * surrender_set_output
 * ========================================================================= */
void surrender_set_output(enum rs_surrender_output mode)
{
    surrender_mode = (int)mode;
}

/* ========================================================================= *
 * surrender_log_attempt
 * ========================================================================= */
void surrender_log_attempt(struct rproc *rp, enum recovery_strategy strategy,
    int result, const char *desc)
{
    struct rs_recovery_data *rd;
    struct rs_attempt_entry *entry;

    if (!rp)
        return;

    rd = &rp->r_recovery;

    /* Bounds check. */
    if (rd->rrd_attempt_count >= RS_MAX_ATTEMPT_LOG)
        return;

    /* Fill in the entry. */
    entry = &rd->rrd_attempt_log[rd->rrd_attempt_count];
    entry->strategy = strategy;
    entry->result = result;

    if (desc != NULL && desc[0] != '\0') {
        strlcpy(entry->desc, desc, RS_SURRENDER_DESC_LEN);
    } else {
        strlcpy(entry->desc, strategy_name(strategy), RS_SURRENDER_DESC_LEN);
    }

    rd->rrd_attempt_count++;
}

/* ========================================================================= *
 * surrender_render
 * ========================================================================= */
void surrender_render(struct rproc *rp, enum fail_reason reason,
    const struct rs_diag_packet *dp)
{
    int mode;
    int i;
    struct rs_recovery_data *rd;
    struct rprocpub *rpub;
    char line_buf[RS_SURRENDER_BOX_WIDTH];
    int attempt_count, crash_cycles;
    const char *time_str;
    int use_ansi;

    if (!rp)
        return;

    rpub = rp->r_pub;
    rd = &rp->r_recovery;

    /* Determine output mode. */
    if (surrender_mode == RS_SURRENDER_AUTO) {
        mode = detect_output_mode();
    } else {
        mode = surrender_mode;
    }

    /* ANSI is supported on serial consoles regardless of CP437 mode. */
    use_ansi = 1;

    /* Calculate attempt statistics. */
    attempt_count = rd->rrd_attempt_count;
    crash_cycles = rd->rrd_attempts;  /* total calls to execute_recovery_plan */

    /* =================================================================== *
     * Render the surrender box
     * =================================================================== */

    /* Print a blank line before the box for visual separation. */
    printf("\n");

    /* Top border. */
    if (use_ansi) printf("%s", ANSI_RED);
    print_horizontal(mode, 1);

    /* Title line. */
    snprintf(line_buf, sizeof(line_buf),
        "  %s-- Anis: \"%s\" --%s",
        use_ansi ? ANSI_BOLD ANSI_CYAN : "",
        "I've tried everything I could...",
        use_ansi ? ANSI_RESET ANSI_RED : "");
    print_box_line(mode, line_buf);

    /* Empty separator. */
    print_box_line(mode, NULL);

    /* Section: Service info. */
    if (use_ansi) printf("%s", ANSI_CYAN);
    print_box_section(mode, "Service");
    if (use_ansi) printf("%s", ANSI_RED);

    /* Service label and endpoint. */
    snprintf(line_buf, sizeof(line_buf),
        "  Service:   %s (%s, ep=%d)",
        rpub->label, rpub->proc_name, rpub->endpoint);
    print_box_line(mode, line_buf);

    /* PID (may not be available for kernel tasks). */
    if (rp->r_pid >= 0) {
        snprintf(line_buf, sizeof(line_buf),
            "  PID:       %d", rp->r_pid);
        print_box_line(mode, line_buf);
    }

    /* Uptime before crash. */
    if (dp != NULL && dp->d_svc_res.dsr_service_uptime > 0) {
        time_str = format_time(dp->d_svc_res.dsr_service_uptime);
        snprintf(line_buf, sizeof(line_buf),
            "  Uptime:    %s", time_str);
        print_box_line(mode, line_buf);
    }

    /* Section: Cause. */
    print_box_line(mode, NULL);
    if (use_ansi) printf("%s", ANSI_YELLOW);
    print_box_section(mode, "Cause");
    if (use_ansi) printf("%s", ANSI_RED);

    /* Signal info. */
    if (dp != NULL && dp->d_signal > 0) {
        snprintf(line_buf, sizeof(line_buf),
            "  Signal:    %s (%d)",
            signal_num_to_string(dp->d_signal), dp->d_signal);
        print_box_line(mode, line_buf);
    }

    /* Fail reason. */
    snprintf(line_buf, sizeof(line_buf),
        "  Reason:    %s",
        fail_reason_to_string(reason));
    print_box_line(mode, line_buf);

    /* Recommendation. */
    if (dp != NULL && dp->d_recommendation[0] != '\0') {
        snprintf(line_buf, sizeof(line_buf),
            "  Suggest:   %.60s",
            dp->d_recommendation);
        print_box_line(mode, line_buf);
    }

    /* Section: Attempt history. */
    print_box_line(mode, NULL);
    if (use_ansi) printf("%s", ANSI_YELLOW);
    print_box_section(mode, "Recovery Attempts");
    if (use_ansi) printf("%s", ANSI_RED);

    snprintf(line_buf, sizeof(line_buf),
        "  Attempts:  %d (over %d crash cycle%s)",
        attempt_count, crash_cycles,
        crash_cycles == 1 ? "" : "s");
    print_box_line(mode, line_buf);

    /* List each attempt. */
    for (i = 0; i < attempt_count && i < RS_MAX_ATTEMPT_LOG; i++) {
        struct rs_attempt_entry *entry = &rd->rrd_attempt_log[i];
        const char *result_str;
        int name_len;

        if (use_ansi) {
            result_str = format_result(entry->result);
        } else {
            result_str = format_result_plain(entry->result);
        }

        /* Format: "    N. Strategy name             → RESULT" */
        snprintf(line_buf, sizeof(line_buf),
            "    %d. %s",
            i + 1, entry->desc);

        /* Pad to align the → arrow. */
        name_len = (int)strlen(line_buf);
        if (name_len < 45) {
            memset(line_buf + name_len, ' ', (size_t)(45 - name_len));
            line_buf[45] = '\0';
        }

        /* Append result with arrow. */
        snprintf(line_buf + strlen(line_buf),
            sizeof(line_buf) - strlen(line_buf),
            " -> %s", result_str);

        /* Strip ANSI codes for indentation calculation.
         * We handle this by printing the raw line instead of using
         * print_box_line(), to account for ANSI escape sequences
         * in the visible length calculation. */
        {
            int pad_len;
            int visible_len;

            printf("%s ", box_char(mode, BOX_VERT, ASC_VERT));

            /* Calculate padding for right side. Visible length is
             * roughly the string length minus ANSI codes. */
            {
                int j, visible = 0;
                for (j = 0; line_buf[j] != '\0'; j++) {
                    if (line_buf[j] == '\x1B') {
                        /* Skip ANSI escape sequence. */
                        while (line_buf[j] != '\0' && line_buf[j] != 'm')
                            j++;
                    } else {
                        visible++;
                    }
                }
                visible_len = visible;
            }

            printf("%s", line_buf);

            pad_len = RS_SURRENDER_BOX_WIDTH - 4 - visible_len;
            if (pad_len > 0) {
                int p;
                for (p = 0; p < pad_len; p++)
                    printf(" ");
            }

            printf(" %s\n", box_char(mode, BOX_VERT, ASC_VERT));
        }
    }

    /* Section: System state (if available). */
    if (dp != NULL && (dp->d_sys_res.dsr_free_mem > 0 ||
        dp->d_sys_res.dsr_total_procs > 0)) {

        print_box_line(mode, NULL);
        if (use_ansi) printf("%s", ANSI_CYAN);
        print_box_section(mode, "System State");
        if (use_ansi) printf("%s", ANSI_RED);

        if (dp->d_sys_res.dsr_free_mem > 0) {
            double free_mb = (double)dp->d_sys_res.dsr_free_mem / (1024.0 * 1024.0);
            snprintf(line_buf, sizeof(line_buf),
                "    Memory:   %.1f MB free", free_mb);
            print_box_line(mode, line_buf);
        }

        if (dp->d_sys_res.dsr_total_procs > 0) {
            snprintf(line_buf, sizeof(line_buf),
                "    Processes: %d total", dp->d_sys_res.dsr_total_procs);
            print_box_line(mode, line_buf);
        }
    }

    /* Section: Log path. */
    print_box_line(mode, NULL);
    if (use_ansi) printf("%s", ANSI_GREEN);
    print_box_section(mode, "Diagnostics Saved");
    if (use_ansi) printf("%s", ANSI_RED);

    snprintf(line_buf, sizeof(line_buf),
        "    Log: /var/log/rs/crash/%s.*.log",
        rpub->label);
    print_box_line(mode, line_buf);

    /* Personal message from Anis. */
    print_box_line(mode, NULL);
    if (use_ansi) printf("%s", ANSI_CYAN);
    {
        const char *msg_lines[] = {
            "  \"I've tried everything I could, but",
            "   %s keeps crashing. Please check",
            "   the diagnostic log for details.\"",
        };
        int mi;
        char msg_buf[RS_SURRENDER_BOX_WIDTH];

        for (mi = 0; mi < 3; mi++) {
            if (mi == 1) {
                snprintf(msg_buf, sizeof(msg_buf), msg_lines[mi],
                    rpub->label);
            } else {
                snprintf(msg_buf, sizeof(msg_buf), "%s", msg_lines[mi]);
            }
            print_box_line(mode, msg_buf);
        }
    }

    /* Empty line. */
    print_box_line(mode, NULL);

    /* Footer: Anis will wait. */
    if (use_ansi) printf("%s", ANSI_BOLD ANSI_YELLOW);
    {
        snprintf(line_buf, sizeof(line_buf),
            "  -- Anis will wait for your instructions. --");
        print_box_line(mode, line_buf);
    }
    if (use_ansi) printf("%s", ANSI_RED);

    /* Bottom border. */
    print_horizontal(mode, 0);

    /* Reset ANSI. */
    if (use_ansi) printf("%s", ANSI_RESET);

    /* Blank line after the box. */
    printf("\n");
}

/* ========================================================================= *
 * surrender_notify
 * ========================================================================= */
void surrender_notify(const char *service_label, enum fail_reason reason,
    int signal)
{
    printf("\n");
    printf("+--- Anis ---+\n");
    printf("| Service %s failed: %s (signal %d)\n",
        service_label ? service_label : "?",
        fail_reason_to_string(reason), signal);
    printf("| All recovery strategies exhausted.\n");
    printf("+------------+\n");
    printf("\n");
}
