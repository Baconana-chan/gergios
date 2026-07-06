/* modprobe.h — GergiOS Binding Policy Engine
 *
 * Configuration-based driver loading: JSON policy files map PCI device IDs
 * to driver binaries.  Supports Linux-compatible PCI alias patterns
 * (pci:v0000VVVVd0000DDDDsv0000SSSSsd0000ssssbc00BBsc00SSi00PP)
 * with wildcards, modprobe-by-name, and automatic hot-plug dispatch.
 *
 * Config files are loaded from /etc/gergios/modprobe.d/*.json at startup.
 * Each file contains an array of entries mapping PCI aliases to drivers.
 *
 * Usage:
 *   #include "modprobe.h"
 *
 *   // Load all config files
 *   modprobe_init();
 *
 *   // Load driver for a newly discovered device
 *   int r = modprobe_by_device(dev);
 *
 *   // Or by name (like modprobe e1000)
 *   int r = modprobe_by_name("e1000");
 */

#ifndef _GERGIOS_MODPROBE_H
#define _GERGIOS_MODPROBE_H

#include <stdint.h>
#include <stddef.h>
#include "gergios_driver.h"
#include "gergios_device.h"

/*===========================================================================*
 *		Constants                                                 *
 *===========================================================================*/

#define MODPROBE_MAX_ENTRIES     512   /* Max entries across all config files */
#define MODPROBE_ALIAS_MAX       128   /* Max PCI alias string length */
#define MODPROBE_NAME_MAX        64    /* Max driver name length */
#define MODPROBE_PATH_MAX        256   /* Max driver binary path length */
#define MODPROBE_OPTIONS_MAX     1024  /* Max module options string length */
#define MODPROBE_DIR_PATH        "/etc/gergios/modprobe.d"
#define MODPROBE_DEFAULT_DIR     "/lib/modules/gergios"

/*===========================================================================*
 *		PCI Alias Format (Linux-compatible)                      *
 *===========================================================================*
 *
 * Full format:
 *   pci:v0000VVVVd0000DDDDsv0000SSSSsd0000ssssbc00BBsc00SSi00PP
 *
 * Components:
 *   v0000VVVV  — vendor ID        (hex, 4-char, no leading zeros for "*")
 *   d0000DDDD  — device ID
 *   sv0000SSSS — subvendor
 *   sd0000ssss — subdevice
 *   bc00BB     — base class       (hex, 2-char)
 *   sc00SS     — subclass
 *   i00PP      — programming interface
 *
 * Wildcards: any component can be replaced with "*" (e.g. "pci:v00008086d*")
 * Short form: only matched fields need to be present
 */

/* Parse a PCI alias string into a gergios_device_id table entry.
 * Returns 0 on success, negative errno on parse error.
 * Fields not present in the alias are set to 0xFFFF (wildcard). */
int modprobe_alias_parse(const char *alias, struct gergios_device_id *id);

/* Generate a PCI alias from device identification.
 * Returns the number of characters written (excluding NUL), or negative
 * on truncation.  The output is always NUL-terminated. */
int modprobe_alias_generate(char *out, size_t outsize,
    uint16_t vendor, uint16_t device,
    uint16_t subvendor, uint16_t subdevice,
    uint32_t class_code);

/* Check whether a PCI alias matches a glob pattern (supporting "*"
 * wildcards per component).  Returns 1 if matched, 0 if not. */
int modprobe_alias_match(const char *alias, const char *pattern);

/*===========================================================================*
 *		Config file structures                                  *
 *===========================================================================*/

/* A single modprobe entry parsed from a config file.
 * Contains the parsed alias pattern and the driver to load. */
struct modprobe_entry {
	char            alias_pattern[MODPROBE_ALIAS_MAX];
	char            driver_name[MODPROBE_NAME_MAX];
	char            driver_path[MODPROBE_PATH_MAX];
	char            options[MODPROBE_OPTIONS_MAX];
	struct gergios_device_id id;     /* Parsed from alias_pattern */
	int             has_wildcard;    /* 1 if alias contains wildcards */
	int             valid;           /* 1 if entry is populated */
};

/* Loaded modprobe configuration (all parsed config files combined). */
struct modprobe_config {
	unsigned int    num_entries;
	struct modprobe_entry entries[MODPROBE_MAX_ENTRIES];
};

/*===========================================================================*
 *		Public API                                               *
 *===========================================================================*/

/* Initialise the modprobe engine.
 * Loads all .json config files from MODPROBE_DIR_PATH.
 * Also scans MODPROBE_DEFAULT_DIR for available .ko files.
 * Returns the number of entries loaded, or negative errno on error.
 * Safe to call multiple times (reloads config). */
int modprobe_init(void);

/* Load a single JSON config file into the modprobe configuration.
 * The file is expected to have the format:
 *   {
 *     "modprobe": [
 *       { "alias": "pci:...", "driver": "e1000", "path": "/sbin/e1000" },
 *       ...
 *     ]
 *   }
 * Returns 0 on success, negative errno on error. */
int modprobe_load_file(const char *path);

/* Load all .json files from a directory (non-recursive).
 * Returns the number of files loaded, or negative errno on error. */
int modprobe_load_dir(const char *dirpath);

/* Load a driver by its name (like modprobe e1000).
 * Searches the configuration for entries with matching driver_name,
 * then for each matching entry, attempts to start the driver.
 * Returns 0 on success (driver started), negative errno on failure. */
int modprobe_by_name(const char *name);

/* Load a driver matching a device's PCI identification.
 * Generates a full alias from the device, then searches for the best
 * matching entry (most specific non-wildcard match wins).
 * If found, starts the driver via RS_UP.
 * Returns 0 on success, negative errno if no driver found or start failed. */
int modprobe_by_device(const struct gergios_device *dev);

/* Load a driver by its PCI alias string directly.
 * Useful for hot-plug event handlers that receive an alias.
 * Returns 0 on success, negative errno on failure. */
int modprobe_by_alias(const char *alias);

/* Load a .ko file directly (bypassing config).
 * Used for explicit "insmod /path/to/driver.ko" operations.
 * Loads the ELF, relocates, resolves symbols via the kernel shim,
 * and calls init_module().
 * Returns 0 on success, negative errno on error. */
int modprobe_insmod(const char *ko_path);

/* Remove/stop a loaded driver.
 * Returns 0 on success, negative errno on failure. */
int modprobe_rmmod(const char *name);

/* Debug: print all loaded configuration entries to stdout. */
void modprobe_dump(void);

/* Get a pointer to the current modprobe configuration.
 * Returns NULL if modprobe_init() has not been called. */
const struct modprobe_config *modprobe_get_config(void);

/* Convert a gergios_device_id entry back to a PCI alias string.
 * Fields set to 0xFFFF (any) are output as "*".
 * Returns number of chars written, negative on truncation. */
int modprobe_alias_from_id(char *out, size_t outsize,
    const struct gergios_device_id *id);

#endif /* _GERGIOS_MODPROBE_H */
