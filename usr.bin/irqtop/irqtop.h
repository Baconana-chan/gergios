/*
 * irqtop.h — Common types and declarations for irqtop(1).
 */

#ifndef IRQTOP_H
#define IRQTOP_H

#include <sys/types.h>
#include <minix/com.h>
#include <minix/endpoint.h>
#include <stdint.h>
#include <time.h>

/* Number of IRQ threads — must match kernel/irq_thread.h */
#define IRQTOP_IRQ_THREADS		64

/* Maximum history window entries (for rolling stats) */
#define IRQTOP_MAX_HISTORY		120

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

/* Rolling latency history entry */
struct irqtop_history_entry {
	uint64_t	latency;	/* TSC ticks for this sample */
	time_t		timestamp;	/* when it was recorded */
};

/* Per-IRQ rolling history (tracked across snapshots) */
struct irqtop_history {
	struct irqtop_history_entry entries[IRQTOP_MAX_HISTORY];
	int		count;		/* number of valid entries */
	int		head;		/* circular buffer head */
	uint64_t	last_min;	/* min in current window */
	uint64_t	last_max;	/* max in current window */
	uint64_t	last_avg;	/* avg in current window */
	uint64_t	last_p50;	/* median */
	uint64_t	last_p95;	/* 95th percentile */
	uint64_t	last_p99;	/* 99th percentile */
};

/* Sorting modes */
enum sort_mode {
	SORT_IRQ = 0,		/* by IRQ number (default) */
	SORT_LATENCY,		/* by max_latency descending */
	SORT_HITS,		/* by handled_count descending */
	SORT_NAME,		/* by IRQ name alphabetically */
	SORT_PRIO,		/* by RT priority descending */
};

/* Output formats */
enum output_format {
	OUTPUT_TABLE = 0,	/* default formatted table */
	OUTPUT_JSON,		/* JSON */
	OUTPUT_CSV,		/* CSV */
};

/* Logging modes */
enum log_mode {
	LOG_NONE = 0,		/* no logging */
	LOG_SNAPSHOT,		/* log every snapshot to file */
	LOG_PEAK,		/* only log when latency exceeds threshold */
};

/* Runtime configuration */
struct irqtop_config {
	/* Display */
	double		delay;			/* refresh interval (seconds) */
	int		one_shot;		/* print once and exit */
	uint64_t	warn_max;		/* highlight IRQs with max_lat > this */
	int		use_nsec;		/* show latency in nanoseconds */
	int		cpu_mhz;		/* CPU frequency for TSC→ns conversion */

	/* Sorting */
	enum sort_mode	sort;			/* column to sort by */

	/* Filtering */
	int		registered_only;	/* -r: show only registered IRQs */
	int		specific_irq;		/* -i N: show only this IRQ (-1 = all) */
	uint64_t	lat_threshold;		/* -t N: only IRQs with max_lat > N */

	/* Output */
	enum output_format output;		/* table/json/csv */

	/* Logging */
	enum log_mode	logging;		/* logging mode */
	const char	*log_path;		/* log file path */

	/* History */
	int		history_enabled;	/* enable rolling stats */
};

/* Global config instance */
extern struct irqtop_config g_cfg;

/* History instances (one per IRQ) */
extern struct irqtop_history g_history[IRQTOP_IRQ_THREADS];

/* Function declarations */
const char *irq_name(int irq);

/* stats.c */
int  stats_fetch(struct irqtop_stats *stats, int n);
int  stats_detect_cpu_mhz(void);
void stats_update_history(struct irqtop_stats *stats, int n);
void stats_sort(struct irqtop_stats *stats, int n);
int  stats_filter(const struct irqtop_stats *s);
struct irqtop_history *stats_get_history(int idx);

/* display.c */
void display_clear(void);
void display_header(void);
void display_row(const struct irqtop_stats *s, int idx);
void display_summary(const struct irqtop_stats *stats, int n);

/* output.c */
void output_json(const struct irqtop_stats *stats, int n);
void output_csv_header(void);
void output_csv_row(const struct irqtop_stats *s, int idx);

/* logging.c */
int  logging_open(const char *path);
void logging_close(void);
void logging_snapshot(const struct irqtop_stats *stats, int n,
		      time_t now);
void logging_peak(const struct irqtop_stats *stats, int n,
		  time_t now);

#endif /* IRQTOP_H */
