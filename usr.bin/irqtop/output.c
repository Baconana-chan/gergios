/*
 * output.c — JSON and CSV output formatters for irqtop(1).
 */

#include <sys/types.h>
#include <stdio.h>
#include <string.h>
#include <minix/com.h>
#include "irqtop.h"

/* Convert TSC ticks to nanoseconds if -u is set */
static uint64_t fmt_val(uint64_t tsc_ticks)
{
	if (g_cfg.use_nsec && g_cfg.cpu_mhz > 0)
		return (tsc_ticks * 1000UL) / (uint64_t)g_cfg.cpu_mhz;
	return tsc_ticks;
}

/* Print a JSON-escaped string */
static void json_esc(FILE *fp, const char *s)
{
	fputc('"', fp);
	while (*s) {
		switch (*s) {
		case '"':  fputs("\\\"", fp); break;
		case '\\': fputs("\\\\", fp); break;
		case '\n': fputs("\\n", fp);  break;
		case '\t': fputs("\\t", fp);  break;
		default:   fputc(*s, fp);     break;
		}
		s++;
	}
	fputc('"', fp);
}

/* Output all stats as a JSON object */
void output_json(const struct irqtop_stats *stats, int n)
{
	int first = 1;
	int i;

	printf("{\n");
	printf("  \"timestamp\": %lld,\n",
	       (long long)time(NULL));
	printf("  \"cpu_mhz\": %d,\n", g_cfg.cpu_mhz);
	printf("  \"latency_unit\": ");
	json_esc(stdout, g_cfg.use_nsec && g_cfg.cpu_mhz > 0 ? "ns" : "tsc");
	printf(",\n");
	printf("  \"irqs\": [\n");

	for (i = 0; i < n; i++) {
		uint64_t avg_lat = 0;

		if (!stats_filter(&stats[i]))
			continue;

		if (stats[i].handled_count > 0)
			avg_lat = stats[i].total_latency /
				  stats[i].handled_count;

		if (!first)
			printf(",\n");
		first = 0;

		printf("    {\n");
		printf("      \"irq\": %d,\n", stats[i].irq);
		printf("      \"name\": ");
		json_esc(stdout, irq_name(stats[i].irq));
		printf(",\n");
		printf("      \"priority\": %d,\n", stats[i].rt_prio);
		printf("      \"registered\": %s,\n",
		       stats[i].registered ? "true" : "false");
		printf("      \"endpoint\": %d,\n", stats[i].endpoint);
		printf("      \"handled\": %llu,\n",
		       (unsigned long long)stats[i].handled_count);
		printf("      \"runs\": %llu,\n",
		       (unsigned long long)stats[i].run_count);
		printf("      \"last_latency\": %llu,\n",
		       (unsigned long long)fmt_val(stats[i].last_latency));
		printf("      \"avg_latency\": %llu,\n",
		       (unsigned long long)fmt_val(avg_lat));
		printf("      \"max_latency\": %llu,\n",
		       (unsigned long long)fmt_val(stats[i].max_latency));
		printf("      \"total_latency_raw\": %llu\n",
		       (unsigned long long)stats[i].total_latency);
		printf("    }");
	}

	printf("\n  ]\n");
	printf("}\n");
}

/* Print CSV header row */
void output_csv_header(void)
{
	printf("irq,name,priority,registered,endpoint,handled,runs,"
	       "last_latency,avg_latency,max_latency\n");
}

/* Print a single CSV data row */
void output_csv_row(const struct irqtop_stats *s, int idx)
{
	uint64_t avg_lat = 0;

	(void)idx;

	if (s->handled_count > 0)
		avg_lat = s->total_latency / s->handled_count;

	printf("%d,%s,%d,%s,%d,%llu,%llu,%llu,%llu,%llu\n",
	       s->irq,
	       irq_name(s->irq),
	       s->rt_prio,
	       s->registered ? "yes" : "no",
	       s->endpoint,
	       (unsigned long long)s->handled_count,
	       (unsigned long long)s->run_count,
	       (unsigned long long)fmt_val(s->last_latency),
	       (unsigned long long)fmt_val(avg_lat),
	       (unsigned long long)fmt_val(s->max_latency));
}
