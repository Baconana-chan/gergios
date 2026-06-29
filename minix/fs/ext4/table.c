/* table.c — ext4 filesystem server call table.
 *
 * Maps VFS requests (via libfsdriver) to ext4 operations.
 * Phase 1: read-only stubs.
 */

#include "ffi.h"
#include <minix/fsdriver.h>
#include <minix/libminixfs.h>
#include <minix/bdev.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <stdio.h>
#include <errno.h>

/* ─── Forward declarations ────────────────────────────────────────── */

static int  ext4_mount_cb(dev_t dev, unsigned int flags,
			   struct fsdriver_node *root_node,
			   unsigned int *res_flags);
static void ext4_unmount_cb(void);
static int  ext4_lookup_cb(ino_t dir_nr, char *name,
			   struct fsdriver_node *node, int *is_mountpt);
static int  ext4_putnode_cb(ino_t ino_nr, unsigned int count);
static ssize_t ext4_read_cb(ino_t ino_nr, struct fsdriver_data *data,
			    size_t bytes, off_t pos, int call);
static int  ext4_stat_cb(ino_t ino_nr, struct stat *buf);
static int  ext4_statvfs_cb(struct statvfs *buf);
static void ext4_sync_cb(void);
static void ext4_driver_cb(dev_t dev, char *label);
static ssize_t ext4_bread_cb(dev_t dev, struct fsdriver_data *data,
			     size_t bytes, off_t pos, int call);
static void ext4_bflush_cb(dev_t dev);

/* ─── Global io context (from main.c) ─────────────────────────────── */

extern struct ext4_io_ctx ext4_ctx;

/* ─── fsdriver table ──────────────────────────────────────────────── */

struct fsdriver ext4_table = {
	.fdr_mount	= ext4_mount_cb,
	.fdr_unmount	= ext4_unmount_cb,
	.fdr_lookup	= ext4_lookup_cb,
	.fdr_putnode	= ext4_putnode_cb,
	.fdr_read	= ext4_read_cb,
	/* Phase 1: write/peek/trunc not supported yet */
	/* Phase 1: create/mkdir/mknod/link/unlink/rmdir/rename not supported yet */
	/* Phase 1: slink/rdlink/chown/chmod/utime not supported */
	.fdr_stat	= ext4_stat_cb,
	.fdr_statvfs	= ext4_statvfs_cb,
	.fdr_sync	= ext4_sync_cb,
	.fdr_driver	= ext4_driver_cb,
	.fdr_bread	= ext4_bread_cb,
	/* fdr_bwrite, fdr_bpeek use lmfs_bio too */
	.fdr_bwrite	= ext4_bread_cb,
	.fdr_bpeek	= ext4_bread_cb,
	.fdr_bflush	= ext4_bflush_cb,
};

/* ─── Callback implementations ────────────────────────────────────── */

static int
ext4_mount_cb(dev_t dev, unsigned int flags,
	      struct fsdriver_node *root_node,
	      unsigned int *res_flags)
{
	int r, readonly;
	extern int ext4_read_super(struct ext4_io_ctx *);

	printf("ext4: mount called, dev=%llx\n", (unsigned long long)dev);

	readonly = (flags & REQ_RDONLY) ? 1 : 0;

	/* Open the block device */
	if (bdev_open(dev, readonly ? BDEV_R_BIT : (BDEV_R_BIT|BDEV_W_BIT))
	    != OK) {
		return EINVAL;
	}

	ext4_ctx.dev = dev;

	/* Read and parse the superblock */
	r = ext4_read_super(&ext4_ctx);
	if (r != OK) {
		bdev_close(dev);
		return r;
	}

	/* Set up block cache */
	lmfs_set_blocksize(ext4_ctx.sbi.block_size);

	/* Get root inode info */
	/* TODO: read inode 2 (EXT4_ROOT_INO) via Rust FFI */
	root_node->fn_ino_nr = 2;	/* ext4 root inode */
	root_node->fn_mode   = S_IFDIR | 0755;
	root_node->fn_size   = 1024;
	root_node->fn_uid    = 0;
	root_node->fn_gid    = 0;
	root_node->fn_dev    = NO_DEV;

	*res_flags = RES_NOFLAGS;

	printf("ext4: mounted successfully (readonly=%d)\n", readonly);
	return OK;
}

static void
ext4_unmount_cb(void)
{
	printf("ext4: unmount\n");
	bdev_close(ext4_ctx.dev);
}

static int
ext4_lookup_cb(ino_t dir_nr, char *name,
	       struct fsdriver_node *node, int *is_mountpt)
{
	/* Phase 1: stub — lookup not yet implemented */
	printf("ext4: lookup '%s' in dir %llu (stub)\n",
	       name, (unsigned long long)dir_nr);
	return ENOENT;
}

static int
ext4_putnode_cb(ino_t ino_nr, unsigned int count)
{
	/* Phase 1: stub */
	return OK;
}

static ssize_t
ext4_read_cb(ino_t ino_nr, struct fsdriver_data *data,
	     size_t bytes, off_t pos, int call)
{
	/* Phase 1: stub */
	printf("ext4: read ino=%llu pos=%lld bytes=%zu (stub)\n",
	       (unsigned long long)ino_nr, (long long)pos, bytes);
	return 0;
}

static int
ext4_stat_cb(ino_t ino_nr, struct stat *buf)
{
	/* Phase 1: stub — return minimal stat for root */
	if (ino_nr == 2) {
		memset(buf, 0, sizeof(*buf));
		buf->st_mode  = S_IFDIR | 0755;
		buf->st_nlink = 2;
		buf->st_size  = 1024;
		buf->st_blksize = ext4_ctx.sbi.block_size;
		return OK;
	}
	return ENOENT;
}

static int
ext4_statvfs_cb(struct statvfs *buf)
{
	memset(buf, 0, sizeof(*buf));
	buf->f_bsize  = ext4_ctx.sbi.block_size;
	buf->f_frsize = ext4_ctx.sbi.block_size;
	buf->f_blocks = ext4_ctx.sbi.blocks_count;
	buf->f_bfree  = ext4_ctx.sbi.blocks_count; /* XXX: no free tracking yet */
	buf->f_files  = ext4_ctx.sbi.inodes_count;
	buf->f_ffree  = ext4_ctx.sbi.inodes_count; /* XXX */
	buf->f_namemax = 255;
	return OK;
}

static void
ext4_sync_cb(void)
{
	/* Phase 1: no-op */
}

static void
ext4_driver_cb(dev_t dev, char *label)
{
	lmfs_driver(dev, label);
}

static ssize_t
ext4_bread_cb(dev_t dev, struct fsdriver_data *data,
	      size_t bytes, off_t pos, int call)
{
	return lmfs_bio(dev, data, bytes, pos, call);
}

static void
ext4_bflush_cb(dev_t dev)
{
	lmfs_bflush(dev);
}
