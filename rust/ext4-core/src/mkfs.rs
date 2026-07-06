//! mkfs.ext4 — C FFI function to create an ext4 filesystem on a block device
//!
//! The `ext4_mkfs()` function is the Rust core of the `mkfs.ext4` tool.
//! It accepts a raw file descriptor and creates a minimal ext4 filesystem
//! with a single block group, root directory, lost+found, and extents.
//!
//! This module uses POSIX file I/O (`std::os::unix`). On non-Unix platforms
//! (e.g., Windows), the function returns ENOTSUP at runtime.

use core::ffi::c_int;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(unix)]
use std::os::unix::io::FromRawFd;

#[cfg(unix)]
use std::time::SystemTime;
#[cfg(unix)]
use std::collections::hash_map::DefaultHasher;
#[cfg(unix)]
use std::hash::{Hash, Hasher};

#[cfg(unix)]
use crate::types::*;
#[cfg(unix)]
use crate::superblock::serialize_superblock;
#[cfg(unix)]
use crate::group_desc::serialize_group_desc;
#[cfg(unix)]
use crate::inode::{new_inode, serialize_inode, init_extent_tree};
#[cfg(unix)]
use crate::dir::init_dir_block;

/// Maximum number of blocks in a single block group (128MB at 4096-byte blocks).
const BLOCKS_PER_GROUP: u32 = 32768;

/// First non-reserved inode.
const FIRST_DATA_INODE: u32 = 11;

/// Inode number for the root directory.
const ROOT_INO: u32 = 2;

/// Inode number for lost+found.
const LOST_FOUND_INO: u32 = 11;

/// Create a minimal ext4 filesystem on the given file descriptor.
///
/// Parameters:
/// - `fd`: open file descriptor (writable, seekable)
/// - `block_size`: 1024, 2048, or 4096
/// - `blocks_count`: total blocks in the device (clamped to 1 block group)
///
/// Returns 0 on success, or a positive errno value on failure.
///
/// # Safety
/// `fd` must be a valid, writable file descriptor.
#[no_mangle]
pub unsafe extern "C" fn ext4_mkfs(fd: c_int, block_size: u32, blocks_count: u64) -> c_int {
    // On non-Unix platforms, return ENOTSUP
    #[cfg(not(unix))]
    {
        let _ = (fd, block_size, blocks_count);
        return 95;
    }

    #[cfg(unix)]
    {
        ext4_mkfs_impl(fd, block_size, blocks_count)
    }
}

/// Actual implementation (Unix only).
#[cfg(unix)]
unsafe fn ext4_mkfs_impl(fd: c_int, block_size: u32, blocks_count: u64) -> c_int {
    if fd < 0 {
        return 9; // EBADF
    }

    let bs = match block_size {
        1024 | 2048 | 4096 => block_size as usize,
        _ => return 22,
    };

    let bg_blocks = if blocks_count > BLOCKS_PER_GROUP as u64 {
        BLOCKS_PER_GROUP as u64
    } else {
        blocks_count
    };

    let inodes_per_group: u32 = ((bg_blocks * bs as u64 / 16384).max(16) as u32).min(8192);
    let desc_size: usize = 64;
    let first_data_block: u32 = if bs > 1024 { 0 } else { 1 };
    let log_block_size_val: u32 = match bs {
        1024 => 0,
        2048 => 1,
        4096 => 2,
        _ => 2,
    };

    // Compute layout
    let gdt_block = first_data_block as u64 + 1;
    let block_bitmap_block = gdt_block + 1;
    let inode_bitmap_block = block_bitmap_block + 1;
    let inode_table_block = inode_bitmap_block + 1;

    let inode_size: usize = 256;
    let inodes_per_block = bs / inode_size;
    let inode_table_blocks =
        (inodes_per_group as u64 + inodes_per_block as u64 - 1) / inodes_per_block as u64;
    let data_start_block = inode_table_block + inode_table_blocks;
    let total_metadata_blocks = data_start_block;
    let free_blocks = bg_blocks - total_metadata_blocks;

    // ── UUID ──────────────────────────────────────────────────────
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    now.hash(&mut hasher);
    let hash = hasher.finish();
    let uuid: [u8; 16] = [
        now.as_secs() as u8,
        (now.as_secs() >> 8) as u8,
        (now.as_secs() >> 16) as u8,
        (now.as_secs() >> 24) as u8,
        now.subsec_nanos() as u8,
        (now.subsec_nanos() >> 8) as u8,
        (now.subsec_nanos() >> 16) as u8,
        (now.subsec_nanos() >> 24) as u8,
        hash as u8,
        (hash >> 8) as u8,
        (hash >> 16) as u8,
        (hash >> 24) as u8,
        (hash >> 32) as u8,
        (hash >> 40) as u8,
        (hash >> 48) as u8,
        (hash >> 56) as u8,
    ];

    // ── Feature flags ─────────────────────────────────────────────
    let incompat = EXT4_FEATURE_INCOMPAT_FILETYPE
        | EXT4_FEATURE_INCOMPAT_EXTENTS
        | EXT4_FEATURE_INCOMPAT_FLEX_BG;

    let ro_compat = EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER
        | EXT4_FEATURE_RO_COMPAT_LARGE_FILE
        | EXT4_FEATURE_RO_COMPAT_GDT_CSUM
        | EXT4_FEATURE_RO_COMPAT_DIR_NLINK
        | EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE;

    let timestamp = now.as_secs() as u32;

    // ── Build superblock ──────────────────────────────────────────
    let sb = Ext4Superblock {
        s_inodes_count: inodes_per_group,
        s_blocks_count_lo: bg_blocks as u32,
        s_blocks_count_hi: (bg_blocks >> 32) as u32,
        s_free_blocks_count_lo: free_blocks as u32,
        s_free_inodes_count: inodes_per_group - FIRST_DATA_INODE + 1,
        s_first_data_block: first_data_block,
        s_log_block_size: log_block_size_val,
        s_log_cluster_size: log_block_size_val,
        s_blocks_per_group: BLOCKS_PER_GROUP,
        s_clusters_per_group: BLOCKS_PER_GROUP,
        s_inodes_per_group: inodes_per_group,
        s_mtime: timestamp,
        s_wtime: timestamp,
        s_mnt_count: 0,
        s_max_mnt_count: 0xFFFF,
        s_magic: EXT4_SUPER_MAGIC,
        s_state: 1,
        s_errors: 1,
        s_minor_rev_level: 0,
        s_lastcheck: timestamp,
        s_checkinterval: 0,
        s_creator_os: 0,
        s_rev_level: 1,
        s_def_resuid: 0,
        s_def_resgid: 0,
        s_first_ino: FIRST_DATA_INODE,
        s_inode_size: inode_size as u16,
        s_block_group_nr: 0,
        s_feature_compat: 0,
        s_feature_incompat: incompat,
        s_feature_ro_compat: ro_compat,
        s_uuid: uuid,
        s_volume_name: *b"gergios\0\0\0\0\0\0\0\0\0",
        s_last_mounted: [0u8; 64],
        s_algorithm_usage_bitmap: 0,
        s_prealloc_blocks: 0,
        s_prealloc_dir_blocks: 0,
        s_reserved_gdt_blocks: 0,
        s_journal_uuid: [0u8; 16],
        s_journal_inum: 0,
        s_journal_dev: 0,
        s_last_orphan: 0,
        s_hash_seed: [0u32; 4],
        s_def_hash_version: 0,
        s_jnl_backup_type: 0,
        s_desc_size: desc_size as u16,
        s_default_mount_opts: 0,
        s_first_meta_bg: 0,
        s_mkfs_time: timestamp,
        s_jnl_blocks: [0u32; 17],
        s_free_blocks_count_hi: 0,
        s_inodes_count_hi: 0,
        s_free_inodes_count_hi: 0,
        s_minor_extra_isize: 0,
        s_want_extra_isize: 32,
        s_flags: 0,
        s_raid_stride: 0,
        s_mmp_interval: 0,
        s_mmp_block: 0,
        s_raid_stripe_width: 0,
        s_log_groups_per_flex: 0,
        s_checksum_type: 0,
        s_reserved_pad: 0,
        s_kbytes_written: 0,
        s_snapshot_inum: 0,
        s_snapshot_id: 0,
        s_snapshot_r_blocks: 0,
        s_snapshot_list: 0,
        s_error_count: 0,
        s_first_error_time: 0,
        s_first_error_ino: 0,
        s_first_error_block: 0,
        s_first_error_func: [0u8; 32],
        s_first_error_line: 0,
        s_last_error_time: 0,
        s_last_error_ino: 0,
        s_last_error_line: 0,
        s_last_error_block: 0,
        s_last_error_func: [0u8; 32],
        s_mount_opts: [0u8; 64],
        s_usr_quota_inum: 0,
        s_grp_quota_inum: 0,
        s_overhead_blocks: total_metadata_blocks as u32,
        s_backup_bgs: [0u32; 2],
        s_encrypt_algos: [0u8; 4],
        s_encrypt_pw_salt: [0u8; 16],
        s_lpf_ino: LOST_FOUND_INO,
        s_prj_quota_inum: 0,
        s_checksum_seed: 0,
        s_wtime_hi: 0,
        s_mtime_hi: 0,
        s_mkfs_time_hi: 0,
        s_lastcheck_hi: 0,
        s_first_error_time_hi: 0,
        s_last_error_time_hi: 0,
        s_pad: [0u8; 2],
        s_checksum: 0,
    };

    // Wrap fd in ManuallyDrop to avoid closing it
    let file = std::mem::ManuallyDrop::new(File::from_raw_fd(fd));

    let write_at = |buf: &[u8], offset: u64| -> bool {
        file.write_all_at(buf, offset).is_ok()
    };
    let read_at = |buf: &mut [u8], offset: u64| -> bool {
        file.read_exact_at(buf, offset).is_ok()
    };

    // ── Zero block 0 + write superblock at offset 1024 ────────────
    let zero_buf = vec![0u8; bs];
    if !write_at(&zero_buf, 0) { return 5; }
    let mut sb_buf = vec![0u8; 1024];
    serialize_superblock(&sb, &mut sb_buf);
    if !write_at(&sb_buf, 1024) { return 5; }

    // ── Write group descriptor ────────────────────────────────────
    let gd = Ext4GroupDesc {
        bg_block_bitmap_lo: block_bitmap_block as u32,
        bg_inode_bitmap_lo: inode_bitmap_block as u32,
        bg_inode_table_lo: inode_table_block as u32,
        bg_free_blocks_count_lo: free_blocks as u16,
        bg_free_inodes_count_lo: (inodes_per_group - FIRST_DATA_INODE + 1) as u16,
        bg_used_dirs_count_lo: 2,
        bg_flags: 0,
        bg_exclude_bitmap_lo: 0,
        bg_block_bitmap_csum_lo: 0,
        bg_inode_bitmap_csum_lo: 0,
        bg_itable_unused_lo: 0,
        bg_checksum: 0,
        bg_block_bitmap_hi: 0,
        bg_inode_bitmap_hi: 0,
        bg_inode_table_hi: 0,
        bg_free_blocks_count_hi: 0,
        bg_free_inodes_count_hi: 0,
        bg_used_dirs_count_hi: 0,
        bg_itable_unused_hi: 0,
        bg_exclude_bitmap_hi: 0,
        bg_block_bitmap_csum_hi: 0,
        bg_inode_bitmap_csum_hi: 0,
        bg_reserved: 0,
    };
    let mut gdt_buf = vec![0u8; bs];
    serialize_group_desc(&gd, &mut gdt_buf[..desc_size], desc_size);
    if !write_at(&gdt_buf, gdt_block * bs as u64) { return 5; }

    // ── Write block bitmap ────────────────────────────────────────
    let mut block_bitmap = vec![0u8; bs];
    for b in 0..total_metadata_blocks as usize {
        let byte = b / 8;
        let bit = b % 8;
        if byte < bs { block_bitmap[byte] |= 1 << bit; }
    }
    if !write_at(&block_bitmap, block_bitmap_block * bs as u64) { return 5; }

    // ── Write inode bitmap ────────────────────────────────────────
    let mut inode_bitmap = vec![0u8; bs];
    for ino in 0..FIRST_DATA_INODE as usize {
        let byte = ino / 8;
        let bit = ino % 8;
        if byte < bs { inode_bitmap[byte] |= 1 << bit; }
    }
    if !write_at(&inode_bitmap, inode_bitmap_block * bs as u64) { return 5; }

    // ── Write inode table ─────────────────────────────────────────
    let it_blocks = inode_table_blocks as usize;
    let mut inode_table = vec![0u8; it_blocks * bs];

    // Root inode (inode 2)
    let mut root_inode = new_inode(EXT4_S_IFDIR | 0o755, 0, 0);
    root_inode.i_links_count = 2;
    init_extent_tree(&mut root_inode);
    let root_off = 1 * inode_size;
    if root_off + inode_size <= inode_table.len() {
        serialize_inode(&mut inode_table[root_off..root_off + inode_size], &root_inode, &sb, Some(ROOT_INO));
    }

    // Lost+found inode (inode 11)
    let mut lf_inode = new_inode(EXT4_S_IFDIR | 0o755, 0, 0);
    lf_inode.i_links_count = 2;
    init_extent_tree(&mut lf_inode);
    let lf_off = (LOST_FOUND_INO as usize - 1) * inode_size;
    if lf_off + inode_size <= inode_table.len() {
        serialize_inode(&mut inode_table[lf_off..lf_off + inode_size], &lf_inode, &sb, Some(LOST_FOUND_INO));
    }

    if !write_at(&inode_table, inode_table_block * bs as u64) { return 5; }

    // ── Write root directory data block ───────────────────────────
    let root_data_block = data_start_block;
    let mut dir_block = vec![0u8; bs];
    init_dir_block(&mut dir_block, ROOT_INO, ROOT_INO, 0);
    if !write_at(&dir_block, root_data_block * bs as u64) { return 5; }

    // ── Update inode table with extent entries ────────────────────
    let mut inode_table2 = vec![0u8; it_blocks * bs];
    if !read_at(&mut inode_table2, inode_table_block * bs as u64) { return 5; }

    let sectors_per_block = bs as u64 / 512;
    let ri_ext_off = root_off + 12;
    if ri_ext_off + 12 <= inode_table2.len() {
        inode_table2[ri_ext_off..ri_ext_off + 4].copy_from_slice(&0u32.to_le_bytes());
        inode_table2[ri_ext_off + 4..ri_ext_off + 6].copy_from_slice(&1u16.to_le_bytes());
        inode_table2[ri_ext_off + 6..ri_ext_off + 8].copy_from_slice(&((root_data_block >> 32) as u16).to_le_bytes());
        inode_table2[ri_ext_off + 8..ri_ext_off + 12].copy_from_slice(&(root_data_block as u32).to_le_bytes());
        inode_table2[root_off + 2..root_off + 4].copy_from_slice(&1u16.to_le_bytes());
        inode_table2[root_off + 4..root_off + 8].copy_from_slice(&(bs as u32).to_le_bytes());
        inode_table2[root_off + 28..root_off + 32].copy_from_slice(&(sectors_per_block as u32).to_le_bytes());
    }

    let lf_data_block = data_start_block + 1;
    let li_ext_off = lf_off + 12;
    if li_ext_off + 12 <= inode_table2.len() {
        inode_table2[li_ext_off..li_ext_off + 4].copy_from_slice(&0u32.to_le_bytes());
        inode_table2[li_ext_off + 4..li_ext_off + 6].copy_from_slice(&1u16.to_le_bytes());
        inode_table2[li_ext_off + 6..li_ext_off + 8].copy_from_slice(&((lf_data_block >> 32) as u16).to_le_bytes());
        inode_table2[li_ext_off + 8..li_ext_off + 12].copy_from_slice(&(lf_data_block as u32).to_le_bytes());
        inode_table2[lf_off + 2..lf_off + 4].copy_from_slice(&1u16.to_le_bytes());
        inode_table2[lf_off + 4..lf_off + 8].copy_from_slice(&(bs as u32).to_le_bytes());
        inode_table2[lf_off + 28..lf_off + 32].copy_from_slice(&(sectors_per_block as u32).to_le_bytes());
    }

    if !write_at(&inode_table2, inode_table_block * bs as u64) { return 5; }

    // ── Write lost+found directory data block ─────────────────────
    let mut lf_block = vec![0u8; bs];
    init_dir_block(&mut lf_block, LOST_FOUND_INO, ROOT_INO, 0);
    if !write_at(&lf_block, lf_data_block * bs as u64) { return 5; }

    0
}
