/* capsh — capability shell utility.
 *
 * View and modify process capabilities from the command line.
 *
 * Usage:
 *   capsh --get            Show effective capabilities
 *   capsh --bound          Show bounding set
 *   capsh --set=CAP_LIST   Set capabilities (comma-separated names)
 *   capsh --list           List all known capabilities
 *   capsh --decode=HEX     Decode a hex mask to capability names
 *   capsh --help           Show usage
 */

#include <sys/capability.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <err.h>
#include <getopt.h>

static const struct {
	const char *name;
	cap_t bit;
} cap_names[] = {
	{ "SYS_RAWIO",		CAP_SYS_RAWIO		},
	{ "NET_RAW",		CAP_NET_RAW		},
	{ "NET_BIND",		CAP_NET_BIND		},
	{ "NET_ADMIN",		CAP_NET_ADMIN		},
	{ "SYS_ADMIN",		CAP_SYS_ADMIN		},
	{ "SYS_BOOT",		CAP_SYS_BOOT		},
	{ "IPC_OWNER",		CAP_IPC_OWNER		},
	{ "FS_MOUNT",		CAP_FS_MOUNT		},
	{ "FS_CHOWN",		CAP_FS_CHOWN		},
	{ "FS_DAC_OVERRIDE",	CAP_FS_DAC_OVERRIDE	},
	{ "VM_MAP",		CAP_VM_MAP		},
	{ "IRQ_ALLOC",		CAP_IRQ_ALLOC		},
	{ "PCI_ACCESS",		CAP_PCI_ACCESS		},
	{ NULL,			0			},
};

static void
print_cap_mask(const char *label, cap_t caps)
{
	int i, first = 1;

	printf("%-12s 0x%04llx  (", label, (unsigned long long)caps);
	for (i = 0; cap_names[i].name != NULL; i++) {
		if (caps & cap_names[i].bit) {
			if (!first) printf(",");
			printf("%s", cap_names[i].name);
			first = 0;
		}
	}
	if (first) printf("(none)");
	printf(")\n");
}

static void
cmd_get(void)
{
	cap_t caps;
	if (cap_get_proc(&caps) != 0)
		err(EXIT_FAILURE, "cap_get_proc");
	print_cap_mask("effective", caps);
}

static void
cmd_bound(void)
{
	cap_t caps;
	if (cap_get_bound(&caps) != 0)
		err(EXIT_FAILURE, "cap_get_bound");
	print_cap_mask("bounding", caps);
}

static void
cmd_set(const char *arg)
{
	cap_t caps = 0;
	char *buf, *tok, *save;

	buf = strdup(arg);
	if (!buf) err(EXIT_FAILURE, "strdup");

	tok = strtok_r(buf, ",", &save);
	while (tok != NULL) {
		int i;
		for (i = 0; cap_names[i].name != NULL; i++) {
			if (strcasecmp(tok, cap_names[i].name) == 0) {
				caps |= cap_names[i].bit;
				break;
			}
		}
		if (cap_names[i].name == NULL) {
			/* Try parsing as hex number */
			char *end;
			unsigned long long v = strtoull(tok, &end, 0);
			if (*end == '\0') {
				caps |= (cap_t)v;
			} else {
				warnx("unknown capability '%s', ignoring", tok);
			}
		}
		tok = strtok_r(NULL, ",", &save);
	}
	free(buf);

	if (cap_set_proc(caps) != 0)
		err(EXIT_FAILURE, "cap_set_proc");

	printf("capabilities set: 0x%04llx\n", (unsigned long long)caps);
}

static void
cmd_list(void)
{
	int i;
	printf("Known capabilities (%d total):\n", CAP_MAX);
	printf("  %-20s  %-6s  %s\n", "Name", "Bit", "Mask");
	printf("  %-20s  %-6s  %s\n", "----", "---", "----");
	for (i = 0; cap_names[i].name != NULL; i++) {
		printf("  %-20s  bit %-2d 0x%04llx\n",
		    cap_names[i].name, i,
		    (unsigned long long)cap_names[i].bit);
	}
	printf("\nPredefined sets:\n");
	printf("  %-20s  0x%04llx\n", "CAP_BASE", (unsigned long long)CAP_BASE);
	printf("  %-20s  0x%04llx\n", "CAP_SYSTEM", (unsigned long long)CAP_SYSTEM);
	printf("  %-20s  0x%04llx\n", "CAP_DRIVER", (unsigned long long)CAP_DRIVER);
	printf("  %-20s  0x%04llx\n", "CAP_NETWORK", (unsigned long long)CAP_NETWORK);
	printf("  %-20s  0x%04llx\n", "CAP_ADMIN", (unsigned long long)CAP_ADMIN);
	printf("  %-20s  0x%04llx\n", "CAP_FULL", (unsigned long long)CAP_FULL);
}

static void
cmd_decode(const char *arg)
{
	char *end;
	unsigned long long v = strtoull(arg, &end, 0);
	if (*end != '\0')
		errx(EXIT_FAILURE, "invalid hex number: '%s'", arg);
	print_cap_mask("decode", (cap_t)v);
}

static void
usage(void)
{
	fprintf(stderr,
	    "usage: capsh --get\n"
	    "       capsh --bound\n"
	    "       capsh --set=CAP_LIST\n"
	    "       capsh --list\n"
	    "       capsh --decode=HEX\n"
	    "       capsh --help\n"
	    "\n"
	    "CAP_LIST is a comma-separated list of capability names\n"
	    "(e.g., IPC_OWNER,NET_BIND) or a hex mask.\n"
	    "Use --list to see all available capabilities.\n");
	exit(EXIT_FAILURE);
}

int
main(int argc, char **argv)
{
	static struct option long_opts[] = {
		{ "get",	no_argument,		0, 'g' },
		{ "bound",	no_argument,		0, 'b' },
		{ "set",	required_argument,	0, 's' },
		{ "list",	no_argument,		0, 'l' },
		{ "decode",	required_argument,	0, 'd' },
		{ "help",	no_argument,		0, 'h' },
		{ NULL,		0,			0, 0   },
	};
	int c;
	int cmd_count = 0;

	while ((c = getopt_long(argc, argv, "gbs:ld:h", long_opts, NULL)) != -1) {
		switch (c) {
		case 'g':
			cmd_get();
			cmd_count++;
			break;
		case 'b':
			cmd_bound();
			cmd_count++;
			break;
		case 's':
			cmd_set(optarg);
			cmd_count++;
			break;
		case 'l':
			cmd_list();
			cmd_count++;
			break;
		case 'd':
			cmd_decode(optarg);
			cmd_count++;
			break;
		case 'h':
		default:
			usage();
			/* NOTREACHED */
		}
	}

	if (cmd_count == 0)
		usage();

	return 0;
}
