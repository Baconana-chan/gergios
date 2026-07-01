/* ffi_bridge.c — C bridge to Rust ext4-core parser.
 *
 * This file implements the block I/O callbacks and wraps the Rust FFI
 * functions for use by the ext4 fsdriver server.
 *
 * The bridge uses libminixfs (lmfs_get_block) for block cache access
 * and converts between MINIX and ext4 data structures.
 *
 * IMPORTANT: This file implements the real block/inode allocator callbacks
 * that manipulate the on-disk bitmaps and update group descriptor /
 * superblock free counts. This is CRITICAL for any write operation to work.
 */

#include "ffi.h"
#include <minix/libminixfs.h>
#include <minix/fsdriver.h>
#include <minix/bdev.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>
#include <stdlib.h>
#include <stdint.h>
#include <limits.h>

/* ─── Bitmap helper macros ─────────────────────────────────────────── */

/* Number of bits in a 32-bit word */
#define BITS_PER_WORD		(sizeof(uint32_t) * CHAR_BIT)

/* ─── Forward declarations ────────────────────────────────────────── */

static int read_superblock(struct ext4_io_ctx *io_ctx, uint8_t *sb_buf);
static int write_superblock(struct ext4_io_ctx *io_ctx, const uint8_t *sb_buf);
static int read_group_desc_block(struct ext4_io_ctx *io_ctx, uint64_t gdt_block_nr,
				 uint8_t *buf, uint32_t block_size);
static int write_group_desc_block(struct ext4_io_ctx *io_ctx, uint64_t gdt_block_nr,
				  const uint8_t *buf, uint32_t block_size);
static int update_group_desc_free_count(struct ext4_io_ctx *io_ctx,
					uint32_t group, int delta_blocks,
					int delta_inodes);

/* ─── Block I/O callback context ──────────────────────────────────── */

struct ext4_io_ctx {
	dev_t dev;			/* block device */
	struct ext4_sb_info sbi;	/* parsed superblock info */
};

/* Block read callback for Rust FFI.
 * Called by Rust code to read a single filesystem block.
 * ctx is a pointer to struct ext4_io_ctx.
 * Non-static so table.c can reference it as extern. */
int
ext4_read_block_cb(void *ctx, uint64_t block_nr, uint8_t *buf,
		   uint32_t block_size)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;

	/* Use lmfs_bio to read the block */
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)buf;
	data.size = block_size;

	int r = lmfs_bio(io_ctx->dev, &data, block_size,
		     (off_t)(block_nr * block_size), FSC_READ);
	if (r != OK) {
		printf("ext4: block read error at block %llu: %d\n",
		       (unsigned long long)block_nr, r);
		return 5; /* EIO */
	}

	return 0;
}

/* Block write callback for Rust FFI.
 * Called by Rust code to write a single filesystem block.
 * Uses lmfs_bio with FSC_WRITE. */
int
ext4_write_block_cb(void *ctx, uint64_t block_nr, const uint8_t *buf,
		    uint32_t block_size)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;

	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)buf;
	data.size = block_size;

	int r = lmfs_bio(io_ctx->dev, &data, block_size,
		     (off_t)(block_nr * block_size), FSC_WRITE);
	if (r != OK) {
		printf("ext4: block write error at block %llu: %d\n",
		       (unsigned long long)block_nr, r);
		return 5; /* EIO */
	}

	return 0;
}

/* ─── Helper: read/write the superblock ────────────────────────────── */

/*
 * Read the superblock (at byte offset 1024) into sb_buf.
 * sb_buf must be at least 1024 bytes.
 */
static int
read_superblock(struct ext4_io_ctx *io_ctx, uint8_t *sb_buf)
{
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)sb_buf;
	data.size = 1024;

	return lmfs_bio(io_ctx->dev, &data, 1024, 1024, FSC_READ);
}

/*
 * Write the superblock back to disk.
 */
static int
write_superblock(struct ext4_io_ctx *io_ctx, const uint8_t *sb_buf)
{
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)sb_buf;
	data.size = 1024;

	return lmfs_bio(io_ctx->dev, &data, 1024, 1024, FSC_WRITE);
}

/* ─── Helper: read/write the group descriptor table blocks ─────────── */

/*
 * Calculate the GDT block number and offset for a given group.
 * Returns the physical block number and the byte offset within that block.
 */
static void
gdt_location(const struct ext4_sb_info *sbi, uint32_t group,
	     uint64_t *out_block_nr, uint32_t *out_offset)
{
	uint32_t block_size = sbi->block_size;
	uint16_t desc_size = sbi->desc_size;
	uint32_t first_data_block = (block_size > 1024) ? 0 : 1;
	uint32_t descs_per_block = block_size / desc_size;

	/* GDT starts right after the boot block / superblock block */
	uint64_t gdt_start_block = first_data_block + 1;

	*out_block_nr = gdt_start_block + (group / descs_per_block);
	*out_offset = (group % descs_per_block) * desc_size;
}

/*
 * Read the GDT block containing the descriptor for the given group.
 */
static int
read_group_desc_block(struct ext4_io_ctx *io_ctx, uint64_t gdt_block_nr,
		      uint8_t *buf, uint32_t block_size)
{
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)buf;
	data.size = block_size;

	return lmfs_bio(io_ctx->dev, &data, block_size,
			(off_t)(gdt_block_nr * block_size), FSC_READ);
}

/*
 * Write a GDT block back to disk.
 */
static int
write_group_desc_block(struct ext4_io_ctx *io_ctx, uint64_t gdt_block_nr,
		       const uint8_t *buf, uint32_t block_size)
{
	struct fsdriver_data data;
	data.endpt = SELF;
	data.ptr = (char *)buf;
	data.size = block_size;

	return lmfs_bio(io_ctx->dev, &data, block_size,
			(off_t)(gdt_block_nr * block_size), FSC_WRITE);
}

/* ─── Helper: update group descriptor free counts on disk ──────────── */

/*
 * Update the free_blocks_count and/or free_inodes_count in a group
 * descriptor on disk. delta_blocks/delta_inodes are signed adjustments.
 * Also updates the in-memory sbi for statvfs.
 */
static int
update_group_desc_free_count(struct ext4_io_ctx *io_ctx,
			     uint32_t group, int delta_blocks,
			     int delta_inodes)
{
	struct ext4_sb_info *sbi = &io_ctx->sbi;
	uint32_t block_size = sbi->block_size;
	uint16_t desc_size = sbi->desc_size;

	uint64_t gdt_block_nr;
	uint32_t offset;
	gdt_location(sbi, group, &gdt_block_nr, &offset);

	/* Read the entire GDT block */
	uint8_t *gdt_buf = malloc(block_size);
	if (gdt_buf == NULL) return ENOMEM;

	int r = read_group_desc_block(io_ctx, gdt_block_nr, gdt_buf, block_size);
	if (r != OK) {
		free(gdt_buf);
		return r;
	}

	/* Update free_blocks_count at offset + 12 (2 bytes LE) */
	if (delta_blocks != 0) {
		uint16_t cur_blocks = (uint16_t)(
			gdt_buf[offset + 12] |
			(gdt_buf[offset + 13] << 8));
		cur_blocks += (int16_t)delta_blocks;
		gdt_buf[offset + 12] = cur_blocks & 0xFF;
		gdt_buf[offset + 13] = (cur_blocks >> 8) & 0xFF;

		/* Update in-memory sbi for statvfs */
		sbi->free_blocks_count += delta_blocks;
	}

	/* Update free_inodes_count at offset + 14 (2 bytes LE) */
	if (delta_inodes != 0) {
		uint16_t cur_inodes = (uint16_t)(
			gdt_buf[offset + 14] |
			(gdt_buf[offset + 15] << 8));
		cur_inodes += (int16_t)delta_inodes;
		gdt_buf[offset + 14] = cur_inodes & 0xFF;
		gdt_buf[offset + 15] = (cur_inodes >> 8) & 0xFF;

		/* Update in-memory sbi for statvfs */
		sbi->free_inodes_count += delta_inodes;
	}

	/* Update GD checksum before writing back */
	ext4_update_gd_csum(sbi, group, gdt_buf + offset, desc_size);

	/* Write the GDT block back */
	r = write_group_desc_block(io_ctx, gdt_block_nr, gdt_buf, block_size);
	free(gdt_buf);
	return r;
}

/* ─── Helper: update superblock free counts on disk ────────────────── */

/*
 * Synchronize the in-memory sbi free counts to the on-disk superblock.
 * This is called after a batch of allocation/free operations.
 * Call sparingly — each call reads and writes the superblock.
 */
static int
sync_superblock_free_counts(struct ext4_io_ctx *io_ctx)
{
	uint8_t sb_buf[1024];
	int r = read_superblock(io_ctx, sb_buf);
	if (r != OK) return r;

	/* Update s_free_blocks_count_lo at offset 12 (4 bytes LE) */
	uint32_t free_lo = (uint32_t)(io_ctx->sbi.free_blocks_count & 0xFFFFFFFF);
	sb_buf[12] = free_lo & 0xFF;
	sb_buf[13] = (free_lo >> 8) & 0xFF;
	sb_buf[14] = (free_lo >> 16) & 0xFF;
	sb_buf[15] = (free_lo >> 24) & 0xFF;

	/* Update s_free_blocks_count_hi at offset 376 (4 bytes LE) — upper 32 bits */
	uint32_t free_hi = (uint32_t)(io_ctx->sbi.free_blocks_count >> 32);
	sb_buf[376] = free_hi & 0xFF;
	sb_buf[377] = (free_hi >> 8) & 0xFF;
	sb_buf[378] = (free_hi >> 16) & 0xFF;
	sb_buf[379] = (free_hi >> 24) & 0xFF;

	/* Update s_free_inodes_count at offset 16 (4 bytes LE) */
	uint32_t free_inodes = (uint32_t)(io_ctx->sbi.free_inodes_count & 0xFFFFFFFF);
	sb_buf[16] = free_inodes & 0xFF;
	sb_buf[17] = (free_inodes >> 8) & 0xFF;
	sb_buf[18] = (free_inodes >> 16) & 0xFF;
	sb_buf[19] = (free_inodes >> 24) & 0xFF;

	/* Update superblock checksum before writing back */
	ext4_update_sb_csum(&io_ctx->sbi, sb_buf);

	return write_superblock(io_ctx, sb_buf);
}

/* ─── Bitmap operations ────────────────────────────────────────────── */

/*
 * Find the next zero bit in the bitmap, starting from 'start_bit'.
 * Returns the bit index, or -1 if all bits are set.
 * bitmap is a byte array, max_bits is the number of valid bits.
 */
static int
find_next_zero_bit(const uint8_t *bitmap, int max_bits, int start_bit)
{
	for (int i = start_bit; i < max_bits; i++) {
		int byte_idx = i / 8;
		int bit_idx = i % 8;
		if (!(bitmap[byte_idx] & (1 << bit_idx))) {
			return i;
		}
	}
	return -1;
}

/*
 * Set a bit in the bitmap to 1.
 * Returns 0 if the bit was already set, 1 if it was clear.
 */
static int
set_bit(uint8_t *bitmap, int bit)
{
	int byte_idx = bit / 8;
	int bit_idx = bit % 8;
	int was_set = (bitmap[byte_idx] >> bit_idx) & 1;
	bitmap[byte_idx] |= (uint8_t)(1 << bit_idx);
	return was_set;
}

/*
 * Clear a bit in the bitmap to 0.
 * Returns 0 if the bit was already clear, 1 if it was set.
 */
static int
clear_bit(uint8_t *bitmap, int bit)
{
	int byte_idx = bit / 8;
	int bit_idx = bit % 8;
	int was_set = (bitmap[byte_idx] >> bit_idx) & 1;
	bitmap[byte_idx] &= (uint8_t)(~(1 << bit_idx));
	return was_set;
}

/* ─── Group descriptor read via FFI (uses Rust ext4_read_group_descriptor) ── */

/*
 * Read a group descriptor via the Rust FFI.
 * Returns 0 on success, or an errno code.
 */
static int
read_gd_ffi(struct ext4_io_ctx *io_ctx, uint32_t group,
	    struct ext4_gd_info *gd)
{
	return ext4_read_group_descriptor(&io_ctx->sbi, group, gd,
					  (void *)io_ctx, ext4_read_block_cb);
}

/* ─── Real allocator callbacks ─────────────────────────────────────── */

/*
 * Allocate a single physical block.
 *
 * Strategy:
 * 1. Scan block groups (starting from group 0) for one with free blocks.
 * 2. Read the block bitmap for that group.
 * 3. Find the first zero bit (free block).
 * 4. Set the bit and mark the buffer dirty.
 * 5. Update the group descriptor's free_blocks_count (decrement).
 * 6. Update the in-memory sbi.
 * 7. Return the physical block number (absolute).
 *
 * Returns the physical block number, or 0 on failure (no space / error).
 */
uint64_t
ext4_alloc_block_cb(void *ctx)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;
	const struct ext4_sb_info *sbi = &io_ctx->sbi;
	uint32_t block_size = sbi->block_size;
	uint32_t blocks_per_group = sbi->blocks_per_group;
	uint32_t first_data_block = (block_size > 1024) ? 0 : 1;
	uint32_t groups_count = sbi->block_groups_count;
	uint64_t total_blocks = sbi->blocks_count;

	/* Scan groups for free blocks */
	for (uint32_t group = 0; group < groups_count; group++) {
		struct ext4_gd_info gd;
		struct buf *bp;
		int r;

		r = read_gd_ffi(io_ctx, group, &gd);
		if (r != OK) continue;

		if (gd.free_blocks_count == 0) continue;

		/* Read the block bitmap for this group */
		r = lmfs_get_block(&bp, io_ctx->dev, gd.block_bitmap, NORMAL);
		if (r != OK) continue;

		/* Find first zero bit in the bitmap */
		uint8_t *bitmap = (uint8_t *)bp->data;
		int max_bits = blocks_per_group;
		int bit = find_next_zero_bit(bitmap, max_bits, 0);

		if (bit < 0) {
			lmfs_put_block(bp);
			continue; /* No free block in this group (shouldn't happen) */
		}

		/* Compute absolute block number */
		uint64_t abs_block = first_data_block +
				     (uint64_t)group * blocks_per_group +
				     bit;

		if (abs_block >= total_blocks) {
			lmfs_put_block(bp);
			continue;
		}

		/* Set the bit in the bitmap */
		set_bit(bitmap, bit);
		lmfs_markdirty(bp);
		lmfs_put_block(bp);

		/* Update group descriptor free count */
		r = update_group_desc_free_count(io_ctx, group, -1, 0);
		if (r != OK) {
			/* Revert the bit if we can't update GD */
			lmfs_get_block(&bp, io_ctx->dev, gd.block_bitmap, NO_READ);
			if (bp) {
				clear_bit((uint8_t *)bp->data, bit);
				lmfs_markdirty(bp);
				lmfs_put_block(bp);
			}
			return 0;
		}

		/* Sync superblock (batch-friendly: do outside loop for bulk ops) */
		sync_superblock_free_counts(io_ctx);

		return abs_block;
	}

	return 0; /* No space left */
}

/*
 * Free a range of physical blocks.
 *
 * Handles the case where the block range spans multiple groups.
 * For each block in the range:
 * 1. Calculate the group and bit position.
 * 2. Read the block bitmap, clear the bit.
 * 3. Update the group descriptor's free_blocks_count (increment).
 *
 * Returns 0 on success, or an errno code.
 */
int
ext4_free_blocks_cb(void *ctx, uint64_t block_nr, uint32_t count)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;
	const struct ext4_sb_info *sbi = &io_ctx->sbi;
	uint32_t block_size = sbi->block_size;
	uint32_t blocks_per_group = sbi->blocks_per_group;
	uint32_t first_data_block = (block_size > 1024) ? 0 : 1;
	uint64_t total_blocks = sbi->blocks_count;

	if (count == 0) return 0;

	/* Track last group to avoid reading the same bitmap/GD repeatedly */
	int last_group = -1;
	struct buf *bp = NULL;
	int blocks_freed_this_group = 0;

	for (uint32_t i = 0; i < count; i++) {
		uint64_t abs_block = block_nr + i;

		if (abs_block >= total_blocks) break;

		/* Calculate group and bit within group */
		uint32_t group = (uint32_t)((abs_block - first_data_block) /
					    blocks_per_group);
		int bit = (int)((abs_block - first_data_block) % blocks_per_group);

		if ((int)group != last_group) {
			/* Flush previous group's bitmap and GD */
			if (bp != NULL) {
				lmfs_markdirty(bp);
				lmfs_put_block(bp);
				bp = NULL;

				if (blocks_freed_this_group > 0) {
					update_group_desc_free_count(
						io_ctx, last_group,
						blocks_freed_this_group, 0);
					blocks_freed_this_group = 0;
				}
			}

			last_group = group;

			/* Read the block bitmap for the new group */
			struct ext4_gd_info gd;
			int r = read_gd_ffi(io_ctx, group, &gd);
			if (r != OK) continue;

			r = lmfs_get_block(&bp, io_ctx->dev,
					   gd.block_bitmap, NORMAL);
			if (r != OK) {
				bp = NULL;
				continue;
			}
		}

		if (bp == NULL) continue;

		/* Clear the bit */
		uint8_t *bitmap = (uint8_t *)bp->data;
		clear_bit(bitmap, bit);
		blocks_freed_this_group++;
	}

	/* Flush last group's bitmap and GD */
	if (bp != NULL) {
		lmfs_markdirty(bp);
		lmfs_put_block(bp);

		if (blocks_freed_this_group > 0 && last_group >= 0) {
			update_group_desc_free_count(io_ctx, last_group,
						     blocks_freed_this_group, 0);
		}
	}

	/* Sync superblock */
	sync_superblock_free_counts(io_ctx);

	return 0;
}

/*
 * Allocate a new inode.
 *
 * Strategy:
 * 1. Scan block groups starting from group 0 for one with free inodes.
 * 2. Read the inode bitmap.
 * 3. Find the first zero bit.
 * 4. Set it, mark buffer dirty.
 * 5. Update group descriptor and in-memory counts.
 * 6. Return the absolute inode number.
 *
 * Returns the inode number (1-based), or 0 on failure.
 */
uint32_t
ext4_alloc_inode_cb(void *ctx)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;
	const struct ext4_sb_info *sbi = &io_ctx->sbi;
	uint32_t inodes_per_group = sbi->inodes_per_group;
	uint32_t groups_count = sbi->block_groups_count;
	uint64_t total_inodes = sbi->inodes_count;

	for (uint32_t group = 0; group < groups_count; group++) {
		struct ext4_gd_info gd;
		struct buf *bp;
		int r;

		r = read_gd_ffi(io_ctx, group, &gd);
		if (r != OK) continue;

		if (gd.free_inodes_count == 0) continue;

		/* Read the inode bitmap for this group */
		r = lmfs_get_block(&bp, io_ctx->dev, gd.inode_bitmap, NORMAL);
		if (r != OK) continue;

		/* Find first zero bit in the bitmap */
		uint8_t *bitmap = (uint8_t *)bp->data;
		int max_bits = inodes_per_group;
		int bit = find_next_zero_bit(bitmap, max_bits, 0);

		if (bit < 0) {
			lmfs_put_block(bp);
			continue;
		}

		/* Compute absolute inode number (1-based) */
		uint32_t ino = group * inodes_per_group + bit + 1;

		if ((uint64_t)ino > total_inodes) {
			lmfs_put_block(bp);
			continue;
		}

		/* Skip reserved inodes (0..first_ino-1) */
		if (ino < sbi->first_ino) {
			/* Mark it and continue searching */
			set_bit(bitmap, bit);
			lmfs_markdirty(bp);
			lmfs_put_block(bp);

			/* We still need to count it as used, but we can't
			 * give it out. Update counts and try next bit.
			 * Actually, reserved inodes are already marked in the
			 * bitmap from mkfs. So this shouldn't happen.
			 * If it does, we skip this bit and try the next one. */
			update_group_desc_free_count(io_ctx, group, 0, -1);
			sync_superblock_free_counts(io_ctx);
			continue;
		}

		/* Set the bit in the bitmap */
		set_bit(bitmap, bit);
		lmfs_markdirty(bp);
		lmfs_put_block(bp);

		/* Update group descriptor free count */
		r = update_group_desc_free_count(io_ctx, group, 0, -1);
		if (r != OK) {
			/* Revert */
			lmfs_get_block(&bp, io_ctx->dev, gd.inode_bitmap, NO_READ);
			if (bp) {
				clear_bit((uint8_t *)bp->data, bit);
				lmfs_markdirty(bp);
				lmfs_put_block(bp);
			}
			return 0;
		}

		/* Sync superblock */
		sync_superblock_free_counts(io_ctx);

		return ino;
	}

	return 0; /* No space left */
}

/*
 * Free an inode (mark it as unused in the inode bitmap).
 *
 * Clears the bit in the inode bitmap and updates the group descriptor's
 * free_inodes_count.
 *
 * Returns 0 on success, or an errno code.
 */
int
ext4_free_inode_cb(void *ctx, uint32_t ino)
{
	struct ext4_io_ctx *io_ctx = (struct ext4_io_ctx *)ctx;
	const struct ext4_sb_info *sbi = &io_ctx->sbi;
	uint32_t inodes_per_group = sbi->inodes_per_group;
	uint32_t first_ino = sbi->first_ino;

	if (ino == 0 || ino > sbi->inodes_count) {
		return EINVAL;
	}

	/* Don't free reserved inodes */
	if (ino < first_ino) {
		return 0; /* Silently ignore */
	}

	/* Calculate group and bit within group (inode numbers are 1-based) */
	uint32_t group = (ino - 1) / inodes_per_group;
	int bit = (ino - 1) % inodes_per_group;

	struct ext4_gd_info gd;
	struct buf *bp;
	int r;

	r = read_gd_ffi(io_ctx, group, &gd);
	if (r != OK) return r;

	/* Read the inode bitmap */
	r = lmfs_get_block(&bp, io_ctx->dev, gd.inode_bitmap, NORMAL);
	if (r != OK) return r;

	/* Clear the bit in the bitmap */
	uint8_t *bitmap = (uint8_t *)bp->data;
	clear_bit(bitmap, bit);
	lmfs_markdirty(bp);
	lmfs_put_block(bp);

	/* Update group descriptor free inodes count */
	r = update_group_desc_free_count(io_ctx, group, 0, 1);
	if (r != OK) return r;

	/* Sync superblock */
	sync_superblock_free_counts(io_ctx);

	return 0;
}

/* ─── Orphan inode cleanup ───────────────────────────────────────── */

/*
 * Walk the orphan inode list and clean up any inodes that were unlinked
 * but still open at the time of the last crash/unclean unmount.
 *
 * For each orphan inode (links_count == 0):
 * 1. Truncate all data blocks (free via extent_truncate mechanism)
 * 2. Clear the inode bitmap via free_inode_cb
 * 3. Zero out the inode in the inode table
 * 4. Clear s_last_orphan in the superblock
 *
 * Returns 0 on success, or an errno code.
 * It is safe to call on a clean filesystem (s_last_orphan == 0 => no-op).
 */
int
ext4_orphan_cleanup(struct ext4_io_ctx *io_ctx)
{
	struct ext4_sb_info *sbi = &io_ctx->sbi;
	uint32_t orphan_ino;
	int ret = 0;
	int any_cleaned = 0;

	orphan_ino = sbi->last_orphan;
	if (orphan_ino == 0) {
		return 0; /* No orphans — nothing to do */
	}

	printf("ext4: orphan cleanup: starting with inode %u\n", orphan_ino);

	while (orphan_ino != 0 && orphan_ino <= sbi->inodes_count) {
		struct ext4_inode_info info;
		uint32_t next_orphan;

		printf("ext4: orphan cleanup: processing inode %u\n", orphan_ino);

		/* Read the orphan inode to get its dtime (next orphan) and
		 * to trigger truncation of data blocks. */
		ret = ext4_read_inode(sbi, orphan_ino, &info,
				      (void *)io_ctx, ext4_read_block_cb);
		if (ret != 0) {
			printf("ext4: orphan cleanup: failed to read inode %u: %d\n",
			       orphan_ino, ret);
			break;
		}

		next_orphan = info.dtime;

		/* Truncate all data blocks (free blocks via extent tree).
		 * ext4_truncate handles extent tree walking and block freeing. */
		if (info.blocks > 0) {
			ret = ext4_truncate(sbi, orphan_ino, 0,
					    (void *)io_ctx,
					    ext4_read_block_cb,
					    ext4_write_block_cb,
					    ext4_free_blocks_cb);
			if (ret != 0) {
				printf("ext4: orphan cleanup: truncate inode %u failed: %d\n",
				       orphan_ino, ret);
				break;
			}
		}

		/* Free the inode bitmap entry */
		ret = ext4_free_inode_cb((void *)io_ctx, orphan_ino);
		if (ret != 0) {
			printf("ext4: orphan cleanup: free inode %u bitmap failed: %d\n",
			       orphan_ino, ret);
			break;
		}

		/* Zero out the inode in the inode table.
		 * We re-read the inode table block and clear the inode slot.
		 * First, calculate which block and offset the inode is at. */
		{
			uint32_t block_size = sbi->block_size;
			uint32_t inode_size = sbi->inode_size;
			uint32_t inodes_per_block = block_size / inode_size;

			uint32_t group = (orphan_ino - 1) / sbi->inodes_per_group;
			uint32_t index = (orphan_ino - 1) % sbi->inodes_per_group;

			struct ext4_gd_info gd;
			ret = read_gd_ffi(io_ctx, group, &gd);
			if (ret != 0) break;

			uint64_t inode_table = gd.inode_table;
			uint64_t block_offset = index / inodes_per_block;
			uint32_t in_block_offset = (index % inodes_per_block) * inode_size;
			uint64_t inode_block_nr = inode_table + block_offset;

			/* Read the block, zero out the inode slot, write back */
			uint8_t *block_buf = malloc(block_size);
			if (block_buf == NULL) {
				ret = ENOMEM;
				break;
			}

			ret = ext4_read_block_cb((void *)io_ctx, inode_block_nr,
						block_buf, block_size);
			if (ret != 0) {
				free(block_buf);
				break;
			}

			/* Clear the entire inode slot */
			memset(block_buf + in_block_offset, 0, inode_size);

			ret = ext4_write_block_cb((void *)io_ctx, inode_block_nr,
						 block_buf, block_size);
			free(block_buf);
			if (ret != 0) break;
		}

		any_cleaned = 1;
		orphan_ino = next_orphan;
	}

	/* Clear s_last_orphan in the superblock */
	if (any_cleaned) {
		uint8_t sb_buf[1024];
		ret = read_superblock(io_ctx, sb_buf);
		if (ret == 0) {
			/* Write 0 to s_last_orphan at offset 264 (4 bytes LE) */
			sb_buf[264] = 0;
			sb_buf[265] = 0;
			sb_buf[266] = 0;
			sb_buf[267] = 0;

			/* Also clear the in-memory copy */
			sbi->last_orphan = 0;

			ret = write_superblock(io_ctx, sb_buf);
			if (ret == 0) {
				printf("ext4: orphan cleanup: complete, "
				       "orphan list cleared\n");
			}
		}
	}

	return ret;
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
	       "inodes=%llu, groups=%u, metadata_csum=%s\n",
	       sbi->block_size,
	       (unsigned long long)sbi->blocks_count,
	       (unsigned long long)sbi->inodes_count,
	       sbi->block_groups_count,
	       sbi->csum_seed ? "yes" : "no");

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

	/* Parse via Rust FFI (also computes csum_seed if METADATA_CSUM is enabled) */
	r = ext4_parse_superblock(sb_buf, &ctx->sbi);
	if (r != 0) {
		printf("ext4: superblock parse failed: %d\n", r);
		return r;
	}

	/* If metadata_csum is enabled, verify all checksums at mount time */
	if (ctx->sbi.csum_seed != 0) {
		struct ext4_csum_result csum_res;
		r = ext4_verify_all_csums(&ctx->sbi, sb_buf, &csum_res,
					  (void *)ctx, ext4_read_block_cb);
		if (r == 0) {
			if (!csum_res.sb_valid) {
				printf("ext4: WARNING: superblock checksum mismatch!\n");
			}
			if (!csum_res.gd_valid) {
				printf("ext4: WARNING: group descriptor checksum mismatch "
				       "(first bad group: %u)\n",
				       csum_res.gd_failed);
			}
			if (!csum_res.root_inode_valid) {
				printf("ext4: WARNING: root inode checksum mismatch!\n");
			}
			if (csum_res.sb_valid && csum_res.gd_valid &&
			    csum_res.root_inode_valid) {
				printf("ext4: all metadata checksums valid\n");
			}
		} else {
			printf("ext4: checksum verification error: %d\n", r);
		}
	}

	return ext4_mount(&ctx->sbi);
}
