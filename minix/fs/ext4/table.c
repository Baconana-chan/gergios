/* table.c — ext4 filesystem server call table.
 *
 * Maps VFS requests (via libfsdriver) to ext4 operations.
 * Phase 4a: read path via Rust FFI (lookup, read, stat working).
 */

#include "ffi.h"
#include <minix/fsdriver.h>
#include <minix/libminixfs.h>
#include <minix/bdev.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/dirent.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <stdlib.h>
#include <time.h>

/* ─── Forward declarations ────────────────────────────────────────── */

static int  ext4_mount_cb(dev_t dev, unsigned int flags,
			   struct fsdriver_node *root_node,
			   unsigned int *res_flags);
static void ext4_unmount_cb(void);
static int  ext4_lookup_cb(ino_t dir_nr, char *name,
			   struct fsdriver_node *node, int *is_mountpt);
static int  ext4_putnode_cb(ino_t ino_nr, unsigned int count);static ssize_t ext4_read_cb(ino_t ino_nr, struct fsdriver_data *data,
			     size_t bytes, off_t pos, int call);
static ssize_t ext4_write_cb(ino_t ino_nr, struct fsdriver_data *data,
			      size_t bytes, off_t pos, int call);
static ssize_t ext4_getdents_cb(ino_t ino_nr, struct fsdriver_data *data,
				size_t bytes, off_t *pos, int call);
static int  ext4_stat_cb(ino_t ino_nr, struct stat *buf);
static int  ext4_statvfs_cb(struct statvfs *buf);
/* Phase 5: write operations */
static int  ext4_trunc_cb(ino_t ino_nr, off_t start, off_t end);
static int  ext4_link_cb(ino_t dir_nr, char *name, ino_t ino_nr);
static int  ext4_unlink_cb(ino_t dir_nr, char *name, int call);
static int  ext4_create_cb(ino_t dir_nr, char *name, mode_t mode,
			   uid_t uid, gid_t gid, struct fsdriver_node *node);
static int  ext4_mkdir_cb(ino_t dir_nr, char *name, mode_t mode,
			   uid_t uid, gid_t gid);
static int  ext4_rename_cb(ino_t old_dir, char *old_name,
			   ino_t new_dir, char *new_name);
static int  ext4_rmdir_cb(ino_t dir_nr, char *name, int call);
static void ext4_seek_cb(ino_t ino_nr);
/* Phase 6: metadata operations */
static int  ext4_chown_cb(ino_t ino_nr, uid_t uid, gid_t gid, mode_t *mode);
static int  ext4_chmod_cb(ino_t ino_nr, mode_t *mode);
static int  ext4_utime_cb(ino_t ino_nr, struct timespec *atime,
			  struct timespec *mtime);
static int  ext4_slink_cb(ino_t dir_nr, char *name, uid_t uid, gid_t gid,
			  struct fsdriver_data *data, size_t bytes);
static ssize_t ext4_rdlink_cb(ino_t ino_nr, struct fsdriver_data *data,
			       size_t bytes);
static int  ext4_mknod_cb(ino_t dir_nr, char *name, mode_t mode, uid_t uid,
			  gid_t gid, dev_t rdev);
static int  ext4_mountpt_cb(ino_t ino_nr);
static ssize_t ext4_peek_cb(ino_t ino_nr, struct fsdriver_data *data,
			    size_t bytes, off_t pos, int call);

static void ext4_sync_cb(void);
static void ext4_driver_cb(dev_t dev, char *label);
static ssize_t ext4_bread_cb(dev_t dev, struct fsdriver_data *data,
			     size_t bytes, off_t pos, int call);
static void ext4_bflush_cb(dev_t dev);

/* ─── Global io context (from main.c) ─────────────────────────────── */

extern struct ext4_io_ctx ext4_ctx;

/* Block read callback wrapper — defined in ffi_bridge.c */
extern int ext4_read_block_cb(void *, uint64_t, uint8_t *, uint32_t);

/* ─── fsdriver table ──────────────────────────────────────────────── */

struct fsdriver ext4_table = {
	.fdr_mount	= ext4_mount_cb,
	.fdr_unmount	= ext4_unmount_cb,
	.fdr_lookup	= ext4_lookup_cb,
	.fdr_putnode	= ext4_putnode_cb,
	.fdr_read	= ext4_read_cb,
	/* Phase 2 deferred: peek not supported yet */
	.fdr_write	= ext4_write_cb,
	.fdr_peek	= ext4_peek_cb,
	.fdr_getdents	= ext4_getdents_cb,
	.fdr_rmdir	= ext4_rmdir_cb,
	.fdr_mknod	= ext4_mknod_cb,
	.fdr_rename	= ext4_rename_cb,
	.fdr_create	= ext4_create_cb,
	.fdr_mkdir	= ext4_mkdir_cb,
	.fdr_slink	= ext4_slink_cb,
	.fdr_rdlink	= ext4_rdlink_cb,
	.fdr_chown	= ext4_chown_cb,
	.fdr_chmod	= ext4_chmod_cb,
	.fdr_utime	= ext4_utime_cb,
	.fdr_mountpt	= ext4_mountpt_cb,
	.fdr_seek	= ext4_seek_cb,
	.fdr_trunc	= ext4_trunc_cb,
	.fdr_trunc2	= ext4_trunc_cb,
	.fdr_link	= ext4_link_cb,
	.fdr_unlink	= ext4_unlink_cb,
	.fdr_stat	= ext4_stat_cb,
	.fdr_statvfs	= ext4_statvfs_cb,
	.fdr_sync	= ext4_sync_cb,
	.fdr_driver	= ext4_driver_cb,
	.fdr_bread	= ext4_bread_cb,
	.fdr_bwrite	= ext4_bread_cb,
	.fdr_bpeek	= ext4_bread_cb,
	.fdr_bflush	= ext4_bflush_cb,
};

/* ─── Helper: read a directory inode via FFI ─────────────────────── */

static int
read_inode_info(ino_t ino_nr, struct ext4_inode_info *info)
{
	return ext4_read_inode(&ext4_ctx.sbi, ino_nr, info,
			      (void *)&ext4_ctx, ext4_read_block_cb);
}

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

	/* Read root inode (inode 2) via Rust FFI */
	struct ext4_inode_info root_info;
	r = read_inode_info(EXT4_ROOT_INO, &root_info);
	if (r != OK) {
		printf("ext4: failed to read root inode: %d\n", r);
		bdev_close(dev);
		return r;
	}

	root_node->fn_ino_nr = EXT4_ROOT_INO;
	root_node->fn_mode   = root_info.mode;
	root_node->fn_size   = root_info.size;
	root_node->fn_uid    = root_info.uid;
	root_node->fn_gid    = root_info.gid;
	root_node->fn_dev    = NO_DEV;

	*res_flags = RES_NOFLAGS;

	printf("ext4: mounted, root ino=%u mode=0%o size=%llu "
	       "blocks_free=%llu inodes_free=%llu\n",
	       EXT4_ROOT_INO, root_info.mode,
	       (unsigned long long)root_info.size,
	       (unsigned long long)ext4_ctx.sbi.free_blocks_count,
	       (unsigned long long)ext4_ctx.sbi.free_inodes_count);

	/* Perform orphan inode cleanup if the filesystem was not cleanly
	 * unmounted (s_last_orphan != 0 implies there were in-flight
	 * unlink operations at the time of the last crash).
	 * Skip if mounted read-only (no write access to bitmaps). */
	if (ext4_ctx.sbi.last_orphan != 0 && !readonly) {
		extern int ext4_orphan_cleanup(struct ext4_io_ctx *);
		r = ext4_orphan_cleanup(&ext4_ctx);
		if (r != OK) {
			printf("ext4: orphan cleanup failed: %d\n", r);
			/* Non-fatal: we still mounted successfully */
		}
	}

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
	uint32_t out_ino;
	uint8_t out_type;

	int r = ext4_lookup(&ext4_ctx.sbi, dir_nr, name, &out_ino, &out_type,
			    (void *)&ext4_ctx, ext4_read_block_cb);
	if (r != OK) {
		return r;
	}

	/* Read the found inode to fill node info */
	struct ext4_inode_info info;
	r = read_inode_info(out_ino, &info);
	if (r != OK) {
		return r;
	}

	node->fn_ino_nr = out_ino;
	node->fn_mode   = info.mode;
	node->fn_size   = info.size;
	node->fn_uid    = info.uid;
	node->fn_gid    = info.gid;
	node->fn_dev    = NO_DEV;
	*is_mountpt = FALSE;

	return OK;
}

static int
ext4_putnode_cb(ino_t ino_nr, unsigned int count)
{
	/* No inode cache yet — always succeed */
	(void)ino_nr;
	(void)count;
	return OK;
}

/* ─── Phase 5: write operation callbacks ──────────────────────────── */

static ssize_t
ext4_write_cb(ino_t ino_nr, struct fsdriver_data *data,
	      size_t bytes, off_t pos, int call)
{
	uint32_t bytes_written = 0;
	uint8_t *buf;
	int r;

	if (bytes == 0) {
		return 0;
	}

	buf = malloc(bytes);
	if (buf == NULL) {
		return ENOMEM;
	}

	/* Copy user data from VFS */
	r = fsdriver_copyout(data, 0, buf, bytes);
	if (r != OK) {
		free(buf);
		return r;
	}

	r = ext4_write_file(&ext4_ctx.sbi, ino_nr, (uint64_t)pos,
			    buf, (uint32_t)bytes, &bytes_written,
			    (void *)&ext4_ctx,
			    ext4_read_block_cb,
			    ext4_write_block_cb,
			    ext4_alloc_block_cb);
	if (r != OK) {
		free(buf);
		return r;
	}

	free(buf);
	return (ssize_t)bytes_written;
}

static int
ext4_trunc_cb(ino_t ino_nr, off_t start, off_t end)
{
	/* MINIX fsdriver uses start/end for hole-punching;
	 * for truncation (shrinking), start=0, end=new_size.
	 * If start != 0 or end == 0, we honor the new size from end.
	 * Check if this is a truncation (start=0) or hole-punch (start>0).
	 * We only handle truncation (shrinking to size 'end'). */
	if (start != 0) {
		/* Hole-punching not yet implemented */
		return ENOTSUP;
	}

	int r = ext4_truncate(&ext4_ctx.sbi, ino_nr, (uint64_t)end,
			      (void *)&ext4_ctx,
			      ext4_read_block_cb,
			      ext4_write_block_cb,
			      ext4_free_blocks_cb);
	if (r != 0) {
		return r;
	}

	return OK;
}

static int
ext4_link_cb(ino_t dir_nr, char *name, ino_t ino_nr)
{
	/* Read the target inode to determine its mode (for file_type) */
	struct ext4_inode_info info;
	int r = read_inode_info(ino_nr, &info);
	if (r != OK) {
		return r;
	}

	r = ext4_link(&ext4_ctx.sbi, dir_nr, name, ino_nr, info.mode,
		      (void *)&ext4_ctx,
		      ext4_read_block_cb,
		      ext4_write_block_cb,
		      ext4_alloc_block_cb);
	if (r != 0) {
		return r;
	}

	return OK;
}

/* ─── Phase 6: create/mkdir callbacks ─────────────────────────────── */

static int
ext4_create_cb(ino_t dir_nr, char *name, mode_t mode,
	       uid_t uid, gid_t gid, struct fsdriver_node *node)
{
	uint32_t out_ino;

	int r = ext4_create(&ext4_ctx.sbi, dir_nr, name,
			    (uint16_t)(mode & 0xFFF), /* permission bits only */
			    (uint16_t)uid, (uint16_t)gid, &out_ino,
			    (void *)&ext4_ctx,
			    ext4_read_block_cb,
			    ext4_write_block_cb,
			    ext4_alloc_block_cb,
			    ext4_alloc_inode_cb);
	if (r != OK) {
		return r;
	}

	/* Read back the new inode to fill node info */
	struct ext4_inode_info info;
	r = read_inode_info(out_ino, &info);
	if (r != OK) {
		return r;
	}

	node->fn_ino_nr = out_ino;
	node->fn_mode   = info.mode;
	node->fn_size   = info.size;
	node->fn_uid    = info.uid;
	node->fn_gid    = info.gid;
	node->fn_dev    = NO_DEV;

	return OK;
}

static int
ext4_mkdir_cb(ino_t dir_nr, char *name, mode_t mode,
	      uid_t uid, gid_t gid)
{
	int r = ext4_mkdir(&ext4_ctx.sbi, dir_nr, name,
			   (uint16_t)(mode & 0xFFF), /* permission bits */
			   (uint16_t)uid, (uint16_t)gid,
			   (void *)&ext4_ctx,
			   ext4_read_block_cb,
			   ext4_write_block_cb,
			   ext4_alloc_block_cb,
			   ext4_alloc_inode_cb);
	if (r != OK) {
		return r;
	}

	return OK;
}

static int
ext4_rename_cb(ino_t old_dir, char *old_name,
	       ino_t new_dir, char *new_name)
{
	int r = ext4_rename(&ext4_ctx.sbi, old_dir, old_name,
			    new_dir, new_name,
			    (void *)&ext4_ctx,
			    ext4_read_block_cb,
			    ext4_write_block_cb,
			    ext4_alloc_block_cb,
			    ext4_free_blocks_cb,
			    ext4_free_inode_cb);
	if (r != 0) {
		return r;
	}

	return OK;
}

static int
ext4_unlink_cb(ino_t dir_nr, char *name, int call)
{
	/* call is REQ_UNLINK (regular) or REQ_RMDIR — handled by VFS, 
	 * the FS just unlinks the directory entry */
	(void)call;

	int r = ext4_unlink(&ext4_ctx.sbi, dir_nr, name,
			    (void *)&ext4_ctx,
			    ext4_read_block_cb,
			    ext4_write_block_cb,
			    ext4_free_blocks_cb,
			    ext4_free_inode_cb);
	if (r != 0) {
		return r;
	}

	return OK;
}

/* ─── Convert ext4 file_type to MINIX dirent type ─────────────────── */

static unsigned int
ext4_file_type_to_dt(uint8_t file_type)
{
	switch (file_type) {
	case EXT4_FT_REG_FILE:	return DT_REG;
	case EXT4_FT_DIR:	return DT_DIR;
	case EXT4_FT_SYMLINK:	return DT_LNK;
	case EXT4_FT_CHRDEV:	return DT_CHR;
	case EXT4_FT_BLKDEV:	return DT_BLK;
	case EXT4_FT_FIFO:	return DT_FIFO;
	case EXT4_FT_SOCK:	return DT_SOCK;
	default:		return DT_UNKNOWN;
	}
}

/* ─── getdents callback ──────────────────────────────────────────── */

#define GETDENTS_BUFSIZE (sizeof(struct dirent) + 255 + 1)
#define GETDENTS_ENTRIES 8

static ssize_t
ext4_getdents_cb(ino_t ino_nr, struct fsdriver_data *data,
		 size_t bytes, off_t *posp, int call)
{
	static char getdents_buf[GETDENTS_BUFSIZE * GETDENTS_ENTRIES];
	struct fsdriver_dentry fsdentry;
	uint64_t pos;
	uint32_t count, i;
	int r;
	ssize_t total;

	/* ext4 dirent entries buffer (filled by Rust FFI) */
	struct ext4_dirent entries[GETDENTS_ENTRIES];

	pos = (uint64_t)*posp;

	fsdriver_dentry_init(&fsdentry, data, bytes, getdents_buf,
			     sizeof(getdents_buf));

	for (;;) {
		r = ext4_readdir(&ext4_ctx.sbi, ino_nr, &pos,
				 entries, GETDENTS_ENTRIES, &count,
				 (void *)&ext4_ctx,
				 ext4_read_block_cb);
		if (r != OK) {
			return r;
		}

		if (count == 0) {
			break; /* EOF */
		}

		for (i = 0; i < count; i++) {
			struct ext4_dirent *ent = &entries[i];
			r = fsdriver_dentry_add(&fsdentry, ent->ino,
						ent->name, ent->name_len,
						ext4_file_type_to_dt(ent->file_type));
			if (r <= 0) {
				/* Buffer full or error */
				*posp = (off_t)pos;
				total = fsdriver_dentry_finish(&fsdentry);
				return total;
			}
		}
	}

	/* All entries processed */
	*posp = (off_t)pos;
	total = fsdriver_dentry_finish(&fsdentry);

	/* Update atime on the directory inode */
	/* TODO: atime update when Rust atime support is added */

	return total;
}

static int
ext4_rmdir_cb(ino_t dir_nr, char *name, int call)
{
	/* call is REQ_RMDIR — VFS-specific flag, FS just unlinks entry
	 * and frees the directory inode/data */
	(void)call;

	int r = ext4_rmdir(&ext4_ctx.sbi, dir_nr, name,
			   (void *)&ext4_ctx,
			   ext4_read_block_cb,
			   ext4_write_block_cb,
			   ext4_free_blocks_cb,
			   ext4_free_inode_cb);
	if (r != 0) {
		return r;
	}

	return OK;
}

static ssize_t
ext4_read_cb(ino_t ino_nr, struct fsdriver_data *data,
	     size_t bytes, off_t pos, int call)
{
	uint32_t bytes_read = 0;
	uint8_t *buf;
	int r;

	/* Zero-byte read — nothing to do */
	if (bytes == 0) {
		return 0;
	}

	/* Allocate buffer for reading */
	buf = malloc(bytes);
	if (buf == NULL) {
		return ENOMEM;
	}

	/* Read via Rust FFI */
	r = ext4_read_file(&ext4_ctx.sbi, ino_nr, (uint64_t)pos,
			   buf, (uint32_t)bytes, &bytes_read,
			   (void *)&ext4_ctx, ext4_read_block_cb);
	if (r != OK) {
		free(buf);
		return r;
	}

	/* Copy data to user space */
	if (bytes_read > 0) {
		r = fsdriver_copyin(data, 0, buf, bytes_read);
		if (r != OK) {
			free(buf);
			return r;
		}
	}

	free(buf);
	return (ssize_t)bytes_read;
}

static int
ext4_stat_cb(ino_t ino_nr, struct stat *buf)
{
	struct ext4_inode_info info;
	int r = read_inode_info(ino_nr, &info);
	if (r != OK) {
		return r;
	}

	memset(buf, 0, sizeof(*buf));
	buf->st_mode   = info.mode;
	buf->st_nlink  = info.links_count;
	buf->st_size   = info.size;
	buf->st_uid    = info.uid;
	buf->st_gid    = info.gid;
	buf->st_blksize = ext4_ctx.sbi.block_size;
	buf->st_blocks = info.blocks;
	buf->st_atime  = info.atime;
	buf->st_ctime  = info.ctime;
	buf->st_mtime  = info.mtime;
	buf->st_dev    = ext4_ctx.dev;
	buf->st_ino    = ino_nr;

	return OK;
}

static int
ext4_statvfs_cb(struct statvfs *buf)
{
	memset(buf, 0, sizeof(*buf));
	buf->f_bsize   = ext4_ctx.sbi.block_size;
	buf->f_frsize  = ext4_ctx.sbi.block_size;
	buf->f_blocks  = ext4_ctx.sbi.blocks_count;
	buf->f_bfree   = ext4_ctx.sbi.free_blocks_count;
	buf->f_bavail  = ext4_ctx.sbi.free_blocks_count;
	buf->f_files   = ext4_ctx.sbi.inodes_count;
	buf->f_ffree   = ext4_ctx.sbi.free_inodes_count;
	buf->f_favail  = ext4_ctx.sbi.free_inodes_count;
	buf->f_namemax = 255;
	return OK;
}

/* ─── Seek callback — update atime ──────────────────────────────── */

static void
ext4_seek_cb(ino_t ino_nr)
{
	/* VFS calls fdr_seek on lseek/llseek to notify the FS.
	 * ext2 uses this merely to inhibit read-ahead (i_seek = ISEEK).
	 * Since ext4 doesn't use MINIX's buffer cache for inodes,
	 * we update atime to match POSIX semantics.
	 *
	 * We read the inode first to preserve mtime (only atime changes).
	 * Atime update is best-effort: failures are silently ignored. */
	struct ext4_inode_info info;
	int r;

	r = read_inode_info(ino_nr, &info);
	if (r != OK) {
		return;
	}

	ext4_utime(&ext4_ctx.sbi, ino_nr,
		    (uint32_t)time(NULL),	/* atime = now */
		    info.mtime,			/* mtime unchanged */
		    (void *)&ext4_ctx,
		    ext4_read_block_cb, ext4_write_block_cb);
}

/* ─── Phase 6: metadata operation callbacks ──────────────────────── */

static int
ext4_chown_cb(ino_t ino_nr, uid_t uid, gid_t gid, mode_t *mode)
{
	uint16_t m = (uint16_t)*mode;
	int r = ext4_chown(&ext4_ctx.sbi, ino_nr, (uint16_t)uid, (uint16_t)gid,
			   &m, (void *)&ext4_ctx,
			   ext4_read_block_cb, ext4_write_block_cb);
	if (r != OK) {
		return r;
	}
	*mode = (mode_t)m;
	return OK;
}

static int
ext4_chmod_cb(ino_t ino_nr, mode_t *mode)
{
	uint16_t m = (uint16_t)*mode;
	int r = ext4_chmod(&ext4_ctx.sbi, ino_nr, &m,
			   (void *)&ext4_ctx,
			   ext4_read_block_cb, ext4_write_block_cb);
	if (r != OK) {
		return r;
	}
	*mode = (mode_t)m;
	return OK;
}

static int
ext4_utime_cb(ino_t ino_nr, struct timespec *atime, struct timespec *mtime)
{
	return ext4_utime(&ext4_ctx.sbi, ino_nr,
			  (uint32_t)atime->tv_sec, (uint32_t)mtime->tv_sec,
			  (void *)&ext4_ctx,
			  ext4_read_block_cb, ext4_write_block_cb);
}

static int
ext4_mknod_cb(ino_t dir_nr, char *name, mode_t mode, uid_t uid, gid_t gid,
	      dev_t rdev)
{
	return ext4_mknod(&ext4_ctx.sbi, dir_nr, name,
			  (uint16_t)mode, (uint16_t)uid, (uint16_t)gid,
			  (uint32_t)rdev,
			  (void *)&ext4_ctx,
			  ext4_read_block_cb, ext4_write_block_cb,
			  ext4_alloc_block_cb, ext4_alloc_inode_cb);
}

static int
ext4_slink_cb(ino_t dir_nr, char *name, uid_t uid, gid_t gid,
	      struct fsdriver_data *data, size_t bytes)
{
	char *target;
	int r;

	target = malloc(bytes + 1);
	if (target == NULL) {
		return ENOMEM;
	}

	r = fsdriver_copyin(data, 0, target, bytes);
	if (r != OK) {
		free(target);
		return r;
	}
	target[bytes] = '\0';

	r = ext4_symlink(&ext4_ctx.sbi, dir_nr, name, target,
			 (uint16_t)uid, (uint16_t)gid,
			 (void *)&ext4_ctx,
			 ext4_read_block_cb, ext4_write_block_cb,
			 ext4_alloc_block_cb, ext4_alloc_inode_cb);

	free(target);
	return r;
}

static ssize_t
ext4_rdlink_cb(ino_t ino_nr, struct fsdriver_data *data, size_t bytes)
{
	uint32_t bytes_read = 0;
	uint8_t *buf;
	int r;

	buf = malloc(bytes);
	if (buf == NULL) {
		return ENOMEM;
	}

	r = ext4_readlink(&ext4_ctx.sbi, ino_nr, buf, (uint32_t)bytes,
			  &bytes_read, (void *)&ext4_ctx,
			  ext4_read_block_cb);
	if (r != OK) {
		free(buf);
		return r;
	}

	if (bytes_read > 0) {
		r = fsdriver_copyin(data, 0, buf, bytes_read);
		if (r != OK) {
			free(buf);
			return r;
		}
	}

	free(buf);
	return (ssize_t)bytes_read;
}

static int
ext4_mountpt_cb(ino_t ino_nr)
{
	/* ext4 doesn't support mount points inside the FS */
	(void)ino_nr;
	return FALSE;
}

static ssize_t
ext4_peek_cb(ino_t ino_nr, struct fsdriver_data *data,
	     size_t bytes, off_t pos, int call)
{
	/* Peek is like read — tells VM about block allocation.
	 * We read the data to determine which blocks exist.
	 * If the file has holes, ext4_read_file will read zeros.
	 * For now, just read the data (same as normal read). */
	return ext4_read_cb(ino_nr, data, bytes, pos, call);
}

static void
ext4_sync_cb(void)
{
	/* TODO: sync journal */
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
