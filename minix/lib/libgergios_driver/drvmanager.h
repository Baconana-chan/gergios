/* drvmanager.h — GergiOS Driver Manager Orchestrator
 *
 * Central component that ties together the ELF .ko loader (Phase 7.6a),
 * the Kernel API shim (Phase 7.6b), and the Binding Policy Engine
 * (Phase 7.6c):
 *
 *   1. Tracks all loaded modules (.ko + native) in a unified registry
 *   2. Manages the lifecycle (load → init → probe → unload)
 *   3. Provides reference counting for rmmod safety
 *   4. Handles hotplug event dispatch → modprobe → auto-load
 *   5. Implements proper modprobe_rmmod() through cleanup_module + elf_free_module
 *
 * Usage:
 *   #include "drvmanager.h"
 *   #include "modprobe.h"
 *
 *   drvmanager_init();
 *   modprobe_init();
 *
 *   // Load a .ko driver
 *   drvmanager_load_ko("e1000e", "/lib/modules/e1000e.ko");
 *
 *   // Hotplug event → auto-dispatch
 *   drvmanager_hotplug_dispatch(dev);
 *
 *   // List loaded modules (like lsmod)
 *   drvmanager_list();
 *
 *   // Unload a module (like rmmod)
 *   drvmanager_unload("e1000e");
 */

#ifndef _GERGIOS_DRVMANAGER_H
#define _GERGIOS_DRVMANAGER_H

#include <stdint.h>
#include <stddef.h>
#include "gergios_driver.h"
#include "gergios_device.h"
#include "elf_loader.h"

/*===========================================================================*
 *		Constants                                                 *
 *===========================================================================*/

/* Maximum number of simultaneously loaded modules */
#define DRVMANAGER_MAX_MODULES		64

/* Maximum length of a module name or path */
#define DRVMANAGER_NAME_MAX		64
#define DRVMANAGER_PATH_MAX		256

/* Maximum number of dependencies per module */
#define DRVMANAGER_MAX_DEPS		16

/* Maximum number of devices bound to a single driver instance */
#define DRVMANAGER_MAX_DEVICES		32

/* Maximum number of module parameters */
#define DRVMANAGER_MAX_PARAMS		32

/*===========================================================================*
 *		Module type and state                                    *
 *===========================================================================*/

enum drvmanager_module_type {
	DRVMANAGER_TYPE_UNKNOWN = 0,
	DRVMANAGER_TYPE_KO,		/* Linux .ko module (ELF loaded) */
	DRVMANAGER_TYPE_SO,		/* Native GergiOS .so shared object (dlopen) */
	DRVMANAGER_TYPE_NATIVE,		/* Native MINIX driver (RS_UP) */
};

enum drvmanager_module_state {
	DRVMANAGER_STATE_UNLOADED = 0,
	DRVMANAGER_STATE_LOADING,	/* elf_load_file in progress */
	DRVMANAGER_STATE_LOADED,	/* init_module succeeded */
	DRVMANAGER_STATE_ACTIVE,	/* probe succeeded, running */
	DRVMANAGER_STATE_FAILED,	/* init or probe failed */
	DRVMANAGER_STATE_UNLOADING,	/* cleanup_module in progress */
};

/*===========================================================================*
 *		Parameters (module options)                              *
 *===========================================================================*/

struct drvmanager_param {
	char name[64];
	char value[256];
};

/*===========================================================================*
 *		Loaded module descriptor                                *
 *===========================================================================*/

struct drvmanager_module {
	/* Identity */
	char name[DRVMANAGER_NAME_MAX];
	char path[DRVMANAGER_PATH_MAX];
	enum drvmanager_module_type type;

	/* Lifecycle state */
	enum drvmanager_module_state state;
	int  refcount;
	int  in_use;		/* 1 = slot is occupied */

	/* Type-specific data */
	union {
		struct {
			/* TYPE_KO: pointer to loaded ELF module */
			struct elf_loaded_module *elf_mod;
		} ko;
		struct {
			/* TYPE_SO: dlopen handle + function pointers */
			void *dl_handle;
			int  (*so_init)(void);
			void (*so_cleanup)(void);
		} so;
		struct {
			/* TYPE_NATIVE: endpoint of the running driver */
			int endpoint;
		} native;
	} u;

	/* Dependencies: names of modules this module depends on */
	int   num_deps;
	char  deps[DRVMANAGER_MAX_DEPS][DRVMANAGER_NAME_MAX];

	/* Bound devices (PCI identifiers for probed devices) */
	int   num_devices;
	struct {
		uint16_t vendor;
		uint16_t device;
		char     alias[128];	/* PCI alias string */
	} devices[DRVMANAGER_MAX_DEVICES];

	/* Parameters */
	int num_params;
	struct drvmanager_param params[DRVMANAGER_MAX_PARAMS];

	/* License / flags */
	int  gpl_compat;	/* 1 = GPL-compatible license */
	char license[64];
	char vermagic[64];
};

/*===========================================================================*
 *		Public API                                               *
 *===========================================================================*/

/* Initialise the Driver Manager.
 * Clears the module registry and resets all state.
 * Safe to call multiple times. */
void drvmanager_init(void);

/* Load a .ko module and track it in the registry.
 * Calls modprobe_insmod() internally but handles all tracking:
 *   - Looks up the module name from .modinfo
 *   - Registers in the module table
 *   - Tracks dependencies (from "depends" field in .modinfo)
 *   - Records GPL compatibility from license
 *
 * @param ko_path  Full path to the .ko file
 * @return 0 on success, negative errno on failure */
int drvmanager_load_ko(const char *ko_path);

/* Load a .so shared object driver and track it in the registry.
 * Uses dlopen() to load the shared object, dlsym() to find
 * init_module() and cleanup_module() symbols, then calls init().
 *
 * @param so_path  Full path to the .so file
 * @return 0 on success, negative errno on failure */
int drvmanager_load_so(const char *so_path);

/* Load a .ko or .so module by name (resolved via modprobe config).
 * Searches the modprobe config for the driver entry, then calls
 * drvmanager_load_ko or drvmanager_load_so with the resolved path.
 *
 * @param name  Driver name (e.g. "e1000e", "ath9k")
 * @return 0 on success, negative errno on failure */
int drvmanager_load_by_name(const char *name);

/* Register a native (MINIX) driver as loaded.
 * Used for native drivers started via RS_UP — marks them in the
 * registry so the system can track all active drivers uniformly.
 *
 * @param name      Driver name (e.g. "ahci")
 * @param path      Driver binary path (e.g. "/sbin/ahci")
 * @param endpoint  MINIX endpoint of the running driver process
 * @return 0 on success, negative errno on failure */
int drvmanager_register_native(const char *name, const char *path,
    int endpoint);

/* Unload a module by name.
 * For .ko modules:
 *   1. Checks refcount (must be 0)
 *   2. Checks that no other module depends on this one
 *   3. Calls cleanup_module()
 *   4. Calls elf_free_module() to release all memory
 *   5. Removes from registry
 *
 * For native drivers:
 *   1. Calls gergios_hotplug_rs_down() (RS_DOWN via /sbin/service)
 *   2. Removes from registry
 *
 * @param name  Module name to unload
 * @return 0 on success, negative errno on failure:
 *   -EBUSY  refcount > 0 or other modules depend on this one
 *   -ENOENT not found */
int drvmanager_unload(const char *name);

/* Increment the reference count of a module.
 * Used when a device binds to the driver or another module depends on it.
 * @return 0 on success, -ENOENT if not found */
int drvmanager_ref_get(const char *name);

/* Decrement the reference count of a module.
 * @return 0 on success, -ENOENT if not found */
int drvmanager_ref_put(const char *name);

/* Find a loaded module by name.
 * Returns a pointer to the module descriptor (valid until the module
 * is unloaded), or NULL if not found. */
struct drvmanager_module *drvmanager_find(const char *name);

/* Dispatch a hotplug device discovery event.
 * Called when a new PCI device is found (e.g. from pci_scan or ACPI).
 * The Driver Manager:
 *   1. Calls modprobe_by_device() to find the matching driver
 *   2. If the driver needs a .ko, calls drvmanager_load_ko()
 *   3. If the driver is native, calls gergios_hotplug_rs_up()
 *   4. Registers the loaded driver in the module table
 *   5. Binds the device to the driver
 *
 * @param dev  The newly discovered device
 * @return 0 on success, negative errno on failure */
int drvmanager_hotplug_dispatch(struct gergios_device *dev);

/* List all loaded modules to stdout (like lsmod). */
void drvmanager_list(void);

/* Get detailed status of a specific module.
 * Fills the provided buffer with a human-readable string.
 * Returns the number of characters written, or negative on error. */
int drvmanager_status(const char *name, char *buf, size_t bufsize);

/* Iterate over all loaded modules.
 * @param callback  Called for each module; return non-zero to stop.
 * @param arg       User argument passed to callback.
 * @return The number of modules iterated. */
int drvmanager_foreach(int (*callback)(struct drvmanager_module *mod,
                       void *arg), void *arg);

/* Check if a module can be safely unloaded.
 * Returns 0 if unload is safe, negative errno if not:
 *   -EBUSY  refcount > 0 or modules depend on this one
 *   -ENOENT not found */
int drvmanager_can_unload(const char *name);

/* Get the number of currently loaded modules. */
int drvmanager_count(void);

/* Set a module parameter (like modprobe options="..."). */
int drvmanager_set_param(const char *name, const char *key,
    const char *value);

/* Call init_module for a module in the LOADING state.
 * Supports both TYPE_KO (uses elf_mod->init_module_fn) and
 * TYPE_SO (uses mod->u.so.so_init).
 * This is separated from load_ko/so so the caller can inspect the
 * module before calling init (e.g. check vermagic, set parameters)
 * or submit to the parallel worker pool.
 * Returns 0 on success, negative errno on init failure. */
int drvmanager_init_module(const char *name);

/*===========================================================================*
 *		Batch / Parallel Init API (SMP)                        *
 *===========================================================================*/

/* Initialise the parallel worker pool for SMP module loading.
 * Called automatically by drvmanager_init() with auto-detected CPU
 * count.  Call this explicitly to override the pool size.
 *
 * @param num_threads  Number of worker threads (0 = auto-detect)
 * @return 0 on success, negative errno on failure */
int drvmanager_batch_init(int num_threads);

/* Submit a module (must be in LOADING state) for parallel init_module.
 * The worker pool calls drvmanager_init_module() in a background
 * thread.  Returns immediately after queuing.
 *
 * @param module_name  Name of the module to initialise
 * @return 0 on success (queued), negative errno on failure */
int drvmanager_batch_submit(const char *module_name);

/* Wait for all pending batch init jobs to complete.
 * Blocks until every submitted module has finished init_module.
 * Use after submitting several modules to synchronise. */
void drvmanager_batch_sync(void);

/* Wait for a specific module's batch init to complete.
 * @param module_name  Module to wait for
 * @return 0 on success, -ENOENT if not found in queue */
int drvmanager_batch_wait(const char *module_name);

/* Destroy the parallel worker pool.  Called automatically on shutdown. */
void drvmanager_batch_destroy(void);

#endif /* _GERGIOS_DRVMANAGER_H */
