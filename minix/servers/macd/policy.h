/* Policy engine for macd — MAC policy decision point.
 *
 * The policy engine loads rules from /etc/macd.conf and evaluates
 * MAC check requests against them using first-match-wins semantics.
 *
 * Default policy (when no rules match): DENY (fail-closed).
 * The default config includes 'allow ALL' for backward compatibility.
 *
 * Rule file format (/etc/macd.conf):
 *   # comment
 *   <allow|deny> <op> [from <label>] [to <label>]
 *
 *   op:     ALL, IPC_SEND, IPC_RECEIVE, FILE_ACCESS, PRIVCTL_SET_SYS,
 *           DEVICE_BIND, PROC_FORK, PROC_EXEC, PROC_KILL, RAWIO
 *   label:  service label from system.conf (e.g., vfs, pm, rs, init)
 *           or ANY (matches any endpoint)
 *
 * Examples:
 *   # Allow everything (compatible default)
 *   allow ALL
 *
 *   # Deny IPC from init to VFS
 *   deny IPC_SEND from init to vfs
 *
 *   # Deny all file access by init
 *   deny FILE_ACCESS from init to ANY
 */

#ifndef _MACD_POLICY_H
#define _MACD_POLICY_H

#include <minix/mac.h>

/* Maximum length of a service label in a policy rule. */
#define MAC_LABEL_MAX 32

/*===========================================================================*
 *                    Rule structure (linked list)                            *
 *===========================================================================*/
struct mac_rule {
	int action;			/* MAC_ALLOW or MAC_DENY */
	int op_type;			/* MAC_* hook type, or 0 for ALL */
	char from_label[MAC_LABEL_MAX];/* "" for ANY */
	char to_label[MAC_LABEL_MAX];	/* "" for ANY */
	int from_endpoint_valid;	/* 1 if from_endpoint is resolved */
	endpoint_t from_endpoint;	/* cached resolved endpoint */
	int to_endpoint_valid;		/* 1 if to_endpoint is resolved */
	endpoint_t to_endpoint;		/* cached resolved endpoint */
	struct mac_rule *next;		/* next rule in chain */
};

/*===========================================================================*
 *                    Binary policy format constants                        *
 *===========================================================================*/

#define MAC_BIN_MAGIC    0x4D414350   /* "MACP" in ASCII */
#define MAC_BIN_VERSION  1

struct mac_policy_header {
	uint32_t magic;
	uint32_t version;
	uint32_t num_rules;
};

struct mac_rule_bin {
	int32_t  action;
	int32_t  op_type;
	char     from_label[32];
	char     to_label[32];
};

/*===========================================================================*
 *                    API                                                      *
 *===========================================================================*/

/* Initialize the policy engine: try binary /etc/macd.policy first,
 * fall back to text /etc/macd.conf. */
void policy_init(void);

/* Load rules from a compiled binary policy file (mac-compile output).
 * Returns number of rules loaded, or negative on error.
 */
int policy_load_binary(const char *path);

/* Return the number of loaded policy rules via 'count'. */
void policy_count_rules(int *count);

/* Evaluate a MAC check request against the rule list.
 * Returns MAC_ALLOW or MAC_DENY.
 * First matching rule wins. If no rule matches, returns MAC_DENY.
 */
int policy_check(int what, mac_context_t *ctx);

/* Reload rules from config file (e.g., after SIGHUP). */
void policy_reload(void);

/* Free all allocated rule resources. */
void policy_cleanup(void);

#endif /* _MACD_POLICY_H */
