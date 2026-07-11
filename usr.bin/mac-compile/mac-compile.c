/* mac-compile — MAC Policy Compiler for GergiOS.
 *
 * Phase 3.4: Compiles human-readable MAC policy rules from
 * /etc/macd.conf format into a compact binary format that the
 * macd daemon can load quickly.
 *
 * Usage:
 *   mac-compile [-o output] [-v] [input_file]
 *
 * If input_file is omitted, reads from stdin.
 * If -o is omitted, writes to stdout.
 *
 * Binary format:
 *   Header:  magic(4) + version(4) + num_rules(4) = 12 bytes
 *   Each rule: action(4) + op_type(4) + from_label(32) + to_label(32) = 72 bytes
 *
 * Rule syntax (same as /etc/macd.conf):
 *   <allow|deny> <op> [from <label>] [to <label>]
 *   # comments start with #
 */

#include <sys/types.h>
#include <sys/stat.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <errno.h>
#include <unistd.h>
#include <stdint.h>

/* MAC constants — duplicated from <minix/mac.h> so this tool can
 * compile standalone without MINIX system includes. */
#ifndef MAC_ALLOW
#define MAC_ALLOW    0
#define MAC_DENY     (-1)
#endif

#ifndef MAC_IPC_SEND
#define MAC_IPC_SEND        1
#define MAC_IPC_RECEIVE     2
#define MAC_FILE_ACCESS     3
#define MAC_PRIVCTL_SET_SYS 4
#define MAC_DEVICE_BIND     5
#define MAC_PROC_FORK       6
#define MAC_PROC_EXEC       7
#define MAC_PROC_KILL       8
#define MAC_RAWIO           9
#endif

/*===========================================================================*
 *                    Binary format definitions                              *
 *===========================================================================*/
#define MAC_BIN_MAGIC    0x4D414350   /* "MACP" in ASCII */
#define MAC_BIN_VERSION  1
#define MAC_LABEL_MAX    32

struct mac_policy_header {
	uint32_t magic;
	uint32_t version;
	uint32_t num_rules;
};

struct mac_rule_bin {
	int32_t  action;
	int32_t  op_type;
	char     from_label[MAC_LABEL_MAX];
	char     to_label[MAC_LABEL_MAX];
};

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
		while (isspace((unsigned char)*p))
			p++;
		if (*p == '\0')
			break;

		tokens[count++] = p;

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
	return -1;
}

/*===========================================================================*
 *                    Validate and count rules from file                    *
 *===========================================================================*/
static int count_and_validate(FILE *fp, int verbose)
{
	char line[256];
	char *trimmed;
	char *tokens[16];
	int num_tokens;
	int count = 0;
	int line_num = 0;
	int errors = 0;

	rewind(fp);

	while (fgets(line, sizeof(line), fp) != NULL) {
		line_num++;
		trimmed = trim_ws(line);

		if (trimmed[0] == '\0' || trimmed[0] == '#')
			continue;

		num_tokens = tokenize(trimmed, tokens, 16);
		if (num_tokens < 2) {
			fprintf(stderr, "line %d: too few tokens\n", line_num);
			errors++;
			continue;
		}

		/* Validate action. */
		if (strcmp(tokens[0], "allow") != 0 &&
		    strcmp(tokens[0], "deny") != 0) {
			fprintf(stderr, "line %d: expected 'allow' or 'deny', "
			    "got '%s'\n", line_num, tokens[0]);
			errors++;
			continue;
		}

		/* Validate operation. */
		if (lookup_op_type(tokens[1]) < 0) {
			fprintf(stderr, "line %d: unknown operation '%s'\n",
			    line_num, tokens[1]);
			errors++;
			continue;
		}

		count++;
	}

	if (errors > 0) {
		fprintf(stderr, "mac-compile: %d error(s) in input\n", errors);
		return -1;
	}

	if (verbose)
		fprintf(stderr, "mac-compile: %d rule(s) validated\n", count);

	rewind(fp);
	return count;
}

/*===========================================================================*
 *                    Compile a single rule into binary                     *
 *===========================================================================*/
static int compile_rule(char *tokens[], int num_tokens,
    struct mac_rule_bin *rule_bin)
{
	int i;

	memset(rule_bin, 0, sizeof(*rule_bin));

	/* Action. */
	if (strcmp(tokens[0], "allow") == 0)
		rule_bin->action = MAC_ALLOW;
	else
		rule_bin->action = MAC_DENY;

	/* Operation. */
	rule_bin->op_type = lookup_op_type(tokens[1]);

	/* Optional: from <label>, to <label>. */
	for (i = 2; i < num_tokens - 1; i += 2) {
		if (strcmp(tokens[i], "from") == 0) {
			strncpy(rule_bin->from_label, tokens[i + 1],
			    MAC_LABEL_MAX - 1);
			rule_bin->from_label[MAC_LABEL_MAX - 1] = '\0';
		} else if (strcmp(tokens[i], "to") == 0) {
			strncpy(rule_bin->to_label, tokens[i + 1],
			    MAC_LABEL_MAX - 1);
			rule_bin->to_label[MAC_LABEL_MAX - 1] = '\0';
		} else {
			fprintf(stderr, "mac-compile: unexpected keyword "
			    "'%s' in rule\n", tokens[i]);
			return -1;
		}
	}

	return 0;
}

/*===========================================================================*
 *                    Compile all rules to binary output                    *
 *===========================================================================*/
static int compile_file(FILE *in_fp, FILE *out_fp, int verbose)
{
	char line[256];
	char *trimmed;
	char *tokens[16];
	int num_tokens;
	int num_rules;
	int written = 0;
	struct mac_policy_header header;
	struct mac_rule_bin rule_bin;

	/* First pass: validate and count rules. */
	num_rules = count_and_validate(in_fp, verbose);
	if (num_rules < 0)
		return -1;

	/* Write header. */
	memset(&header, 0, sizeof(header));
	header.magic = MAC_BIN_MAGIC;
	header.version = MAC_BIN_VERSION;
	header.num_rules = (uint32_t)num_rules;

	if (fwrite(&header, sizeof(header), 1, out_fp) != 1) {
		fprintf(stderr, "mac-compile: write error: %s\n",
		    strerror(errno));
		return -1;
	}

	/* Second pass: compile each rule. */
	while (fgets(line, sizeof(line), in_fp) != NULL) {
		trimmed = trim_ws(line);

		if (trimmed[0] == '\0' || trimmed[0] == '#')
			continue;

		num_tokens = tokenize(trimmed, tokens, 16);
		if (num_tokens < 2)
			continue;

		if (compile_rule(tokens, num_tokens, &rule_bin) != 0)
			continue;

		if (fwrite(&rule_bin, sizeof(rule_bin), 1, out_fp) != 1) {
			fprintf(stderr, "mac-compile: write error: %s\n",
			    strerror(errno));
			return -1;
		}
		written++;
	}

	if (verbose) {
		fprintf(stderr, "mac-compile: wrote %d rule(s) to output "
		    "(%zu bytes)\n", written,
		    sizeof(header) + (size_t)written * sizeof(rule_bin));
	}

	return 0;
}

/*===========================================================================*
 *                    Usage                                                 *
 *===========================================================================*/
static void usage(void)
{
	fprintf(stderr,
	    "Usage: mac-compile [-o output] [-v] [input_file]\n"
	    "Compile MAC policy rules into binary format.\n"
	    "\n"
	    "If input_file is omitted, reads from stdin.\n"
	    "If -o is omitted, writes to stdout.\n"
	    "\n"
	    "Options:\n"
	    "  -o FILE   Write output to FILE instead of stdout\n"
	    "  -v        Verbose: print rule count summary\n"
	    "  -h        Show this help\n");
}

/*===========================================================================*
 *                    Main                                                  *
 *===========================================================================*/
int main(int argc, char *argv[])
{
	const char *progname = argv[0];
	const char *out_path = NULL;
	int opt;
	int verbose = 0;
	FILE *in_fp = stdin;
	FILE *out_fp = stdout;
	int exit_code = 0;

	while ((opt = getopt(argc, argv, "o:vh")) != -1) {
		switch (opt) {
		case 'o':
			out_path = optarg;
			break;
		case 'v':
			verbose = 1;
			break;
		case 'h':
			usage();
			return 0;
		default:
			usage();
			return 1;
		}
	}

	/* Open input file if specified. */
	if (optind < argc) {
		in_fp = fopen(argv[optind], "r");
		if (in_fp == NULL) {
			fprintf(stderr, "%s: cannot open %s: %s\n",
			    progname, argv[optind], strerror(errno));
			return 1;
		}
	}

	/* Open output file if specified. */
	if (out_path != NULL) {
		out_fp = fopen(out_path, "wb");
		if (out_fp == NULL) {
			fprintf(stderr, "%s: cannot open %s: %s\n",
			    progname, out_path, strerror(errno));
			if (in_fp != stdin)
				fclose(in_fp);
			return 1;
		}
	}

	/* Compile. */
	if (compile_file(in_fp, out_fp, verbose) != 0)
		exit_code = 1;

	/* Cleanup. */
	if (in_fp != stdin)
		fclose(in_fp);
	if (out_fp != stdout)
		fclose(out_fp);

	return exit_code;
}
