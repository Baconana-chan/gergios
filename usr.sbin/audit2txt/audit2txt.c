/* audit2txt — Audit log formatter for GergiOS.
 *
 * Phase 5.6: Converts auditd log files (pipe-delimited text format) to
 * human-readable formatted output.
 *
 * Log format (input):
 *   serial|timestamp_ticks|type_name|result|subject|object|extra_hex
 *
 * Output format:
 *   [HH:MM:SS.mmm]  TYPE           subj=X  obj=Y  result=NAME   [extra]
 *
 * Usage:
 *   audit2txt [file ...]           Read and format audit log(s)
 *   audit2txt -f [file]            Follow mode (tail -f like)
 *   audit2txt -t TYPE [file ...]   Filter by event type
 *   audit2txt -p ENDPOINT [...]    Filter by process endpoint
 *   audit2txt -h                   Show help
 *
 * Without a file argument, reads from stdin.
 */

#include <sys/types.h>
#include <sys/stat.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <time.h>
#include <ctype.h>
#include <signal.h>

/* Program name for error messages. */
static const char *progname;

/* Default HZ value if we cannot determine it at runtime. */
#define DEFAULT_HZ      100

/*===========================================================================*
 *                    Result code to string                                 *
 *===========================================================================*/
static const char *result_name(int result)
{
	switch (result) {
	case 0:             return "OK";
	case 1:             return "EPERM";
	case 2:             return "ENOENT";
	case 4:             return "EINTR";
	case 5:             return "EIO";
	case 6:             return "ENXIO";
	case 11:            return "EAGAIN";
	case 12:            return "ENOMEM";
	case 13:            return "EACCES";
	case 14:            return "EFAULT";
	case 15:            return "ENOTBLK";
	case 16:            return "EBUSY";
	case 17:            return "EEXIST";
	case 22:            return "EINVAL";
	case 23:            return "ENFILE";
	case 24:            return "EMFILE";
	case 27:            return "EFBIG";
	case 28:            return "ENOSPC";
	case 30:            return "EROFS";
	case 31:            return "EMLINK";
	case 34:            return "ERANGE";
	case 38:            return "ENOSYS";
	case 40:            return "ENOTSUP";
	case 41:            return "EOPNOTSUPP";
	case 114:           return "EDEADEPT";
	case 115:           return "EDEADSRCDST";
	default:
		if (result < 0)
			return "NEG_ERR";
		return "ERR_UNK";
	}
}

/*===========================================================================*
 *                    Format timestamp (ticks -> wall/relative)             *
 *===========================================================================*/
static void format_timestamp(unsigned long long ticks, char *buf, size_t len)
{
	unsigned long long ms_total;
	unsigned long h, m, s, ms;

	/* Convert ticks to milliseconds, then to HH:MM:SS.mmm.
	 * Using DEFAULT_HZ since we may not have sys_hz() in a standalone tool. */
	ms_total = (ticks * 1000ULL) / DEFAULT_HZ;
	h = (unsigned long)(ms_total / 3600000ULL);
	m = (unsigned long)((ms_total / 60000ULL) % 60);
	s = (unsigned long)((ms_total / 1000ULL) % 60);
	ms = (unsigned long)(ms_total % 1000);

	snprintf(buf, len, "[%02lu:%02lu:%02lu.%03lu]", h, m, s, ms);
}

/*===========================================================================*
 *                    Parse and format one line                             *
 *===========================================================================*
 * Returns 1 if the line was printed, 0 if filtered out, -1 on parse error.
 */
static int process_line(const char *line,
    const char *filter_type, int filter_endpoint)
{
	unsigned long serial;
	unsigned long long ticks;
	char type_name[32];
	int result;
	int subject;
	int object;
	char extra[128];
	int n;
	char ts_buf[24];
	const char *result_str;

	/* Skip comment lines (# ROTATE, etc.). */
	if (line[0] == '#')
		return 0;

	/* Parse: serial|ticks|type|result|subject|object|extra */
	extra[0] = '\0';
	n = sscanf(line, "%lu|%llu|%31s|%d|%d|%d|%127s",
	    &serial, &ticks, type_name, &result, &subject, &object, extra);

	if (n < 6) {
		/* Malformed line — skip silently. */
		return -1;
	}

	/* Apply filters. */
	if (filter_type != NULL && strcmp(type_name, filter_type) != 0)
		return 0;

	if (filter_endpoint >= 0 &&
	    subject != filter_endpoint && object != filter_endpoint)
		return 0;

	/* Format timestamp. */
	format_timestamp(ticks, ts_buf, sizeof(ts_buf));

	/* Format result. */
	result_str = result_name(result);

	/* Print formatted line.
	 * Format:  [HH:MM:SS.mmm]  TYPE          subj=XX  obj=YY  result=NAME  extra */
	printf("%-16s  %-14s  subj=%-6d  obj=%-6d  %s",
	    ts_buf, type_name, subject, object, result_str);

	/* Append extra data if present. */
	if (n >= 7 && extra[0] != '\0' && extra[0] != '-')
		printf("  %s", extra);

	putchar('\n');

	return 1;
}

/*===========================================================================*
 *                    Read and process a file                               *
 *===========================================================================*/
static int process_file(FILE *fp, int follow_mode,
    const char *filter_type, int filter_endpoint)
{
	char line[512];
	long last_pos = 0;
	int count = 0;
	int r;

	if (follow_mode) {
		/* Tail-follow mode: read existing content, then wait for new data. */
		while (fgets(line, sizeof(line), fp) != NULL) {
			r = process_line(line,
			    filter_type, filter_endpoint);
			if (r > 0)
				count++;
		}
		last_pos = ftell(fp);

		for (;;) {
			/* Sleep 1 second between polls. */
			sleep(1);
			clearerr(fp);

			while (fgets(line, sizeof(line), fp) != NULL) {
				r = process_line(line, follow_mode,
				    filter_type, filter_endpoint);
				if (r > 0)
					count++;
			}

			/* Check for file rotation: if file was truncated or
			 * renamed and a new one created, its inode may change.
			 * We detect this by checking if ftell moved backwards
			 * (e.g., file was truncated). */
			if (ferror(fp)) {
				/* File may have been rotated. Try reopening. */
				clearerr(fp);
				break;
			}

			/* If the file shrank, we may be reading a new file.
			 * Reset to beginning. */
			if (ftell(fp) < last_pos) {
				fseek(fp, 0, SEEK_SET);
			}
			last_pos = ftell(fp);
		}
	} else {
		/* Normal mode: read entire file. */
		while (fgets(line, sizeof(line), fp) != NULL) {
			r = process_line(line,
			    filter_type, filter_endpoint);
			if (r > 0)
				count++;
		}
	}

	return count;
}

/*===========================================================================*
 *                    Usage                                                 *
 *===========================================================================*/
static void usage(void)
{
	fprintf(stderr,
	    "Usage: %s [options] [file ...]\n"
	    "Read and format audit log files.\n"
	    "Without a file argument, reads from stdin.\n"
	    "\n"
	    "Options:\n"
	    "  -f          Follow mode (keep reading, like tail -f)\n"
	    "  -t TYPE     Show only events of TYPE (AUTH_FAILURE, IPC_DENIED, etc.)\n"
	    "  -p ENDPOINT Show only events involving ENDPOINT (subject or object)\n"
	    "  -h          Show this help\n",
	    progname);
}

/*===========================================================================*
 *                    Main                                                  *
 *===========================================================================*/
int main(int argc, char *argv[])
{
	int opt;
	int follow_mode = 0;
	const char *filter_type = NULL;
	int filter_endpoint = -1;
	int exit_code = 0;
	int i;

	progname = argv[0];

	/* Parse options. */
	while ((opt = getopt(argc, argv, "ft:p:h")) != -1) {
		switch (opt) {
		case 'f':
			follow_mode = 1;
			break;

		case 't':
			filter_type = optarg;
			break;

		case 'p':
			filter_endpoint = atoi(optarg);
			break;

		case 'h':
			usage();
			return 0;

		default:
			usage();
			return 1;
		}
	}

	/* Process files or stdin. */
	if (optind < argc) {
		/* One or more files specified. */
		for (i = optind; i < argc; i++) {
			FILE *fp;

			if (strcmp(argv[i], "-") == 0) {
				fp = stdin;
			} else {
				fp = fopen(argv[i], "r");
				if (fp == NULL) {
					fprintf(stderr, "%s: cannot open %s: %s\n",
					    progname, argv[i], strerror(errno));
					exit_code = 1;
					continue;
				}
			}

			if (process_file(fp, follow_mode,
			    filter_type, filter_endpoint) < 0)
				exit_code = 1;

			if (fp != stdin)
				fclose(fp);
		}
	} else {
		/* Read from stdin. */
		process_file(stdin, follow_mode, filter_type, filter_endpoint);
	}

	return exit_code;
}
