/*
 * stats.c — IRQ thread statistics collection, history, sorting, filtering.
 *
 * MINIX compatibility: uses plain qsort() instead of qsort_r()
 * (qsort_r is not available in MINIX libc).
 */

#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <minix/com.h>
#include <minix/syslib.h>
#include <minix/type.h>
#include "irqtop.h"

/* Global sort mode — used by cmp_stats callback (MINIX lacks qsort_r) */
static enum sort_mode sort_mode = SORT_IRQ;

/* Comparison function for sorting IRQ stats */
static int cmp_stats(const void *a, const void *b)
{
	const struct irqtop_stats *sa = (const struct irqtop_stats *)a;
	const struct irqtop_stats *sb = (const struct irqtop_stats *)b;

	switch (sort_mode) {
	case SORT_LATENCY:
		/* Descending by max_latency */
		if (sb->max_latency > sa->max_latency) return 1;
		if (sb->max_latency < sa->max_latency) return -1;
		return sa->irq - sb->irq;
	case SORT_HITS:
		/* Descending by handled_count */
		if (sb->handled_count > sa->handled_count) return 1;
		if (sb->handled_count < sa->handled_count) return -1;
		return sa->irq - sb->irq;
	case SORT_NAME:
		/* Alphabetical by name */
		return strcmp(irq_name(sa->irq), irq_name(sb->irq));
	case SORT_PRIO:
		/* Descending by RT priority */
		if (sb->rt_prio > sa->rt_prio) return 1;
		if (sb->rt_prio < sa->rt_prio) return -1;
		return sa->irq - sb->irq;
	case SORT_IRQ:
	default:
		return sa->irq - sb->irq;
	}
}

/* Sort an array of irqtop_stats by the configured sort mode */
void stats_sort(struct irqtop_stats *stats, int n)
{
	if (!stats || n <= 0)
		return;
	sort_mode = g_cfg.sort;
	qsort(stats, (size_t)n, sizeof(struct irqtop_stats), cmp_stats);
}

/* Try to detect CPU frequency in MHz.
 * Returns 0 if detection fails (caller should use default).
 *
 * Detection methods (in order of preference):
 *   1. /sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq (scaled kHz)
 *   2. sysctl hw.cpuspeed
 *   3. /proc/cpuinfo "cpu MHz" line
 */
int stats_detect_cpu_mhz(void)
{
	FILE *fp;
	char buf[256];
	unsigned long val;

	/* Method 1: sysfs cpufreq (modern Linux, some MINIX builds) */
	fp = fopen("/sys/devices/system/cpu/cpu0/cpufreq/"
		   "cpuinfo_max_freq", "r");
	if (fp) {
		if (fgets(buf, sizeof(buf), fp)) {
			val = strtoul(buf, NULL, 10);
			fclose(fp);
			if (val > 0 && val < 10000000) {
				/* File gives kHz, convert to MHz */
				unsigned long mhz = (val + 500) / 1000;
				if (mhz >= 100 && mhz <= 100000)
					return (int)mhz;
			}
		} else {
			fclose(fp);
		}
	}

	/* Method 2: sysctl hw.cpuspeed */
	fp = popen("sysctl -n hw.cpuspeed 2>/dev/null", "r");
	if (fp) {
		if (fgets(buf, sizeof(buf), fp)) {
			val = strtoul(buf, NULL, 10);
			pclose(fp);
			if (val >= 100 && val <= 100000)
				return (int)val;
		} else {
			pclose(fp);
		}
	}

	/* Method 3: /proc/cpuinfo */
	fp = fopen("/proc/cpuinfo", "r");
	if (fp) {
		while (fgets(buf, sizeof(buf), fp)) {
			if (sscanf(buf, "cpu MHz\t: %lu", &val) == 1) {
				fclose(fp);
				if (val >= 100 && val <= 1000000)
					return (int)val;
				return (int)val;
			}
		}
		fclose(fp);
	}

	return 0; /* detection failed */
}

/* Fetch IRQ thread stats from the kernel.
 * Returns 0 on success, -1 on error. */
int stats_fetch(struct irqtop_stats *stats, int n)
{
	if (!stats || n <= 0)
		return -1;

	if (sys_getinfo(GET_IRQTHREAD_STATS, stats,
			(size_t)(n * sizeof(struct irqtop_stats)),
			0, 0) != OK) {
		return -1;
	}

	return 0;
}

/* Check if an IRQ entry passes the current filter criteria.
 * Returns 1 if it should be shown, 0 if filtered out. */
int stats_filter(const struct irqtop_stats *s)
{
	if (!s)
		return 0;

	/* Filter: registered only */
	if (g_cfg.registered_only && !s->registered)
		return 0;

	/* Filter: specific IRQ */
	if (g_cfg.specific_irq >= 0 && s->irq != g_cfg.specific_irq)
		return 0;

	/* Filter: latency threshold */
	if (g_cfg.lat_threshold > 0 && s->max_latency <= g_cfg.lat_threshold)
		return 0;

	return 1;
}

/* Compare uint64_t for qsort in ascending order */
static int cmp_u64_asc(const void *a, const void *b)
{
	uint64_t va = *(const uint64_t *)a;
	uint64_t vb = *(const uint64_t *)b;
	if (va < vb) return -1;
	if (va > vb) return 1;
	return 0;
}

/* Update rolling history for all IRQs from current stats.
 * Computes min/avg/max/p50/p95/p99 for each IRQ. */
void stats_update_history(struct irqtop_stats *stats, int n)
{
	int i;

	if (!g_cfg.history_enabled)
		return;

	for (i = 0; i < n && i < IRQTOP_IRQ_THREADS; i++) {
		struct irqtop_history *h = &g_history[i];
		struct irqtop_history_entry *entry;
		uint64_t samples[IRQTOP_MAX_HISTORY];
		int scount;
		int j;

		/* Only record history for registered IRQs with activity */
		if (!stats[i].registered || stats[i].handled_count == 0)
			continue;

		/* Add entry to circular buffer */
		entry = &h->entries[h->head];
		entry->latency = stats[i].last_latency;
		entry->timestamp = time(NULL);

		h->head = (h->head + 1) % IRQTOP_MAX_HISTORY;
		if (h->count < IRQTOP_MAX_HISTORY)
			h->count++;

		/* Collect samples */
		scount = 0;
		for (j = 0; j < h->count; j++) {
			samples[scount++] = h->entries[j].latency;
		}

		if (scount == 0)
			continue;

		qsort(samples, (size_t)scount, sizeof(uint64_t), cmp_u64_asc);

		/* Min */
		h->last_min = samples[0];

		/* Max */
		h->last_max = samples[scount - 1];

		/* Average */
		{
			uint64_t sum = 0;
			int k;
			for (k = 0; k < scount; k++)
				sum += samples[k];
			h->last_avg = sum / (uint64_t)scount;
		}

		/* Median (p50) */
		h->last_p50 = samples[scount / 2];

		/* p95 */
		{
			int idx95 = (scount * 95 + 99) / 100;
			if (idx95 >= scount) idx95 = scount - 1;
			h->last_p95 = samples[idx95];
		}

		/* p99 */
		{
			int idx99 = (scount * 99 + 99) / 100;
			if (idx99 >= scount) idx99 = scount - 1;
			h->last_p99 = samples[idx99];
		}
	}
}

/* Retrieve the history for a given IRQ index */
struct irqtop_history *stats_get_history(int idx)
{
	if (idx < 0 || idx >= IRQTOP_IRQ_THREADS)
		return NULL;
	return &g_history[idx];
}
