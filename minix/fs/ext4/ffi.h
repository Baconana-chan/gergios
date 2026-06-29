/* ext4 FFI interface — C bridge to Rust ext4-core parser.
 *
 * This header declares the C-compatible types and functions exported
 * by the Rust ext4-core static library.
 */

#ifndef _MINIX_EXT4_FFI_H
#define _MINIX_EXT4_FFI_H

#include <sys/types.h>
#include <stdint.h>

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
};

struct ext4_dirent {
	uint32_t ino;
	uint8_t  file_type;
	uint8_t  name_len;
	char     name[255];
};

/* Read block callback type */
typedef int (*ext4_read_block_cb)(void *ctx, uint64_t block_nr,
				  uint8_t *buf, uint32_t block_size);

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
 * Returns 0 on success and fills out_ino, or a POSIX errno. */
int ext4_lookup(const struct ext4_sb_info *sbi, uint32_t dir_ino,
		const char *name, uint32_t *out_ino, uint8_t *out_type);

/* Read data from a file at the given offset.
 * Returns 0 on success and fills bytes_read, or a POSIX errno. */
int ext4_read_file(const struct ext4_sb_info *sbi, uint32_t ino,
		   uint64_t offset, uint8_t *buf, uint32_t count,
		   uint32_t *bytes_read);

/* Get file/directory stat info. */
int ext4_stat(const struct ext4_sb_info *sbi, uint32_t ino,
	      uint16_t *mode, uint64_t *size,
	      uint16_t *uid, uint16_t *gid);

/* Get filesystem statistics. */
int ext4_statvfs(const struct ext4_sb_info *sbi,
		 uint32_t *block_size, uint64_t *blocks_total,
		 uint64_t *blocks_free, uint64_t *inodes_total,
		 uint64_t *inodes_free);

#endif /* _MINIX_EXT4_FFI_H */
