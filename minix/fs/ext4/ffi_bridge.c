/* ffi_bridge.c — C bridge to Rust ext4-core parser.
 *
 * This file implements the block I/O callbacks and wraps the Rust FFI
 * functions for use by the ext4 fsdriver server.
 *
 * The bridge uses libminixfs (lmfs_get_block) for block cache access
 * and converts between MINIX and ext4 data structures.
 */

#include "ffi.h"
#include <minix/libminixfs.h>
#include <minix/fsdriver.h>
#include <minix/bdev.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>

/* ─── Block I/O callback context ──────────────────────────────────── */

struct ext4_io_ctx {
	dev_t dev;			/* block device */
	struct ext4_sb_info sbi;	/* parsed superblock info */
};

/* Block read callback for Rust FFI.
 * Called by Rust code to read a single filesystem block. */
static int
ext4_read_block_cb(void *ctx, uint64_t block_nr, uint8_t *buf,
		   uint32_t block_size)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;
	int r;
	size_t bytes;

	/* Use lmfs_bio to read the block */
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)buf;
	data.size = block_size;

	r = lmfs_bio(io_ctx->dev, &data, block_size,
		     (off_t)(block_nr * block_size), FSC_READ);
	if (r != OK) {
		printf("ext4: block read error at block %llu: %d\n",
		       (unsigned long long)block_nr, r);
		return 5; /* EIO */
	}

	/* If the block is shorter than block_size, zero the rest.
	 * This shouldn't happen for aligned reads, but be safe. */
	if (bytes < block_size) {
		memset(buf + bytes, 0, block_size - bytes);
	}

	return 0;
}

/* ─── Public API ──────────────────────────────────────────────────── */

/* Parse the ext4 superblock, reading from the given device. */
int
ext4_mount(const struct ext4_sb_info *sbi)
{
	/* Validate superblock features */
	if (!sbi->has_extents) {
		printf("ext4: filesystem does not support extents\n");
		return ENOTSUP;
	}
	if (sbi->feature_incompat & 0xFFFFF900) {
		/* Check for unsupported features:
		 * COMPRESSION, JOURNAL_DEV, META_BG, MMP,
		 * EA_INODE, DIRDATA, CSUM_SEED, LARGEDIR,
		 * INLINE_DATA, ENCRYPT
		 */
		printf("ext4: unsupported INCOMPAT features: 0x%08x\n",
		       sbi->feature_incompat);
		return ENOTSUP;
	}

	printf("ext4: mounted, blocksize=%u, total_blocks=%llu, "
	       "inodes=%llu, groups=%u\n",
	       sbi->block_size,
	       (unsigned long long)sbi->blocks_count,
	       (unsigned long long)sbi->inodes_count,
	       sbi->block_groups_count);

	return OK;
}

/* Read the superblock from a device and parse it. */
int
ext4_read_super(struct ext4_io_ctx *ctx)
{
	uint8_t sb_buf[1024];
	int r;

	/* Read superblock at offset 1024 using lmfs_bio */
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)sb_buf;
	data.size = sizeof(sb_buf);

	r = lmfs_bio(ctx->dev, &data, sizeof(sb_buf), 1024, FSC_READ);
	if (r != OK) {
		printf("ext4: failed to read superblock: %d\n", r);
		return r;
	}

	/* Parse via Rust FFI */
	r = ext4_parse_superblock(sb_buf, &ctx->sbi);
	if (r != 0) {
		printf("ext4: superblock parse failed: %d\n", r);
		return r;
	}

	return ext4_mount(&ctx->sbi);
}
