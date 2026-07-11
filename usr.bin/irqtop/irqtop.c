/*
 * irqtop(1) — display per-IRQ thread statistics in real time.
 *
 * Reads IRQ thread stats from the kernel via sys_getinfo(GET_IRQTHREAD_STATS)
 * and shows a live-updating table of IRQ vectors with:
 *   - IRQ number and name
 *   - SCHED_FIFO priority
 *   - Handled count and run count
 *   - Last, average, and max handler latency (TSC ticks or nanoseconds)
 *   - Registration status
 *   - Optional rolling latency history (P50, P95, P99)
 *
 * Usage:
 *   irqtop                    # 1-second refresh, default view
 *   irqtop -d 0.5             # 500 ms refresh
 *   irqtop -n                 # one-shot (non-interactive)
 *   irqtop -s lat             # sort by max latency
 *   irqtop -r                 # show only registered IRQs
 *   irqtop -i 4               # show only IRQ 4 (serial1)
 *   irqtop -t 10000           # only IRQs with max_lat > 10000
 *   irqtop -u                 # show latency in nanoseconds
 *   irqtop -o json            # JSON output
 *   irqtop -o csv             # CSV output
 *   irqtop -l /tmp/irq.log    # log snapshots to file
 *   irqtop -p -m 5000         # peak detection: log when >5000 ticks
 *   irqtop -m 10000           # highlight IRQs with max latency > 10K
 *   irqtop -H                 # enable rolling history (P50/P95/P99)
 *   irqtop -C 2400            # override CPU MHz (for -u conversion)
 *   irqtop -h                 # help
 *
 * Latency is measured in TSC (Time-Stamp Counter) ticks by default.
 * Use -u to convert to nanoseconds (requires CPU MHz detection or -C).
 */

#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <err.h>
#include <errno.h>
#include <minix/com.h>
#include <minix/syslib.h>
#include <minix/endpoint.h>
#include "irqtop.h"

/* Global configuration instance */
struct irqtop_config g_cfg;

/* History instances (one per IRQ) */
struct irqtop_history g_history[IRQTOP_IRQ_THREADS];

static void usage(void)
{
	fprintf(stderr,
		"Usage: %s [options]\n"
		"\n"
		"Display options:\n"
		"  -d delay     Refresh delay in seconds (default: 1.0)\n"
		"  -n           One-shot mode (print once, no clear)\n"
		"  -m warn_max  Highlight IRQs with max_lat > warn_max\n"
		"  -u           Show latency in nanoseconds (vs TSC ticks)\n"
		"  -C mhz       CPU frequency in MHz (for -u conversion)\n"
		"  -H           Enable rolling history (P50/P95/P99 columns)\n"
		"\n"
		"Sorting:\n"
		"  -s col       Sort by: irq (default), lat, hits, name, prio\n"
		"\n"
		"Filtering:\n"
		"  -r           Show only registered IRQs\n"
		"  -i N         Show only IRQ N (e.g. -i 4)\n"
		"  -t N         Only IRQs with max_lat > N ticks\n"
		"\n"
		"Output:\n"
		"  -o fmt       Output format: table (default), json, csv\n"
		"\n"
		"Logging:\n"
		"  -l file      Log snapshots to file (CSV format)\n"
		"  -p           Peak detection: log only when max_lat > -m\n"
		"\n"
		"  -h           Display this help and exit\n",
		getprogname());
}

int main(int argc, char *argv[])
{
	int ch;
	int auto_detect_cpu = 1;

	/* Initialize configuration defaults */
	memset(&g_cfg, 0, sizeof(g_cfg));
	g_cfg.delay = 1.0;
	g_cfg.one_shot = 0;
	g_cfg.warn_max = 0;
	g_cfg.use_nsec = 0;
	g_cfg.cpu_mhz = 0;
	g_cfg.sort = SORT_IRQ;
	g_cfg.registered_only = 0;
	g_cfg.specific_irq = -1;
	g_cfg.lat_threshold = 0;
	g_cfg.output = OUTPUT_TABLE;
	g_cfg.logging = LOG_NONE;
	g_cfg.log_path = NULL;
	g_cfg.history_enabled = 0;

	/* Initialize history */
	memset(g_history, 0, sizeof(g_history));

	/* Parse options */
	while ((ch = getopt(argc, argv, "d:hm:nC:s:ri:t:o:l:puH")) != -1) {
		switch (ch) {
		case 'd':
			g_cfg.delay = strtod(optarg, NULL);
			if (g_cfg.delay <= 0)
				g_cfg.delay = 1.0;
			break;
		case 'h':
			usage();
			return 0;
		case 'm':
			g_cfg.warn_max = strtoull(optarg, NULL, 10);
			break;
		case 'n':
			g_cfg.one_shot = 1;
			break;
		case 'C':
			g_cfg.cpu_mhz = (int)strtol(optarg, NULL, 10);
			auto_detect_cpu = 0;
			if (g_cfg.cpu_mhz <= 0)
				g_cfg.cpu_mhz = 2000;
			break;
		case 'u':
			g_cfg.use_nsec = 1;
			break;
		case 's':
			if (strcmp(optarg, "irq") == 0)
				g_cfg.sort = SORT_IRQ;
			else if (strcmp(optarg, "lat") == 0)
				g_cfg.sort = SORT_LATENCY;
			else if (strcmp(optarg, "hits") == 0)
				g_cfg.sort = SORT_HITS;
			else if (strcmp(optarg, "name") == 0)
				g_cfg.sort = SORT_NAME;
			else if (strcmp(optarg, "prio") == 0)
				g_cfg.sort = SORT_PRIO;
			else
				errx(1, "Unknown sort column: %s "
				     "(use: irq, lat, hits, name, prio)",
				     optarg);
			break;
		case 'r':
			g_cfg.registered_only = 1;
			break;
		case 'i':
			g_cfg.specific_irq = (int)strtol(optarg, NULL, 10);
			if (g_cfg.specific_irq < 0)
				g_cfg.specific_irq = 0;
			if (g_cfg.specific_irq >= IRQTOP_IRQ_THREADS)
				errx(1, "IRQ number out of range "
				     "(0-%d)", IRQTOP_IRQ_THREADS - 1);
			break;
		case 't':
			g_cfg.lat_threshold = strtoull(optarg, NULL, 10);
			break;
		case 'o':
			if (strcmp(optarg, "json") == 0)
				g_cfg.output = OUTPUT_JSON;
			else if (strcmp(optarg, "csv") == 0)
				g_cfg.output = OUTPUT_CSV;
			else if (strcmp(optarg, "table") == 0)
				g_cfg.output = OUTPUT_TABLE;
			else
				errx(1, "Unknown output format: %s "
				     "(use: table, json, csv)", optarg);
			break;
		case 'l':
			g_cfg.log_path = optarg;
			g_cfg.logging = LOG_SNAPSHOT;
			break;
		case 'p':
			g_cfg.logging = LOG_PEAK;
			if (!g_cfg.log_path)
				g_cfg.log_path = "/var/log/irqtop.peaks";
			break;
		case 'H':
			g_cfg.history_enabled = 1;
			break;
		default:
			usage();
			return 1;
		}
	}

	argc -= optind;
	argv += optind;

	/* Auto-detect CPU MHz if needed */
	if (auto_detect_cpu && (g_cfg.use_nsec || g_cfg.logging != LOG_NONE)) {
		g_cfg.cpu_mhz = stats_detect_cpu_mhz();
		if (g_cfg.cpu_mhz == 0)
			g_cfg.cpu_mhz = 2000; /* reasonable default */
	}

	/* Open log file if requested */
	if (g_cfg.log_path && g_cfg.logging != LOG_NONE) {
		if (logging_open(g_cfg.log_path) != 0)
			warnx("Cannot open log file: %s", g_cfg.log_path);
	}

	/* Handle one-shot mode first */
	if (g_cfg.one_shot) {
		struct irqtop_stats stats[IRQTOP_IRQ_THREADS];

		if (stats_fetch(stats, IRQTOP_IRQ_THREADS) != 0)
			errx(1, "sys_getinfo(GET_IRQTHREAD_STATS) failed");

		stats_sort(stats, IRQTOP_IRQ_THREADS);

		switch (g_cfg.output) {
		case OUTPUT_JSON:
			output_json(stats, IRQTOP_IRQ_THREADS);
			break;
		case OUTPUT_CSV:
			output_csv_header();
			{
				int i;
				for (i = 0; i < IRQTOP_IRQ_THREADS; i++) {
					if (stats_filter(&stats[i]))
						output_csv_row(&stats[i], i);
				}
			}
			break;
		case OUTPUT_TABLE:
		default:
			display_header();
			{
				int i;
				for (i = 0; i < IRQTOP_IRQ_THREADS; i++) {
					if (stats_filter(&stats[i]))
						display_row(&stats[i], i);
				}
			}
			display_summary(stats, IRQTOP_IRQ_THREADS);
			break;
		}

		logging_close();
		return 0;
	}

	/* Interactive mode: loop until interrupted */
	for (;;) {
		struct irqtop_stats stats[IRQTOP_IRQ_THREADS];
		time_t now;
		char timebuf[64];
		int i;

		if (stats_fetch(stats, IRQTOP_IRQ_THREADS) != 0) {
			warnx("sys_getinfo(GET_IRQTHREAD_STATS) failed");
			sleep(1);
			continue;
		}

		/* Sort stats according to config */
		stats_sort(stats, IRQTOP_IRQ_THREADS);

		/* Update rolling history if enabled */
		if (g_cfg.history_enabled)
			stats_update_history(stats, IRQTOP_IRQ_THREADS);

		/* Clear screen and show title */
		display_clear();
		time(&now);
		strftime(timebuf, sizeof(timebuf),
			 "%Y-%m-%d %H:%M:%S", localtime(&now));
		printf("irqtop - %s  (delay %.1fs)",
		       timebuf, g_cfg.delay);
		if (g_cfg.logging != LOG_NONE && g_cfg.log_path)
			printf("  [logging: %s]", g_cfg.log_path);
		printf("\n");

		/* Render table */
		display_header();
		for (i = 0; i < IRQTOP_IRQ_THREADS; i++) {
			if (stats_filter(&stats[i]))
				display_row(&stats[i], i);
		}
		display_summary(stats, IRQTOP_IRQ_THREADS);

		/* Log snapshot or peak detection */
		if (g_cfg.logging == LOG_SNAPSHOT && g_cfg.log_path) {
			logging_snapshot(stats, IRQTOP_IRQ_THREADS, now);
		} else if (g_cfg.logging == LOG_PEAK && g_cfg.log_path) {
			logging_peak(stats, IRQTOP_IRQ_THREADS, now);
		}

		if (g_cfg.delay <= 0)
			break;

		sleep(g_cfg.delay);
	}

	logging_close();
	return 0;
}
