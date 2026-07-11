/*
 * display.c — Formatted table display with color coding, headers, and summary.
 */

#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <minix/com.h>
#include "irqtop.h"

/* Simulated IRQ names for common vectors */
static const char *irq_name_static(int irq)
{
	static const char *names[16] = {
		"timer",	"keyboard",	"cascade",	"serial2",
		"serial1",	"sound",	"floppy",	"parallel",
		"rtc",		"acpi",		"nic1",		"nic2",
		"mouse",	"math",		"primary",	"ata"
	};
	static char buf[32];
	if (irq >= 0 && irq < 16 && names[irq])
		return names[irq];
	if (irq >= 48 && irq < 64) {
		snprintf(buf, sizeof(buf), "msix-%d", irq - 48);
		return buf;
	}
	snprintf(buf, sizeof(buf), "irq%d", irq);
	return buf;
}

const char *irq_name(int irq)
{
	return irq_name_static(irq);
}

/* Convert TSC ticks to nanoseconds if -u is set */
static uint64_t fmt_latency(uint64_t tsc_ticks)
{
	if (g_cfg.use_nsec && g_cfg.cpu_mhz > 0) {
		/* ns = tsc_ticks * 1000 / cpu_mhz */
		return (tsc_ticks * 1000UL) / (uint64_t)g_cfg.cpu_mhz;
	}
	return tsc_ticks;
}

static const char *latency_unit(void)
{
	if (g_cfg.use_nsec && g_cfg.cpu_mhz > 0)
		return "ns";
	return "tsc";
}

/* ANSI color codes for latency severity */
#define COLOR_RESET	"\033[0m"
#define COLOR_BOLD	"\033[1m"
#define COLOR_DIM	"\033[2m"
#define COLOR_RED	"\033[31m"
#define COLOR_GREEN	"\033[32m"
#define COLOR_YELLOW	"\033[33m"
#define COLOR_CYAN	"\033[36m"
#define COLOR_REVERSE	"\033[7m"
#define COLOR_BG_RED	"\033[41m\033[37m"

/* Choose color for latency value based on severity thresholds */
static const char *latency_color(uint64_t raw_tsc)
{
	uint64_t threshold = 1000; /* default TSC threshold */

	if (g_cfg.use_nsec && g_cfg.cpu_mhz > 0) {
		/* ~500 ns at 2 GHz = 1000 TSC ticks */
		threshold = (uint64_t)g_cfg.cpu_mhz / 2;
		if (threshold < 100) threshold = 100;
	}

	if (raw_tsc < threshold)
		return COLOR_GREEN;
	if (raw_tsc < threshold * 10)
		return COLOR_YELLOW;
	return COLOR_RED;
}

/* Clear screen and move cursor home */
void display_clear(void)
{
	write(STDOUT_FILENO, "\033[H\033[J", 6);
}

/* Print table header */
void display_header(void)
{
	const char *lat_col = (g_cfg.use_nsec && g_cfg.cpu_mhz > 0)
		? "LAST_NS" : "LAST_LAT";
	const char *avg_col = (g_cfg.use_nsec && g_cfg.cpu_mhz > 0)
		? "AVG_NS" : "AVG_LAT";
	const char *max_col = (g_cfg.use_nsec && g_cfg.cpu_mhz > 0)
		? "MAX_NS" : "MAX_LAT";

	printf(COLOR_BOLD);
	printf("%-4s %-8s %5s %5s %12s %12s %12s %12s %12s %-6s",
	       "IRQ", "NAME", "PRIO", "CPU",
	       "HANDLED", "RUN",
	       lat_col, avg_col, max_col, "REG");
	if (g_cfg.history_enabled) {
		printf(" %9s %9s %9s", "P50", "P95", "P99");
	}
	printf("\n");
	printf(COLOR_RESET);
}

/* Format a uint64_t with comma separators for readability */
static void fmt_comma(char *buf, size_t bufsz, uint64_t val)
{
	char tmp[32];
	int len, i, j = 0;

	snprintf(tmp, sizeof(tmp), "%llu",
		 (unsigned long long)val);
	len = (int)strlen(tmp);

	/* Add commas every 3 digits from the right */
	for (i = 0; i < len; i++) {
		if (i > 0 && (len - i) % 3 == 0)
			buf[j++] = ',';
		buf[j++] = tmp[i];
	}
	buf[j] = '\0';
}

/* Print a single table row */
void display_row(const struct irqtop_stats *s, int idx)
{
	uint64_t avg_lat = 0;
	uint64_t f_last, f_avg, f_max;
	const char *reg_str = s->registered ? "yes" : "no";
	const char *lc;

	if (s->handled_count > 0)
		avg_lat = s->total_latency / s->handled_count;

	f_last = fmt_latency(s->last_latency);
	f_avg = fmt_latency(avg_lat);
	f_max = fmt_latency(s->max_latency);

	/* Color: use max latency for the row color */
	lc = latency_color(s->max_latency);

	printf("%s%-4d ", lc, s->irq);
	printf("%-8s ", irq_name(s->irq));
	printf("%5d ", s->rt_prio);
	printf("%5d ", 0); /* CPU field (kernel doesn't export per-CPU data) */

	{
		char hbuf[16], rbuf[16], lbuf[16], abuf[16], mbuf[16];
		fmt_comma(hbuf, sizeof(hbuf), s->handled_count);
		fmt_comma(rbuf, sizeof(rbuf), s->run_count);
		fmt_comma(lbuf, sizeof(lbuf), f_last);
		fmt_comma(abuf, sizeof(abuf), f_avg);
		fmt_comma(mbuf, sizeof(mbuf), f_max);
		printf("%12s %12s %12s %12s %12s %-6s",
		       hbuf, rbuf, lbuf, abuf, mbuf, reg_str);
	}

	printf(COLOR_RESET);

	/* History columns */
	if (g_cfg.history_enabled && s->registered &&
	    idx >= 0 && idx < IRQTOP_IRQ_THREADS) {
		struct irqtop_history *h = &g_history[idx];
		if (h->count > 0) {
			uint64_t p50, p95, p99;
			p50 = fmt_latency(h->last_p50);
			p95 = fmt_latency(h->last_p95);
			p99 = fmt_latency(h->last_p99);
			printf(" %s%9llu %9llu %9llu%s",
			       latency_color(h->last_p99),
			       (unsigned long long)p50,
			       (unsigned long long)p95,
			       (unsigned long long)p99,
			       COLOR_RESET);
		} else {
			printf(" %9s %9s %9s", "-", "-", "-");
		}
	}

	printf("\n");
}

/* Print summary footer with aggregate statistics */
void display_summary(const struct irqtop_stats *stats, int n)
{
	int total_reg = 0, total_handled = 0, total_run = 0;
	int i;
	uint64_t max_lat = 0, sum_lat = 0;
	uint64_t now_max_lat = 0;

	for (i = 0; i < n; i++) {
		if (stats[i].registered) {
			total_reg++;
			total_handled += (int)stats[i].handled_count;
			total_run += (int)stats[i].run_count;
			if (stats[i].max_latency > max_lat)
				max_lat = stats[i].max_latency;
			if (stats[i].last_latency > now_max_lat)
				now_max_lat = stats[i].last_latency;
			sum_lat += stats[i].total_latency;
		}
	}

	printf(COLOR_DIM);
	printf("\nRegistered: %d  Handled: %d  Runs: %d  "
	       "Worst: %s%llu%s  Now worst: %s%llu%s",
	       total_reg, total_handled, total_run,
	       latency_color(max_lat),
	       (unsigned long long)fmt_latency(max_lat),
	       COLOR_DIM,
	       latency_color(now_max_lat),
	       (unsigned long long)fmt_latency(now_max_lat),
	       COLOR_DIM);

	if (g_cfg.cpu_mhz > 0) {
		printf("  CPU: %d MHz  Unit: %s", g_cfg.cpu_mhz,
		       latency_unit());
	}

	printf("\n");
	printf(COLOR_RESET);
}
