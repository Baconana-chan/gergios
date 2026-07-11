/* rust/ext4-core/include/ext4.h
 *
 * C-compatible header for the ext4-core Rust library.
 * Auto-generated from rust/ext4-core/src/ffi.rs.
 *
 * Usage:
 *   #include <ext4.h>
 *   // link against: -lext4_core (libext4_core.a)
 *
 * All functions return 0 on success, or a POSIX errno value on failure.
 * Memory management: call ext4_sb_info_size() to allocate, then pass
 * pointers to the FFI functions.
 */

#ifndef EXT4_CORE_H
#define EXT4_CORE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===================================================================
 * C-compatible structures (mirrors of Rust repr(C) types)
 * =================================================================== */

/** Parsed superblock info (output of ext4_parse_superblock). */
typedef struct ext4_sb_info {
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
    uint32_t csum_seed;       /* CRC-32C seed for metadata_csum */
} ext4_sb_info;

/** Parsed inode info (output of ext4_read_inode). */
typedef struct ext4_inode_info {
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
} ext4_inode_info;

/** Directory entry (output of ext4_readdir). */
typedef struct ext4_dirent {
    uint32_t ino;
    uint8_t  file_type;
    uint8_t  name_len;
    char     name[255];
} ext4_dirent;

/** Group descriptor info (output of ext4_read_group_descriptor). */
typedef struct ext4_gd_info {
    uint64_t block_bitmap;
    uint64_t inode_bitmap;
    uint64_t inode_table;
    uint16_t free_blocks_count;
    uint16_t free_inodes_count;
    uint16_t used_dirs_count;
} ext4_gd_info;

/** Batch checksum verification result (output of ext4_verify_all_csums). */
typedef struct ext4_csum_result {
    uint8_t  sb_valid;
    uint8_t  gd_valid;
    uint32_t gd_failed;       /* 0xFFFFFFFF if all passed */
    uint8_t  root_inode_valid;
} ext4_csum_result;

/* ===================================================================
 * Callback types
 * =================================================================== */

/** Block read callback. Returns 0 on success, non-zero on error. */
typedef int (*ext4_read_block_cb)(void *ctx, uint64_t block_nr,
                                  uint8_t *buf, uint32_t block_size);

/** Block write callback. Returns 0 on success, non-zero on error. */
typedef int (*ext4_write_block_cb)(void *ctx, uint64_t block_nr,
                                   const uint8_t *buf, uint32_t block_size);

/** Free blocks callback (called when truncating). */
typedef int (*ext4_free_blocks_cb)(void *ctx, uint64_t block_nr,
                                   uint32_t count);

/** Free inode callback. */
typedef int (*ext4_free_inode_cb)(void *ctx, uint32_t ino);

/** Allocate a new block. Returns physical block number or 0 on failure. */
typedef uint64_t (*ext4_alloc_block_cb)(void *ctx);

/** Allocate a new inode. Returns inode number or 0 on failure. */
typedef uint32_t (*ext4_alloc_inode_cb)(void *ctx);

/* ===================================================================
 * FFI Functions
 * =================================================================== */

/* --- Superblock --- */

/**
 * Parse and validate the ext4 superblock from raw data.
 *
 * @param data   Pointer to at least 1024 bytes of raw superblock data.
 * @param sbi    Output: parsed superblock info.
 * @return 0 on success, EINVAL (22) on bad magic / invalid fields.
 */
int ext4_parse_superblock(const uint8_t *data, ext4_sb_info *sbi);

/** Return the size of ext4_sb_info (for memory allocation). */
size_t ext4_sb_info_size(void);

/**
 * Compute the CRC-32C seed from the superblock UUID.
 * Must be called before any checksum operations.
 */
int ext4_compute_csum_seed(ext4_sb_info *sbi, const uint8_t *sb_data);

/**
 * Update the superblock checksum (s_checksum at offset 672).
 */
int ext4_update_sb_csum(const ext4_sb_info *sbi, uint8_t *sb_data);

/**
 * Verify the superblock checksum.
 * Returns 0 if valid, EBADMSG (74) on mismatch.
 */
int ext4_verify_sb_csum(const ext4_sb_info *sbi, const uint8_t *sb_data);

/* --- Group Descriptor Checksums --- */

/**
 * Update a group descriptor's checksum (bg_checksum at offset 30).
 */
int ext4_update_gd_csum(const ext4_sb_info *sbi, uint32_t group,
                        uint8_t *gd_data, uint16_t desc_size);

/**
 * Verify a group descriptor's checksum.
 */
int ext4_verify_gd_csum(const ext4_sb_info *sbi, uint32_t group,
                        const uint8_t *gd_data, uint16_t desc_size);

/**
 * Verify an inode's checksum.
 */
int ext4_verify_inode_csum(const ext4_sb_info *sbi, uint32_t ino,
                           const uint8_t *inode_data, uint16_t inode_size);

/**
 * Batch-verify all metadata checksums at mount time.
 * Reads GDT and root inode via read_block callback.
 */
int ext4_verify_all_csums(const ext4_sb_info *sbi, const uint8_t *sb_data,
                          ext4_csum_result *result, void *ctx,
                          ext4_read_block_cb read_block);

/* --- Inode Operations --- */

/**
 * Read an inode from the filesystem.
 *
 * @param sbi        Parsed superblock info.
 * @param ino        Inode number to read.
 * @param info       Output: parsed inode info.
 * @param ctx        Opaque context for callbacks.
 * @param read_block Block read callback.
 * @return 0 on success, or POSIX errno.
 */
int ext4_read_inode(const ext4_sb_info *sbi, uint32_t ino,
                    ext4_inode_info *info, void *ctx,
                    ext4_read_block_cb read_block);

/**
 * Get file/directory stat info.
 */
int ext4_stat(const ext4_sb_info *sbi, uint32_t ino,
              uint16_t *mode, uint64_t *size,
              uint16_t *uid, uint16_t *gid,
              void *ctx, ext4_read_block_cb read_block);

/**
 * Change file ownership (chown/chgrp).
 */
int ext4_chown(const ext4_sb_info *sbi, uint32_t ino,
               uint16_t uid, uint16_t gid, uint16_t *mode,
               void *ctx, ext4_read_block_cb read_block,
               ext4_write_block_cb write_block);

/**
 * Change file mode (chmod).
 */
int ext4_chmod(const ext4_sb_info *sbi, uint32_t ino, uint16_t *mode,
               void *ctx, ext4_read_block_cb read_block,
               ext4_write_block_cb write_block);

/**
 * Update file timestamps (utime).
 */
int ext4_utime(const ext4_sb_info *sbi, uint32_t ino,
               uint32_t atime, uint32_t mtime,
               void *ctx, ext4_read_block_cb read_block,
               ext4_write_block_cb write_block);

/* --- Directory Operations --- */

/**
 * Lookup a file name in a directory.
 *
 * @param sbi        Parsed superblock info.
 * @param dir_ino    Inode of the directory.
 * @param name       File name (null-terminated).
 * @param out_ino    Output: inode number of the found entry.
 * @param out_type   Output: file type.
 * @param ctx        Opaque context for callbacks.
 * @param read_block Block read callback.
 * @return 0 on success, ENOENT (2) if not found.
 */
int ext4_lookup(const ext4_sb_info *sbi, uint32_t dir_ino,
                const char *name, uint32_t *out_ino, uint8_t *out_type,
                void *ctx, ext4_read_block_cb read_block);

/**
 * Read directory entries.
 *
 * @param sbi         Parsed superblock info.
 * @param ino         Directory inode.
 * @param pos         In/Out: current read position (updated after call).
 * @param entries     Output buffer for ext4_dirent structs.
 * @param max_entries Maximum number of entries to return.
 * @param count       Output: number of entries actually returned.
 * @param ctx         Opaque context for callbacks.
 * @param read_block  Block read callback.
 */
int ext4_readdir(const ext4_sb_info *sbi, uint32_t ino,
                 uint64_t *pos, ext4_dirent *entries,
                 uint32_t max_entries, uint32_t *count,
                 void *ctx, ext4_read_block_cb read_block);

/**
 * Read data from a file at the given offset.
 */
int ext4_read_file(const ext4_sb_info *sbi, uint32_t ino,
                   uint64_t offset, uint8_t *buf, uint32_t count,
                   uint32_t *bytes_read, void *ctx,
                   ext4_read_block_cb read_block);

/**
 * Write data to a file at the given offset (allocates blocks as needed).
 */
int ext4_write_file(const ext4_sb_info *sbi, uint32_t ino,
                    uint64_t offset, const uint8_t *buf, uint32_t count,
                    uint32_t *bytes_written, void *ctx,
                    ext4_read_block_cb read_block,
                    ext4_write_block_cb write_block,
                    ext4_alloc_block_cb alloc_block);

/**
 * Read a single group descriptor from disk.
 */
int ext4_read_group_descriptor(const ext4_sb_info *sbi, uint32_t group,
                               ext4_gd_info *gd_info, void *ctx,
                               ext4_read_block_cb read_block);

/* --- File Mutation --- */

/**
 * Create a regular file.
 */
int ext4_create(const ext4_sb_info *sbi, uint32_t dir_ino,
                const char *name, uint16_t mode,
                uint16_t uid, uint16_t gid, uint32_t *out_ino,
                void *ctx, ext4_read_block_cb read_block,
                ext4_write_block_cb write_block,
                ext4_alloc_block_cb alloc_block,
                ext4_alloc_inode_cb alloc_inode);

/**
 * Create a directory.
 */
int ext4_mkdir(const ext4_sb_info *sbi, uint32_t dir_ino,
               const char *name, uint16_t mode,
               uint16_t uid, uint16_t gid,
               void *ctx, ext4_read_block_cb read_block,
               ext4_write_block_cb write_block,
               ext4_alloc_block_cb alloc_block,
               ext4_alloc_inode_cb alloc_inode);

/**
 * Create a device node (mknod).
 */
int ext4_mknod(const ext4_sb_info *sbi, uint32_t dir_ino,
               const char *name, uint16_t mode,
               uint16_t uid, uint16_t gid, uint32_t rdev,
               void *ctx, ext4_read_block_cb read_block,
               ext4_write_block_cb write_block,
               ext4_alloc_block_cb alloc_block,
               ext4_alloc_inode_cb alloc_inode);

/**
 * Remove a file (unlink).
 */
int ext4_unlink(const ext4_sb_info *sbi, uint32_t dir_ino,
                const char *name,
                void *ctx, ext4_read_block_cb read_block,
                ext4_write_block_cb write_block,
                ext4_free_blocks_cb free_blocks,
                ext4_free_inode_cb free_inode);

/**
 * Remove an empty directory.
 */
int ext4_rmdir(const ext4_sb_info *sbi, uint32_t dir_ino,
               const char *name,
               void *ctx, ext4_read_block_cb read_block,
               ext4_write_block_cb write_block,
               ext4_free_blocks_cb free_blocks,
               ext4_free_inode_cb free_inode);

/**
 * Create a hard link.
 */
int ext4_link(const ext4_sb_info *sbi, uint32_t dir_ino,
              const char *name, uint32_t target_ino, uint16_t target_mode,
              void *ctx, ext4_read_block_cb read_block,
              ext4_write_block_cb write_block,
              ext4_alloc_block_cb alloc_block);

/**
 * Rename a file (move between directories).
 */
int ext4_rename(const ext4_sb_info *sbi,
                uint32_t old_dir_ino, const char *old_name,
                uint32_t new_dir_ino, const char *new_name,
                void *ctx, ext4_read_block_cb read_block,
                ext4_write_block_cb write_block,
                ext4_alloc_block_cb alloc_block,
                ext4_free_blocks_cb free_blocks,
                ext4_free_inode_cb free_inode);

/**
 * Truncate a file to a new (smaller) size.
 */
int ext4_truncate(const ext4_sb_info *sbi, uint32_t ino, uint64_t new_size,
                  void *ctx, ext4_read_block_cb read_block,
                  ext4_write_block_cb write_block,
                  ext4_free_blocks_cb free_blocks);

#ifdef __cplusplus
}
#endif

#endif /* EXT4_CORE_H */
