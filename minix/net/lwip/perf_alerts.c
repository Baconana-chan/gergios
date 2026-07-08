/*
 * perf_alerts.c — Network performance alert implementation.
 *
 * Alerts are rate-limited and logged via syslog(3) with LOG_DAEMON
 * facility.  Each alert type has a configurable threshold and a cooldown
 * timer to prevent log flooding.
 *
 * A periodic tick (perf_alerts_tick) resets the per-tick counters.
 * If a counter exceeds its threshold, a syslog alert is emitted (at most
 * once per cooldown period).
 *
 * All thresholds are configurable via sysctl under minix.lwip.alerts.*
 */

#include "perf_alerts.h"

#include "lwip.h"

#include <syslog.h>

/* ── Alert state per type ────────────────────────────────────────── */
struct perf_alert {
	uint32_t	pa_count;		/* events since last tick */
	uint32_t	pa_threshold;		/* max events per tick */
	clock_t		pa_last_alert;		/* tick count of last alert */
	clock_t		pa_cooldown;		/* min ticks between alerts */
	const char *	pa_name;		/* human-readable name */
	int		pa_log_level;		/* syslog level */
};

/* ── Global enable flag and state ───────────────────────────────── */
static int perf_alerts_enabled = 1;

/* ── Per-alert state ─────────────────────────────────────────────── */
static struct perf_alert perf_alert_drop = {
	.pa_threshold	= PERF_ALERTS_DROP_THRESH_DEF,
	.pa_cooldown	= sys_hz() * PERF_ALERTS_COOLDOWN_S,
	.pa_name	= "packet-drop",
	.pa_log_level	= LOG_WARNING,
};

static struct perf_alert perf_alert_tcp_rst = {
	.pa_threshold	= PERF_ALERTS_RST_THRESH_DEF,
	.pa_cooldown	= sys_hz() * PERF_ALERTS_COOLDOWN_S,
	.pa_name	= "tcp-rst",
	.pa_log_level	= LOG_WARNING,
};

static struct perf_alert perf_alert_oom = {
	.pa_threshold	= PERF_ALERTS_OOM_THRESH_DEF,
	.pa_cooldown	= sys_hz() * PERF_ALERTS_COOLDOWN_S,
	.pa_name	= "oom",
	.pa_log_level	= LOG_ERR,
};

/* Latency is special: it fires on any single event above threshold. */
static struct perf_alert perf_alert_latency = {
	.pa_threshold	= 1,			/* fire on any high latency */
	.pa_cooldown	= sys_hz() * PERF_ALERTS_COOLDOWN_S,
	.pa_name	= "high-latency",
	.pa_log_level	= LOG_WARNING,
};
static uint32_t perf_alerts_latency_us = PERF_ALERTS_LATENCY_US_DEF;

static struct perf_alert perf_alert_rate_limit = {
	.pa_threshold	= PERF_ALERTS_RATELIMIT_THRESH_DEF,
	.pa_cooldown	= sys_hz() * PERF_ALERTS_COOLDOWN_S,
	.pa_name	= "rate-limit-hit",
	.pa_log_level	= LOG_WARNING,
};

/* ── Current system tick (cached once per tick) ──────────────────── */
static clock_t perf_alerts_now;

/*
 * Check if an alert should be emitted for the given alert state.
 * Returns TRUE if the alert should be logged, FALSE if we're still
 * in the cooldown period.
 */
static int
perf_alert_should_fire(struct perf_alert * alert)
{

	if (!perf_alerts_enabled)
		return 0;

	if (alert->pa_count < alert->pa_threshold)
		return 0;

	if (perf_alerts_now - alert->pa_last_alert < alert->pa_cooldown)
		return 0;

	alert->pa_last_alert = perf_alerts_now;

	return 1;
}

/*
 * Emit a simple alert message with no additional details.
 */
static void
perf_alert_emit(struct perf_alert * alert)
{

	syslog(alert->pa_log_level, "perf: %s threshold exceeded (%u events)",
	    alert->pa_name, alert->pa_count);
}

/*
 * Emit an alert with an interface name context.
 */
static void
perf_alert_emit_ifname(struct perf_alert * alert, const char *ifname)
{

	syslog(alert->pa_log_level,
	    "perf: %s on %s threshold exceeded (%u events)",
	    alert->pa_name, ifname, alert->pa_count);
}

/*
 * Emit a latency alert with the measured duration.
 */
static void
perf_alert_emit_latency(struct perf_alert * alert, uint32_t duration_us)
{

	syslog(alert->pa_log_level,
	    "perf: %s %.3f ms (threshold %u ms)",
	    alert->pa_name, (double)duration_us / 1000.0,
	    perf_alerts_latency_us / 1000);
}

/* ═══════════════════════════════════════════════════════════════════
 * Public API
 * ═══════════════════════════════════════════════════════════════════ */

/*
 * Record a packet drop on an interface.
 */
void
perf_alerts_drop(const char *ifname)
{

	perf_alert_drop.pa_count++;

	if (perf_alert_should_fire(&perf_alert_drop))
		perf_alert_emit_ifname(&perf_alert_drop, ifname);
}

/*
 * Record a TCP RST event.
 */
void
perf_alerts_tcp_rst(void)
{

	perf_alert_tcp_rst.pa_count++;

	if (perf_alert_should_fire(&perf_alert_tcp_rst))
		perf_alert_emit(&perf_alert_tcp_rst);
}

/*
 * Record an out-of-memory (allocation failure) event.
 */
void
perf_alerts_oom(void)
{

	perf_alert_oom.pa_count++;

	if (perf_alert_should_fire(&perf_alert_oom))
		perf_alert_emit(&perf_alert_oom);
}

/*
 * Record a high-latency event.  duration_us is the measured duration
 * in microseconds.  An alert is fired immediately if the duration
 * exceeds the configurable latency threshold, subject to cooldown.
 */
void
perf_alerts_latency(uint32_t duration_us)
{

	perf_alert_latency.pa_count++;

	if (perf_alerts_enabled && duration_us >= perf_alerts_latency_us) {
		if (perf_alerts_now - perf_alert_latency.pa_last_alert >=
		    perf_alert_latency.pa_cooldown) {
			perf_alert_latency.pa_last_alert = perf_alerts_now;
			perf_alert_emit_latency(&perf_alert_latency,
			    duration_us);
		}
	}
}

/*
 * Record a rate-limiter activation event (e.g., ICMP, ARP, NDP).
 */
void
perf_alerts_rate_limit(void)
{

	perf_alert_rate_limit.pa_count++;

	if (perf_alert_should_fire(&perf_alert_rate_limit))
		perf_alert_emit(&perf_alert_rate_limit);
}

/*
 * Periodic tick: update the cached tick count and reset per-tick
 * counters.  This should be called from the main timer (e.g., every
 * ~2 seconds, same as the SYN cookie timer).
 */
void
perf_alerts_tick(void)
{

	perf_alerts_now = getticks();

	perf_alert_drop.pa_count = 0;
	perf_alert_tcp_rst.pa_count = 0;
	perf_alert_oom.pa_count = 0;
	perf_alert_latency.pa_count = 0;
	perf_alert_rate_limit.pa_count = 0;
}

/* ═══════════════════════════════════════════════════════════════════
 * RMIB sysctl handlers (minix.lwip.alerts.*)
 * ═══════════════════════════════════════════════════════════════════ */

static ssize_t
perf_alerts_drop_thresh_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(perf_alert_drop.pa_threshold);

	if ((r = rmib_copyout(oldp, 0, &perf_alert_drop.pa_threshold,
	    sizeof(perf_alert_drop.pa_threshold))) < 0)
		return r;
	return (ssize_t)sizeof(perf_alert_drop.pa_threshold);
}

static ssize_t
perf_alerts_rst_thresh_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(perf_alert_tcp_rst.pa_threshold);

	if ((r = rmib_copyout(oldp, 0, &perf_alert_tcp_rst.pa_threshold,
	    sizeof(perf_alert_tcp_rst.pa_threshold))) < 0)
		return r;
	return (ssize_t)sizeof(perf_alert_tcp_rst.pa_threshold);
}

static ssize_t
perf_alerts_oom_thresh_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(perf_alert_oom.pa_threshold);

	if ((r = rmib_copyout(oldp, 0, &perf_alert_oom.pa_threshold,
	    sizeof(perf_alert_oom.pa_threshold))) < 0)
		return r;
	return (ssize_t)sizeof(perf_alert_oom.pa_threshold);
}

static ssize_t
perf_alerts_latency_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(perf_alerts_latency_us);

	if ((r = rmib_copyout(oldp, 0, &perf_alerts_latency_us,
	    sizeof(perf_alerts_latency_us))) < 0)
		return r;
	return (ssize_t)sizeof(perf_alerts_latency_us);
}

static ssize_t
perf_alerts_rate_limit_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(perf_alert_rate_limit.pa_threshold);

	if ((r = rmib_copyout(oldp, 0, &perf_alert_rate_limit.pa_threshold,
	    sizeof(perf_alert_rate_limit.pa_threshold))) < 0)
		return r;
	return (ssize_t)sizeof(perf_alert_rate_limit.pa_threshold);
}

/* The minix.lwip.alerts RMIB subtree. */
static struct rmib_node minix_lwip_alerts_table[] = {
	RMIB_INTPTR(RMIB_RW, &perf_alerts_enabled, "enabled",
	    "Enable performance alerts (syslog)"),
	RMIB_FUNC(RMIB_RW | CTLTYPE_INT, sizeof(uint32_t),
	    perf_alerts_drop_thresh_handler, "drop_thresh",
	    "Packet drop threshold per tick"),
	RMIB_FUNC(RMIB_RW | CTLTYPE_INT, sizeof(uint32_t),
	    perf_alerts_rst_thresh_handler, "rst_thresh",
	    "TCP RST threshold per tick"),
	RMIB_FUNC(RMIB_RW | CTLTYPE_INT, sizeof(uint32_t),
	    perf_alerts_oom_thresh_handler, "oom_thresh",
	    "OOM threshold per tick"),
	RMIB_FUNC(RMIB_RW | CTLTYPE_INT, sizeof(uint32_t),
	    perf_alerts_latency_handler, "latency_us",
	    "High-latency threshold in microseconds"),
	RMIB_FUNC(RMIB_RW | CTLTYPE_INT, sizeof(uint32_t),
	    perf_alerts_rate_limit_handler, "rate_limit_thresh",
	    "Rate-limit hits threshold per tick"),
};

static struct rmib_node minix_lwip_alerts_node =
    RMIB_NODE(RMIB_RO, minix_lwip_alerts_table, "alerts",
	"Performance alert thresholds");

/*
 * Initialize the performance alerts module.
 */
void
perf_alerts_init(void)
{

	/* Initialize syslog.  Ident is "lwip" to match the service name. */
	openlog("lwip", LOG_PID | LOG_NDELAY, LOG_DAEMON);

	/* Initialize tick cache. */
	perf_alerts_now = getticks();

	/* Register the sysctl subtree. */
	mibtree_register_lwip(&minix_lwip_alerts_node);

	syslog(LOG_INFO, "perf: alerts initialized (enabled=%d)", 
	    perf_alerts_enabled);
}
