/*
 * logging.c — File logging and peak detection for irqtop(1).
 *
 * Supports:
 *   - Full snapshot logging (-l file): append a formatted line per poll
 *   - Peak detection (-p): only log when max_latency > warn_max threshold
 */

#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>
#include "irqtop.h"

static FILE *log_file = NULL;
static const char *log_path = NULL;

/* Open log file for appending.
 * Returns 0 on success, -1 on error. */
int logging_open(const char *path)
{
	if (!path || !path[0])
		return -1;

	log_file = fopen(path, "a");
	if (!log_file)
		return -1;

	log_path = path;
	setbuf(log_file, NULL); /* unbuffered for peak detection */
	return 0;
}

/* Close the log file */
void logging_close(void)
{
	if (log_file) {
		fclose(log_file);
		log_file = NULL;
	}
	log_path = NULL;
}

/* Write a full snapshot of all IRQ stats to the log file.
 * Format: CSV-like with header on first write (detected by file size 0). */
void logging_snapshot(const struct irqtop_stats *stats, int n,
		      time_t now)
{
	int i;
	char timebuf[32];
	struct tm *tm;

	if (!log_file)
		return;

	tm = localtime(&now);
	if (!tm)
		return;
	strftime(timebuf, sizeof(timebuf), "%Y-%m-%d %H:%M:%S", tm);

	/* Check if file is empty — write header */
	{
		long pos = ftell(log_file);
		if (pos == 0) {
			fprintf(log_file,
				"# irqtop snapshot log\n"
				"# Columns: timestamp,irq,name,prio,"
				"registered,handled,run,"
				"last_lat,avg_lat,max_lat,"
				"total_lat_raw\n");
		}
	}

	for (i = 0; i < n; i++) {
		uint64_t avg_lat = 0;

		if (!stats_filter(&stats[i]))
			continue;

		if (stats[i].handled_count > 0)
			avg_lat = stats[i].total_latency /
				  stats[i].handled_count;

		fprintf(log_file, "%s,%d,%s,%d,%d,%llu,%llu,"
			"%llu,%llu,%llu,%llu\n",
			timebuf,
			stats[i].irq,
			irq_name(stats[i].irq),
			stats[i].rt_prio,
			stats[i].registered,
			(unsigned long long)stats[i].handled_count,
			(unsigned long long)stats[i].run_count,
			(unsigned long long)stats[i].last_latency,
			(unsigned long long)avg_lat,
			(unsigned long long)stats[i].max_latency,
			(unsigned long long)stats[i].total_latency);
	}
}

/* Peak detection: only log lines where max_latency exceeds warn_max.
 * Format is the same as snapshot but only includes rows that breach. */
void logging_peak(const struct irqtop_stats *stats, int n,
		  time_t now)
{
	int i;
	int wrote_header = 0;
	char timebuf[32];
	struct tm *tm;

	if (!log_file)
		return;

	tm = localtime(&now);
	if (!tm)
		return;
	strftime(timebuf, sizeof(timebuf), "%Y-%m-%d %H:%M:%S", tm);

	for (i = 0; i < n; i++) {
		uint64_t avg_lat = 0;

		if (!stats_filter(&stats[i]))
			continue;

		/* Only log when max_latency exceeds warning threshold */
		if (g_cfg.warn_max > 0 &&
		    stats[i].max_latency <= g_cfg.warn_max)
			continue;

		if (stats[i].handled_count > 0)
			avg_lat = stats[i].total_latency /
				  stats[i].handled_count;

		if (!wrote_header) {
			fprintf(log_file,
				"# PEAK %s — IRQs exceeding "
				"max_lat > %llu\n",
				timebuf,
				(unsigned long long)g_cfg.warn_max);
			wrote_header = 1;
		}

		fprintf(log_file, "PEAK %s,%d,%s,%d,%d,%llu,%llu,"
			"%llu,%llu,%llu,%llu\n",
			timebuf,
			stats[i].irq,
			irq_name(stats[i].irq),
			stats[i].rt_prio,
			stats[i].registered,
			(unsigned long long)stats[i].handled_count,
			(unsigned long long)stats[i].run_count,
			(unsigned long long)stats[i].last_latency,
			(unsigned long long)avg_lat,
			(unsigned long long)stats[i].max_latency,
			(unsigned long long)stats[i].total_latency);
	}
}
