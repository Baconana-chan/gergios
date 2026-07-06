/* drvmanager.c — GergiOS Driver Manager Orchestrator Implementation
 *
 * Ties together the ELF .ko loader, Kernel API shim, and Binding Policy
 * Engine into a unified Driver Manager with module tracking, lifecycle
 * management, reference counting, dependency tracking, and hotplug dispatch.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/stat.h>
#include <dlfcn.h>
#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/rs.h>

#include "drvmanager.h"
#include "modprobe.h"
#include "elf_loader.h"
#include "kernel_shim.h"
#include "hotplug.h"
#include "drvmanager_pool.h"

/*===========================================================================*
 *		Module registry                                           *
 *===========================================================================*/

static struct drvmanager_module g_modules[DRVMANAGER_MAX_MODULES];
static int g_initialised = 0;

/*===========================================================================*
 *		Initialisation                                           *
 *===========================================================================*/

void drvmanager_init(void)
{
	memset(g_modules, 0, sizeof(g_modules));
	g_initialised = 1;
	printf("drvmanager: initialised (%d module slots)\n",
	    DRVMANAGER_MAX_MODULES);

	/* Auto-init the parallel worker pool for SMP */
	if (drmgr_pool_init(0) == 0) {
		printf("drvmanager: SMP worker pool ready (%d threads)\n",
		    drmgr_pool_thread_count());
	}
}

/*===========================================================================*
 *		Internal helpers                                         *
 *===========================================================================*/

static struct drvmanager_module *find_module(const char *name)
{
	if (!name) return NULL;
	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (g_modules[i].in_use &&
		    strcmp(g_modules[i].name, name) == 0)
			return &g_modules[i];
	}
	return NULL;
}

static struct drvmanager_module *alloc_slot(void)
{
	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (!g_modules[i].in_use) {
			memset(&g_modules[i], 0, sizeof(g_modules[i]));
			g_modules[i].in_use = 1;
			return &g_modules[i];
		}
	}
	return NULL;
}

static void free_slot(struct drvmanager_module *mod)
{
	if (mod) {
		memset(mod, 0, sizeof(*mod));
	}
}

/*===========================================================================*
 *		Parsing .modinfo dependencies                           *
 *===========================================================================*/

/* Parse a comma-separated dependency list from .modinfo "depends" field.
 * Format: "e1000,scsi_mod,libata" or empty string for no deps. */
static int parse_depends(struct drvmanager_module *mod, const char *dep_str)
{
	char buf[256];
	char *saveptr, *token;

	if (!dep_str || dep_str[0] == '\0')
		return 0;

	strncpy(buf, dep_str, sizeof(buf) - 1);
	buf[sizeof(buf) - 1] = '\0';

	token = strtok_r(buf, ",", &saveptr);
	while (token && mod->num_deps < DRVMANAGER_MAX_DEPS) {
		/* Trim whitespace */
		while (*token == ' ' || *token == '\t') token++;
		char *end = token + strlen(token) - 1;
		while (end > token && (*end == ' ' || *end == '\t')) end--;
		*(end + 1) = '\0';

		if (token[0]) {
			strncpy(mod->deps[mod->num_deps], token,
			    sizeof(mod->deps[0]) - 1);
			mod->deps[mod->num_deps][sizeof(mod->deps[0]) - 1] = '\0';
			mod->num_deps++;
		}

		token = strtok_r(NULL, ",", &saveptr);
	}

	return mod->num_deps;
}

/*===========================================================================*
 *		Dependency auto-resolution (depmod-style)               *
 *===========================================================================*/

/* Recursion guard for circular dependency detection. */
#define DEP_RESOLVE_MAX_CHAIN	16

static const char *dep_resolve_chain[DEP_RESOLVE_MAX_CHAIN];
static int dep_resolve_depth = 0;

/* Check if a module name is in the current resolution chain (cycle check). */
static int dep_in_chain(const char *name)
{
	for (int i = 0; i < dep_resolve_depth; i++) {
		if (strcmp(dep_resolve_chain[i], name) == 0)
			return 1;
	}
	return 0;
}

/* Find the .ko path for a dependency by name.
 * Checks: 1) modprobe config entries, 2) /lib/modules/gergios/<name>.ko */
static const char *resolve_dep_path(const char *name,
    char *out_buf, size_t outsize)
{
	/* Search modprobe config entries */
	const struct modprobe_config *cfg = modprobe_get_config();
	if (cfg) {
		for (unsigned int i = 0; i < cfg->num_entries; i++) {
			if (!cfg->entries[i].valid) continue;
			if (strcmp(cfg->entries[i].driver_name, name) != 0)
				continue;
			size_t plen = strlen(cfg->entries[i].driver_path);
			if (plen >= 3 &&
			    strcmp(cfg->entries[i].driver_path + plen - 3,
			    ".ko") == 0) {
				strncpy(out_buf, cfg->entries[i].driver_path,
				    outsize - 1);
				out_buf[outsize - 1] = '\0';
				return out_buf;
			}
		}
	}

	/* Fallback: search multiple paths for <name>.ko */
{
	static const char *g_module_search_dirs[] = {
	    "/lib/modules/gergios",
	    "/lib/modules/gergios/kernel/drivers/extra",
	    "/lib/modules/gergios/kernel/drivers/net",
	    "/lib/modules/gergios/kernel/drivers/block",
	    "/lib/modules/gergios/kernel/drivers/ata",
	    "/lib/modules/gergios/kernel/drivers/nvme",
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
	    NULL
	};

	struct stat st;
	for (int i = 0; g_module_search_dirs[i]; i++) {
		snprintf(out_buf, outsize, "%s/%s.ko",
		    g_module_search_dirs[i], name);
		if (stat(out_buf, &st) == 0 && S_ISREG(st.st_mode))
			return out_buf;
	}

	/* Also try the flat /lib/modules/gergios/<name>.so fallback */
	snprintf(out_buf, outsize, "/lib/modules/gergios/%s.so", name);
	if (stat(out_buf, &st) == 0 && S_ISREG(st.st_mode))
		return out_buf;
}

	return NULL;
}

/* Resolve and auto-load a single dependency.
 * Called recursively for each dep of a module. */
static int dep_resolve_one(const char *dep_name)
{
	char dep_path[DRVMANAGER_PATH_MAX];
	const char *resolved_path;

	if (!dep_name || !dep_name[0]) return 0;

	/* Already loaded — just inc refcount */
	struct drvmanager_module *existing = find_module(dep_name);
	if (existing) {
		if (existing->state != DRVMANAGER_STATE_FAILED) {
			drvmanager_ref_get(dep_name);
			printf("drvmanager: dep '%s' already loaded (ref->%d)\n",
			    dep_name, existing->refcount);
			return 0;
		}
		printf("drvmanager: dep '%s' previously FAILED\n", dep_name);
		return -ELIBBAD;
	}

	/* Cycle detection */
	if (dep_in_chain(dep_name)) {
		printf("drvmanager: CIRCULAR dependency: ");
		for (int i = 0; i < dep_resolve_depth; i++)
			printf("%s -> ", dep_resolve_chain[i]);
		printf("%s\n", dep_name);
		return -EDEADLK;
	}

	/* Resolve path */
	resolved_path = resolve_dep_path(dep_name, dep_path, sizeof(dep_path));
	if (!resolved_path) {
		printf("drvmanager: dep '%s' not found on disk\n", dep_name);
		return -ENOENT;
	}

	printf("drvmanager: loading dep '%s' from '%s'\n",
	    dep_name, resolved_path);

	/* Push onto resolution chain (cycle detection) */
	if (dep_resolve_depth >= DEP_RESOLVE_MAX_CHAIN) {
		printf("drvmanager: dep chain too deep (>%d)\n",
		    DEP_RESOLVE_MAX_CHAIN);
		return -ELOOP;
	}
	dep_resolve_chain[dep_resolve_depth] = dep_name;
	dep_resolve_depth++;

	/* Recursively load */
	int ret = drvmanager_load_ko(resolved_path);

	/* Pop from chain */
	dep_resolve_depth--;

	return ret;
}

/* Auto-load all declared dependencies of a module.
 * Called after parsing .modinfo, before init_module. */
static int dep_auto_load_all(struct drvmanager_module *mod)
{
	if (!mod || mod->num_deps == 0) return 0;

	printf("drvmanager: resolving %d dep(s) for '%s'\n",
	    mod->num_deps, mod->name);

	for (int i = 0; i < mod->num_deps; i++) {
		int ret = dep_resolve_one(mod->deps[i]);
		if (ret != 0) {
			printf("drvmanager: dep '%s' failed for '%s': %d\n",
			    mod->deps[i], mod->name, ret);
			return ret;
		}
	}

	printf("drvmanager: all deps resolved for '%s'\n", mod->name);
	return 0;
}

/*===========================================================================*
 *		Loading .so shared object modules                   *
 *===========================================================================*/

int drvmanager_load_so(const char *so_path)
{
	struct drvmanager_module *mod;
	void *handle;
	int (*init_fn)(void);
	void (*cleanup_fn)(void);
	char *error;

	if (!g_initialised) drvmanager_init();
	if (!so_path) return -EINVAL;

	/* Check if already loaded */
	char path_key[DRVMANAGER_PATH_MAX];
	strncpy(path_key, so_path, sizeof(path_key) - 1);
	path_key[sizeof(path_key) - 1] = '\0';

	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (g_modules[i].in_use &&
		    strcmp(g_modules[i].path, path_key) == 0) {
			printf("drvmanager: module already loaded from %s\n", so_path);
			return 0;
		}
	}

	/* Allocate a slot */
	mod = alloc_slot();
	if (!mod) {
		printf("drvmanager: module table full\n");
		return -ENOMEM;
	}

	mod->type = DRVMANAGER_TYPE_SO;
	mod->state = DRVMANAGER_STATE_LOADING;
	strncpy(mod->path, so_path, sizeof(mod->path) - 1);
	mod->path[sizeof(mod->path) - 1] = '\0';

	printf("drvmanager: loading .so module '%s'\n", so_path);

	/* dlopen the shared object */
	handle = dlopen(so_path, RTLD_NOW | RTLD_LOCAL);
	if (!handle) {
		error = dlerror();
		printf("drvmanager: dlopen('%s') failed: %s\n",
		    so_path, error ? error : "unknown error");
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return -ELIBACC;
	}

	/* Resolve init_module */
	init_fn = (int (*)(void))dlsym(handle, "init_module");
	if ((error = dlerror()) != NULL) {
		printf("drvmanager: dlsym(init_module) failed: %s\n", error);
		dlclose(handle);
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return -ENOSYS;
	}

	/* Resolve cleanup_module (optional) */
	cleanup_fn = (void (*)(void))dlsym(handle, "cleanup_module");
	dlerror();  /* Clear any error */

	/* Extract module name from filename */
	const char *base = strrchr(so_path, '/');
	base = base ? base + 1 : so_path;
	size_t len = strlen(base);
	if (len > 3 && strcmp(base + len - 3, ".so") == 0)
		len -= 3;
	if (len >= sizeof(mod->name))
		len = sizeof(mod->name) - 1;
	memcpy(mod->name, base, len);
	mod->name[len] = '\0';

	/* Check for name conflict */
	if (find_module(mod->name) && find_module(mod->name) != mod) {
		printf("drvmanager: module name '%s' already loaded\n",
		    mod->name);
		dlclose(handle);
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return -EEXIST;
	}

	/* Store handles */
	mod->u.so.dl_handle = handle;
	mod->u.so.so_init = init_fn;
	mod->u.so.so_cleanup = cleanup_fn;

	printf("drvmanager: .so loaded, name='%s', init=%p\n",
	    mod->name, (void *)init_fn);

	/* Call init_module through the standard path (same as .ko)
	 * This allows both synchronous and pool-submitted init to work. */
	int ret = drvmanager_init_module(mod->name);
	if (ret != 0) {
		printf("drvmanager: init_module for '%s' returned %d\n",
		    mod->name, ret);
		if (cleanup_fn) cleanup_fn();
		dlclose(handle);
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return ret;
	}

	printf("drvmanager: .so module '%s' loaded and initialised\n",
	    mod->name);
	return 0;
}

/*===========================================================================*
 *		Loading .ko modules (with depmod auto-resolution)      *
 *===========================================================================*/

int drvmanager_load_ko(const char *ko_path)
{
	struct drvmanager_module *mod;
	struct elf_loaded_module *elf_mod = NULL;
	int ret;
	const char *mod_name, *license, *depends, *vermagic;
	extern const struct elf_host_symbol gergios_kernel_syms[];
	extern const size_t gergios_kernel_nsyms;

	if (!g_initialised) drvmanager_init();
	if (!ko_path) return -EINVAL;

	/* Check if already loaded */
	char path_key[DRVMANAGER_PATH_MAX];
	strncpy(path_key, ko_path, sizeof(path_key) - 1);
	path_key[sizeof(path_key) - 1] = '\0';

	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (g_modules[i].in_use &&
		    strcmp(g_modules[i].path, path_key) == 0) {
			printf("drvmanager: module '%s' already loaded from %s\n",
			    g_modules[i].name, ko_path);
			return 0;
		}
	}

	/* Allocate a slot (in LOADING state) */
	mod = alloc_slot();
	if (!mod) {
		printf("drvmanager: module table full\n");
		return -ENOMEM;
	}

	mod->type = DRVMANAGER_TYPE_KO;
	mod->state = DRVMANAGER_STATE_LOADING;
	strncpy(mod->path, ko_path, sizeof(mod->path) - 1);
	mod->path[sizeof(mod->path) - 1] = '\0';

	printf("drvmanager: loading .ko module '%s'\n", ko_path);

	/* Load the ELF .ko file */
	ret = elf_load_file(ko_path,
	    gergios_kernel_syms, gergios_kernel_nsyms,
	    &elf_mod);
	if (ret != 0) {
		printf("drvmanager: elf_load_file('%s') failed: %d\n",
		    ko_path, ret);
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return ret;
	}

	mod->u.ko.elf_mod = elf_mod;

	/* Extract module name from .modinfo */
	mod_name = elf_get_modinfo(elf_mod, "name");
	if (!mod_name)
		mod_name = elf_mod->name[0] ? elf_mod->name : NULL;
	if (!mod_name) {
		/* Fallback: use filename without .ko suffix */
		const char *base = strrchr(ko_path, '/');
		base = base ? base + 1 : ko_path;
		size_t len = strlen(base);
		if (len > 3 && strcmp(base + len - 3, ".ko") == 0)
			len -= 3;
		if (len >= sizeof(mod->name))
			len = sizeof(mod->name) - 1;
		memcpy(mod->name, base, len);
		mod->name[len] = '\0';
	} else {
		strncpy(mod->name, mod_name, sizeof(mod->name) - 1);
		mod->name[sizeof(mod->name) - 1] = '\0';
	}

	/* Check if name conflicts with existing module */
	if (find_module(mod->name) && find_module(mod->name) != mod) {
		printf("drvmanager: module name '%s' already loaded\n",
		    mod->name);
		if (mod->u.ko.elf_mod)
			elf_free_module(mod->u.ko.elf_mod);
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return -EEXIST;
	}

	/* Extract license */
	license = elf_get_modinfo(elf_mod, "license");
	if (license) {
		strncpy(mod->license, license, sizeof(mod->license) - 1);
		mod->license[sizeof(mod->license) - 1] = '\0';

		/* Check GPL compatibility */
		if (strcmp(license, "GPL") == 0 ||
		    strcmp(license, "GPL v2") == 0 ||
		    strcmp(license, "GPL v3") == 0 ||
		    strcmp(license, "GPL and additional rights") == 0 ||
		    strncmp(license, "Dual", 4) == 0) {
			mod->gpl_compat = 1;
		}
	}

	/* Extract vermagic */
	vermagic = elf_get_modinfo(elf_mod, "vermagic");
	if (vermagic) {
		strncpy(mod->vermagic, vermagic, sizeof(mod->vermagic) - 1);
		mod->vermagic[sizeof(mod->vermagic) - 1] = '\0';
		printf("drvmanager: vermagic = '%s'\n", mod->vermagic);
	}

	/* Parse dependencies */
	depends = elf_get_modinfo(elf_mod, "depends");
	parse_depends(mod, depends);
	if (mod->num_deps > 0) {
		printf("drvmanager: module '%s' depends on: ", mod->name);
		for (int i = 0; i < mod->num_deps; i++)
			printf("%s%s", i > 0 ? ", " : "", mod->deps[i]);
		printf("\n");
	}

	/* Debug dump */
	elf_dump_module(elf_mod);

	printf("drvmanager: .ko loaded, name='%s'%s%s\n",
	    mod->name,
	    mod->gpl_compat ? " GPL" : "",
	    license ? "" : " (no license)");

	/* Auto-resolve dependencies (depmod-style) BEFORE init_module */
	ret = dep_auto_load_all(mod);
	if (ret != 0) {
		printf("drvmanager: failed to resolve dependencies for "
		    "'%s': %d\n", mod->name, ret);
		if (mod->u.ko.elf_mod) {
			if (elf_mod->cleanup_module_fn) {
				void (*cleanup)(void) =
				    (void (*)(void))elf_mod->cleanup_module_fn;
				cleanup();
			}
			elf_free_module(mod->u.ko.elf_mod);
		}
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return ret;
	}

	/* Call init_module */
	ret = drvmanager_init_module(mod->name);
	if (ret != 0) {
		printf("drvmanager: init_module for '%s' failed: %d\n",
		    mod->name, ret);
		if (mod->u.ko.elf_mod) {
			if (elf_mod->cleanup_module_fn) {
				void (*cleanup)(void) =
				    (void (*)(void))elf_mod->cleanup_module_fn;
				cleanup();
			}
			elf_free_module(mod->u.ko.elf_mod);
		}
		mod->state = DRVMANAGER_STATE_FAILED;
		free_slot(mod);
		return ret;
	}

	printf("drvmanager: module '%s' loaded and initialised\n",
	    mod->name);
	return 0;
}

int drvmanager_init_module(const char *name)
{
	struct drvmanager_module *mod = find_module(name);
	int ret = 0;

	if (!mod) return -ENOENT;
	if (mod->state != DRVMANAGER_STATE_LOADING) return -EALREADY;

	if (mod->type == DRVMANAGER_TYPE_KO && mod->u.ko.elf_mod) {
		/* .ko module: call init_module via ELF-resolved function pointer */
		struct elf_loaded_module *elf_mod = mod->u.ko.elf_mod;

		if (elf_mod->init_module_fn) {
			int (*init_int)(void) =
			    (int (*)(void))elf_mod->init_module_fn;

			printf("drvmanager: calling init_module for '%s' at %p\n",
			    mod->name, elf_mod->init_module_fn);

			ret = init_int();
		} else {
			printf("drvmanager: warning — no init_module in '%s'\n",
			    mod->name);
		}

	} else if (mod->type == DRVMANAGER_TYPE_SO && mod->u.so.so_init) {
		/* .so module: call init_module via dlsym-resolved function pointer */
		printf("drvmanager: calling .so init_module for '%s'\n",
		    mod->name);

		ret = mod->u.so.so_init();

	} else {
		printf("drvmanager: init_module: unsupported type for '%s'\n",
		    mod->name);
		return -EINVAL;
	}

	if (ret != 0) {
		printf("drvmanager: init_module for '%s' returned %d\n",
		    mod->name, ret);
		mod->state = DRVMANAGER_STATE_FAILED;
		return ret;
	}

	mod->state = DRVMANAGER_STATE_LOADED;
	mod->refcount = 1;	/* Start with one reference */
	printf("drvmanager: init_module '%s' succeeded\n", mod->name);
	return 0;
}

int drvmanager_load_by_name(const char *name)
{
	if (!name) return -EINVAL;
	if (!g_initialised) drvmanager_init();
	if (!strcmp(name, "")) return -EINVAL;

	/* Check if already loaded */
	if (find_module(name)) {
		printf("drvmanager: module '%s' already loaded\n", name);
		return 0;
	}

	/* Try modprobe_by_name first — for both native and .ko drivers */
	int r = modprobe_by_name(name);
	if (r == 0) {
		/* For native drivers, register them after successful RS_UP.
		 * For .ko drivers, modprobe_by_name eventually calls
		 * drvmanager_load_ko if the entry is a .ko file. */
		return 0;
	}

	/* Search hierarchical module paths */
	{
		static const char *g_mod_search_dirs[] = {
		    "/lib/modules/gergios",
		    "/lib/modules/gergios/kernel/drivers/extra",
		    "/lib/modules/gergios/kernel/drivers/net",
		    "/lib/modules/gergios/kernel/drivers/block",
		    "/lib/modules/gergios/kernel/drivers/ata",
		    "/lib/modules/gergios/kernel/drivers/nvme",
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
		    NULL
		};

		char path[DRVMANAGER_PATH_MAX];
		struct stat st;

		for (int i = 0; g_mod_search_dirs[i]; i++) {
			snprintf(path, sizeof(path), "%s/%s.ko",
			    g_mod_search_dirs[i], name);
			if (stat(path, &st) == 0 && S_ISREG(st.st_mode)) {
				return drvmanager_load_ko(path);
			}
		}

		/* Try .so fallback */
		snprintf(path, sizeof(path), "/lib/modules/gergios/%s.so", name);
		if (stat(path, &st) == 0 && S_ISREG(st.st_mode)) {
			return drvmanager_load_so(path);
		}
	}

	printf("drvmanager: module '%s' not found\n", name);
	return -ENOENT;
}

/*===========================================================================*
 *		Native driver registration                              *
 *===========================================================================*/

int drvmanager_register_native(const char *name, const char *path,
    int endpoint)
{
	struct drvmanager_module *mod;

	if (!g_initialised) drvmanager_init();
	if (!name) return -EINVAL;

	if (find_module(name)) {
		printf("drvmanager: native driver '%s' already registered\n",
		    name);
		return 0;
	}

	mod = alloc_slot();
	if (!mod) {
		printf("drvmanager: module table full\n");
		return -ENOMEM;
	}

	mod->type = DRVMANAGER_TYPE_NATIVE;
	mod->state = DRVMANAGER_STATE_ACTIVE;
	mod->refcount = 1;
	mod->u.native.endpoint = endpoint;

	strncpy(mod->name, name, sizeof(mod->name) - 1);
	mod->name[sizeof(mod->name) - 1] = '\0';

	if (path) {
		strncpy(mod->path, path, sizeof(mod->path) - 1);
		mod->path[sizeof(mod->path) - 1] = '\0';
	}

	printf("drvmanager: registered native driver '%s' (ep=%d)\n",
	    name, endpoint);
	return 0;
}

/*===========================================================================*
 *		Unloading modules                                        *
 *===========================================================================*/

int drvmanager_can_unload(const char *name)
{
	struct drvmanager_module *mod = find_module(name);
	if (!mod) return -ENOENT;

	if (mod->refcount > 0) {
		printf("drvmanager: cannot unload '%s': refcount=%d\n",
		    name, mod->refcount);
		return -EBUSY;
	}

	/* Check if any other module depends on this one */
	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (!g_modules[i].in_use) continue;
		if (&g_modules[i] == mod) continue;
		for (int j = 0; j < g_modules[i].num_deps; j++) {
			if (strcmp(g_modules[i].deps[j], name) == 0) {
				printf("drvmanager: cannot unload '%s': "
				    "depended on by '%s'\n",
				    name, g_modules[i].name);
				return -EBUSY;
			}
		}
	}

	return 0;
}

int drvmanager_unload(const char *name)
{
	struct drvmanager_module *mod = find_module(name);
	if (!mod) {
		printf("drvmanager: module '%s' not found\n", name);
		return -ENOENT;
	}

	/* Check if unload is safe */
	int r = drvmanager_can_unload(name);
	if (r != 0)
		return r;

	printf("drvmanager: unloading module '%s'...\n", name);

	mod->state = DRVMANAGER_STATE_UNLOADING;

	if (mod->type == DRVMANAGER_TYPE_KO) {
		struct elf_loaded_module *elf_mod = mod->u.ko.elf_mod;

		/* Call cleanup_module */
		if (elf_mod && elf_mod->cleanup_module_fn) {
			void (*cleanup)(void) =
			    (void (*)(void))elf_mod->cleanup_module_fn;
			printf("drvmanager: calling cleanup_module for '%s'\n",
			    name);
			cleanup();
		}

		/* Free ELF module memory */
		if (elf_mod) {
			elf_free_module(elf_mod);
			mod->u.ko.elf_mod = NULL;
		}

		printf("drvmanager: module '%s' unloaded (memory freed)\n",
		    name);

	} else if (mod->type == DRVMANAGER_TYPE_SO) {
		/* Call cleanup_module if available */
		if (mod->u.so.so_cleanup) {
			printf("drvmanager: calling cleanup_module for '%s' (.so)\n",
			    name);
			mod->u.so.so_cleanup();
		}

		/* dlclose the shared object */
		if (mod->u.so.dl_handle) {
			dlclose(mod->u.so.dl_handle);
			mod->u.so.dl_handle = NULL;
		}

		printf("drvmanager: .so module '%s' unloaded\n", name);

	} else if (mod->type == DRVMANAGER_TYPE_NATIVE) {
		/* Stop native driver via RS_DOWN */
		r = gergios_hotplug_rs_down(name);
		if (r != 0) {
			printf("drvmanager: RS_DOWN for '%s' failed: %d\n",
			    name, r);
			/* Continue anyway — the driver may already be dead */
		}
		printf("drvmanager: native driver '%s' stopped\n", name);
	}

	/* Decrement refcount on all dependencies */
	for (int i = 0; i < mod->num_deps; i++) {
		struct drvmanager_module *dep = find_module(mod->deps[i]);
		if (dep && dep->refcount > 0) {
			drvmanager_ref_put(mod->deps[i]);
			printf("drvmanager: dep '%s' refcount decremented (%d -> %d)\n",
			    mod->deps[i], dep->refcount + 1, dep->refcount);
		}
	}

	/* Remove from registry */
	mod->state = DRVMANAGER_STATE_UNLOADED;
	free_slot(mod);

	return 0;
}

/*===========================================================================*
 *		Reference counting                                       *
 *===========================================================================*/

int drvmanager_ref_get(const char *name)
{
	struct drvmanager_module *mod = find_module(name);
	if (!mod) return -ENOENT;
	mod->refcount++;
	return 0;
}

int drvmanager_ref_put(const char *name)
{
	struct drvmanager_module *mod = find_module(name);
	if (!mod) return -ENOENT;
	if (mod->refcount > 0)
		mod->refcount--;
	return 0;
}

/*===========================================================================*
 *		Hotplug dispatch                                       *
 *===========================================================================*/

int drvmanager_hotplug_dispatch(struct gergios_device *dev)
{
	if (!dev) return -EINVAL;
	if (!g_initialised) drvmanager_init();

	printf("drvmanager: hotplug dispatch for %04x:%04x (class %06x)\n",
	    dev->vendor_id, dev->device_id, dev->class_code);

	/* Use modprobe to find and load the matching driver */
	return modprobe_by_device(dev);
}

/*===========================================================================*
 *		Diagnostics / listing                                  *
 *===========================================================================*/

void drvmanager_list(void)
{
	int count = 0;

	printf("=== Loaded Modules ===\n");
	printf("%-24s %-8s %-10s %-6s %-8s %s\n",
	    "Module", "Type", "State", "Refs", "Deps", "Path");

	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (!g_modules[i].in_use) continue;
		count++;

		const struct drvmanager_module *m = &g_modules[i];

		const char *type_str = "?";
		switch (m->type) {
		case DRVMANAGER_TYPE_KO:     type_str = ".ko"; break;
		case DRVMANAGER_TYPE_SO:     type_str = ".so"; break;
		case DRVMANAGER_TYPE_NATIVE: type_str = "native"; break;
		default: break;
		}

		const char *state_str = "?";
		switch (m->state) {
		case DRVMANAGER_STATE_LOADED:   state_str = "loaded"; break;
		case DRVMANAGER_STATE_ACTIVE:   state_str = "active"; break;
		case DRVMANAGER_STATE_FAILED:   state_str = "failed"; break;
		case DRVMANAGER_STATE_LOADING:  state_str = "loading"; break;
		case DRVMANAGER_STATE_UNLOADING:state_str = "unload"; break;
		default: break;
		}

		printf("%-24s %-8s %-10s %-6d %-8d %s\n",
		    m->name, type_str, state_str,
		    m->refcount, m->num_deps,
		    m->path[0] ? m->path : "-");

		if (m->num_deps > 0) {
			printf("  %-24s deps: ", "");
			for (int j = 0; j < m->num_deps; j++)
				printf("%s%s", j > 0 ? ", " : "", m->deps[j]);
			printf("\n");
		}

		if (m->num_devices > 0) {
			printf("  %-24s devices: ", "");
			for (int j = 0; j < m->num_devices; j++)
				printf("%s%04x:%04x",
				    j > 0 ? ", " : "",
				    m->devices[j].vendor,
				    m->devices[j].device);
			printf("\n");
		}
	}

	printf("--- %d module(s) loaded ---\n", count);
}

int drvmanager_status(const char *name, char *buf, size_t bufsize)
{
	struct drvmanager_module *mod = find_module(name);
	if (!mod) return -ENOENT;

	int n = 0;

	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Module:     %s\n", mod->name);
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Path:       %s\n", mod->path[0] ? mod->path : "-");
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Type:       %s\n",
	    mod->type == DRVMANAGER_TYPE_KO ? ".ko" :
	    mod->type == DRVMANAGER_TYPE_SO ? ".so" :
	    mod->type == DRVMANAGER_TYPE_NATIVE ? "native" : "unknown");
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "State:      %s\n",
	    mod->state == DRVMANAGER_STATE_LOADED ? "loaded" :
	    mod->state == DRVMANAGER_STATE_ACTIVE ? "active" :
	    mod->state == DRVMANAGER_STATE_LOADING ? "loading" :
	    mod->state == DRVMANAGER_STATE_FAILED ? "failed" :
	    mod->state == DRVMANAGER_STATE_UNLOADING ? "unloading" : "?");
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Refcount:   %d\n", mod->refcount);
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "License:    %s%s\n",
	    mod->license[0] ? mod->license : "(none)",
	    mod->gpl_compat ? " [GPL-compat]" : "");
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Vermagic:   %s\n",
	    mod->vermagic[0] ? mod->vermagic : "-");

	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Deps:       ");
	if (mod->num_deps == 0) {
		n += snprintf(buf + n,
		    bufsize > (size_t)n ? bufsize - n : 0, "(none)");
	} else {
		for (int i = 0; i < mod->num_deps; i++)
			n += snprintf(buf + n,
			    bufsize > (size_t)n ? bufsize - n : 0,
			    "%s%s", i > 0 ? ", " : "", mod->deps[i]);
	}
	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0, "\n");

	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Devices:    %d\n", mod->num_devices);

	n += snprintf(buf + n, bufsize > (size_t)n ? bufsize - n : 0,
	    "Params:     %d\n", mod->num_params);

	return n;
}

int drvmanager_foreach(int (*callback)(struct drvmanager_module *mod,
                       void *arg), void *arg)
{
	int count = 0;
	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (!g_modules[i].in_use) continue;
		count++;
		if (callback && callback(&g_modules[i], arg) != 0)
			break;
	}
	return count;
}

int drvmanager_count(void)
{
	int count = 0;
	for (int i = 0; i < DRVMANAGER_MAX_MODULES; i++) {
		if (g_modules[i].in_use) count++;
	}
	return count;
}

int drvmanager_set_param(const char *name, const char *key,
    const char *value)
{
	struct drvmanager_module *mod = find_module(name);
	if (!mod) return -ENOENT;
	if (mod->num_params >= DRVMANAGER_MAX_PARAMS) return -ENOSPC;

	struct drvmanager_param *p = &mod->params[mod->num_params];
	strncpy(p->name, key, sizeof(p->name) - 1);
	strncpy(p->value, value, sizeof(p->value) - 1);
	mod->num_params++;

	return 0;
}

/*===========================================================================*
 *		Batch / Parallel Init API (SMP)                        *
 *===========================================================================*/

int drvmanager_batch_init(int num_threads)
{
	return drmgr_pool_init(num_threads);
}

int drvmanager_batch_submit(const char *module_name)
{
	struct drvmanager_module *mod = drvmanager_find(module_name);
	if (!mod) return -ENOENT;
	if (mod->state != DRVMANAGER_STATE_LOADING) return -EALREADY;

	printf("drvmanager: submitting '%s' to parallel init pool\n",
	    module_name);
	return drmgr_pool_submit(module_name);
}

void drvmanager_batch_sync(void)
{
	drmgr_pool_sync();
}

int drvmanager_batch_wait(const char *module_name)
{
	return drmgr_pool_wait(module_name);
}

void drvmanager_batch_destroy(void)
{
	drmgr_pool_destroy();
}
