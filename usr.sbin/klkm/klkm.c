/* klkm.c — GergiOS Kernel Loadable Kernel Module Manager
 *
 * Single binary that serves dual purpose:
 *   1. Daemon mode (invoked as "klkm"): MINIX service managing all
 *      loaded kernel modules via IPC protocol (minix/klkm.h)
 *   2. CLI tool mode (symlinked as modprobe, insmod, rmmod):
 *      communicates with daemon via IPC
 *
 * Links libgergios_driver for ELF loader, kernel shim, modprobe, drvmanager.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/ipc.h>
#include <minix/sef.h>
#include <minix/rs.h>
#include <minix/ds.h>
#include <minix/com.h>
#include <minix/klkm.h>

/* libgergios_driver */
#include "modprobe.h"
#include "drvmanager.h"
#include "gergios_driver.h"
#include "gergios_device.h"

/*===========================================================================*
 *      DAEMON MODE — SEF callbacks                                        *
 *===========================================================================*/

static int sef_cb_init_fresh(int type __unused,
    sef_init_info_t *info __unused)
{
    printf("klkm: initialising GergiOS Driver Manager\n");

    drvmanager_init();

    int n = modprobe_init();
    printf("klkm: modprobe loaded %d entries\n", n >= 0 ? n : 0);

    return OK;
}

static void sef_local_startup(void)
{
    sef_setcb_init_fresh(sef_cb_init_fresh);
    sef_setcb_init_restart(sef_cb_init_fresh);
    sef_startup();
}

/*===========================================================================*
 *      DAEMON MODE — IPC handler helpers                                  *
 *===========================================================================*/

struct list_callback_arg {
    char *buf;
    size_t size;
    int pos;
};

static int list_callback(struct drvmanager_module *mod, void *arg)
{
    struct list_callback_arg *ca = (struct list_callback_arg *)arg;
    int n;

    const char *state_str;
    switch (mod->state) {
    case DRVMANAGER_STATE_LOADING:  state_str = "Loading "; break;
    case DRVMANAGER_STATE_LOADED:   state_str = "Loaded  "; break;
    case DRVMANAGER_STATE_ACTIVE:   state_str = "Active  "; break;
    case DRVMANAGER_STATE_FAILED:   state_str = "Failed  "; break;
    case DRVMANAGER_STATE_UNLOADING:state_str = "Unload  "; break;
    default:                        state_str = "Unknown "; break;
    }

    const char *type_str = (mod->type == DRVMANAGER_TYPE_KO) ? ".ko" : "native";

    n = snprintf(ca->buf + ca->pos, ca->size > (size_t)ca->pos ?
        ca->size - (size_t)ca->pos : 0,
        "%-20s %s ref=%d %s\n",
        mod->name, state_str, mod->refcount, type_str);

    if (n > 0)
        ca->pos += n;

    return 0;  /* continue iteration */
}

/*===========================================================================*
 *      DAEMON MODE — IPC handlers                                         *
 *===========================================================================*/

static int handle_load_name(const message *m_in)
{
    char name[KLKM_STR_MAX];
    strncpy(name, m_in->KLKM_STR, sizeof(name) - 1);
    name[sizeof(name) - 1] = '\0';

    printf("klkm: load by name '%s'\n", name);
    int r = modprobe_by_name(name);
    if (r != 0)
        printf("klkm: modprobe_by_name('%s') failed: %d\n", name, r);
    return r;
}

static int handle_load_ko(const message *m_in)
{
    char path[KLKM_STR_MAX];
    strncpy(path, m_in->KLKM_STR, sizeof(path) - 1);
    path[sizeof(path) - 1] = '\0';

    printf("klkm: load .ko '%s'\n", path);
    int r = drvmanager_load_ko(path);
    if (r != 0)
        printf("klkm: drvmanager_load_ko('%s') failed: %d\n", path, r);
    return r;
}

static int handle_unload(const message *m_in)
{
    char name[KLKM_STR_MAX];
    strncpy(name, m_in->KLKM_STR, sizeof(name) - 1);
    name[sizeof(name) - 1] = '\0';

    printf("klkm: unload '%s'\n", name);
    int r = drvmanager_unload(name);
    if (r != 0)
        printf("klkm: drvmanager_unload('%s') failed: %d\n", name, r);
    return r;
}

static int handle_list(const message *m_in, message *m_out)
{
    int count = drvmanager_count();
    m_out->KLKM_COUNT_VAL = count;

    /* Get caller-provided buffer for the formatted list */
    endpoint_t caller = m_in->m_source;
    vir_bytes caller_buf = (vir_bytes)m_in->KLKM_BUF;
    size_t buf_size = (size_t)m_in->KLKM_BUF_SIZE;

    if (!caller_buf || buf_size == 0) {
        /* No buffer provided — just return count */
        return 0;
    }

    /* Build formatted list in a local buffer */
    char local_buf[4096];
    size_t limit = buf_size < sizeof(local_buf) ? buf_size : sizeof(local_buf);
    local_buf[0] = '\0';

    if (count > 0) {
        struct list_callback_arg ca;
        ca.buf = local_buf;
        ca.size = limit;
        ca.pos = 0;
        drvmanager_foreach(list_callback, &ca);
    } else {
        snprintf(local_buf, limit, "(no modules loaded)\n");
    }

    /* Write to caller's buffer via sys_datacopy */
    int r = sys_datacopy(SELF, (vir_bytes)local_buf,
        caller, caller_buf, strlen(local_buf) + 1);
    if (r != OK) {
        printf("klkm: sys_datacopy for list failed: %d\n", r);
    }

    return 0;
}

static int handle_count(message *m_out)
{
    m_out->KLKM_COUNT_VAL = drvmanager_count();
    return 0;
}

static int handle_status(const message *m_in, message *m_out)
{
    char name[KLKM_STR_MAX];
    char status_buf[KLKM_STR_MAX];

    strncpy(name, m_in->KLKM_STR, sizeof(name) - 1);
    name[sizeof(name) - 1] = '\0';

    int r = drvmanager_status(name, status_buf, sizeof(status_buf));
    if (r < 0) {
        snprintf(status_buf, sizeof(status_buf),
            "module '%s' not found", name);
        m_out->KLKM_STATE_VAL = -1;
    } else {
        m_out->KLKM_STATE_VAL = 0;
    }

    strncpy(m_out->KLKM_RESP_STR, status_buf, sizeof(status_buf));
    return 0;
}

/*===========================================================================*
 *      DAEMON MODE — main loop                                            *
 *===========================================================================*/

static int daemon_main(void)
{
    message m_in, m_out;
    int r, ipc_status;

    sef_local_startup();

    printf("klkm: GergiOS Driver Manager daemon ready\n");
    printf("klkm: accepting IPC requests (protocol 0x%04X-0x%04X)\n",
        KLKM_LOAD_NAME, KLKM_COUNT);

    for (;;) {
        memset(&m_out, 0, sizeof(m_out));

        r = sef_receive_status(ANY, &m_in, &ipc_status);
        if (r != OK) {
            printf("klkm: sef_receive failed: %d\n", r);
            continue;
        }

        if (is_ipc_notify(ipc_status)) {
            printf("klkm: notification from %d\n", m_in.m_source);
            continue;
        }

        if (IPC_STATUS_CALL(ipc_status) != SENDREC) {
            m_out.m_type = EDONTREPLY;
            continue;
        }

        switch (m_in.m_type) {
        case KLKM_LOAD_NAME: r = handle_load_name(&m_in); break;
        case KLKM_LOAD_KO:   r = handle_load_ko(&m_in);   break;
        case KLKM_UNLOAD:    r = handle_unload(&m_in);    break;
        case KLKM_LIST:      r = handle_list(&m_in, &m_out); break;
        case KLKM_COUNT:     r = handle_count(&m_out);    break;
        case KLKM_STATUS:    r = handle_status(&m_in, &m_out); break;
        default:
            printf("klkm: unknown request type 0x%x from %d\n",
                m_in.m_type, m_in.m_source);
            r = ENOSYS;
            break;
        }

        m_out.m_type = r;
        if ((r = ipc_sendnb(m_in.m_source, &m_out)) != OK)
            printf("klkm: ipc_sendnb failed: %d\n", r);
    }

    return 0;
}

/*===========================================================================*
 *      CLI TOOL MODE — communication with daemon                          *
 *===========================================================================*/

static endpoint_t klkm_endpoint = NONE;

static int find_klkm(void)
{
    if (klkm_endpoint != NONE) return OK;
    if (minix_rs_lookup(KLKM_SERVICE_NAME, &klkm_endpoint) != OK) {
        fprintf(stderr, "error: KLKM service '%s' not running\n",
            KLKM_SERVICE_NAME);
        return ENOENT;
    }
    return OK;
}

static int klkm_request(int cmd, const char *str, message *m_out)
{
    int r;
    message m_in;

    r = find_klkm();
    if (r != OK) return r;

    memset(&m_in, 0, sizeof(m_in));
    m_in.m_type = cmd;
    if (str) {
        strncpy(m_in.KLKM_STR, str, KLKM_STR_MAX - 1);
        m_in.KLKM_STR[KLKM_STR_MAX - 1] = '\0';
    }

    if ((r = ipc_sendrec(klkm_endpoint, &m_in)) != OK) {
        fprintf(stderr, "error: IPC to KLKM failed: %d\n", r);
        return r;
    }

    if (m_out) *m_out = m_in;
    return m_in.m_type;  /* 0 = OK, negative = errno */
}

static void usage_modprobe(void)
{
    fprintf(stderr,
        "Usage: modprobe <name>         Load driver by name\n"
        "       modprobe -r <name>      Unload driver\n"
        "       modprobe -l             List loaded modules\n"
        "       modprobe -s <name>      Show module status\n");
}

static int tool_modprobe(int argc, char *argv[])
{
    message m;
    int r;

    if (argc < 2) { usage_modprobe(); return 1; }

    if (strcmp(argv[1], "-r") == 0) {
        if (argc < 3) { usage_modprobe(); return 1; }
        r = klkm_request(KLKM_UNLOAD, argv[2], &m);
        if (r == 0) printf("modprobe: unloaded '%s'\n", argv[2]);
        else fprintf(stderr, "modprobe: unload '%s' failed: %d\n",
            argv[2], r);
        return r ? 1 : 0;
    } else if (strcmp(argv[1], "-l") == 0) {
        {
            char list_buf[4096];
            memset(&m, 0, sizeof(m));
            m.m_type = KLKM_LIST;
            m.KLKM_BUF = (char *)list_buf;
            m.KLKM_BUF_SIZE = sizeof(list_buf);
            r = ipc_sendrec(klkm_endpoint, &m);
            if (r != OK) {
                fprintf(stderr, "modprobe: IPC error: %d\n", r);
                return 1;
            }
            if (m.m_type != 0) {
                fprintf(stderr, "modprobe: list failed: %d\n", m.m_type);
                return 1;
            }
            printf("=== Loaded Kernel Modules ===\n");
            printf("Count: %d\n", m.KLKM_COUNT_VAL);
            printf("%s", list_buf);
        }
        return 0;
    } else if (strcmp(argv[1], "-s") == 0) {
        if (argc < 3) { usage_modprobe(); return 1; }
        r = klkm_request(KLKM_STATUS, argv[2], &m);
        if (r == 0) printf("%s\n", m.KLKM_RESP_STR);
        else fprintf(stderr, "modprobe: status '%s' failed: %d\n",
            argv[2], r);
        return r ? 1 : 0;
    } else {
        r = klkm_request(KLKM_LOAD_NAME, argv[1], &m);
        if (r == 0) printf("modprobe: loaded '%s'\n", argv[1]);
        else fprintf(stderr, "modprobe: load '%s' failed: %d\n",
            argv[1], r);
        return r ? 1 : 0;
    }
}

static int tool_insmod(int argc, char *argv[])
{
    message m;
    if (argc < 2) {
        fprintf(stderr, "Usage: insmod <path.ko>\n");
        return 1;
    }
    int r = klkm_request(KLKM_LOAD_KO, argv[1], &m);
    if (r == 0) printf("insmod: loaded '%s'\n", argv[1]);
    else fprintf(stderr, "insmod: '%s' failed: %d\n", argv[1], r);
    return r ? 1 : 0;
}

static int tool_rmmod(int argc, char *argv[])
{
    message m;
    if (argc < 2) {
        fprintf(stderr, "Usage: rmmod <name>\n");
        return 1;
    }
    int r = klkm_request(KLKM_UNLOAD, argv[1], &m);
    if (r == 0) printf("rmmod: unloaded '%s'\n", argv[1]);
    else fprintf(stderr, "rmmod: '%s' failed: %d\n", argv[1], r);
    return r ? 1 : 0;
}

/*===========================================================================*
 *      ENTRY POINT — dispatches based on argv[0]                          *
 *===========================================================================*/

int main(int argc, char *argv[])
{
    const char *prog = strrchr(argv[0], '/');
    prog = prog ? prog + 1 : argv[0];

    if (strcmp(prog, "modprobe") == 0)
        return tool_modprobe(argc, argv);
    else if (strcmp(prog, "insmod") == 0)
        return tool_insmod(argc, argv);
    else if (strcmp(prog, "rmmod") == 0)
        return tool_rmmod(argc, argv);
    else
        return daemon_main();
}
