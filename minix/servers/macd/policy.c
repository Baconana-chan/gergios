/* policy.c — MAC policy engine.
 *
 * Loads rules from /etc/macd.conf, evaluates MAC check requests
 * using first-match-wins semantics.
 *
 * Rule format:
 *   <allow|deny> <op> [from <label>] [to <label>]
 *
 *   Labels are resolved to endpoints via minix_rs_lookup() at check time.
 *   "ANY" (or omitted) matches any source/destination endpoint.
 *
 * Default when no rule matches: DENY (fail-closed).
 */

#include "policy.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <minix/rs.h>
#include <minix/endpoint.h>

/* Config file path. */
#define MACD_CONF_PATH "/etc/macd.conf"

/* Maximum line length in config file. */
#define MACD_CONF_LINE_MAX 256

/* Head of the rule linked list. */
static struct mac_rule *rule_list = NULL;

/*===========================================================================*
 *                    Operation name → type mapping                          *
 *===========================================================================*/
static const struct {
	const char *name;
	int type;
} op_table[] = {
	{ "ALL",		0		},
	{ "IPC_SEND",		MAC_IPC_SEND	},
	{ "IPC_RECEIVE",	MAC_IPC_RECEIVE	},
	{ "FILE_ACCESS",	MAC_FILE_ACCESS	},
	{ "PRIVCTL_SET_SYS",	MAC_PRIVCTL_SET_SYS },
	{ "DEVICE_BIND",	MAC_DEVICE_BIND	},
	{ "PROC_FORK",		MAC_PROC_FORK	},
	{ "PROC_EXEC",		MAC_PROC_EXEC	},
	{ "PROC_KILL",		MAC_PROC_KILL	},
	{ "RAWIO",		MAC_RAWIO	},
	{ NULL,			0		},
};

/*===========================================================================*
 *                    Helper: trim whitespace                                *
 *===========================================================================*/
static char *trim_ws(char *s)
{
	char *end;

	while (isspace((unsigned char)*s))
		s++;
	if (*s == '\0')
		return s;

	end = s + strlen(s) - 1;
	while (end > s && isspace((unsigned char)*end))
		end--;
	*(end + 1) = '\0';

	return s;
}

/*===========================================================================*
 *                    Helper: tokenize line into words                       *
 *===========================================================================*/
static int tokenize(char *line, char *tokens[], int max_tokens)
{
	int count = 0;
	char *p = line;

	while (*p && count < max_tokens) {
		/* Skip whitespace. */
		while (isspace((unsigned char)*p))
			p++;
		if (*p == '\0')
			break;

		tokens[count++] = p;

		/* Skip to next whitespace. */
		while (*p && !isspace((unsigned char)*p))
			p++;
		if (*p) {
			*p = '\0';
			p++;
		}
	}

	return count;
}

/*===========================================================================*
 *                    Look up operation type by name                        *
 *===========================================================================*/
static int lookup_op_type(const char *name)
{
	int i;

	for (i = 0; op_table[i].name != NULL; i++) {
		if (strcmp(name, op_table[i].name) == 0)
			return op_table[i].type;
	}

	return -1; /* unknown */
}

/*===========================================================================*
 *                    Parse one rule from tokens                            *
 *===========================================================================*/
/*===========================================================================*
 *                    Cache a rule's endpoint (resolve label → endpoint)    *
 *===========================================================================*/
static void cache_rule_endpoints(struct mac_rule *rule)
{
	/* Resolve from_label. */
	if (rule->from_label[0] != '\0' &&
	    strcmp(rule->from_label, "ANY") != 0) {
		if (minix_rs_lookup(rule->from_label,
		    &rule->from_endpoint) == OK) {
			rule->from_endpoint_valid = 1;
		}
	}

	/* Resolve to_label. */
	if (rule->to_label[0] != '\0' &&
	    strcmp(rule->to_label, "ANY") != 0) {
		if (minix_rs_lookup(rule->to_label,
		    &rule->to_endpoint) == OK) {
			rule->to_endpoint_valid = 1;
		}
	}
}

static struct mac_rule *parse_rule(char *tokens[], int num_tokens)
{
	struct mac_rule *rule;
	int i;

	if (num_tokens < 2)
		return NULL;

	/* Allocate rule. */
	rule = (struct mac_rule *)malloc(sizeof(struct mac_rule));
	if (rule == NULL)
		return NULL;

	memset(rule, 0, sizeof(*rule));
	rule->action = MAC_DENY;

	/* First token: allow or deny. */
	if (strcmp(tokens[0], "allow") == 0) {
		rule->action = MAC_ALLOW;
	} else if (strcmp(tokens[0], "deny") == 0) {
		rule->action = MAC_DENY;
	} else {
		free(rule);
		return NULL;
	}

	/* Second token: operation type. */
	rule->op_type = lookup_op_type(tokens[1]);
	if (rule->op_type < 0) {
		printf("macd: unknown operation '%s' in config\n", tokens[1]);
		free(rule);
		return NULL;
	}

	/* Optional: from <label>, to <label>. */
	for (i = 2; i < num_tokens - 1; i += 2) {
		if (strcmp(tokens[i], "from") == 0) {
			strlcpy(rule->from_label, tokens[i + 1],
			    sizeof(rule->from_label));
		} else if (strcmp(tokens[i], "to") == 0) {
			strlcpy(rule->to_label, tokens[i + 1],
			    sizeof(rule->to_label));
		} else {
			printf("macd: unexpected keyword '%s' in config\n",
			    tokens[i]);
			free(rule);
			return NULL;
		}
	}

	/* Pre-resolve endpoints for caching. */
	cache_rule_endpoints(rule);

	return rule;
}

/*===========================================================================*
 *                    Load rules from config file                           *
 *===========================================================================*/
static void load_rules(void)
{
	FILE *fp;
	char line[MACD_CONF_LINE_MAX];
	char *trimmed;
	char *tokens[16];
	int num_tokens;
	struct mac_rule *rule;
	struct mac_rule **nextp = &rule_list;

	fp = fopen(MACD_CONF_PATH, "r");
	if (fp == NULL) {
		/* Config file doesn't exist — use default allow-all rule. */
		printf("macd: %s not found, using default allow-all policy\n",
		    MACD_CONF_PATH);

		rule = (struct mac_rule *)malloc(sizeof(struct mac_rule));
		if (rule != NULL) {
			memset(rule, 0, sizeof(*rule));
			rule->action = MAC_ALLOW;
			rule->op_type = 0; /* ALL */
			rule->next = NULL;
			rule_list = rule;
		}
		return;
	}

	printf("macd: loading policy from %s\n", MACD_CONF_PATH);

	while (fgets(line, sizeof(line), fp) != NULL) {
		trimmed = trim_ws(line);

		/* Skip empty lines and comments. */
		if (trimmed[0] == '\0' || trimmed[0] == '#')
			continue;

		num_tokens = tokenize(trimmed, tokens, 16);
		if (num_tokens < 2)
			continue;

		rule = parse_rule(tokens, num_tokens);
		if (rule == NULL) {
			printf("macd: skipping bad rule: %s\n", trimmed);
			continue;
		}

		/* Append to list. */
		*nextp = rule;
		nextp = &rule->next;
	}

	fclose(fp);

	/* Log summary. */
	{
		int count = 0;
		for (rule = rule_list; rule != NULL; rule = rule->next)
			count++;
		printf("macd: loaded %d policy rule(s)\n", count);
	}
}

/*===========================================================================*
 *                    Resolve a label to an endpoint (with caching)         *
 *===========================================================================*/
/*
 * Try to resolve a label to its endpoint. Returns the endpoint via 'ep'.
 * Returns OK on success, EINVAL if label is "ANY" or empty, or a negative
 * errno from minix_rs_lookup() if the service is not yet registered.
 */
static int resolve_label(const char *label, endpoint_t *ep)
{
	if (label == NULL || label[0] == '\0')
		return EINVAL;

	/* "ANY" matches everything — handled separately in rule_matches(). */
	if (strcmp(label, "ANY") == 0)
		return EINVAL;

	/* Use minix_rs_lookup to resolve service label to endpoint. */
	return minix_rs_lookup(label, ep);
}

/*===========================================================================*
 *                    Extract source endpoint from context                  *
 *===========================================================================*/
static endpoint_t ctx_get_src(int what, mac_context_t *ctx)
{
	switch (what) {
	case MAC_IPC_SEND:
	case MAC_IPC_RECEIVE:
		return ctx->ipc.mac_src;
	case MAC_FILE_ACCESS:
		return ctx->file.mac_proc;
	case MAC_PRIVCTL_SET_SYS:
		return ctx->privctl.mac_caller;
	case MAC_DEVICE_BIND:
		return ctx->device.mac_driver;
	case MAC_PROC_FORK:
	case MAC_PROC_EXEC:
	case MAC_PROC_KILL:
		return ctx->proc.mac_caller;
	default:
		return NONE;
	}
}

/*===========================================================================*
 *                    Extract target endpoint from context                  *
 *===========================================================================*/
static endpoint_t ctx_get_dst(int what, mac_context_t *ctx)
{
	switch (what) {
	case MAC_IPC_SEND:
	case MAC_IPC_RECEIVE:
		return ctx->ipc.mac_dst;
	case MAC_FILE_ACCESS:
		return ctx->file.mac_fs_ep;
	case MAC_PRIVCTL_SET_SYS:
		return ctx->privctl.mac_target;
	case MAC_DEVICE_BIND:
		return ctx->device.mac_devman;
	case MAC_PROC_FORK:
	case MAC_PROC_EXEC:
	case MAC_PROC_KILL:
		return ctx->proc.mac_target;
	default:
		return NONE;
	}
}

/*===========================================================================*
 *                    Check if a rule matches the request                   *
 *===========================================================================*/
static int rule_matches(struct mac_rule *rule, int what, mac_context_t *ctx)
{
	/* Check operation type. 0 = ALL (matches everything). */
	if (rule->op_type != 0 && rule->op_type != what)
		return 0;

	/* Check source label (if specified). */
	if (rule->from_label[0] != '\0' &&
	    strcmp(rule->from_label, "ANY") != 0) {
		endpoint_t ctx_ep = ctx_get_src(what, ctx);

		/* Use cached endpoint if valid; otherwise try to resolve. */
		if (!rule->from_endpoint_valid) {
			if (resolve_label(rule->from_label,
			    &rule->from_endpoint) == OK) {
				rule->from_endpoint_valid = 1;
			}
		}

		/* If we still don't have a valid endpoint, no match. */
		if (!rule->from_endpoint_valid)
			return 0;

		if (rule->from_endpoint != ctx_ep)
			return 0;
	}

	/* Check destination label (if specified). */
	if (rule->to_label[0] != '\0' &&
	    strcmp(rule->to_label, "ANY") != 0) {
		endpoint_t ctx_ep = ctx_get_dst(what, ctx);

		/* Use cached endpoint if valid; otherwise try to resolve. */
		if (!rule->to_endpoint_valid) {
			if (resolve_label(rule->to_label,
			    &rule->to_endpoint) == OK) {
				rule->to_endpoint_valid = 1;
			}
		}

		/* If we still don't have a valid endpoint, no match. */
		if (!rule->to_endpoint_valid)
			return 0;

		if (rule->to_endpoint != ctx_ep)
			return 0;
	}

	return 1; /* all checks passed, rule matches */
}

/*===========================================================================*
 *                    policy_init                                             *
 *===========================================================================*/
void policy_init(void)
{
	load_rules();
}

/*===========================================================================*
 *                    policy_check                                            *
 *===========================================================================*/
int policy_check(int what, mac_context_t *ctx)
{
	struct mac_rule *rule;

	/* Walk the rule list. First match wins. */
	for (rule = rule_list; rule != NULL; rule = rule->next) {
		if (rule_matches(rule, what, ctx)) {
			return rule->action;
		}
	}

	/* No rule matched — fail-closed: DENY. */
	return MAC_DENY;
}

/*===========================================================================*
 *                    policy_reload                                           *
 *===========================================================================*/
void policy_reload(void)
{
	/* Free existing rules. */
	policy_cleanup();

	/* Reload from config. */
	load_rules();
}

/*===========================================================================*
 *                    policy_cleanup                                          *
 *===========================================================================*/
void policy_cleanup(void)
{
	struct mac_rule *rule, *next;

	rule = rule_list;
	while (rule != NULL) {
		next = rule->next;
		free(rule);
		rule = next;
	}

	rule_list = NULL;
}
