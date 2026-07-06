/* mfsfuse — FUSE adapter for read-only access to MINIX MFS partitions.
 *
 * Mount an MFS partition via FUSE:
 *   mfsfuse /dev/c0d0p1 /mnt/mfs
 *   ls -la /mnt/mfs
 *   cat /mnt/mfs/somefile
 *   fusermount -u /mnt/mfs
 *
 * Supports MFS V3 (magic 0x4d5a) read-only.
 * Uses POSIX pread() for raw block device access.
 * Block/zone mapping handles direct, indirect, and double-indirect blocks.
 */

#include <fuse.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <limits.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/statvfs.h>

/* ========================================================================
 * MFS on-disk format constants and structures
 * ======================================================================== */

#define MFS_SUPER_MAGIC_V3  0x4d5a
#define SUPER_BLOCK_BYTES   1024   /* superblock lives at offset 1024 */
#define START_BLOCK         2      /* first block after boot+super */
#define MFS_DIRSIZ          60     /* max filename length in dir entry */
#define V2_NR_DZONES        7      /* number of direct zones in inode */
#define V2_NR_TZONES        10     /* total zone entries in inode */
#define ROOT_INODE          1      /* inode number of root directory */

/* Superblock (MFS V3 on-disk format) */
struct mfs_sb {
	uint32_t  s_ninodes;
	uint16_t  s_nzones;
	int16_t   s_imap_blocks;
	int16_t   s_zmap_blocks;
	uint16_t  s_firstdatazone_old;
	int16_t   s_log_zone_size;
	uint16_t  s_flags;
	int32_t   s_max_size;
	uint32_t  s_zones;
	uint16_t  s_magic;
	int16_t   s_pad2;
	uint16_t  s_block_size;
	uint8_t   s_disk_version;
} __attribute__((packed));

/* Inode (V2 on-disk format, used by MFS V3) */
struct mfs_inode {
	uint16_t  i_mode;
	uint16_t  i_nlinks;
	uint16_t  i_uid;
	uint16_t  i_gid;
	int32_t   i_size;
	int32_t   i_atime;
	int32_t   i_mtime;
	int32_t   i_ctime;
	uint32_t  i_zone[V2_NR_TZONES];
} __attribute__((packed));

/* Directory entry */
struct mfs_dirent {
	uint32_t  d_ino;
	char      d_name[MFS_DIRSIZ];
} __attribute__((packed));

/* Entry returned by path resolution */
struct mfs_entry {
	uint32_t         ino;
	struct mfs_inode inode;
};

/* ========================================================================
 * mfsfuse context — per-mount state
 * ======================================================================== */

struct mfsfuse_ctx {
	int            fd;              /* block device fd */
	unsigned int   block_size;
	unsigned int   imap_blocks;
	unsigned int   zmap_blocks;
	unsigned int   ndzones;         /* = V2_NR_DZONES = 7 */
	unsigned int   nindirs;         /* zones per indirect block */
	unsigned int   inodes_per_block;
	uint32_t       ninodes;
	uint32_t       zones;
	uint32_t       first_datazone;
	uint32_t       inode_start;     /* block where inode table starts */
};

static struct mfsfuse_ctx *g_ctx; /* set in main(), used by FUSE callbacks */

/* ========================================================================
 * Block I/O
 * ======================================================================== */

static int
read_block(struct mfsfuse_ctx *ctx, uint32_t block_nr, void *buf)
{
	off_t offset = (off_t)block_nr * ctx->block_size;
	ssize_t n;

	n = pread(ctx->fd, buf, ctx->block_size, offset);
	if (n < 0)
		return -errno;
	if ((size_t)n != ctx->block_size)
		return -EIO;
	return 0;
}

/* ========================================================================
 * Superblock parsing
 * ======================================================================== */

static int
read_superblock(struct mfsfuse_ctx *ctx)
{
	uint8_t sbuf[SUPER_BLOCK_BYTES + sizeof(struct mfs_sb)];
	struct mfs_sb *sb = (struct mfs_sb *)(sbuf + SUPER_BLOCK_BYTES);
	int r;

	r = read_block(ctx, 0, sbuf);
	if (r != 0)
		return r;

	if (sb->s_magic != MFS_SUPER_MAGIC_V3) {
		fprintf(stderr, "mfsfuse: not MFS V3 (magic=0x%04x)\n", sb->s_magic);
		return -EINVAL;
	}

	ctx->block_size    = sb->s_block_size;
	ctx->imap_blocks   = (unsigned int)sb->s_imap_blocks;
	ctx->zmap_blocks   = (unsigned int)sb->s_zmap_blocks;
	ctx->ndzones       = V2_NR_DZONES;
	ctx->nindirs       = ctx->block_size / sizeof(uint32_t);
	ctx->ninodes       = sb->s_ninodes;
	ctx->zones         = sb->s_zones;

	if (ctx->block_size < 512 || ctx->block_size > 65536 ||
	    (ctx->block_size % 512) != 0) {
		fprintf(stderr, "mfsfuse: invalid block size %u\n", ctx->block_size);
		return -EINVAL;
	}

	ctx->inodes_per_block = ctx->block_size / sizeof(struct mfs_inode);

	if (sb->s_firstdatazone_old != 0) {
		ctx->first_datazone = sb->s_firstdatazone_old;
	} else {
		uint32_t off = START_BLOCK + ctx->imap_blocks + ctx->zmap_blocks;
		off += (ctx->ninodes + ctx->inodes_per_block - 1) /
		       ctx->inodes_per_block;
		ctx->first_datazone = off;
	}

	ctx->inode_start = START_BLOCK + ctx->imap_blocks + ctx->zmap_blocks;

	fprintf(stderr, "mfsfuse: MFS V3, block_size=%u, zones=%u, "
		"inodes=%u, first_datazone=%u\n",
		ctx->block_size, ctx->zones, ctx->ninodes, ctx->first_datazone);
	return 0;
}

/* ========================================================================
 * Inode reading
 * ======================================================================== */

static int
read_inode(struct mfsfuse_ctx *ctx, uint32_t ino, struct mfs_inode *inode)
{
	uint8_t buf[ctx->block_size];
	uint32_t block, offset;
	int r;

	if (ino == 0 || ino > ctx->ninodes)
		return -ENOENT;

	ino--; /* convert to 0-based index */
	block  = ctx->inode_start + ino / ctx->inodes_per_block;
	offset = (ino % ctx->inodes_per_block) * sizeof(struct mfs_inode);

	r = read_block(ctx, block, buf);
	if (r != 0)
		return r;

	memcpy(inode, buf + offset, sizeof(struct mfs_inode));
	return 0;
}

/* ========================================================================
 * Block/zone mapping: convert logical zone to physical block number.
 * Handles direct, indirect, and double-indirect zone pointers.
 * ======================================================================== */

static int
read_map(struct mfsfuse_ctx *ctx, const struct mfs_inode *inode,
	 uint32_t zone_nr, uint32_t *phys)
{
	uint8_t buf[ctx->block_size];
	int r;

	/* Direct zones */
	if (zone_nr < ctx->ndzones) {
		*phys = inode->i_zone[zone_nr];
		return 0;
	}
	zone_nr -= ctx->ndzones;

	/* Single indirect: i_zone[7] */
	if (zone_nr < ctx->nindirs) {
		if (inode->i_zone[7] == 0) { *phys = 0; return 0; }
		r = read_block(ctx, inode->i_zone[7], buf);
		if (r != 0) return r;
		*phys = ((uint32_t *)buf)[zone_nr];
		return 0;
	}
	zone_nr -= ctx->nindirs;

	/* Double indirect: i_zone[8] */
	{
		uint32_t idx1 = zone_nr / ctx->nindirs;
		uint32_t idx2 = zone_nr % ctx->nindirs;

		if (idx1 >= ctx->nindirs || inode->i_zone[8] == 0)
			return -EFBIG;

		r = read_block(ctx, inode->i_zone[8], buf);
		if (r != 0) return r;
		uint32_t indir = ((uint32_t *)buf)[idx1];
		if (indir == 0) { *phys = 0; return 0; }

		r = read_block(ctx, indir, buf);
		if (r != 0) return r;
		*phys = ((uint32_t *)buf)[idx2];
		return 0;
	}
}

/* Convert a file offset to a physical block number (0 = sparse/hole). */
static uint32_t
offset_to_block(struct mfsfuse_ctx *ctx, const struct mfs_inode *inode,
		off_t offset)
{
	uint32_t zone = (uint32_t)(offset / ctx->block_size);
	uint32_t phys;

	if (read_map(ctx, inode, zone, &phys) != 0)
		return 0;
	return phys;
}

/* ========================================================================
 * Directory iteration
 * ======================================================================== */

/* Read next valid dirent starting at *pos. Skips empty and dot entries.
 * Returns 1 on entry found (entry filled), 0 at EOF, negative on error. */
static int
read_next_dirent(struct mfsfuse_ctx *ctx, const struct mfs_inode *dir_inode,
		 off_t *pos, struct mfs_dirent *entry)
{
	uint8_t dbuf[ctx->block_size];
	uint32_t cached_block = (uint32_t)-1;
	uint32_t phys;
	int r;

	while (*pos + (off_t)sizeof(*entry) <= dir_inode->i_size) {
		phys = offset_to_block(ctx, dir_inode, *pos);

		if (phys != 0 && phys != cached_block) {
			r = read_block(ctx, phys, dbuf);
			if (r != 0) return r;
			cached_block = phys;
		}

		if (phys != 0) {
			uint32_t off = *pos % ctx->block_size;
			memcpy(entry, dbuf + off, sizeof(*entry));
		} else {
			memset(entry, 0, sizeof(*entry));
		}

		*pos += sizeof(*entry);

		if (entry->d_ino != 0 &&
		    strcmp(entry->d_name, ".")  != 0 &&
		    strcmp(entry->d_name, "..") != 0)
			return 1;
	}
	return 0;
}

/* Look up a single path component in a directory.
 * On success fills entry and returns 0; returns negative errno. */
static int
lookup_in_dir(struct mfsfuse_ctx *ctx, uint32_t dir_ino,
	      const char *name, struct mfs_entry *entry)
{
	struct mfs_inode dir;
	struct mfs_dirent de;
	off_t pos = 0;
	int r;

	/* Handle . and .. directly */
	if (strcmp(name, ".") == 0) {
		entry->ino = dir_ino;
		return read_inode(ctx, dir_ino, &entry->inode);
	}
	if (strcmp(name, "..") == 0) {
		/* For root, .. is root */
		if (dir_ino == ROOT_INODE) {
			entry->ino = ROOT_INODE;
			return read_inode(ctx, ROOT_INODE, &entry->inode);
		}
		/* For non-root, scan directory for .. entry */
		r = read_inode(ctx, dir_ino, &dir);
		if (r != 0) return r;
		while ((r = read_next_dirent(ctx, &dir, &pos, &de)) > 0) {
			if (strcmp(de.d_name, "..") == 0) {
				entry->ino = de.d_ino;
				return read_inode(ctx, de.d_ino, &entry->inode);
			}
		}
		return r == 0 ? read_inode(ctx, ROOT_INODE, &entry->inode) : r;
	}

	/* Normal lookup */
	r = read_inode(ctx, dir_ino, &dir);
	if (r != 0) return r;
	if (!S_ISDIR(dir.i_mode))
		return -ENOTDIR;

	while ((r = read_next_dirent(ctx, &dir, &pos, &de)) > 0) {
		if (strncmp(de.d_name, name, MFS_DIRSIZ) == 0) {
			entry->ino = de.d_ino;
			return read_inode(ctx, de.d_ino, &entry->inode);
		}
	}
	return r == 0 ? -ENOENT : r;
}

/* Resolve an absolute path to an inode entry.
 * Does NOT follow symlinks (returns the symlink inode itself). */
static int
path_resolve(struct mfsfuse_ctx *ctx, const char *path,
	     struct mfs_entry *entry)
{
	char copy[PATH_MAX];
	char *save, *tok;
	uint32_t cur_ino = ROOT_INODE;
	int r;

	if (path[0] != '/')
		return -ENOENT;

	/* Root directory */
	if (strcmp(path, "/") == 0) {
		entry->ino = ROOT_INODE;
		return read_inode(ctx, ROOT_INODE, &entry->inode);
	}

	path++; /* skip leading / */
	if (*path == '\0') {
		entry->ino = ROOT_INODE;
		return read_inode(ctx, ROOT_INODE, &entry->inode);
	}

	strncpy(copy, path, sizeof(copy) - 1);
	copy[sizeof(copy) - 1] = '\0';

	tok = strtok_r(copy, "/", &save);
	while (tok != NULL) {
		r = lookup_in_dir(ctx, cur_ino, tok, entry);
		if (r != 0) return r;
		cur_ino = entry->ino;
		tok = strtok_r(NULL, "/", &save);
	}

	return 0;
}

/* ========================================================================
 * FUSE callbacks
 * ======================================================================== */

static int
mfs_getattr(const char *path, struct stat *stbuf)
{
	struct mfs_entry entry;
	int r;

	memset(stbuf, 0, sizeof(*stbuf));
	r = path_resolve(g_ctx, path, &entry);
	if (r != 0) return r;

	stbuf->st_ino   = entry.ino;
	stbuf->st_mode  = entry.inode.i_mode;
	stbuf->st_nlink = entry.inode.i_nlinks;
	stbuf->st_uid   = entry.inode.i_uid;
	stbuf->st_gid   = entry.inode.i_gid;
	stbuf->st_size  = entry.inode.i_size;
	stbuf->st_blksize = g_ctx->block_size;
	stbuf->st_blocks  = (entry.inode.i_size + 511) / 512;
	stbuf->st_atime = entry.inode.i_atime;
	stbuf->st_mtime = entry.inode.i_mtime;
	stbuf->st_ctime = entry.inode.i_ctime;

	return 0;
}

static int
mfs_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
	    off_t offset, struct fuse_file_info *fi)
{
	struct mfs_entry entry;
	struct mfs_dirent de;
	off_t pos = 0;
	int r;

	(void)offset;
	(void)fi;

	r = path_resolve(g_ctx, path, &entry);
	if (r != 0) return r;
	if (!S_ISDIR(entry.inode.i_mode))
		return -ENOTDIR;

	/* Always add . and .. */
	filler(buf, ".",  NULL, 0);
	filler(buf, "..", NULL, 0);

	while ((r = read_next_dirent(g_ctx, &entry.inode, &pos, &de)) > 0) {
		struct stat st;
		memset(&st, 0, sizeof(st));
		st.st_ino = de.d_ino;
		/* Determine file type for readdir */
		{
			struct mfs_inode fi;
			if (read_inode(g_ctx, de.d_ino, &fi) == 0)
				st.st_mode = fi.i_mode;
		}
		filler(buf, de.d_name, &st, 0);
	}

	return r < 0 ? r : 0;
}

static int
mfs_open(const char *path, struct fuse_file_info *fi)
{
	struct mfs_entry entry;
	int r;

	r = path_resolve(g_ctx, path, &entry);
	if (r != 0) return r;

	if ((fi->flags & O_ACCMODE) != O_RDONLY)
		return -EACCES;

	/* Directories must be opened with O_RDONLY only */
	if (S_ISDIR(entry.inode.i_mode))
		return 0;

	/* Regular files, symlinks, etc. */
	return 0;
}

static int
mfs_read(const char *path, char *buf, size_t size, off_t offset,
	 struct fuse_file_info *fi)
{
	struct mfs_entry entry;
	off_t file_size;
	off_t pos = offset;
	size_t remain = size;
	int r;

	(void)fi;

	r = path_resolve(g_ctx, path, &entry);
	if (r != 0) return r;

	file_size = entry.inode.i_size;
	if (offset >= file_size)
		return 0;
	if (offset + (off_t)remain > file_size)
		remain = file_size - offset;

	while (remain > 0) {
		uint32_t phys = offset_to_block(g_ctx, &entry.inode, pos);
		uint32_t off_in_block = (uint32_t)(pos % g_ctx->block_size);
		size_t chunk = g_ctx->block_size - off_in_block;
		if (chunk > remain) chunk = remain;

		if (phys == 0) {
			/* Sparse block — read as zeros */
			memset(buf, 0, chunk);
		} else {
			uint8_t block_buf[ctx->block_size];
			r = read_block(g_ctx, phys, block_buf);
			if (r != 0) return r;
			memcpy(buf, block_buf + off_in_block, chunk);
		}
		buf    += chunk;
		pos    += chunk;
		remain -= chunk;
	}

	return (int)(size - remain);
}

static int
mfs_readlink(const char *path, char *buf, size_t size)
{
	struct mfs_entry entry;
	int r;

	r = path_resolve(g_ctx, path, &entry);
	if (r != 0) return r;
	if (!S_ISLNK(entry.inode.i_mode))
		return -EINVAL;

	/* For MFS V3, symlink targets <= 60 bytes are stored in i_zone
	 * (specifically in the zone array, which for small files
	 * doubles as data storage). For longer targets, they use
	 * a data block like regular file data.
	 *
	 * We handle both cases by reading via mfs_read logic. */
	{
		size_t len = (size_t)entry.inode.i_size;
		if (len > size - 1) len = size - 1;
		ssize_t n = mfs_read(path, buf, len, 0, NULL);
		if (n < 0) return (int)n;
		buf[n] = '\0';
	}

	return 0;
}

static int
mfs_statvfs(const char *path, struct statvfs *svb)
{
	uint32_t total_blocks;
	(void)path;

	memset(svb, 0, sizeof(*svb));
	total_blocks = g_ctx->zones; /* zones == blocks for MFS V3 */

	svb->f_bsize   = g_ctx->block_size;
	svb->f_frsize  = g_ctx->block_size;
	svb->f_blocks  = total_blocks;
	svb->f_bfree   = 0; /* read-only, we don't parse bitmaps */
	svb->f_bavail  = 0;
	svb->f_files   = g_ctx->ninodes;
	svb->f_ffree   = 0;
	svb->f_favail  = 0;
	svb->f_namemax = MFS_DIRSIZ - 1;

	return 0;
}

/* ========================================================================
 * FUSE operations table
 * ======================================================================== */

static struct fuse_operations mfs_ops = {
	.getattr  = mfs_getattr,
	.readdir  = mfs_readdir,
	.open     = mfs_open,
	.read     = mfs_read,
	.readlink = mfs_readlink,
	.statfs   = mfs_statvfs,
};

/* ========================================================================
 * main
 * ======================================================================== */

int
main(int argc, char *argv[])
{
	struct mfsfuse_ctx ctx;
	int r;

	if (argc < 2) {
		fprintf(stderr, "Usage: mfsfuse <device> <mountpoint> [options]\n");
		fprintf(stderr, "  Mount an MFS partition via FUSE.\n");
		fprintf(stderr, "  Example: mfsfuse /dev/c0d0p1 /mnt/mfs\n");
		return 1;
	}

	/* Open the block device for reading */
	ctx.fd = open(argv[1], O_RDONLY);
	if (ctx.fd < 0) {
		perror("mfsfuse: open");
		return 1;
	}

	/* Read and parse superblock (read_superblock reads block 0 internally) */
	r = read_superblock(&ctx);
	if (r != 0) {
		close(ctx.fd);
		return 1;
	}

	g_ctx = &ctx;

	/* Shift argv so mountpoint is argv[1] as fuse_main expects */
	argv[1] = argv[2];
	argv[2] = NULL;
	return fuse_main(argc - 1, argv, &mfs_ops, NULL);
}
