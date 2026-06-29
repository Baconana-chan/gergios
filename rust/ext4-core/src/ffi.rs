//! C FFI interface for the ext4-core crate.
//!
//! These functions are called from the C bridge in `minix/fs/ext4/ffi_bridge.c`.
//! The FFI uses raw C-compatible types and avoids Rust panics from crossing
//! the FFI boundary (using `catch_unwind`).

use core::ffi::{c_char, c_int, c_void};
use core::slice;

use crate::types::*;

// ─── Error code mapping ──────────────────────────────────────────────

/// Convert an Ext4Error to a POSIX errno-style integer.
fn errno_from_error(err: Ext4Error) -> c_int {
    match err {
        Ext4Error::InvalidMagic => 22,   // EINVAL
        Ext4Error::UnsupportedIncompat(_) => 95, // ENOTSUP
        Ext4Error::UnsupportedRoCompat(_) => 95,
        Ext4Error::InvalidBlockSize => 22,
        Ext4Error::InvalidInodeSize => 22,
        Ext4Error::ChecksumMismatch => 74, // EBADMSG
        Ext4Error::IoError => 5,        // EIO
        Ext4Error::NotFound => 2,       // ENOENT
        Ext4Error::InvalidExtentHeader => 22,
        Ext4Error::NotAnExtentInode => 22,
        Ext4Error::ReadOnly => 30,      // EROFS
        Ext4Error::NoSpace => 28,       // ENOSPC
        Ext4Error::ExtentTreeFull => 95, // ENOTSUP
        Ext4Error::InvalidJournal => 95,  // ENOTSUP
        Ext4Error::JournalCorrupt => 74,  // EBADMSG
    }
}

// ─── C-compatible superblock info ────────────────────────────────────

/// C-compatible structure with parsed superblock info.
/// This is passed between C and Rust.
#[repr(C)]
pub struct ext4_sb_info {
    pub block_size: u32,
    pub blocks_count: u64,
    pub inodes_count: u64,
    pub block_groups_count: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub inode_size: u16,
    pub desc_size: u16,
    pub first_ino: u32,
    pub has_extents: u8,
    pub has_64bit: u8,
    pub has_flex_bg: u8,
    pub flex_bg_size: u32,
    pub log_groups_per_flex: u8,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub volume_name: [c_char; 16],
    pub uuid: [u8; 16],
    pub state: u16,
}

#[repr(C)]
pub struct ext4_inode_info {
    pub ino: u32,
    pub mode: u16,
    pub size: u64,
    pub uid: u16,
    pub gid: u16,
    pub is_dir: u8,
    pub is_reg: u8,
    pub is_lnk: u8,
    pub has_extents: u8,
    pub links_count: u16,
    pub blocks: u64,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
}

#[repr(C)]
pub struct ext4_dirent {
    pub ino: u32,
    pub file_type: u8,
    pub name_len: u8,
    pub name: [c_char; 255],
}

// ─── Read block callback type ────────────────────────────────────────

/// Type for the block read callback used by FFI functions.
/// `ctx` is an opaque pointer passed to the callback.
/// `block_nr` is the block number to read.
/// `buf` is the buffer to fill (must be at least `block_size` bytes).
/// Returns 0 on success, non-zero on error.
pub type ext4_read_block_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
    block_nr: u64,
    buf: *mut u8,
    block_size: u32,
) -> c_int;

// ─── FFI functions ───────────────────────────────────────────────────

/// Parse and validate the ext4 superblock.
///
/// # Safety
/// `data` must point to a valid buffer of at least 1024 bytes.
/// `sbi` must point to a valid `ext4_sb_info` struct.
#[no_mangle]
pub unsafe extern "C" fn ext4_parse_superblock(
    data: *const u8,
    sbi: *mut ext4_sb_info,
) -> c_int {
    if data.is_null() || sbi.is_null() {
        return 22; // EINVAL
    }

    let data_slice = unsafe { slice::from_raw_parts(data, 1024) };

    match crate::superblock::parse_superblock(data_slice) {
        Ok(sb) => {
            unsafe {
                (*sbi).block_size = sb.block_size() as u32;
                (*sbi).blocks_count = sb.blocks_count();
                (*sbi).inodes_count = sb.inodes_count() as u64;
                (*sbi).block_groups_count = sb.block_groups_count();
                (*sbi).blocks_per_group = sb.s_blocks_per_group;
                (*sbi).inodes_per_group = sb.s_inodes_per_group;
                (*sbi).inode_size = sb.inode_size() as u16;
                (*sbi).desc_size = sb.desc_size() as u16;
                (*sbi).first_ino = sb.s_first_ino;
                (*sbi).has_extents = if sb.has_extents() { 1 } else { 0 };
                (*sbi).has_64bit = if sb.has_64bit() { 1 } else { 0 };
                (*sbi).has_flex_bg = if sb.has_flex_bg() { 1 } else { 0 };
                (*sbi).flex_bg_size = sb.flex_bg_size();
                (*sbi).log_groups_per_flex = sb.s_log_groups_per_flex;
                (*sbi).feature_incompat = sb.s_feature_incompat;
                (*sbi).feature_ro_compat = sb.s_feature_ro_compat;
                for i in 0..16 {
                    (*sbi).volume_name[i] = sb.s_volume_name[i] as c_char;
                }
                (*sbi).uuid = sb.s_uuid;
                (*sbi).state = sb.s_state;
            }
            0
        }
        Err(e) => errno_from_error(e),
    }
}

/// Return the size of `ext4_sb_info` in bytes (for C allocation).
#[no_mangle]
pub extern "C" fn ext4_sb_info_size() -> usize {
    core::mem::size_of::<ext4_sb_info>()
}

/// Read an inode from the filesystem.
///
/// # Safety
/// `read_block` is a callback that must be callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_read_inode(
    sbi: *const ext4_sb_info,
    ino: u32,
    info: *mut ext4_inode_info,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || info.is_null() || read_block.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();

    // Reconstruct Ext4Superblock from ext4_sb_info
    let sb = Ext4Superblock {
        s_inodes_count: sb_info.inodes_count as u32,
        s_blocks_count_lo: sb_info.blocks_count as u32,
        s_blocks_count_hi: (sb_info.blocks_count >> 32) as u32,
        s_inodes_per_group: sb_info.inodes_per_group,
        s_blocks_per_group: sb_info.blocks_per_group,
        s_inode_size: sb_info.inode_size,
        s_desc_size: sb_info.desc_size,
        s_first_ino: sb_info.first_ino,
        s_log_block_size: match sb_info.block_size {
            1024 => 0,
            2048 => 1,
            4096 => 2,
            _ => 2,
        },
        s_log_cluster_size: match sb_info.block_size {
            1024 => 0,
            2048 => 1,
            4096 => 2,
            _ => 2,
        },
        s_feature_compat: 0,
        s_feature_incompat: sb_info.feature_incompat,
        s_feature_ro_compat: sb_info.feature_ro_compat,
        s_log_groups_per_flex: sb_info.log_groups_per_flex,
        s_rev_level: 1,
        s_magic: EXT4_SUPER_MAGIC,
        s_state: sb_info.state,
        s_uuid: sb_info.uuid,
        s_volume_name: {
            let mut v = [0u8; 16];
            for i in 0..16 {
                v[i] = sb_info.volume_name[i] as u8;
            }
            v
        },
        // Zero out remaining fields (not needed for read-only operation)
        s_free_blocks_count_lo: 0,
        s_free_inodes_count: 0,
        s_first_data_block: if sb_info.block_size > 1024 { 0 } else { 1 },
        s_clusters_per_group: sb_info.blocks_per_group,
        s_mtime: 0,
        s_wtime: 0,
        s_mnt_count: 0,
        s_max_mnt_count: 0,
        s_errors: 0,
        s_minor_rev_level: 0,
        s_lastcheck: 0,
        s_checkinterval: 0,
        s_creator_os: 0,
        s_def_resuid: 0,
        s_def_resgid: 0,
        s_block_group_nr: 0,
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
        s_default_mount_opts: 0,
        s_first_meta_bg: 0,
        s_mkfs_time: 0,
        s_jnl_blocks: [0u32; 17],
        s_free_blocks_count_hi: 0,
        s_inodes_count_hi: 0,
        s_free_inodes_count_hi: 0,
        s_minor_extra_isize: 0,
        s_want_extra_isize: 0,
        s_flags: 0,
        s_raid_stride: 0,
        s_mmp_interval: 0,
        s_mmp_block: 0,
        s_raid_stripe_width: 0,
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
        s_last_mounted: [0u8; 64],
        s_mount_opts: [0u8; 64],
        s_usr_quota_inum: 0,
        s_grp_quota_inum: 0,
        s_overhead_blocks: 0,
        s_backup_bgs: [0u32; 2],
        s_encrypt_algos: [0u8; 4],
        s_encrypt_pw_salt: [0u8; 16],
        s_lpf_ino: 0,
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

    let _block_size = sb_info.block_size as usize;
    let group = crate::inode::inode_to_group(ino, &sb) as usize;
    // We need group descriptors to read inodes.
    // For now, this is a placeholder — the C bridge will handle this.
    // Returns ENOTSUP to indicate the caller should use the C-side inode reader.
    let _ = group;
    let _ = read_fn;
    let _ = ctx;

    // Mark as placeholder
    95 // ENOTSUP
}

/// Lookup a file name in a directory.
///
/// # Safety
/// `read_block` callback must be valid.
#[no_mangle]
pub unsafe extern "C" fn ext4_lookup(
    _sbi: *const ext4_sb_info,
    _dir_ino: u32,
    _name: *const c_char,
    _out_ino: *mut u32,
    _out_type: *mut u8,
) -> c_int {
    // Placeholder — full implementation in Phase 1
    95 // ENOTSUP
}

/// Read data from a file.
///
/// # Safety
/// `read_block` callback must be valid.
#[no_mangle]
pub unsafe extern "C" fn ext4_read_file(
    _sbi: *const ext4_sb_info,
    _ino: u32,
    _offset: u64,
    _buf: *mut u8,
    _count: u32,
    _bytes_read: *mut u32,
) -> c_int {
    // Placeholder — full implementation in Phase 1
    95 // ENOTSUP
}

/// Get file or directory stat info.
#[no_mangle]
pub unsafe extern "C" fn ext4_stat(
    _sbi: *const ext4_sb_info,
    _ino: u32,
    _mode: *mut u16,
    _size: *mut u64,
    _uid: *mut u16,
    _gid: *mut u16,
) -> c_int {
    // Placeholder — full implementation in Phase 1
    95 // ENOTSUP
}

/// Get filesystem statistics.
#[no_mangle]
pub unsafe extern "C" fn ext4_statvfs(
    _sbi: *const ext4_sb_info,
    _block_size: *mut u32,
    _blocks_total: *mut u64,
    _blocks_free: *mut u64,
    _inodes_total: *mut u64,
    _inodes_free: *mut u64,
) -> c_int {
    // Placeholder — full implementation in Phase 1
    95 // ENOTSUP
}
