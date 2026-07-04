/*
 * irqtop(1) — display per-IRQ thread statistics in real time.
 *
 * Reads IRQ thread stats from the kernel via sys_getinfo(GET_IRQTHREAD_STATS)
 * and shows a live-updating table of IRQ vectors with:
 *   - IRQ number and name
 *   - SCHED_FIFO priority
 *   - Handled count and run count
 *   - Last, average, and max handler latency (TSC ticks)
 *
 * Usage:
 *   irqtop              # 1-second refresh
 *   irqtop -d 0.5       # 500 ms refresh
 *   irqtop -n           # one-shot (non-interactive)
 *   irqtop -m 10000     # highlight IRQs with max latency > 10K ticks
 *
 * Latency is measured in TSC (Time-Stamp Counter) ticks.
 * To convert to nanoseconds: ns = tsc_ticks / (cpu_mhz)
 * On a typical 2 GHz CPU, 1 TSC tick ≈ 0.5 ns.
 */

#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <err.h>
#include <minix/com.h>
#include <minix/syslib.h>
#include <minix/endpoint.h>

/* Number of IRQ threads — must match kernel/irq_thread.h */
#define IRQTOP_IRQ_THREADS	64

/* Per-IRQ thread statistics structure — matches kernel/irq_thread.h */
struct irqtop_stats {
	int		irq;
	int		rt_prio;
	int		registered;
	endpoint_t	endpoint;
	uint64_t	handled_count;
	uint64_t	run_count;
	uint64_t	last_latency;
	uint64_t	max_latency;
	uint64_t	total_latency;
};

/* Simulated IRQ names for common vectors */
static const char *irq_name(int irq)
{
	static const char *names[16] = {
		"timer",	"keyboard",	"cascade",	"serial2",
		"serial1",	"sound",	"floppy",	"parallel",
		"rtc",		"acpi",		"nic1",		"nic2",
		"mouse",	"math",		"primary",	"ata"
	};
	static char buf[32];
	if (irq < 16 && names[irq])
		return names[irq];
	if (irq >= 48 && irq < 64) {
		snprintf(buf, sizeof(buf), "msix-%d", irq - 48);
		return buf;
	}
	snprintf(buf, sizeof(buf), "irq%d", irq);
	return buf;
}

/* Clear screen and move cursor home */
static void clear_screen(void)
{
	write(STDOUT_FILENO, "\033[H\033[J", 6);
}

static void print_header(void)
{
	printf("\033[1m");  /* bold */
	printf("%-4s %-8s %5s %12s %12s %12s %12s %12s %-6s\n",
	       "IRQ", "NAME", "PRIO",
	       "HANDLED", "RUN",
	       "LAST_LAT", "AVG_LAT", "MAX_LAT", "REG");
	printf("\033[0m");  /* reset */
}

static void print_row(const struct irqtop_stats *s, uint64_t warn_max)
{
	const char *reg_str = s->registered ? "yes" : "no";
	uint64_t avg_lat = 0;
	int highlight = 0;

	if (s->handled_count > 0)
		avg_lat = s->total_latency / s->handled_count;

	/* Highlight IRQs with max latency above threshold */
	if (warn_max > 0 && s->max_latency > warn_max)
		highlight = 1;

	if (highlight)
		printf("\033[7m");  /* reverse video */

	printf("%-4d %-8s %5d %12llu %12llu %12llu %12llu %12llu %-6s\n",
	       s->irq, irq_name(s->irq), s->rt_prio,
	       (unsigned long long)s->handled_count,
	       (unsigned long long)s->run_count,
	       (unsigned long long)s->last_latency,
	       (unsigned long long)avg_lat,
	       (unsigned long long)s->max_latency,
	       reg_str);

	if (highlight)
		printf("\033[0m");  /* reset */
}

static void print_summary(const struct irqtop_stats *stats, int n,
			   double delay)
{
	int total_reg = 0, total_handled = 0, total_run = 0;
	int i;
	uint64_t max_lat = 0, sum_lat = 0;

	for (i = 0; i < n; i++) {
		if (stats[i].registered) {
			total_reg++;
			total_handled += stats[i].handled_count;
			total_run += stats[i].run_count;
			if (stats[i].max_latency > max_lat)
				max_lat = stats[i].max_latency;
			sum_lat += stats[i].total_latency;
		}
	}

	printf("\033[2m");  /* dim */
	printf("\nRegistered: %d  Total handled: %d  Total runs: %d  "
	       "Worst latency: %llu  Sum latency: %llu  "
	       "Delay: %.1fs\n",
	       total_reg, total_handled, total_run,
	       (unsigned long long)max_lat,
	       (unsigned long long)sum_lat,
	       delay);
	printf("\033[0m");
}

int main(int argc, char *argv[])
{
	struct irqtop_stats stats[IRQTOP_IRQ_THREADS];
	double delay = 1.0;
	int one_shot = 0;
	int ch;
	uint64_t warn_max = 0;	/* highlight threshold */

	while ((ch = getopt(argc, argv, "d:hm:n")) != -1) {
		switch (ch) {
		case 'd':
			delay = strtod(optarg, NULL);
			if (delay <= 0)
				delay = 1.0;
			break;
		case 'h':
			fprintf(stderr, "Usage: %s [-d delay] [-m warn_max] "
				"[-n]\n", getprogname());
			fprintf(stderr, "  -d delay     Refresh delay "
				"(seconds, default 1.0)\n");
			fprintf(stderr, "  -m warn_max  Highlight IRQs with "
				"max_lat > warn_max\n");
			fprintf(stderr, "  -n           One-shot mode "
				"(no clearing/reprinting)\n");
			return 0;
		case 'm':
			warn_max = strtoull(optarg, NULL, 10);
			break;
		case 'n':
			one_shot = 1;
			break;
		default:
			return 1;
		}
	}

	argc -= optind;
	argv += optind;

	if (one_shot) {
		/* One-shot: print once and exit */
		if (sys_getinfo(GET_IRQTHREAD_STATS, stats,
				sizeof(stats), 0, 0) != OK) {
			errx(1, "sys_getinfo(GET_IRQTHREAD_STATS) failed");
		}
		print_header();
		{
			int i;
			for (i = 0; i < IRQTOP_IRQ_THREADS; i++)
				print_row(&stats[i], warn_max);
		}
		print_summary(stats, IRQTOP_IRQ_THREADS, delay);
		return 0;
	}

	/* Interactive loop: clear screen and reprint */
	for (;;) {
		time_t now;
		char timebuf[64];
		int i;

		if (sys_getinfo(GET_IRQTHREAD_STATS, stats,
				sizeof(stats), 0, 0) != OK) {
			errx(1, "sys_getinfo(GET_IRQTHREAD_STATS) failed");
		}

		clear_screen();
		time(&now);
		strftime(timebuf, sizeof(timebuf),
			 "%Y-%m-%d %H:%M:%S", localtime(&now));
		printf("irqtop - %s  (delay %.1fs)\n", timebuf, delay);

		print_header();
		for (i = 0; i < IRQTOP_IRQ_THREADS; i++)
			print_row(&stats[i], warn_max);
		print_summary(stats, IRQTOP_IRQ_THREADS, delay);

		if (delay <= 0)
			break;

		sleep(delay);
	}

	return 0;
}
