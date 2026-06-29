/* main.c — ext4 filesystem server entry point.
 *
 * This is a MINIX FS server using libfsdriver, following the same
 * pattern as minix/fs/ext2/main.c.
 */

#include <minix/drivers.h>
#include <minix/fsdriver.h>
#include <minix/optset.h>
#include <minix/libminixfs.h>
#include <minix/bdev.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "ffi.h"

/* ─── Global state ────────────────────────────────────────────────── */

static struct ext4_io_ctx ext4_ctx;

/* SEF functions */
static void sef_local_startup(void);
static int sef_cb_init_fresh(int type, sef_init_info_t *info);
static void sef_cb_signal_handler(int signo);

/* ─── Main ────────────────────────────────────────────────────────── */

int
main(int argc, char *argv[])
{
	env_setargs(argc, argv);
	sef_local_startup();

	/* The fsdriver library dispatches VFS requests.
	 * ext4_table is defined in table.c */
	extern struct fsdriver ext4_table;
	fsdriver_task(&ext4_table);

	return 0;
}

/* ─── SEF startup ─────────────────────────────────────────────────── */

static void
sef_local_startup(void)
{
	sef_setcb_init_fresh(sef_cb_init_fresh);
	sef_setcb_signal_handler(sef_cb_signal_handler);
	sef_startup();
}

static int
sef_cb_init_fresh(int UNUSED(type), sef_init_info_t *UNUSED(info))
{
	printf("ext4: GergiOS ext4 filesystem server starting\n");

	lmfs_may_use_vmcache(1);
	lmfs_buf_pool(10);

	return OK;
}

static void
sef_cb_signal_handler(int signo)
{
	if (signo != SIGTERM) return;

	/* TODO: fs_sync */
	fsdriver_terminate();
}
