/* ext4 FFI interface — C bridge to Rust ext4-core parser.
 *
 * This header declares the C-compatible types and functions exported
 * by the Rust ext4-core static library.
 */

#ifndef _MINIX_EXT4_FFI_H
#define _MINIX_EXT4_FFI_H

#include <sys/types.h>
#include <stdint.h>

/* Standard ext4 inode numbers */
#define EXT4_ROOT_INO	2

/* ext4 directory entry file types */
#define EXT4_FT_UNKNOWN		0
#define EXT4_FT_REG_FILE	1
#define EXT4_FT_DIR		2
#define EXT4_FT_CHRDEV		3
#define EXT4_FT_BLKDEV		4
#define EXT4_FT_FIFO		5
#define EXT4_FT_SOCK		6
#define EXT4_FT_SYMLINK		7
#define EXT4_BAD_INO	1
#define EXT4_USR_QUOTA	3
#define EXT4_GRP_QUOTA	4
#define EXT4_BOOT_LOADER	5
#define EXT4_UNDEL_DIR	6
#define EXT4_RESIZE_INO	7
#define EXT4_JOURNAL_INO	8

/* ─── C-compatible structures ─────────────────────────────────────── */

struct ext4_sb_info {
	uint32_t block_size;
	uint64_t blocks_count;
	uint64_t inodes_count;
	uint32_t block_groups_count;
	uint32_t blocks_per_group;
	uint32_t inodes_per_group;
	uint16_t inode_size;
	uint16_t desc_size;
	uint32_t first_ino;
	uint8_t  has_extents;
	uint8_t  has_64bit;
	uint8_t  has_flex_bg;
	uint32_t flex_bg_size;
	uint8_t  log_groups_per_flex;
	uint32_t feature_incompat;
	uint32_t feature_ro_compat;
	char     volume_name[16];
	uint8_t  uuid[16];
	uint16_t state;
	uint64_t free_blocks_count;
	uint64_t free_inodes_count;
	uint32_t last_orphan;
	uint32_t csum_seed;     /* CRC-32C seed for metadata_csum (0 if not enabled) */
};

struct ext4_inode_info {
	uint32_t ino;
	uint16_t mode;
	uint64_t size;
	uint16_t uid;
	uint16_t gid;
	uint8_t  is_dir;
	uint8_t  is_reg;
	uint8_t  is_lnk;
	uint8_t  has_extents;
	uint16_t links_count;
	uint64_t blocks;
	uint32_t atime;
	uint32_t ctime;
	uint32_t mtime;
	uint32_t dtime;
};

struct ext4_dirent {
	uint32_t ino;
	uint8_t  file_type;
	uint8_t  name_len;
	char     name[255];
};

/* Group descriptor info (C-compatible) */
struct ext4_gd_info {
	uint64_t block_bitmap;
	uint64_t inode_bitmap;
	uint64_t inode_table;
	uint16_t free_blocks_count;
	uint16_t free_inodes_count;
	uint16_t used_dirs_count;
};

/* Read block callback type */
typedef int (*ext4_read_block_cb)(void *ctx, uint64_t block_nr,
				  uint8_t *buf, uint32_t block_size);

/* Write block callback type */
typedef int (*ext4_write_block_cb)(void *ctx, uint64_t block_nr,
				   const uint8_t *buf, uint32_t block_size);

/* Free blocks callback type (called during truncation) */
typedef int (*ext4_free_blocks_cb)(void *ctx, uint64_t block_nr,
				   uint32_t count);

/* Free inode callback type (called when inode is fully unlinked) */
typedef int (*ext4_free_inode_cb)(void *ctx, uint32_t ino);

/* Allocate block callback type (returns physical block number, or 0 on failure) */
typedef uint64_t (*ext4_alloc_block_cb)(void *ctx);

/* ─── FFI functions ───────────────────────────────────────────────── */

/* Parse and validate the ext4 superblock (from 1024-byte buffer).
 * Returns 0 on success, or a POSIX errno on failure. */
int ext4_parse_superblock(const uint8_t *data, struct ext4_sb_info *sbi);

/* Return the size of ext4_sb_info for C allocation. */
size_t ext4_sb_info_size(void);

/* Read an inode from the filesystem.
 * Returns 0 on success, or a POSIX errno on failure. */
int ext4_read_inode(const struct ext4_sb_info *sbi, uint32_t ino,
		    struct ext4_inode_info *info,
		    void *ctx, ext4_read_block_cb read_block);

/* Lookup a filename in a directory.
 * Returns 0 on success and fills out_ino/out_type, or a POSIX errno. */
int ext4_lookup(const struct ext4_sb_info *sbi, uint32_t dir_ino,
		const char *name, uint32_t *out_ino, uint8_t *out_type,
		void *ctx, ext4_read_block_cb read_block);

/* Read data from a file at the given offset.
 * Returns 0 on success and fills bytes_read, or a POSIX errno. */
int ext4_read_file(const struct ext4_sb_info *sbi, uint32_t ino,
		   uint64_t offset, uint8_t *buf, uint32_t count,
		   uint32_t *bytes_read,
		   void *ctx, ext4_read_block_cb read_block);

/* Get file/directory stat info (needs block read to load inode). */
int ext4_stat(const struct ext4_sb_info *sbi, uint32_t ino,
	      uint16_t *mode, uint64_t *size,
	      uint16_t *uid, uint16_t *gid,
	      void *ctx, ext4_read_block_cb read_block);

/* Get filesystem statistics (uses superblock fields, no block I/O needed). */
int ext4_statvfs(const struct ext4_sb_info *sbi,
		 uint32_t *block_size, uint64_t *blocks_total,
		 uint64_t *blocks_free, uint64_t *inodes_total,
		 uint64_t *inodes_free);

/* Read a single group descriptor from disk. */
int ext4_read_group_descriptor(const struct ext4_sb_info *sbi,
			       uint32_t group, struct ext4_gd_info *gd_info,
			       void *ctx, ext4_read_block_cb read_block);

/* Truncate a file to a new (smaller) size.
 * Frees data blocks beyond new_size via free_blocks callback. */
int ext4_truncate(const struct ext4_sb_info *sbi, uint32_t ino,
		  uint64_t new_size,
		  void *ctx,
		  ext4_read_block_cb read_block,
		  ext4_write_block_cb write_block,
		  ext4_free_blocks_cb free_blocks);

/* Create a hard link: add a directory entry pointing to an existing inode.
 * Increments target inode's link count. */
int ext4_link(const struct ext4_sb_info *sbi, uint32_t dir_ino,
	      const char *name, uint32_t target_ino, uint16_t target_mode,
	      void *ctx,
	      ext4_read_block_cb read_block,
	      ext4_write_block_cb write_block,
	      ext4_alloc_block_cb alloc_block);

/* Remove a hard link: remove a directory entry, decrement target inode's link count.
 * If link count reaches 0, frees inode data blocks and marks inode as free. */
int ext4_unlink(const struct ext4_sb_info *sbi, uint32_t dir_ino,
		const char *name,
		void *ctx,
		ext4_read_block_cb read_block,
		ext4_write_block_cb write_block,
		ext4_free_blocks_cb free_blocks,
		ext4_free_inode_cb free_inode);

/* Inode allocation callback type (returns inode number, or 0 on failure) */
typedef uint32_t (*ext4_alloc_inode_cb)(void *ctx);

/* Create a regular file and add a directory entry.
 * Allocates a new inode and inserts a directory entry in the parent. */
int ext4_create(const struct ext4_sb_info *sbi, uint32_t dir_ino,
		const char *name, uint16_t mode,
		uint16_t uid, uint16_t gid, uint32_t *out_ino,
		void *ctx,
		ext4_read_block_cb read_block,
		ext4_write_block_cb write_block,
		ext4_alloc_block_cb alloc_block,
		ext4_alloc_inode_cb alloc_inode);

/* Create a directory (same as create, but also initializes . and .. entries). */
int ext4_mkdir(const struct ext4_sb_info *sbi, uint32_t dir_ino,
	       const char *name, uint16_t mode,
	       uint16_t uid, uint16_t gid,
	       void *ctx,
	       ext4_read_block_cb read_block,
	       ext4_write_block_cb write_block,
	       ext4_alloc_block_cb alloc_block,
	       ext4_alloc_inode_cb alloc_inode);

/* Rename/move a file between directories.
 * Removes old_name from old_dir and inserts new_name in new_dir.
 * If new_name exists, it is removed first. */
int ext4_rename(const struct ext4_sb_info *sbi,
		uint32_t old_dir_ino, const char *old_name,
		uint32_t new_dir_ino, const char *new_name,
		void *ctx,
		ext4_read_block_cb read_block,
		ext4_write_block_cb write_block,
		ext4_alloc_block_cb alloc_block,
		ext4_free_blocks_cb free_blocks,
		ext4_free_inode_cb free_inode);

/* Write data to a file at the given offset.
 * Allocates new blocks as needed via alloc_block callback.
 * Updates inode size, block count, and timestamps. */
int ext4_write_file(const struct ext4_sb_info *sbi, uint32_t ino,
		    uint64_t offset, const uint8_t *buf, uint32_t count,
		    uint32_t *bytes_written,
		    void *ctx,
		    ext4_read_block_cb read_block,
		    ext4_write_block_cb write_block,
		    ext4_alloc_block_cb alloc_block);

/* Read directory entries starting at `*pos`.
 * Fills up to `max_entries` ext4_dirent structs, updates `pos` and `count`.
 * Returns 0 on success, or a POSIX errno on failure. */
int ext4_readdir(const struct ext4_sb_info *sbi, uint32_t ino,
		 uint64_t *pos,
		 struct ext4_dirent *entries, uint32_t max_entries,
		 uint32_t *count,
		 void *ctx,
		 ext4_read_block_cb read_block);

/* Remove an empty directory.
 * Verifies the directory is empty, removes the entry from the parent,
 * decrements the parent's link count, and frees the directory's data. */
int ext4_rmdir(const struct ext4_sb_info *sbi, uint32_t dir_ino,
	       const char *name,
	       void *ctx,
	       ext4_read_block_cb read_block,
	       ext4_write_block_cb write_block,
	       ext4_free_blocks_cb free_blocks,
	       ext4_free_inode_cb free_inode);

/* Change ownership of a file (chown/chgrp).
 * Reads the inode, sets uid/gid, and writes it back.
 * `mode` is in/out — returns the new mode after the change. */
int ext4_chown(const struct ext4_sb_info *sbi, uint32_t ino,
		uint16_t uid, uint16_t gid, uint16_t *mode,
		void *ctx,
		ext4_read_block_cb read_block,
		ext4_write_block_cb write_block);

/* Change the mode of a file (chmod).
 * `mode` is in/out — returns the new mode preserved with type bits. */
int ext4_chmod(const struct ext4_sb_info *sbi, uint32_t ino,
	       uint16_t *mode,
	       void *ctx,
	       ext4_read_block_cb read_block,
	       ext4_write_block_cb write_block);

/* Update file timestamps (utime). */
int ext4_utime(const struct ext4_sb_info *sbi, uint32_t ino,
	       uint32_t atime, uint32_t mtime,
	       void *ctx,
	       ext4_read_block_cb read_block,
	       ext4_write_block_cb write_block);

/* Create a device node (mknod).
 * Allocates a new inode, initializes it with the given mode and rdev,
 * and inserts a directory entry in the parent. */
int ext4_mknod(const struct ext4_sb_info *sbi, uint32_t dir_ino,
	       const char *name, uint16_t mode,
	       uint16_t uid, uint16_t gid, uint32_t rdev,
	       void *ctx,
	       ext4_read_block_cb read_block,
	       ext4_write_block_cb write_block,
	       ext4_alloc_block_cb alloc_block,
	       ext4_alloc_inode_cb alloc_inode);

/* Create a symbolic link.
 * For short targets (<= 60 bytes), stores target in i_block.
 * For longer targets, allocates a data block and writes through extent tree. */
int ext4_symlink(const struct ext4_sb_info *sbi, uint32_t dir_ino,
		 const char *name, const char *target,
		 uint16_t uid, uint16_t gid,
		 void *ctx,
		 ext4_read_block_cb read_block,
		 ext4_write_block_cb write_block,
		 ext4_alloc_block_cb alloc_block,
		 ext4_alloc_inode_cb alloc_inode);

/* Read the target of a symbolic link.
 * Handles both fast (i_block) and slow (data blocks) symlinks. */
int ext4_readlink(const struct ext4_sb_info *sbi, uint32_t ino,
		  uint8_t *buf, uint32_t buf_size, uint32_t *bytes_read,
		  void *ctx,
		  ext4_read_block_cb read_block);

/* ─── Checksum verification result ───────────────────────────────── */

struct ext4_csum_result {
	uint8_t  sb_valid;
	uint8_t  gd_valid;
	uint32_t gd_failed;	/* First group with failed checksum, or -1 if all okay */
	uint8_t  root_inode_valid;
};

/* ─── Checksum verification FFI functions ────────────────────────── */

/* Compute the CRC-32C seed from s_uuid (called at mount with METADATA_CSUM).
 * Returns 0 on success, or a POSIX errno. */
int ext4_compute_csum_seed(struct ext4_sb_info *sbi, const uint8_t *sb_data);

/* Verify the superblock checksum (s_checksum at offset 672).
 * Returns 0 if valid/no checksum, EBADMSG (74) on mismatch. */
int ext4_verify_sb_csum(const struct ext4_sb_info *sbi,
			const uint8_t *sb_data);

/* Verify a group descriptor checksum.
 * gd_data points to the raw descriptor bytes (desc_size bytes).
 * Returns 0 if valid/no checksum, EBADMSG (74) on mismatch. */
int ext4_verify_gd_csum(const struct ext4_sb_info *sbi, uint32_t group,
			const uint8_t *gd_data, uint16_t desc_size);

/* Verify an inode checksum.
 * inode_data points to raw inode bytes (inode_size bytes).
 * Returns 0 if valid/no checksum, EBADMSG (74) on mismatch. */
int ext4_verify_inode_csum(const struct ext4_sb_info *sbi, uint32_t ino,
			   const uint8_t *inode_data, uint16_t inode_size);

/* Batch-verify all metadata checksums at mount time.
 * Validates superblock, all group descriptors, and root inode (inode 2).
 * Fills result with pass/fail status.
 * Returns 0 on success (even if checksums are bad — caller decides action). */
int ext4_verify_all_csums(const struct ext4_sb_info *sbi,
			  const uint8_t *sb_data,
			  struct ext4_csum_result *result,
			  void *ctx,
			  ext4_read_block_cb read_block);

/* ─── Checksum update FFI functions ──────────────────────────────── */

/* Update the superblock checksum (s_checksum) in a raw SB buffer.
 * Zeros offset 672, computes CRC-32C over 1024 bytes, writes result.
 * Returns 0 on success, or a POSIX errno. */
int ext4_update_sb_csum(const struct ext4_sb_info *sbi,
			uint8_t *sb_data);

/* Update a group descriptor's checksum (bg_checksum) in a raw GD buffer.
 * Zeros offset 30, computes CRC-32C over desc_size bytes, writes result.
 * For 64-bit descriptors (desc_size > 32), also incorporates group number.
 * Returns 0 on success, or a POSIX errno. */
int ext4_update_gd_csum(const struct ext4_sb_info *sbi,
			uint32_t group, uint8_t *gd_data,
			uint16_t desc_size);

#endif /* _MINIX_EXT4_FFI_H */
