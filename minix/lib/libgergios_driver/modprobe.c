/* modprobe.c — GergiOS Binding Policy Engine Implementation
 *
 * Implements:
 *   - Simple JSON config file parser (no external dependencies)
 *   - PCI alias generation, parsing, and wildcard matching
 *   - Config loading from /etc/gergios/modprobe.d/*.json
 *   - modprobe_by_name() — started driver by name
 *   - modprobe_by_device() — auto-match and start for hot-plug
 *   - modprobe_insmod() — direct .ko loading via ELF loader + kernel shim
 *   - Integration with existing hotplug RS_UP infrastructure
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <ctype.h>
#include <minix/drivers.h>
#include <minix/rs.h>
#include <minix/endpoint.h>

#include "modprobe.h"
#include "elf_loader.h"
#include "kernel_shim.h"
#include "hotplug.h"
#include "drvmanager.h"

/*===========================================================================*
 *		Internal state                                           *
 *===========================================================================*/

static struct modprobe_config g_config;
static int g_initialised = 0;

/*===========================================================================*
 *		Internal helpers — hex parsing/writing                  *
 *===========================================================================*/

static int hex_val(char c)
{
	if (c >= '0' && c <= '9') return c - '0';
	if (c >= 'a' && c <= 'f') return c - 'a' + 10;
	if (c >= 'A' && c <= 'F') return c - 'A' + 10;
	return -1;
}

static char hex_char(unsigned int v)
{
	v &= 0x0F;
	return v < 10 ? '0' + v : 'A' + v - 10;
}

static int hex_parse_4(const char *s, uint16_t *out)
{
	int v = 0;
	for (int i = 0; i < 4; i++) {
		int h = hex_val(s[i]);
		if (h < 0) return -EINVAL;
		v = (v << 4) | h;
	}
	*out = (uint16_t)v;
	return 0;
}

static int hex_parse_2(const char *s, uint8_t *out)
{
	int h1 = hex_val(s[0]);
	int h2 = hex_val(s[1]);
	if (h1 < 0 || h2 < 0) return -EINVAL;
	*out = (uint8_t)((h1 << 4) | h2);
	return 0;
}

/*===========================================================================*
 *		PCI alias parsing (Linux format)                        *
 *===========================================================================*
 *
 * Format: pci:v0000VVVVd0000DDDDsv0000SSSSsd0000ssssbc00BBsc00SSi00PP
 *
 * Components are identified by 2-char prefixes:
 *   v=   vendor ID
 *   d=   device ID
 *   sv=  subsystem vendor
 *   sd=  subsystem device
 *   bc=  base class
 *   sc=  subclass
 *   i=   programming interface
 *
 * Wildcard: a component value of "*" or missing component = wildcard (0xFFFF)
 */

int modprobe_alias_parse(const char *alias, struct gergios_device_id *id)
{
	const char *p;
	uint16_t tmp16;
	uint8_t tmp8;

	if (!alias || !id) return -EINVAL;

	/* Initialise all fields to wildcard */
	memset(id, 0xFF, sizeof(*id));
	id->class = 0xFFFFFFFF;

	/* Must start with "pci:" */
	if (strncmp(alias, "pci:", 4) != 0)
		return -EINVAL;
	p = alias + 4;

	while (*p) {
		if (strncmp(p, "v", 1) == 0 && hex_val(p[1]) >= 0) {
			/* Vendor ID: v0000VVVV */
			if (hex_parse_4(p + 1, &tmp16) == 0)
				id->vendor = tmp16;
			p += 5;
		} else if (strncmp(p, "d", 1) == 0 && hex_val(p[1]) >= 0) {
			/* Device ID: d0000DDDD */
			if (hex_parse_4(p + 1, &tmp16) == 0)
				id->device = tmp16;
			p += 5;
		} else if (strncmp(p, "sv", 2) == 0 && hex_val(p[2]) >= 0) {
			/* Subvendor: sv0000SSSS */
			if (hex_parse_4(p + 2, &tmp16) == 0)
				id->subvendor = tmp16;
			p += 6;
		} else if (strncmp(p, "sd", 2) == 0 && hex_val(p[2]) >= 0) {
			/* Subdevice: sd0000ssss */
			if (hex_parse_4(p + 2, &tmp16) == 0)
				id->subdevice = tmp16;
			p += 6;
		} else if (strncmp(p, "bc", 2) == 0 && hex_val(p[2]) >= 0) {
			/* Base class: bc00BB */
			if (hex_parse_2(p + 4, &tmp8) == 0)
				id->class = (id->class & 0x00FFFFFF) | ((uint32_t)tmp8 << 16);
			p += 6;
		} else if (strncmp(p, "sc", 2) == 0 && hex_val(p[2]) >= 0) {
			/* Subclass: sc00SS */
			if (hex_parse_2(p + 4, &tmp8) == 0)
				id->class = (id->class & 0xFF00FFFF) | ((uint32_t)tmp8 << 8);
			p += 6;
		} else if (strncmp(p, "i", 1) == 0 && hex_val(p[1]) >= 0) {
			/* Programming interface: i00PP */
			if (hex_parse_2(p + 3, &tmp8) == 0)
				id->class = (id->class & 0xFFFF00FF) | (uint32_t)tmp8;
			p += 5;
		} else {
			/* Unknown component or wildcard — skip to next */
			p++;
		}
	}

	return 0;
}

/*===========================================================================*
 *		PCI alias generation                                      *
 *===========================================================================*/

int modprobe_alias_generate(char *out, size_t outsize,
    uint16_t vendor, uint16_t device,
    uint16_t subvendor, uint16_t subdevice,
    uint32_t class_code)
{
	int n;

	if (!out || outsize == 0) return -EINVAL;

	n = snprintf(out, outsize, "pci:v%04Xd%04Xsv%04Xsd%04Xbc%02Xsc%02Xi%02X",
	    vendor, device, subvendor, subdevice,
	    (class_code >> 16) & 0xFF,
	    (class_code >> 8) & 0xFF,
	    class_code & 0xFF);

	if ((size_t)n >= outsize) return -ENOSPC;
	return n;
}

int modprobe_alias_from_id(char *out, size_t outsize,
    const struct gergios_device_id *id)
{
	int n;

	if (!out || !id || outsize == 0) return -EINVAL;

	n = snprintf(out, outsize, "pci:v%04Xd%04Xsv%04Xsd%04Xbc%02Xsc%02Xi%02X",
	    id->vendor & 0xFFFF,
	    id->device & 0xFFFF,
	    id->subvendor & 0xFFFF,
	    id->subdevice & 0xFFFF,
	    (id->class >> 16) & 0xFF,
	    (id->class >> 8) & 0xFF,
	    id->class & 0xFF);

	if ((size_t)n >= outsize) return -ENOSPC;
	return n;
}

/*===========================================================================*
 *		PCI alias matching (wildcard-aware)                     *
 *===========================================================================*
 *
 * Matching rules (following Linux modprobe conventions):
 *   - Pattern "*" matches any alias component
 *   - Pattern pci:v0000* matches any vendor + anything else
 *   - Exact match: all non-wildcard fields must match exactly
 *   - Most specific match wins (fewest wildcards = highest specificity)
 */

/* Count the number of wildcard components in an alias pattern.
 * Lower number = more specific. */
static int alias_wildcard_count(const char *pattern)
{
	int count = 0;
	const char *p = pattern;

	/* Must start with pci: */
	if (strncmp(p, "pci:", 4) != 0) return 999;
	p += 4;

	while (*p) {
		if (*p == '*' || *p == '?') {
			count++;
			p++;
		} else if (strncmp(p, "v*", 2) == 0 ||
		           strncmp(p, "d*", 2) == 0 ||
		           strncmp(p, "sv*", 3) == 0 ||
		           strncmp(p, "sd*", 3) == 0 ||
		           strncmp(p, "bc*", 3) == 0 ||
		           strncmp(p, "sc*", 3) == 0 ||
		           strncmp(p, "i*", 2) == 0) {
			count++;
			/* Skip to next component */
			while (*p && *p != 'v' && *p != 'd' &&
			       !(p[0] == 's' && (p[1] == 'v' || p[1] == 'd')) &&
			       !(p[0] == 'b' && p[1] == 'c') &&
			       !(p[0] == 's' && p[1] == 'c') &&
			       *p != 'i')
				p++;
		} else {
			p++;
		}
	}
	return count;
}

int modprobe_alias_match(const char *alias, const char *pattern)
{
	if (!alias || !pattern) return 0;

	/* Quick check: pattern "*" matches everything */
	if (strcmp(pattern, "*") == 0) return 1;
	if (strcmp(alias, pattern) == 0) return 1;

	/* Parse both into device IDs and compare field by field */
	struct gergios_device_id alias_id, pattern_id;

	if (modprobe_alias_parse(alias, &alias_id) != 0) return 0;
	if (modprobe_alias_parse(pattern, &pattern_id) != 0) return 0;

	/* Check each field: pattern field = 0xFFFF means wildcard */
	if (pattern_id.vendor != 0xFFFF && pattern_id.vendor != alias_id.vendor)
		return 0;
	if (pattern_id.device != 0xFFFF && pattern_id.device != alias_id.device)
		return 0;
	if (pattern_id.subvendor != 0xFFFF && pattern_id.subvendor != alias_id.subvendor)
		return 0;
	if (pattern_id.subdevice != 0xFFFF && pattern_id.subdevice != alias_id.subdevice)
		return 0;
	if (pattern_id.class != 0xFFFFFFFF) {
		uint32_t mask = 0xFFFFFFFF;
		/* For class, only compare fields that are not wildcard */
		if ((pattern_id.class & 0xFF0000) != 0xFF0000)
			mask &= 0xFF0000;
		if ((pattern_id.class & 0x00FF00) != 0x00FF00)
			mask &= 0x00FF00;
		if ((pattern_id.class & 0x0000FF) != 0x0000FF)
			mask &= 0x0000FF;
		if ((pattern_id.class & mask) != (alias_id.class & mask))
			return 0;
	}

	return 1;
}

/* Compare two entries for sort-by-specificity (qsort callback).
 * Returns negative if a is more specific, positive if b is. */
static int entry_cmp_specificity(const void *a, const void *b)
{
	const struct modprobe_entry *ea = (const struct modprobe_entry *)a;
	const struct modprobe_entry *eb = (const struct modprobe_entry *)b;
	int wa, wb;

	/* Invalid entries go last */
	if (!ea->valid && eb->valid) return 1;
	if (ea->valid && !eb->valid) return -1;
	if (!ea->valid && !eb->valid) return 0;

	wa = alias_wildcard_count(ea->alias_pattern);
	wb = alias_wildcard_count(eb->alias_pattern);

	if (wa != wb) return wa - wb;

	/* Same specificity — prefer exact vendor/device match */
	if (ea->id.vendor != 0xFFFF && eb->id.vendor == 0xFFFF) return -1;
	if (ea->id.vendor == 0xFFFF && eb->id.vendor != 0xFFFF) return 1;
	if (ea->id.device != 0xFFFF && eb->id.device == 0xFFFF) return -1;
	if (ea->id.device == 0xFFFF && eb->id.device != 0xFFFF) return 1;

	return strcmp(ea->driver_name, eb->driver_name);
}

/*===========================================================================*
 *		Simple JSON parser                                        *
 *===========================================================================*
 *
 * Parses the modprobe config format:
 *   {
 *     "modprobe": [
 *       { "alias": "pci:...", "driver": "name", "path": "/sbin/name",
 *         "options": "opt1=val1 opt2=val2" },
 *       ...
 *     ]
 *   }
 * The parser is minimal: it only understands this specific structure.
 * Uses pointer-based tokenization (no malloc overhead for parsing).
 */

/* Skip whitespace */
static const char *json_skip_ws(const char *p)
{
	if (!p) return NULL;
	while (*p && (unsigned char)*p <= 32) p++;
	return *p ? p : NULL;
}

/* Expect a specific character, skip whitespace, return next position */
static const char *json_expect(const char *p, char c)
{
	p = json_skip_ws(p);
	if (!p || *p != c) return NULL;
	return p + 1;
}

/* Parse a JSON string (quoted, no escapes supported).
 * Returns pointer after closing quote, writes to out if non-NULL. */
static const char *json_parse_string(const char *p, char *out, size_t outsize)
{
	size_t i = 0;

	p = json_skip_ws(p);
	if (!p || *p != '"') return NULL;
	p++;  /* skip opening quote */

	while (*p && *p != '"' && i + 1 < outsize) {
		/* Handle escape sequences (just \" and \\ for simplicity) */
		if (*p == '\\') {
			p++;
			if (*p == '"' || *p == '\\')
				out[i++] = *p;
			else if (*p == 'n')
				out[i++] = '\n';
			else if (*p == 't')
				out[i++] = '\t';
			else
				out[i++] = '\\';  /* preserve unknown escape */
			p++;
		} else {
			out[i++] = *p++;
		}
	}

	if (*p != '"') return NULL;
	out[i] = '\0';
	return p + 1;  /* skip closing quote */
}

/* Parse a single modprobe entry object.
 *   { "alias": "...", "driver": "...", "path": "...", "options": "..." }
 */
static const char *json_parse_entry(const char *p,
    struct modprobe_entry *entry)
{
	char key[64];
	char val[MODPROBE_PATH_MAX];

	memset(entry, 0, sizeof(*entry));

	/* Expect opening brace */
	p = json_expect(p, '{');
	if (!p) return NULL;

	/* Parse key-value pairs */
	while (1) {
		/* Skip whitespace/comma */
		p = json_skip_ws(p);
		if (!p) return NULL;

		/* Check for closing brace */
		if (*p == '}') return p + 1;

		/* Parse key string */
		p = json_parse_string(p, key, sizeof(key));
		if (!p) return NULL;

		/* Expect colon */
		p = json_expect(p, ':');
		if (!p) return NULL;

		/* Expect comma or continue after value */
		p = json_skip_ws(p);
		if (!p) return NULL;

		if (*p == '"') {
			p = json_parse_string(p, val, sizeof(val));
			if (!p) return NULL;

			if (strcmp(key, "alias") == 0) {
				strncpy(entry->alias_pattern, val,
				    sizeof(entry->alias_pattern) - 1);
			} else if (strcmp(key, "driver") == 0) {
				strncpy(entry->driver_name, val,
				    sizeof(entry->driver_name) - 1);
			} else if (strcmp(key, "path") == 0) {
				strncpy(entry->driver_path, val,
				    sizeof(entry->driver_path) - 1);
			} else if (strcmp(key, "options") == 0) {
				strncpy(entry->options, val,
				    sizeof(entry->options) - 1);
			}
		} else {
			/* Skip unexpected value type */
			while (*p && *p != ',' && *p != '}' && *p != '\n') p++;
		}

		/* Expect comma or closing brace */
		p = json_skip_ws(p);
		if (!p) return NULL;
		if (*p == '}') return p + 1;
		if (*p != ',') return NULL;
		p++;
	}
}

/* Parse the top-level "modprobe" array:
 *   { "modprobe": [ ... ] }
 */
static int json_parse_modprobe_file(const char *data, size_t size)
{
	const char *p = data;
	const char *end = data + size;
	char key[64];
	int count = 0;

	/* Expect opening brace */
	p = json_expect(p, '{');
	if (!p) return -EINVAL;

	while (p && p < end) {
		p = json_skip_ws(p);
		if (!p || *p == '}') break;

		/* Parse key */
		p = json_parse_string(p, key, sizeof(key));
		if (!p) return -EINVAL;

		/* Expect colon */
		p = json_expect(p, ':');
		if (!p) return -EINVAL;

		if (strcmp(key, "modprobe") == 0) {
			/* Expect array */
			p = json_expect(p, '[');
			if (!p) return -EINVAL;

			while (1) {
				p = json_skip_ws(p);
				if (!p || *p == ']') break;

				if (g_config.num_entries >= MODPROBE_MAX_ENTRIES) {
					printf("modprobe: too many entries (%d max)\n",
					    MODPROBE_MAX_ENTRIES);
					break;
				}

				struct modprobe_entry *entry =
				    &g_config.entries[g_config.num_entries];

				p = json_parse_entry(p, entry);
				if (!p) {
					printf("modprobe: failed to parse entry at offset %td\n",
					    (ptrdiff_t)(p ? p - data : 0));
					break;
				}

				/* Validate and post-process entry */
				if (entry->alias_pattern[0] && entry->driver_name[0]) {
					/* Default path if not specified: /sbin/<driver> */
					if (!entry->driver_path[0]) {
						snprintf(entry->driver_path,
						    sizeof(entry->driver_path),
						    "/sbin/%s", entry->driver_name);
					}

					/* Parse alias into device ID */
					if (modprobe_alias_parse(entry->alias_pattern,
					    &entry->id) == 0) {
						entry->valid = 1;
						g_config.num_entries++;
						count++;
					} else {
						printf("modprobe: warning — "
						    "invalid alias '%s' for '%s'\n",
						    entry->alias_pattern,
						    entry->driver_name);
					}
				}

				/* Expect comma or closing bracket */
				p = json_skip_ws(p);
				if (p && *p == ',') p++;
			}

			/* Expect closing bracket */
			p = json_expect(p, ']');
			if (!p) return -EINVAL;
		} else {
			/* Skip unknown key's value */
			while (p && p < end && *p != ',' && *p != '}') p++;
			if (p && *p == ',') p++;
		}
	}

	/* Sort by specificity (most specific first) */
	if (count > 1) {
		qsort(g_config.entries, g_config.num_entries,
		    sizeof(struct modprobe_entry), entry_cmp_specificity);
	}

	return count;
}

/*===========================================================================*
 *		Config file loading                                     *
 *===========================================================================*/

int modprobe_load_file(const char *path)
{
	int fd;
	struct stat st;
	char *data;
	int ret;

	fd = open(path, O_RDONLY);
	if (fd < 0) {
		/* File not found is not an error — just means no config */
		return 0;
	}

	if (fstat(fd, &st) < 0) {
		close(fd);
		return -errno;
	}

	if (st.st_size == 0) {
		close(fd);
		return 0;
	}

	data = malloc(st.st_size + 1);
	if (!data) {
		close(fd);
		return -ENOMEM;
	}

	if (read(fd, data, st.st_size) != st.st_size) {
		close(fd);
		free(data);
		return -EIO;
	}
	close(fd);
	data[st.st_size] = '\0';

	printf("modprobe: loading config from '%s' (%ld bytes)\n",
	    path, (long)st.st_size);

	ret = json_parse_modprobe_file(data, st.st_size);
	free(data);

	if (ret < 0)
		printf("modprobe: error parsing '%s': %d\n", path, ret);
	else
		printf("modprobe: loaded %d entries from '%s'\n", ret, path);

	return ret >= 0 ? 0 : ret;
}

int modprobe_load_dir(const char *dirpath)
{
	DIR *dir;
	struct dirent *dent;
	int count = 0;

	dir = opendir(dirpath);
	if (!dir) {
		/* Directory doesn't exist — try creating it */
		if (mkdir(dirpath, 0755) == 0 || errno == EEXIST) {
			printf("modprobe: created config directory '%s'\n", dirpath);
		}
		return 0;
	}

	while ((dent = readdir(dir)) != NULL) {
		size_t len = strlen(dent->d_name);
		/* Match *.json files */
		if (len < 6 || strcmp(dent->d_name + len - 5, ".json") != 0)
			continue;

		/* Skip dotfiles */
		if (dent->d_name[0] == '.')
			continue;

		char path[MODPROBE_PATH_MAX];
		snprintf(path, sizeof(path), "%s/%s", dirpath, dent->d_name);

		if (modprobe_load_file(path) == 0)
			count++;
	}

	closedir(dir);
	return count;
}

/*===========================================================================*
 *		Module scanning from /lib/modules/gergios/              *
 *===========================================================================*
 *
 * Scans a directory for .ko files and auto-registers them as config entries.
 * This allows "modprobe e1000" to work even without explicit JSON config,
 * by finding the .ko file and inferring driver info from its .modinfo.
 */

static int scan_module_dir(const char *dirpath)
{
	DIR *dir;
	struct dirent *dent;
	int count = 0;

	dir = opendir(dirpath);
	if (!dir) return 0;

	while ((dent = readdir(dir)) != NULL) {
		size_t len = strlen(dent->d_name);
		if (len < 4 || strcmp(dent->d_name + len - 3, ".ko") != 0)
			continue;

		if (dent->d_name[0] == '.') continue;

		/* Build full path */
		char path[MODPROBE_PATH_MAX];
		snprintf(path, sizeof(path), "%s/%s", dirpath, dent->d_name);

		/* Determine if .ko or .so */
		int is_ko = (len >= 3 && strcmp(dent->d_name + len - 3, ".ko") == 0);
		int is_so = (len >= 3 && strcmp(dent->d_name + len - 3, ".so") == 0);
		if (!is_ko && !is_so) continue;
		int suffix_len = is_ko ? 3 : 3;

		/* Strip suffix for driver name */
		char name[MODPROBE_NAME_MAX];
		strncpy(name, dent->d_name, len - suffix_len);
		name[len - suffix_len] = '\0';

		/* Register as a config entry (with wildcard alias — will match
		 * any device, but most-specific config entries will win).
		 * We try to parse .modinfo later during insmod to get exact IDs. */
		if (g_config.num_entries >= MODPROBE_MAX_ENTRIES) break;

		struct modprobe_entry *entry = &g_config.entries[g_config.num_entries];
		strncpy(entry->alias_pattern, "*", sizeof(entry->alias_pattern) - 1);
		strncpy(entry->driver_name, name, sizeof(entry->driver_name) - 1);
		strncpy(entry->driver_path, path, sizeof(entry->driver_path) - 1);
		memset(&entry->id, 0xFF, sizeof(entry->id));
		entry->id.class = 0xFFFFFFFF;
		entry->valid = 1;
		g_config.num_entries++;
		count++;
	}

	closedir(dir);
	return count;
}

/*===========================================================================*
 *		modprobe API                                             *
 *===========================================================================*/

int modprobe_init(void)
{
	int total = 0;

	printf("modprobe: initialising binding policy engine\n");

	memset(&g_config, 0, sizeof(g_config));

	/* Load config files from /etc/gergios/modprobe.d/ */
	int count = modprobe_load_dir(MODPROBE_DIR_PATH);
	if (count < 0)
		printf("modprobe: warning — failed to load config dir '%s': %d\n",
		    MODPROBE_DIR_PATH, count);
	else
		total += count;

	/* Scan /lib/modules/gergios/ for .ko files */
	count = scan_module_dir(MODPROBE_DEFAULT_DIR);
	if (count > 0)
		printf("modprobe: found %d .ko files in '%s'\n",
		    count, MODPROBE_DEFAULT_DIR);
	total += count;

	/* Also scan hierarchical /lib/modules/gergios/kernel/drivers/*/ directories */
	{
		static const char *hier_dirs[] = {
		    "/lib/modules/gergios/kernel/drivers/ata",
		    "/lib/modules/gergios/kernel/drivers/nvme",
		    "/lib/modules/gergios/kernel/drivers/net",
		    "/lib/modules/gergios/kernel/drivers/block",
		    "/lib/modules/gergios/kernel/drivers/usb",
		    "/lib/modules/gergios/kernel/drivers/audio",
		    "/lib/modules/gergios/kernel/drivers/video",
		    "/lib/modules/gergios/kernel/drivers/scsi",
		    "/lib/modules/gergios/kernel/drivers/pci",
		    "/lib/modules/gergios/kernel/drivers/hid",
		    "/lib/modules/gergios/kernel/drivers/input",
		    "/lib/modules/gergios/kernel/drivers/char",
		    "/lib/modules/gergios/kernel/drivers/mmc",
		    "/lib/modules/gergios/kernel/drivers/mtd",
		    "/lib/modules/gergios/kernel/drivers/virtio",
		    "/lib/modules/gergios/kernel/drivers/extra",
		    NULL
		};
		for (int i = 0; hier_dirs[i]; i++) {
			int c = scan_module_dir(hier_dirs[i]);
			if (c > 0) {
				printf("modprobe: found %d .ko files in '%s'\n",
				    c, hier_dirs[i]);
				total += c;
			}
		}
	}

	/* Sort all entries by specificity */
	if (g_config.num_entries > 1) {
		qsort(g_config.entries, g_config.num_entries,
		    sizeof(struct modprobe_entry), entry_cmp_specificity);
	}

	g_initialised = 1;
	printf("modprobe: loaded %u total entries (%d from config, %d from scan)\n",
	    g_config.num_entries, total, count);

	return (int)g_config.num_entries;
}

/* Find the best-matching entry for a device.
 * Most specific (fewest wildcards) match wins. */
static struct modprobe_entry *find_best_match(uint16_t vendor, uint16_t device,
    uint16_t subvendor, uint16_t subdevice, uint32_t class_code)
{
	char alias[MODPROBE_ALIAS_MAX];
	struct modprobe_entry *best = NULL;
	int best_wildcards = 999;

	modprobe_alias_generate(alias, sizeof(alias),
	    vendor, device, subvendor, subdevice, class_code);

	for (unsigned int i = 0; i < g_config.num_entries; i++) {
		struct modprobe_entry *entry = &g_config.entries[i];
		if (!entry->valid) continue;

		if (modprobe_alias_match(alias, entry->alias_pattern)) {
			int wc = alias_wildcard_count(entry->alias_pattern);
			if (wc < best_wildcards) {
				best = entry;
				best_wildcards = wc;
			}
		}
	}

	return best;
}

int modprobe_by_device(const struct gergios_device *dev)
{
	if (!dev) return -EINVAL;
	if (!g_initialised) modprobe_init();

	struct modprobe_entry *entry = find_best_match(
	    dev->vendor_id, dev->device_id,
	    dev->subvendor_id, dev->subdevice_id,
	    dev->class_code);

	if (!entry) {
		printf("modprobe: no driver found for %04x:%04x (class %06x)\n",
		    dev->vendor_id, dev->device_id, dev->class_code);
		return -ENOENT;
	}

	printf("modprobe: loading driver '%s' for %04x:%04x (path=%s)\n",
	    entry->driver_name, dev->vendor_id, dev->device_id,
	    entry->driver_path);

	/* Determine whether this is a native driver or an LKM .ko */
	size_t pathlen = strlen(entry->driver_path);
	if (pathlen >= 3 &&
	    strcmp(entry->driver_path + pathlen - 3, ".ko") == 0) {
		/* It's a .ko module — use insmod path */
		return modprobe_insmod(entry->driver_path);
	} else {
		/* It's a native MINIX driver — use RS_UP */
		int devind = (int)dev->bus_address;
		int r = gergios_hotplug_rs_up(entry->driver_name,
		    entry->driver_path, devind,
		    dev->vendor_id, dev->device_id);
		/* Register with Driver Manager on success */
		if (r == 0) {
			endpoint_t ep;
			if (minix_rs_lookup(entry->driver_name, &ep) == OK)
				drvmanager_register_native(entry->driver_name,
				    entry->driver_path, (int)ep);
		}
		return r;
	}
}

int modprobe_by_alias(const char *alias)
{
	if (!alias) return -EINVAL;
	if (!g_initialised) modprobe_init();

	char alias_full[MODPROBE_ALIAS_MAX];

	/* If alias is just a PCI ID (like "v00008086d0000100E"), auto-form it */
	if (strncmp(alias, "pci:", 4) != 0 &&
	    strncmp(alias, "v", 1) == 0) {
		snprintf(alias_full, sizeof(alias_full), "pci:%s", alias);
		alias = alias_full;
	}

	/* Search for matching entry */
	struct modprobe_entry *best = NULL;
	int best_wildcards = 999;

	for (unsigned int i = 0; i < g_config.num_entries; i++) {
		struct modprobe_entry *entry = &g_config.entries[i];
		if (!entry->valid) continue;

		if (modprobe_alias_match(alias, entry->alias_pattern)) {
			int wc = alias_wildcard_count(entry->alias_pattern);
			if (wc < best_wildcards) {
				best = entry;
				best_wildcards = wc;
			}
		}
	}

	if (!best) {
		printf("modprobe: no driver found for alias '%s'\n", alias);
		return -ENOENT;
	}

	printf("modprobe: alias '%s' matched driver '%s'\n",
	    alias, best->driver_name);

	size_t pathlen = strlen(best->driver_path);
	if (pathlen >= 3 &&
	    strcmp(best->driver_path + pathlen - 3, ".ko") == 0) {
		return modprobe_insmod(best->driver_path);
	} else {
		/* Need a device to pass to RS_UP with alias-only lookup.
		 * Try to start without PCI binding. */
		int r = gergios_hotplug_rs_up(best->driver_name,
		    best->driver_path, -1, 0, 0);
		/* Register with Driver Manager on success */
		if (r == 0) {
			endpoint_t ep;
			if (minix_rs_lookup(best->driver_name, &ep) == OK)
				drvmanager_register_native(best->driver_name,
				    best->driver_path, (int)ep);
		}
		return r;
	}
}

int modprobe_by_name(const char *name)
{
	if (!name) return -EINVAL;
	if (!g_initialised) modprobe_init();

	/* Find all entries with matching driver name */
	for (unsigned int i = 0; i < g_config.num_entries; i++) {
		struct modprobe_entry *entry = &g_config.entries[i];
		if (!entry->valid) continue;
		if (strcmp(entry->driver_name, name) != 0) continue;

		printf("modprobe: loading driver '%s' by name (path=%s)\n",
		    name, entry->driver_path);

		size_t pathlen = strlen(entry->driver_path);
		if (pathlen >= 3 &&
		    strcmp(entry->driver_path + pathlen - 3, ".ko") == 0) {
			return modprobe_insmod(entry->driver_path);
		} else {
			int r = gergios_hotplug_rs_up(entry->driver_name,
			    entry->driver_path, -1, 0, 0);
			/* Register with Driver Manager on success */
			if (r == 0) {
				endpoint_t ep;
				if (minix_rs_lookup(entry->driver_name, &ep) == OK)
					drvmanager_register_native(entry->driver_name,
					    entry->driver_path, (int)ep);
			}
			return r;
		}
	}

	printf("modprobe: driver '%s' not found in config\n", name);
	return -ENOENT;
}

/*===========================================================================*
 *		insmod — direct .ko loading (delegates to drvmanager)  *
 *===========================================================================*/

int modprobe_insmod(const char *path)
{
	/* Detect .so files and route to the .so loader */
	size_t len = strlen(path);
	if (len >= 3 && strcmp(path + len - 3, ".so") == 0) {
		printf("modprobe: insmod '%s' (.so → drvmanager_load_so)\n", path);
		return drvmanager_load_so(path);
	}

	printf("modprobe: insmod '%s' (delegating to drvmanager)\n", path);
	return drvmanager_load_ko(path);
}

int modprobe_rmmod(const char *name)
{
	printf("modprobe: rmmod '%s' (delegating to drvmanager)\n", name);
	return drvmanager_unload(name);
}

/*===========================================================================*
 *		Debug / diagnostic                                      *
 *===========================================================================*/

void modprobe_dump(void)
{
	printf("=== Modprobe Configuration ===\n");
	printf("Initialised: %s\n", g_initialised ? "yes" : "no");
	printf("Total entries: %u\n", g_config.num_entries);
	printf("\n");

	for (unsigned int i = 0; i < g_config.num_entries; i++) {
		const struct modprobe_entry *e = &g_config.entries[i];
		if (!e->valid) continue;
		printf("[%02u] %-20s alias=%-40s path=%s\n",
		    i, e->driver_name, e->alias_pattern, e->driver_path);
		if (e->options[0])
		    printf("     options: %s\n", e->options);
	}
	printf("=== End ===\n");
}

const struct modprobe_config *modprobe_get_config(void)
{
	return g_initialised ? &g_config : NULL;
}
