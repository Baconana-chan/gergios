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
        Ext4Error::InvalidXattr => 95,      // ENOTSUP
    }
}

// ─── Helper: reconstruct Ext4Superblock from ext4_sb_info ────────────

/// Reconstruct a full Ext4Superblock from the C-compatible ext4_sb_info.
/// Fills essential fields from sbi, zeros out the rest.
fn sb_from_sbi(sbi: &ext4_sb_info) -> Ext4Superblock {
    let block_size = sbi.block_size;
    Ext4Superblock {
        s_inodes_count: sbi.inodes_count as u32,
        s_blocks_count_lo: sbi.blocks_count as u32,
        s_blocks_count_hi: (sbi.blocks_count >> 32) as u32,
        s_inodes_per_group: sbi.inodes_per_group,
        s_blocks_per_group: sbi.blocks_per_group,
        s_inode_size: sbi.inode_size,
        s_desc_size: sbi.desc_size,
        s_first_ino: sbi.first_ino,
        s_log_block_size: match block_size {
            1024 => 0,
            2048 => 1,
            4096 => 2,
            _ => 2,
        },
        s_log_cluster_size: match block_size {
            1024 => 0,
            2048 => 1,
            4096 => 2,
            _ => 2,
        },
        s_feature_compat: 0,
        s_feature_incompat: sbi.feature_incompat,
        s_feature_ro_compat: sbi.feature_ro_compat,
        s_log_groups_per_flex: sbi.log_groups_per_flex,
        s_rev_level: 1,
        s_magic: EXT4_SUPER_MAGIC,
        s_state: sbi.state,
        s_uuid: sbi.uuid,
        s_volume_name: {
            let mut v = [0u8; 16];
            for i in 0..16 { v[i] = sbi.volume_name[i] as u8; }
            v
        },
        s_free_blocks_count_lo: 0,
        s_free_inodes_count: 0,
        s_first_data_block: if block_size > 1024 { 0 } else { 1 },
        s_clusters_per_group: sbi.blocks_per_group,
        s_mtime: 0, s_wtime: 0, s_mnt_count: 0, s_max_mnt_count: 0,
        s_errors: 0, s_minor_rev_level: 0, s_lastcheck: 0, s_checkinterval: 0,
        s_creator_os: 0, s_def_resuid: 0, s_def_resgid: 0,
        s_block_group_nr: 0, s_algorithm_usage_bitmap: 0,
        s_prealloc_blocks: 0, s_prealloc_dir_blocks: 0, s_reserved_gdt_blocks: 0,
        s_journal_uuid: [0u8; 16], s_journal_inum: 0, s_journal_dev: 0,
        s_last_orphan: 0, s_hash_seed: [0u32; 4], s_def_hash_version: 0,
        s_jnl_backup_type: 0, s_default_mount_opts: 0, s_first_meta_bg: 0,
        s_mkfs_time: 0, s_jnl_blocks: [0u32; 17],
        s_free_blocks_count_hi: 0, s_inodes_count_hi: 0, s_free_inodes_count_hi: 0,
        s_minor_extra_isize: 0, s_want_extra_isize: 0, s_flags: 0,
        s_raid_stride: 0, s_mmp_interval: 0, s_mmp_block: 0, s_raid_stripe_width: 0,
        s_checksum_type: 0, s_reserved_pad: 0, s_kbytes_written: 0,
        s_snapshot_inum: 0, s_snapshot_id: 0, s_snapshot_r_blocks: 0,
        s_snapshot_list: 0, s_error_count: 0, s_first_error_time: 0,
        s_first_error_ino: 0, s_first_error_block: 0,
        s_first_error_func: [0u8; 32], s_first_error_line: 0,
        s_last_error_time: 0, s_last_error_ino: 0, s_last_error_line: 0,
        s_last_error_block: 0, s_last_error_func: [0u8; 32],
        s_last_mounted: [0u8; 64], s_mount_opts: [0u8; 64],
        s_usr_quota_inum: 0, s_grp_quota_inum: 0, s_overhead_blocks: 0,
        s_backup_bgs: [0u32; 2], s_encrypt_algos: [0u8; 4],
        s_encrypt_pw_salt: [0u8; 16], s_lpf_ino: 0, s_prj_quota_inum: 0,
        s_checksum_seed: 0, s_wtime_hi: 0, s_mtime_hi: 0, s_mkfs_time_hi: 0,
        s_lastcheck_hi: 0, s_first_error_time_hi: 0, s_last_error_time_hi: 0,
        s_pad: [0u8; 2], s_checksum: 0,
    }
}

/// Helper: create a block reader closure from a C callback.
/// This converts the C ext4_read_block_cb into a Rust closure.
unsafe fn make_block_reader(
    ctx: *mut c_void,
    read_fn: ext4_read_block_cb,
    block_size: usize,
) -> impl FnMut(u64, &mut [u8]) -> Ext4Result<()> {
    move |block_nr: u64, buf: &mut [u8]| {
        let ret = unsafe {
            read_fn(ctx, block_nr, buf.as_mut_ptr(), block_size as u32)
        };
        if ret == 0 {
            Ok(())
        } else {
            Err(Ext4Error::IoError)
        }
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
    pub free_blocks_count: u64,
    pub free_inodes_count: u64,
    pub last_orphan: u32,
    pub csum_seed: u32,     /* CRC-32C seed for metadata_csum (0 if not enabled) */
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
    pub dtime: u32,
}

#[repr(C)]
pub struct ext4_dirent {
    pub ino: u32,
    pub file_type: u8,
    pub name_len: u8,
    pub name: [c_char; 255],
}

/// C-compatible structure with group descriptor info.
#[repr(C)]
pub struct ext4_gd_info {
    pub block_bitmap: u64,
    pub inode_bitmap: u64,
    pub inode_table: u64,
    pub free_blocks_count: u16,
    pub free_inodes_count: u16,
    pub used_dirs_count: u16,
}

// ─── Callback types for FFI ──────────────────────────────────────────

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

/// Type for the block write callback.
pub type ext4_write_block_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
    block_nr: u64,
    buf: *const u8,
    block_size: u32,
) -> c_int;

/// Type for the free blocks callback (called when truncating).
/// `block_nr` is the first physical block to free, `count` is the number.
pub type ext4_free_blocks_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
    block_nr: u64,
    count: u32,
) -> c_int;

/// Type for the free inode callback.
pub type ext4_free_inode_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
    ino: u32,
) -> c_int;

/// Type for the block allocation callback.
/// Returns the physical block number on success, or 0 on failure.
pub type ext4_alloc_block_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
) -> u64;

/// Type for the inode allocation callback.
/// Returns the inode number on success, or 0 on failure.
pub type ext4_alloc_inode_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
) -> u32;

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
                (*sbi).free_blocks_count =
                    (sb.s_free_blocks_count_lo as u64) | ((sb.s_free_blocks_count_hi as u64) << 32);
                (*sbi).free_inodes_count = sb.s_free_inodes_count as u64;
                (*sbi).last_orphan = sb.s_last_orphan;
                (*sbi).csum_seed = if sb.has_metadata_csum() {
                    crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid)
                } else {
                    0
                };
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

// ─── Metadata checksum (CRC-32C) update ────────────────────────────

/// Update the superblock checksum (s_checksum) in a raw SB buffer.
/// Zeros offset 672, computes CRC-32C over 1024 bytes, writes result.
///
/// # Safety
/// `sbi` and `sb_data` must point to valid, non-null buffers.
/// `sb_data` must be at least 1024 bytes.
#[no_mangle]
pub unsafe extern "C" fn ext4_update_sb_csum(
    sbi: *const ext4_sb_info,
    sb_data: *mut u8,
) -> c_int {
    if sbi.is_null() || sb_data.is_null() {
        return 22;
    }
    let sbi_ref = unsafe { &*sbi };
    if sbi_ref.csum_seed == 0 {
        return 0;
    }
    let data = unsafe { core::slice::from_raw_parts_mut(sb_data, 1024) };
    // Zero the checksum field at offset 672
    data[672..676].copy_from_slice(&[0u8; 4]);
    let computed = crate::journal::crc32c_le(sbi_ref.csum_seed, &data[..1024]);
    data[672..676].copy_from_slice(&computed.to_le_bytes());
    0
}

/// Update a group descriptor's checksum (bg_checksum) in a raw GD buffer.
/// Zeros offset 30, computes CRC-32C over desc_size bytes, writes result.
/// For 64-bit descriptors (desc_size > 32), also incorporates group number.
///
/// # Safety
/// `sbi` and `gd_data` must point to valid, non-null buffers.
/// `gd_data` must be at least `desc_size` bytes.
#[no_mangle]
pub unsafe extern "C" fn ext4_update_gd_csum(
    sbi: *const ext4_sb_info,
    group: u32,
    gd_data: *mut u8,
    desc_size: u16,
) -> c_int {
    if sbi.is_null() || gd_data.is_null() {
        return 22;
    }
    let sbi_ref = unsafe { &*sbi };
    if sbi_ref.csum_seed == 0 {
        return 0;
    }
    let ds = desc_size as usize;
    let data = unsafe { core::slice::from_raw_parts_mut(gd_data, ds) };
    // Zero the checksum field at offset 30
    data[30..32].copy_from_slice(&[0u8; 2]);
    let mut crc = crate::journal::crc32c_le(sbi_ref.csum_seed, &data[..ds]);
    // For 64-bit descriptors, incorporate the group number
    if desc_size > 32 {
        let bg_le = group.to_le_bytes();
        crc = crate::journal::crc32c_le(crc, &bg_le);
    }
    let csum = (crc as u16).to_le_bytes();
    data[30..32].copy_from_slice(&csum);
    0
}

// ─── Metadata checksum (CRC-32C) validation ─────────────────────────

/// Compute the CRC-32C seed from the superblock data.
/// seed = crc32c_le(0xFFFFFFFF, s_uuid)
/// This is called at mount time if METADATA_CSUM is enabled.
///
/// # Safety
/// `sbi` and `sb_data` must point to valid, non-null buffers.
/// `sb_data` must be at least 1024 bytes.
pub unsafe extern "C" fn ext4_compute_csum_seed(
    sbi: *mut ext4_sb_info,
    sb_data: *const u8,
) -> c_int {
    if sbi.is_null() || sb_data.is_null() {
        return 22; // EINVAL
    }
    let data = unsafe { core::slice::from_raw_parts(sb_data, 1024) };
    // UUID is at offset 104, 16 bytes
    let uuid: [u8; 16] = {
        let mut u = [0u8; 16];
        u.copy_from_slice(&data[104..120]);
        u
    };
    let seed = crate::journal::crc32c_le(0xFFFFFFFF, &uuid);
    unsafe { (*sbi).csum_seed = seed; }
    0
}

/// Verify the superblock checksum (s_checksum at offset 672).
/// Returns 0 if valid or no checksum present, or EBADMSG (74) on mismatch.
///
/// # Safety
/// `sbi` and `sb_data` must point to valid, non-null buffers.
/// `sb_data` must be at least 1024 bytes.
#[no_mangle]
pub unsafe extern "C" fn ext4_verify_sb_csum(
    sbi: *const ext4_sb_info,
    sb_data: *const u8,
) -> c_int {
    if sbi.is_null() || sb_data.is_null() {
        return 22;
    }
    let sbi_ref = unsafe { &*sbi };
    if sbi_ref.csum_seed == 0 {
        return 0; // No checksum feature — always valid
    }
    let data = unsafe { core::slice::from_raw_parts(sb_data, 1024) };
    // s_checksum at offset 672 (4 bytes LE)
    let stored = u32::from_le_bytes([data[672], data[673], data[674], data[675]]);
    if stored == 0 {
        return 0; // No checksum written — skip
    }
    // Zero the checksum field for computation
    let mut zeroed = data.to_vec();
    zeroed[672..676].copy_from_slice(&[0u8; 4]);
    let computed = crate::journal::crc32c_le(sbi_ref.csum_seed, &zeroed);
    if computed == stored { 0 } else { 74 } // EBADMSG
}

/// Verify a group descriptor checksum.
/// `gd_data` must be at least `desc_size` bytes.
/// Returns 0 if valid or no checksum present, or EBADMSG (74) on mismatch.
///
/// # Safety
/// `sbi` and `gd_data` must point to valid, non-null buffers.
#[no_mangle]
pub unsafe extern "C" fn ext4_verify_gd_csum(
    sbi: *const ext4_sb_info,
    group: u32,
    gd_data: *const u8,
    desc_size: u16,
) -> c_int {
    if sbi.is_null() || gd_data.is_null() {
        return 22;
    }
    let sbi_ref = unsafe { &*sbi };
    if sbi_ref.csum_seed == 0 {
        return 0;
    }
    let ds = desc_size as usize;
    let data = unsafe { core::slice::from_raw_parts(gd_data, ds) };
    // bg_checksum at offset 30 (2 bytes LE)
    let stored = u16::from_le_bytes([data[30], data[31]]);
    if stored == 0 {
        return 0; // No checksum written
    }
    // Zero the checksum field
    let mut zeroed = data.to_vec();
    zeroed[30..32].copy_from_slice(&[0u8; 2]);
    let mut crc = crate::journal::crc32c_le(sbi_ref.csum_seed, &zeroed[..ds]);
    // For 64-bit descriptors (desc_size > 32), incorporate the group number
    if desc_size > 32 {
        let bg_le = group.to_le_bytes();
        crc = crate::journal::crc32c_le(crc, &bg_le);
    }
    let computed_low = crc as u16;
    if computed_low == stored { 0 } else { 74 }
}

/// Verify an inode checksum.
/// `inode_data` must be at least `inode_size` bytes.
/// Returns 0 if valid or no checksum present, or EBADMSG (74) on mismatch.
///
/// # Safety
/// `sbi` and `inode_data` must point to valid, non-null buffers.
#[no_mangle]
pub unsafe extern "C" fn ext4_verify_inode_csum(
    sbi: *const ext4_sb_info,
    ino: u32,
    inode_data: *const u8,
    inode_size: u16,
) -> c_int {
    if sbi.is_null() || inode_data.is_null() {
        return 22;
    }
    let sbi_ref = unsafe { &*sbi };
    if sbi_ref.csum_seed == 0 {
        return 0;
    }
    let isize = inode_size as usize;
    if isize < 132 {
        return 0; // No checksum field in small inodes
    }
    let data = unsafe { core::slice::from_raw_parts(inode_data, isize) };
    // i_checksum_hi at offset 130 (2 bytes LE)
    let stored = u16::from_le_bytes([data[130], data[131]]);
    if stored == 0 {
        return 0; // No checksum written
    }
    // Zero the checksum field
    let mut zeroed = data.to_vec();
    zeroed[130..132].copy_from_slice(&[0u8; 2]);
    // Seed with csum_seed, incorporate inode number (4 bytes LE)
    let ino_le = ino.to_le_bytes();
    let mut crc = crate::journal::crc32c_le(sbi_ref.csum_seed, &ino_le);
    // Incorporate i_generation at offset 100 (4 bytes LE)
    if isize >= 104 {
        let gen = [data[100], data[101], data[102], data[103]];
        crc = crate::journal::crc32c_le(crc, &gen);
    }
    // Incorporate the full inode data (with checksum zeroed)
    crc = crate::journal::crc32c_le(crc, &zeroed[..isize]);
    // Compare lower 16 bits against stored i_checksum_hi
    let computed_low = crc as u16;
    if computed_low == stored { 0 } else { 74 }
}

/// Result of batch checksum verification (returned by ext4_verify_all_csums).
#[repr(C)]
pub struct ext4_csum_result {
    pub sb_valid: u8,
    pub gd_valid: u8,
    pub gd_failed: u32,    /* First group with failed checksum, or 0xFFFFFFFF if all okay */
    pub root_inode_valid: u8,
}

/// Batch-verify all metadata checksums at mount time.
///
/// Validates superblock checksum, all group descriptor checksums,
/// and the root inode (inode 2) checksum.
///
/// The caller provides the superblock data and a callback to read blocks.
/// The function reads GDT blocks and the root inode table block.
///
/// # Safety
/// All pointers must be valid and non-null.
#[no_mangle]
pub unsafe extern "C" fn ext4_verify_all_csums(
    sbi: *const ext4_sb_info,
    sb_data: *const u8,
    result: *mut ext4_csum_result,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || sb_data.is_null() || result.is_null() || read_block.is_none() {
        return 22;
    }
    let sbi_ref = unsafe { &*sbi };
    if sbi_ref.csum_seed == 0 {
        // No metadata_csum feature — everything is "valid" by default
        unsafe {
            (*result).sb_valid = 1;
            (*result).gd_valid = 1;
            (*result).gd_failed = 0xFFFFFFFF;
            (*result).root_inode_valid = 1;
        }
        return 0;
    }
    let read_fn = read_block.unwrap();
    let block_size = sbi_ref.block_size as usize;
    let desc_size = sbi_ref.desc_size;
    let groups_count = sbi_ref.block_groups_count;

    // 1. Verify superblock
    let sb_ok = ext4_verify_sb_csum(sbi, sb_data) == 0;

    // 2. Verify all group descriptors
    let mut gd_ok = true;
    let mut first_bad_group = 0xFFFFFFFF;

    // Compute GDT start block (used for both GDT verification and root inode read)
    let first_data_block = if block_size > 1024 { 0u64 } else { 1u64 };
    let gdt_start_block = first_data_block + 1;

    if groups_count > 0 && desc_size >= 32 {
        let descs_per_block = block_size / desc_size as usize;
        let gdt_blocks = ((groups_count as usize * desc_size as usize) + block_size - 1) / block_size;

        for gb in 0..gdt_blocks {
            let block_nr = gdt_start_block + gb as u64;
            let mut buf = vec![0u8; block_size];
            let ret = unsafe { read_fn(ctx, block_nr, buf.as_mut_ptr(), block_size as u32) };
            if ret != 0 {
                gd_ok = false;
                first_bad_group = gb as u32 * descs_per_block as u32;
                break;
            }

            for d in 0..descs_per_block {
                let group = gb as u32 * descs_per_block as u32 + d as u32;
                if group >= groups_count {
                    break;
                }
                let off = d * desc_size as usize;
                if off + desc_size as usize > buf.len() {
                    gd_ok = false;
                    first_bad_group = group;
                    break;
                }
                let gd_ret = ext4_verify_gd_csum(sbi, group, buf.as_ptr().add(off), desc_size);
                if gd_ret != 0 {
                    gd_ok = false;
                    if first_bad_group == 0xFFFFFFFF {
                        first_bad_group = group;
                    }
                }
            }
        }
    }

    // 3. Verify root inode (inode 2)
    let root_ino = 2u32;
    let mut root_inode_ok = true;
    if groups_count > 0 {
        let inodes_per_group = sbi_ref.inodes_per_group;
        let inode_size = sbi_ref.inode_size as usize;
        let group = (root_ino - 1) / inodes_per_group;
        let index = (root_ino - 1) % inodes_per_group;
        let inodes_per_block = block_size / inode_size;

        // Read the group descriptor for the root inode's group
        let gdt_descs_per_block = block_size / desc_size as usize;
        let gdt_block = gdt_start_block + group as u64 / gdt_descs_per_block as u64;
        let gdt_off = (group as usize % gdt_descs_per_block) * desc_size as usize;
        let mut gdt_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, gdt_block, gdt_buf.as_mut_ptr(), block_size as u32) };
        if ret == 0 && gdt_off + 16 <= gdt_buf.len() {
            // We need the inode_table block number from the GD
            let inode_table = u32::from_le_bytes([
                gdt_buf[gdt_off + 8], gdt_buf[gdt_off + 9],
                gdt_buf[gdt_off + 10], gdt_buf[gdt_off + 11],
            ]) as u64;
            // For 64-bit descriptors, incorporate inode_table_hi at GD offset 40 (4 bytes LE)
            let mut inode_table_hi: u64 = 0;
            if sbi_ref.has_64bit != 0 && gdt_off + 40 + 4 <= gdt_buf.len() {
                inode_table_hi = u32::from_le_bytes([
                    gdt_buf[gdt_off + 40], gdt_buf[gdt_off + 41],
                    gdt_buf[gdt_off + 42], gdt_buf[gdt_off + 43],
                ]) as u64;
            }
            let inode_table_full = inode_table | (inode_table_hi << 32);
            let inode_block = inode_table_full + (index as u64 / inodes_per_block as u64);

            let mut inode_buf = vec![0u8; block_size];
            let ret2 = unsafe { read_fn(ctx, inode_block, inode_buf.as_mut_ptr(), block_size as u32) };
            if ret2 == 0 {
                let inode_off = (index as usize % inodes_per_block) * inode_size;
                let inode_ret = ext4_verify_inode_csum(
                    sbi, root_ino,
                    inode_buf.as_ptr().add(inode_off),
                    sbi_ref.inode_size,
                );
                root_inode_ok = inode_ret == 0;
            }
        }
    }

    unsafe {
        (*result).sb_valid = if sb_ok { 1 } else { 0 };
        (*result).gd_valid = if gd_ok { 1 } else { 0 };
        (*result).gd_failed = first_bad_group;
        (*result).root_inode_valid = if root_inode_ok { 1 } else { 0 };
    }
    0
}

/// Read an inode from the filesystem and fill ext4_inode_info.
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
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    // Create a block reader closure
    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the group descriptor for this inode's group
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };

    // Read the inode table block and parse the inode
    let groups = [gd];
    let inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Fill ext4_inode_info
    unsafe {
        (*info).ino = ino;
        (*info).mode = inode.i_mode;
        (*info).size = inode.file_size();
        (*info).uid = inode.i_uid;
        (*info).gid = inode.i_gid;
        (*info).is_dir = if inode.is_dir() { 1 } else { 0 };
        (*info).is_reg = if inode.is_reg() { 1 } else { 0 };
        (*info).is_lnk = if inode.is_lnk() { 1 } else { 0 };
        (*info).has_extents = if inode.has_extents() { 1 } else { 0 };
        (*info).links_count = inode.i_links_count;
        (*info).blocks = inode.blocks_count();
        (*info).atime = inode.i_atime;
        (*info).ctime = inode.i_ctime;
        (*info).mtime = inode.i_mtime;
        (*info).dtime = inode.i_dtime;
    }

    0
}

/// Lookup a file name in a directory.
///
/// # Safety
/// `read_block` callback must be valid.
#[no_mangle]
pub unsafe extern "C" fn ext4_lookup(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    out_ino: *mut u32,
    out_type: *mut u8,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null() || out_ino.is_null() || read_block.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    // Read the name from C string
    let name_str = unsafe { core::ffi::CStr::from_ptr(name) };
    let name_str = match name_str.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    // Create a block reader closure
    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the directory inode
    let group = crate::inode::inode_to_group(dir_ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd];
    let dir_inode = match crate::block::read_inode(&sb, &groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Use lookup_in_dir which handles both linear and htree directories
    match crate::dir::lookup_in_dir(&sb, &dir_inode, dir_ino, name_str, |block: u64, buf: &mut [u8]| {
        // Find the extent for this logical block
        match crate::extent::extent_lookup(&sb, &dir_inode, block, &mut reader) {
            Ok(Some(phys)) => reader(phys, buf),
            Ok(None) => {
                // Sparse block: zero it
                for byte in buf.iter_mut() { *byte = 0; }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }) {
        Ok(Some(entry)) => {
            unsafe {
                *out_ino = entry.inode;
                *out_type = entry.file_type;
            }
            0
        }
        Ok(None) => 2, // ENOENT
        Err(e) => errno_from_error(e),
    }
}

/// Read data from a file at the given offset.
///
/// # Safety
/// `read_block` callback must be valid.
#[no_mangle]
pub unsafe extern "C" fn ext4_read_file(
    sbi: *const ext4_sb_info,
    ino: u32,
    offset: u64,
    buf: *mut u8,
    count: u32,
    bytes_read: *mut u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || buf.is_null() || read_block.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    // Create a block reader closure
    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd];
    let inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Read file data via extent tree
    let buf_slice = unsafe { slice::from_raw_parts_mut(buf, count as usize) };
    match crate::extent::extent_read(&sb, &inode, offset, buf_slice, |block, block_buf| {
        reader(block, block_buf)
    }) {
        Ok(n) => {
            if !bytes_read.is_null() {
                unsafe { *bytes_read = n as u32; }
            }
            0
        }
        Err(e) => errno_from_error(e),
    }
}

/// Get file or directory stat info.
///
/// # Safety
/// `read_block` callback must be valid (needed to read the inode).
#[no_mangle]
pub unsafe extern "C" fn ext4_stat(
    sbi: *const ext4_sb_info,
    ino: u32,
    mode: *mut u16,
    size: *mut u64,
    uid: *mut u16,
    gid: *mut u16,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || read_block.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd];
    let inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    if !mode.is_null() {
        unsafe { *mode = inode.i_mode; }
    }
    if !size.is_null() {
        unsafe { *size = inode.file_size(); }
    }
    if !uid.is_null() {
        unsafe { *uid = inode.i_uid; }
    }
    if !gid.is_null() {
        unsafe { *gid = inode.i_gid; }
    }

    0
}

/// Read a single group descriptor from disk.
///
/// # Safety
/// `read_block` callback must be valid.
#[no_mangle]
pub unsafe extern "C" fn ext4_read_group_descriptor(
    sbi: *const ext4_sb_info,
    group: u32,
    gd_info: *mut ext4_gd_info,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || gd_info.is_null() || read_block.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => {
            unsafe {
                (*gd_info).block_bitmap = gd.block_bitmap(&sb);
                (*gd_info).inode_bitmap = gd.inode_bitmap(&sb);
                (*gd_info).inode_table = gd.inode_table(&sb);
                (*gd_info).free_blocks_count = gd.free_blocks_count(&sb);
                (*gd_info).free_inodes_count = gd.free_inodes_count(&sb);
                (*gd_info).used_dirs_count = gd.bg_used_dirs_count_lo; // TODO: hi
            }
            0
        }
        Err(e) => errno_from_error(e),
    }
}

/// Truncate a file to a new (smaller) size.
///
/// Frees all data blocks beyond `new_size` via `free_blocks` callback,
/// updates the inode extent tree, and writes the inode back to disk.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_truncate(
    sbi: *const ext4_sb_info,
    ino: u32,
    new_size: u64,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    free_blocks: Option<ext4_free_blocks_cb>,
) -> c_int {
    if sbi.is_null() || read_block.is_none() || write_block.is_none() || free_blocks.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let free_fn = free_blocks.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let mut inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute inode table block info for writing the inode back
    let index = crate::inode::inode_to_group_index(ino, &sb);
    let inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_offset = (index as u64 % inodes_per_block) * inode_size as u64;
    let inode_block_nr = inode_table_block + block_offset;

    // free_blocks closure
    let mut free_cb = |phys: u64, count: u32| -> Ext4Result<()> {
        let ret = unsafe { free_fn(ctx, phys, count) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // write_inode closure: serialize the inode into its inode table block
    let write_inode_sb = sb_from_sbi(sb_info); // Clone for closure
    let mut write_inode = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }

        crate::inode::serialize_inode(
            &mut block_buf[in_block_offset as usize..],
            updated_inode,
            &write_inode_sb,
            Some(ino),
        );

        let ret = unsafe { write_fn(ctx, inode_block_nr, block_buf.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    match crate::extent::extent_truncate(&sb, &mut inode, new_size, &mut free_cb, &mut write_inode) {
        Ok(()) => 0,
        Err(e) => errno_from_error(e),
    }
}

/// Create a hard link (add a directory entry pointing to an existing inode).
///
/// Increments the target inode's link count and inserts a new directory entry.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_link(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    target_ino: u32,
    target_mode: u16,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null()
        || read_block.is_none() || write_block.is_none() || alloc_block.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let alloc_fn = alloc_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let name_str = match unsafe { core::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    // Determine file_type from target mode
    let file_type = if target_mode & EXT4_S_IFDIR != 0 { EXT4_FT_DIR }
        else if target_mode & EXT4_S_IFREG != 0 { EXT4_FT_REG_FILE }
        else if target_mode & EXT4_S_IFLNK != 0 { EXT4_FT_SYMLINK }
        else if target_mode & EXT4_S_IFCHR != 0 { EXT4_FT_CHRDEV }
        else if target_mode & EXT4_S_IFBLK != 0 { EXT4_FT_BLKDEV }
        else if target_mode & EXT4_S_IFIFO != 0 { EXT4_FT_FIFO }
        else if target_mode & EXT4_S_IFSOCK != 0 { EXT4_FT_SOCK }
        else { EXT4_FT_UNKNOWN };

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the directory inode
    let group = crate::inode::inode_to_group(dir_ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let mut dir_inode = match crate::block::read_inode(&sb, &groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute inode table block info for writing dir inode back
    let index = crate::inode::inode_to_group_index(dir_ino, &sb);
    let inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_offset = (index as u64 % inodes_per_block) * inode_size as u64;
    let dir_inode_block_nr = inode_table_block + block_offset;

    // write_block closure (writes a data block)
    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // write_inode closure for insert_in_dir
    let write_inode_sb = sb_from_sbi(sb_info);
    let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, dir_inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }

        crate::inode::serialize_inode(
            &mut block_buf[in_block_offset as usize..],
            updated_inode,
            &write_inode_sb,
            Some(dir_ino),
        );

        let ret = unsafe { write_fn(ctx, dir_inode_block_nr, block_buf.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // alloc_block closure
    let mut ab = move || -> Ext4Result<u64> {
        let phys = unsafe { alloc_fn(ctx) };
        if phys == 0 { Err(Ext4Error::NoSpace) } else { Ok(phys) }
    };

    // Read the target inode to increment link count
    let target_group = crate::inode::inode_to_group(target_ino, &sb);
    let target_gd = match crate::group_desc::read_group_descriptor(&sb, target_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let target_groups = [target_gd.clone()];
    let mut target_inode = match crate::block::read_inode(&sb, &target_groups, target_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Increment target inode link count
    crate::inode::inode_link(&mut target_inode);

    // Write target inode back
    let target_index = crate::inode::inode_to_group_index(target_ino, &sb);
    let target_inode_table_block = target_gd.inode_table(&sb);
    let target_block_offset = target_index as u64 / inodes_per_block;
    let target_in_block_offset = (target_index as u64 % inodes_per_block) * inode_size as u64;
    let target_inode_block_nr = target_inode_table_block + target_block_offset;

    {
        let mut target_block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, target_inode_block_nr, target_block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; } // EIO

        crate::inode::serialize_inode(
            &mut target_block_buf[target_in_block_offset as usize..],
            &target_inode,
            &sb,
            Some(target_ino),
        );

        let ret = unsafe { write_fn(ctx, target_inode_block_nr, target_block_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }
    }

    // Insert the directory entry (handles expanding dir + writing dir inode)
    match crate::dir::insert_in_dir(
        &sb, &mut dir_inode, dir_ino, name_str, file_type, target_ino,
        &mut reader, &mut wb, &mut ab, &mut wi,
    ) {
        Ok(()) => 0,
        Err(e) => errno_from_error(e),
    }
}

/// Remove a hard link (remove a directory entry, decrement the target's link count).
///
/// If the link count reaches 0, the inode's data blocks are freed and the inode
/// is marked as deleted in the bitmap.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_unlink(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    free_blocks: Option<ext4_free_blocks_cb>,
    free_inode: Option<ext4_free_inode_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null()
        || read_block.is_none() || write_block.is_none()
        || free_blocks.is_none() || free_inode.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let free_fn = free_blocks.unwrap();
    let free_inode_fn = free_inode.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let name_str = match unsafe { core::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the directory inode
    let group = crate::inode::inode_to_group(dir_ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let dir_inode = match crate::block::read_inode(&sb, &groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // First, look up the name in the directory to find the target inode
    let target_ino = match crate::dir::lookup_in_dir(
        &sb, &dir_inode, dir_ino, name_str,
        |block: u64, buf: &mut [u8]| {
            match crate::extent::extent_lookup(&sb, &dir_inode, block, &mut reader) {
                Ok(Some(phys)) => reader(phys, buf),
                Ok(None) => {
                    for byte in buf.iter_mut() { *byte = 0; }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        },
    ) {
        Ok(Some(entry)) => entry.inode,
        Ok(None) => return 2, // ENOENT
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute dir inode table block info
    let dir_index = crate::inode::inode_to_group_index(dir_ino, &sb);
    let dir_inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let dir_block_offset = dir_index as u64 / inodes_per_block;
    let dir_in_block_offset = (dir_index as u64 % inodes_per_block) * inode_size as u64;
    let dir_inode_block_nr = dir_inode_table_block + dir_block_offset;

    // write_block closure
    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // Remove the directory entry
    match crate::dir::remove_in_dir(&sb, &dir_inode, dir_ino, name_str, &mut reader, &mut wb) {
        Ok(true) => {},
        Ok(false) => return 2, // ENOENT (shouldn't happen since we looked up first)
        Err(e) => return errno_from_error(e),
    }

    // Read the target inode to decrement link count
    let target_group = crate::inode::inode_to_group(target_ino, &sb);
    let target_gd = match crate::group_desc::read_group_descriptor(&sb, target_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let target_groups = [target_gd.clone()];
    let mut target_inode = match crate::block::read_inode(&sb, &target_groups, target_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute target inode table block info
    let target_index = crate::inode::inode_to_group_index(target_ino, &sb);
    let target_inode_table_block = target_gd.inode_table(&sb);
    let target_block_offset = target_index as u64 / inodes_per_block;
    let target_in_block_offset = (target_index as u64 % inodes_per_block) * inode_size as u64;
    let target_inode_block_nr = target_inode_table_block + target_block_offset;

    // Decrement link count; if 0, free inode data and mark deleted
    let should_free = crate::inode::inode_unlink(&mut target_inode);

    if should_free {
        // Free all data blocks + clear inode bitmap
        let mut free_cb = |phys: u64, count: u32| -> Ext4Result<()> {
            let ret = unsafe { free_fn(ctx, phys, count) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        let mut free_ino_cb = |ino: u32| -> Ext4Result<()> {
            let ret = unsafe { free_inode_fn(ctx, ino) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        match crate::inode::free_inode_data(
            &sb, &mut target_inode, target_ino,
            &mut free_cb, &mut free_ino_cb,
        ) {
            Ok(()) => {},
            Err(e) => return errno_from_error(e),
        }

        // Write the zeroed (deleted) inode back to the inode table
        let mut target_block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, target_inode_block_nr, target_block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        crate::inode::serialize_inode(
            &mut target_block_buf[target_in_block_offset as usize..],
            &target_inode,
            &sb,
            None,
        );

        let ret = unsafe { write_fn(ctx, target_inode_block_nr, target_block_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }
    } else {
        // Write the target inode back with decremented link count
        let mut target_block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, target_inode_block_nr, target_block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        crate::inode::serialize_inode(
            &mut target_block_buf[target_in_block_offset as usize..],
            &target_inode,
            &sb,
            Some(target_ino),
        );

        let ret = unsafe { write_fn(ctx, target_inode_block_nr, target_block_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }
    }

    0
}

/// Create a new file (regular or directory) and add a directory entry.
///
/// Allocates a new inode, initializes it, and inserts a directory entry
/// in the parent directory. For directories, also initializes the data block
/// with `.` and `..` entries.
///
/// Returns the new inode number in `out_ino`.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
fn ext4_create_or_mkdir(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    mode: u16,
    uid: u16,
    gid: u16,
    out_ino: *mut u32,
    is_dir: bool,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
    alloc_inode: Option<ext4_alloc_inode_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null() || out_ino.is_null()
        || read_block.is_none() || write_block.is_none()
        || alloc_block.is_none() || alloc_inode.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let alloc_fn = alloc_block.unwrap();
    let alloc_ino_fn = alloc_inode.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let name_str = match unsafe { core::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the parent directory inode
    let parent_group = crate::inode::inode_to_group(dir_ino, &sb);
    let parent_gd = match crate::group_desc::read_group_descriptor(&sb, parent_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let parent_groups = [parent_gd.clone()];
    let mut parent_inode = match crate::block::read_inode(&sb, &parent_groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute parent dir inode table block info
    let parent_index = crate::inode::inode_to_group_index(dir_ino, &sb);
    let parent_inode_table_block = parent_gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let parent_block_offset = parent_index as u64 / inodes_per_block;
    let parent_in_block_offset = (parent_index as u64 % inodes_per_block) * inode_size as u64;
    let parent_inode_block_nr = parent_inode_table_block + parent_block_offset;

    // Allocate a new inode
    let new_ino = unsafe { alloc_ino_fn(ctx) };
    if new_ino == 0 {
        return 28; // ENOSPC
    }

    // Add type bits: S_IFREG for files if no type bit is set
    let actual_mode = if !is_dir && (mode & EXT4_S_IFMT) == 0 {
        mode | EXT4_S_IFREG
    } else {
        mode
    };

    // Create the new inode structure
    let mut new_inode = crate::inode::new_inode(actual_mode, uid, gid);
    crate::inode::init_extent_tree(&mut new_inode);
    if is_dir {
        new_inode.i_links_count = 2; // ext4 dirs have links_count >= 2 (., ..)
    }
    crate::inode::update_timestamps(&mut new_inode, 0, 0, 0);

    // Compute new inode table block info
    let new_group = crate::inode::inode_to_group(new_ino, &sb);
    let new_gd = match crate::group_desc::read_group_descriptor(&sb, new_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let new_index = crate::inode::inode_to_group_index(new_ino, &sb);
    let new_inode_table_block = new_gd.inode_table(&sb);
    let new_block_offset = new_index as u64 / inodes_per_block;
    let new_in_block_offset = (new_index as u64 % inodes_per_block) * inode_size as u64;
    let new_inode_block_nr = new_inode_table_block + new_block_offset;

    // For mkdir: allocate data block and initialize directory
    if is_dir {
        let data_block = unsafe { alloc_fn(ctx) };
        if data_block == 0 {
            return 28; // ENOSPC
        }

        // Initialize directory block with . and ..
        let mut dir_block = vec![0u8; block_size];
                let csum_seed = if sb.has_metadata_csum() {
            crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid)
        } else { 0 };
        crate::dir::init_dir_block(&mut dir_block, new_ino, dir_ino, csum_seed);

        // Write the directory block
        let ret = unsafe { write_fn(ctx, data_block, dir_block.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; } // EIO

        // Insert extent for logical block 0 -> data_block
        let sectors_per_block = block_size as u64 / 512;

        // write_inode closure for extent_insert (writes new inode to its table)
        let write_inode_sb = sb_from_sbi(sb_info);
        {
            // Re-read the inode table block, serialize the new inode, write it back
            let mut inode_block_buf = vec![0u8; block_size];
            let ret = unsafe { read_fn(ctx, new_inode_block_nr, inode_block_buf.as_mut_ptr(), block_size as u32) };
            if ret != 0 { return 5; }

            crate::inode::serialize_inode(
                &mut inode_block_buf[new_in_block_offset as usize..],
                &new_inode,
                &sb,
                Some(new_ino),
            );

            let ret = unsafe { write_fn(ctx, new_inode_block_nr, inode_block_buf.as_ptr(), block_size as u32) };
            if ret != 0 { return 5; }
        }

        // Now read the inode back as mutable, insert extent, and write it back
        let mut inode_block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, new_inode_block_nr, inode_block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        let mut inode_for_extent = match crate::inode::parse_inode(
            &inode_block_buf[new_in_block_offset as usize..], &sb
        ) {
            Ok(inode) => inode,
            Err(e) => return errno_from_error(e),
        };

        // Set size to one block before extent_insert (extent_insert calls write_inode internally)
        crate::inode::set_file_size(&mut inode_for_extent, block_size as u64);
        crate::inode::set_blocks_count(&mut inode_for_extent, sectors_per_block);
        crate::inode::update_timestamps(&mut inode_for_extent, 0, 0, 0);

        let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
            let mut buf2 = vec![0u8; block_size];
            let ret = unsafe { read_fn(ctx, new_inode_block_nr, buf2.as_mut_ptr(), block_size as u32) };
            if ret != 0 { return Err(Ext4Error::IoError); }

            crate::inode::serialize_inode(
                &mut buf2[new_in_block_offset as usize..],
                updated_inode,
                &write_inode_sb,
                Some(new_ino),
            );

            let ret = unsafe { write_fn(ctx, new_inode_block_nr, buf2.as_ptr(), block_size as u32) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        if let Err(e) = crate::extent::extent_insert(
            &sb, &mut inode_for_extent, 0, data_block, 1, &mut wi,
        ) {
            return errno_from_error(e);
        }
    } else {
        // For regular files: just write the empty inode (extent tree initialized)
        let mut inode_block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, new_inode_block_nr, inode_block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        crate::inode::serialize_inode(
            &mut inode_block_buf[new_in_block_offset as usize..],
            &new_inode,
            &sb,
            Some(new_ino),
        );

        let ret = unsafe { write_fn(ctx, new_inode_block_nr, inode_block_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }
    }

    // Set the output inode number
    unsafe { *out_ino = new_ino; }

    // Create closures for insert_in_dir
    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    let mut ab = move || -> Ext4Result<u64> {
        let phys = unsafe { alloc_fn(ctx) };
        if phys == 0 { Err(Ext4Error::NoSpace) } else { Ok(phys) }
    };

    let write_inode_sb = sb_from_sbi(sb_info);
    let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, parent_inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }

        crate::inode::serialize_inode(
            &mut block_buf[parent_in_block_offset as usize..],
            updated_inode,
            &write_inode_sb,
            Some(dir_ino),
        );

        let ret = unsafe { write_fn(ctx, parent_inode_block_nr, block_buf.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // For directories, increment parent's link count (the `..` entry in the
    // new directory counts as a link to the parent — symmetric with ext4_rmdir)
    if is_dir {
        crate::inode::inode_link(&mut parent_inode);
    }

    // Determine file_type from actual_mode (includes S_IFREG for files)
    let file_type = if actual_mode & EXT4_S_IFDIR != 0 { EXT4_FT_DIR }
        else if actual_mode & EXT4_S_IFREG != 0 { EXT4_FT_REG_FILE }
        else if actual_mode & EXT4_S_IFLNK != 0 { EXT4_FT_SYMLINK }
        else if actual_mode & EXT4_S_IFCHR != 0 { EXT4_FT_CHRDEV }
        else if actual_mode & EXT4_S_IFBLK != 0 { EXT4_FT_BLKDEV }
        else if actual_mode & EXT4_S_IFIFO != 0 { EXT4_FT_FIFO }
        else if actual_mode & EXT4_S_IFSOCK != 0 { EXT4_FT_SOCK }
        else { EXT4_FT_UNKNOWN };

    // Insert the directory entry in the parent
    match crate::dir::insert_in_dir(
        &sb, &mut parent_inode, dir_ino, name_str, file_type, new_ino,
        &mut reader, &mut wb, &mut ab, &mut wi,
    ) {
        Ok(()) => 0,
        Err(e) => errno_from_error(e),
    }
}

/// Create a regular file.
#[no_mangle]
pub unsafe extern "C" fn ext4_create(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    mode: u16,
    uid: u16,
    gid: u16,
    out_ino: *mut u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
    alloc_inode: Option<ext4_alloc_inode_cb>,
) -> c_int {
    ext4_create_or_mkdir(
        sbi, dir_ino, name, mode, uid, gid, out_ino, false,
        ctx, read_block, write_block, alloc_block, alloc_inode,
    )
}

/// Create a directory.
#[no_mangle]
pub unsafe extern "C" fn ext4_mkdir(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    mode: u16,
    uid: u16,
    gid: u16,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
    alloc_inode: Option<ext4_alloc_inode_cb>,
) -> c_int {
    let mut out_ino: u32 = 0;
    ext4_create_or_mkdir(
        sbi, dir_ino, name, mode | EXT4_S_IFDIR, uid, gid,
        &mut out_ino, true,
        ctx, read_block, write_block, alloc_block, alloc_inode,
    )
}

/// Check if a directory is empty (only `.` and `..` entries).
fn is_dir_empty<FR>(
    inode: &Ext4Inode,
    sb: &Ext4Superblock,
    mut read_block: FR,
) -> Ext4Result<bool>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let block_size = sb.block_size();
    let num_blocks = (inode.file_size() + block_size as u64 - 1) / block_size as u64;
    let mut block = vec![0u8; block_size];

    for b in 0..num_blocks {
        read_block(b, &mut block)?;
        for entry in crate::dir::DirEntryIter::new(&block) {
            if entry.inode == 0 {
                continue;
            }
            let name = core::str::from_utf8(&entry.name[..entry.name_len as usize])
                .unwrap_or("");
            if name != "." && name != ".." {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Remove an empty directory.
///
/// Verifies the directory is empty (only `.` and `..` entries), removes the
/// directory entry from the parent, decrements the parent's link count, and
/// frees the directory's data blocks and inode.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_rmdir(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    free_blocks: Option<ext4_free_blocks_cb>,
    free_inode: Option<ext4_free_inode_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null()
        || read_block.is_none() || write_block.is_none()
        || free_blocks.is_none() || free_inode.is_none()
    {
        return 22;
    }

    // Disallow removing "." and ".."
    let name_str = match unsafe { core::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };
    if name_str == "." || name_str == ".." {
        return 22; // EINVAL
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let free_fn = free_blocks.unwrap();
    let free_ino_fn = free_inode.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // ── Step 1: Read parent dir inode ───────────────────────────────
    let dir_group = crate::inode::inode_to_group(dir_ino, &sb);
    let dir_gd = match crate::group_desc::read_group_descriptor(&sb, dir_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let dir_groups = [dir_gd.clone()];
    let dir_inode = match crate::block::read_inode(&sb, &dir_groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // ── Step 2: Look up name in parent dir ──────────────────────────
    let (target_ino, file_type) = match crate::dir::lookup_in_dir(
        &sb, &dir_inode, dir_ino, name_str,
        |block: u64, buf: &mut [u8]| {
            match crate::extent::extent_lookup(&sb, &dir_inode, block, &mut reader) {
                Ok(Some(phys)) => reader(phys, buf),
                Ok(None) => {
                    for byte in buf.iter_mut() { *byte = 0; }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        },
    ) {
        Ok(Some(entry)) => (entry.inode, entry.file_type),
        Ok(None) => return 2, // ENOENT
        Err(e) => return errno_from_error(e),
    };

    // ── Step 3: Read target inode → verify it's a directory ────────
    let target_group = crate::inode::inode_to_group(target_ino, &sb);
    let target_gd = match crate::group_desc::read_group_descriptor(&sb, target_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let target_groups = [target_gd.clone()];
    let target_inode = match crate::block::read_inode(&sb, &target_groups, target_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    if !target_inode.is_dir() {
        return 20; // ENOTDIR
    }

    // ── Step 4: Verify directory is empty ───────────────────────────
    // Create a block reader for the target dir using extent_lookup
    let mut target_reader = unsafe { make_block_reader(ctx, read_fn, block_size) };
    let is_empty = match is_dir_empty(
        &target_inode, &sb,
        |block: u64, buf: &mut [u8]| {
            match crate::extent::extent_lookup(&sb, &target_inode, block, &mut target_reader) {
                Ok(Some(phys)) => target_reader(phys, buf),
                Ok(None) => {
                    for byte in buf.iter_mut() { *byte = 0; }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        },
    ) {
        Ok(empty) => empty,
        Err(e) => return errno_from_error(e),
    };

    if !is_empty {
        return 39; // ENOTEMPTY
    }

    // ── Step 5: Remove entry from parent dir ────────────────────────
    {
        let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
            let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        match crate::dir::remove_in_dir(
            &sb, &dir_inode, dir_ino, name_str, &mut reader, &mut wb,
        ) {
            Ok(true) => {},
            Ok(false) => return 2, // ENOENT
            Err(e) => return errno_from_error(e),
        }
    }

    // ── Step 6: Decrement parent dir's link count ───────────────────
    // Pre-compute parent inode table block info
    let dir_index = crate::inode::inode_to_group_index(dir_ino, &sb);
    let dir_inode_table_block = dir_gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let dir_block_offset = dir_index as u64 / inodes_per_block;
    let dir_in_block_offset = (dir_index as u64 % inodes_per_block) * inode_size as u64;
    let dir_inode_block_nr = dir_inode_table_block + dir_block_offset;

    // Re-read parent inode (it may have been modified by insert_in_dir in another call)
    let mut dir_inode_for_update = match crate::block::read_inode(&sb, &dir_groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Check if parent dir has more links to decrement
    let parent_should_free = crate::inode::inode_unlink(&mut dir_inode_for_update);
    if parent_should_free {
        // This should never happen for a well-formed filesystem (parent can't reach 0)
        return 5; // EIO
    }

    // Update parent ctime/mtime and write back
    crate::inode::update_timestamps(&mut dir_inode_for_update, 0, 0, 0);
    {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, dir_inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        crate::inode::serialize_inode(
            &mut block_buf[dir_in_block_offset as usize..],
            &dir_inode_for_update,
            &sb,
            Some(dir_ino),
        );

        let ret = unsafe { write_fn(ctx, dir_inode_block_nr, block_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }
    }

    // ── Step 7: Free target directory's data blocks and inode ───────
    // Pre-compute target inode table block info
    let target_index = crate::inode::inode_to_group_index(target_ino, &sb);
    let target_inode_table_block = target_gd.inode_table(&sb);
    let target_block_offset = target_index as u64 / inodes_per_block;
    let target_in_block_offset = (target_index as u64 % inodes_per_block) * inode_size as u64;
    let target_inode_block_nr = target_inode_table_block + target_block_offset;

    // Re-read target inode (fresh copy from disk)
    let mut target_inode = match crate::block::read_inode(&sb, &target_groups, target_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Decrement link count (removing parent's entry)
    let should_free = crate::inode::inode_unlink(&mut target_inode);
    // For directories, we always free the data (`. ` entry is in the blocks being freed)
    let _ = should_free; // suppress unused warning — we always free for dirs

    // Free all data blocks and clear inode bitmap
    {
        let mut free_cb = |phys: u64, count: u32| -> Ext4Result<()> {
            let ret = unsafe { free_fn(ctx, phys, count) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        let mut free_ino_cb = |ino: u32| -> Ext4Result<()> {
            let ret = unsafe { free_ino_fn(ctx, ino) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        match crate::inode::free_inode_data(
            &sb, &mut target_inode, target_ino,
            &mut free_cb, &mut free_ino_cb,
        ) {
            Ok(()) => {},
            Err(e) => return errno_from_error(e),
        }
    }

    // Write the zeroed (deleted) inode back to the inode table
    {
        let mut target_block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, target_inode_block_nr, target_block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        crate::inode::serialize_inode(
            &mut target_block_buf[target_in_block_offset as usize..],
            &target_inode,
            &sb,
            None,
        );

        let ret = unsafe { write_fn(ctx, target_inode_block_nr, target_block_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }
    }

    0
}

/// Rename a file or directory (move between directories).
///
/// Removes `old_name` from `old_dir` and inserts `new_name` in `new_dir`,
/// both pointing to the same inode. If `new_name` already exists, it is
/// removed first.
///
/// Currently only supports renaming regular files (not directories), as
/// directory rename requires updating the `..` entry.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_rename(
    sbi: *const ext4_sb_info,
    old_dir_ino: u32,
    old_name: *const c_char,
    new_dir_ino: u32,
    new_name: *const c_char,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
    free_blocks: Option<ext4_free_blocks_cb>,
    free_inode: Option<ext4_free_inode_cb>,
) -> c_int {
    if sbi.is_null() || old_name.is_null() || new_name.is_null()
        || read_block.is_none() || write_block.is_none()
        || alloc_block.is_none() || free_blocks.is_none() || free_inode.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let alloc_fn = alloc_block.unwrap();
    let free_fn = free_blocks.unwrap();
    let free_ino_fn = free_inode.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let old_name_str = match unsafe { core::ffi::CStr::from_ptr(old_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };
    let new_name_str = match unsafe { core::ffi::CStr::from_ptr(new_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    // Same name, same directory — no-op
    if old_dir_ino == new_dir_ino && old_name_str == new_name_str {
        return 0;
    }

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // ── Step 1: Read old_dir inode ──────────────────────────────────
    let old_group = crate::inode::inode_to_group(old_dir_ino, &sb);
    let old_gd = match crate::group_desc::read_group_descriptor(&sb, old_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let old_groups = [old_gd.clone()];
    let old_dir_inode = match crate::block::read_inode(&sb, &old_groups, old_dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // ── Step 2: Look up old_name to find target_ino and file_type ───
    let (target_ino, file_type) = match crate::dir::lookup_in_dir(
        &sb, &old_dir_inode, old_dir_ino, old_name_str,
        |block: u64, buf: &mut [u8]| {
            match crate::extent::extent_lookup(&sb, &old_dir_inode, block, &mut reader) {
                Ok(Some(phys)) => reader(phys, buf),
                Ok(None) => {
                    for byte in buf.iter_mut() { *byte = 0; }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        },
    ) {
        Ok(Some(entry)) => (entry.inode, entry.file_type),
        Ok(None) => return 2, // ENOENT
        Err(e) => return errno_from_error(e),
    };

    // Check if target is a directory (not supported)
    let target_group = crate::inode::inode_to_group(target_ino, &sb);
    let target_gd = match crate::group_desc::read_group_descriptor(&sb, target_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let target_groups = [target_gd.clone()];
    let target_inode = match crate::block::read_inode(&sb, &target_groups, target_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    if target_inode.is_dir() {
        return 95; // ENOTSUP — directory rename not yet supported
    }

    // ── Step 3: Read new_dir inode ──────────────────────────────────
    let new_group = crate::inode::inode_to_group(new_dir_ino, &sb);
    let new_gd = match crate::group_desc::read_group_descriptor(&sb, new_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let new_groups = [new_gd.clone()];
    let mut new_dir_inode = match crate::block::read_inode(&sb, &new_groups, new_dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // ── Step 4: Check if new_name exists — if so, unlink it ─────────
    // Capture the existing entry's inode so we decrement the correct ino, not target_ino
    let existing_ino = match crate::dir::lookup_in_dir(
        &sb, &new_dir_inode, new_dir_ino, new_name_str,
        |block: u64, buf: &mut [u8]| {
            match crate::extent::extent_lookup(&sb, &new_dir_inode, block, &mut reader) {
                Ok(Some(phys)) => reader(phys, buf),
                Ok(None) => {
                    for byte in buf.iter_mut() { *byte = 0; }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        },
    ) {
        Ok(Some(entry)) => Some(entry.inode),
        Ok(None) => None,
        Err(_) => None, // Ignore errors — if we can't read, assume doesn't exist
    };

    if let Some(existing_ino) = existing_ino {
        // Remove the existing entry
        let mut wb_temp = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
            let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        if let Err(e) = crate::dir::remove_in_dir(
            &sb, &new_dir_inode, new_dir_ino, new_name_str, &mut reader, &mut wb_temp,
        ) {
            return errno_from_error(e);
        }

        // Decrement the existing entry's inode link count (the one being replaced)
        let _wb_deleted = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
            let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        // Read the existing inode to decrement link count
        let existing_group = crate::inode::inode_to_group(existing_ino, &sb);
        let existing_gd = match crate::group_desc::read_group_descriptor(&sb, existing_group, &mut reader) {
            Ok(gd) => gd,
            Err(e) => return errno_from_error(e),
        };
        let existing_groups = [existing_gd.clone()];
        let mut existing_target_inode = match crate::block::read_inode(&sb, &existing_groups, existing_ino, &mut reader) {
            Ok(inode) => inode,
            Err(e) => return errno_from_error(e),
        };

        let should_free = crate::inode::inode_unlink(&mut existing_target_inode);

        if should_free {
            let mut free_cb = |phys: u64, count: u32| -> Ext4Result<()> {
                let ret = unsafe { free_fn(ctx, phys, count) };
                if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
            };
            let mut free_ino_cb = |ino: u32| -> Ext4Result<()> {
                let ret = unsafe { free_ino_fn(ctx, ino) };
                if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
            };

            if let Err(e) = crate::inode::free_inode_data(
                &sb, &mut existing_target_inode, existing_ino, &mut free_cb, &mut free_ino_cb,
            ) {
                return errno_from_error(e);
            }

            // Write zeroed inode back
            let ti = crate::inode::inode_to_group_index(existing_ino, &sb);
            let t_table = existing_gd.inode_table(&sb);
            let t_blocks_per = block_size as u64 / inode_size as u64;
            let t_b_off = ti as u64 / t_blocks_per;
            let t_ib_off = (ti as u64 % t_blocks_per) * inode_size as u64;
            let t_ib_nr = t_table + t_b_off;

            let mut tbuf = vec![0u8; block_size];
            let ret = unsafe { read_fn(ctx, t_ib_nr, tbuf.as_mut_ptr(), block_size as u32) };
            if ret != 0 { return 5; }
            crate::inode::serialize_inode(&mut tbuf[t_ib_off as usize..], &existing_target_inode, &sb, None);
            let ret = unsafe { write_fn(ctx, t_ib_nr, tbuf.as_ptr(), block_size as u32) };
            if ret != 0 { return 5; }
        } else {
            // Write decremented link count back
            let ti = crate::inode::inode_to_group_index(existing_ino, &sb);
            let t_table = existing_gd.inode_table(&sb);
            let t_blocks_per = block_size as u64 / inode_size as u64;
            let t_b_off = ti as u64 / t_blocks_per;
            let t_ib_off = (ti as u64 % t_blocks_per) * inode_size as u64;
            let t_ib_nr = t_table + t_b_off;

            let mut tbuf = vec![0u8; block_size];
            let ret = unsafe { read_fn(ctx, t_ib_nr, tbuf.as_mut_ptr(), block_size as u32) };
            if ret != 0 { return 5; }
            crate::inode::serialize_inode(&mut tbuf[t_ib_off as usize..], &existing_target_inode, &sb, Some(existing_ino));
            let ret = unsafe { write_fn(ctx, t_ib_nr, tbuf.as_ptr(), block_size as u32) };
            if ret != 0 { return 5; }
        }
    }

    // ── Step 5: Remove old_name from old_dir ────────────────────────
    {
        let mut wb_remove = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
            let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        match crate::dir::remove_in_dir(
            &sb, &old_dir_inode, old_dir_ino, old_name_str, &mut reader, &mut wb_remove,
        ) {
            Ok(true) => {},
            Ok(false) => return 2, // ENOENT
            Err(e) => return errno_from_error(e),
        }
    }

    // ── Step 6: Insert new_name in new_dir ──────────────────────────
    // Pre-compute new_dir inode table block info
    let new_index = crate::inode::inode_to_group_index(new_dir_ino, &sb);
    let new_inode_table_block = new_gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let new_block_offset = new_index as u64 / inodes_per_block;
    let new_in_block_offset = (new_index as u64 % inodes_per_block) * inode_size as u64;
    let new_inode_block_nr = new_inode_table_block + new_block_offset;

    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    let mut ab = move || -> Ext4Result<u64> {
        let phys = unsafe { alloc_fn(ctx) };
        if phys == 0 { Err(Ext4Error::NoSpace) } else { Ok(phys) }
    };

    let write_inode_sb = sb_from_sbi(sb_info);
    let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, new_inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }

            crate::inode::serialize_inode(
                &mut block_buf[new_in_block_offset as usize..],
                updated_inode,
                &write_inode_sb,
                Some(new_dir_ino),
            );

            let ret = unsafe { write_fn(ctx, new_inode_block_nr, block_buf.as_ptr(), block_size as u32) };
            if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        match crate::dir::insert_in_dir(
            &sb, &mut new_dir_inode, new_dir_ino, new_name_str, file_type, target_ino,
            &mut reader, &mut wb, &mut ab, &mut wi,
    ) {
        Ok(()) => 0,
        Err(e) => errno_from_error(e),
    }
}

/// Write data to a file at the given offset.
///
/// Uses the extent tree to write data, allocating new blocks as needed.
/// Updates the inode size, block count, and timestamps.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_write_file(
    sbi: *const ext4_sb_info,
    ino: u32,
    offset: u64,
    buf: *const u8,
    count: u32,
    bytes_written: *mut u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
) -> c_int {
    if sbi.is_null() || buf.is_null()
        || read_block.is_none() || write_block.is_none() || alloc_block.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let alloc_fn = alloc_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let mut inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute inode table block info for writing the inode back
    let index = crate::inode::inode_to_group_index(ino, &sb);
    let inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_offset = (index as u64 % inodes_per_block) * inode_size as u64;
    let inode_block_nr = inode_table_block + block_offset;

    // write_block closure
    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // alloc_block closure
    let mut ab = move || -> Ext4Result<u64> {
        let phys = unsafe { alloc_fn(ctx) };
        if phys == 0 { Err(Ext4Error::NoSpace) } else { Ok(phys) }
    };

    // write_inode closure
    let write_inode_sb = sb_from_sbi(sb_info);
    let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }

        crate::inode::serialize_inode(
            &mut block_buf[in_block_offset as usize..],
            updated_inode,
            &write_inode_sb,
            Some(ino),
        );

        let ret = unsafe { write_fn(ctx, inode_block_nr, block_buf.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // Count must fit in usize for safe slicing
    let count_usize = count as usize;
    let data_slice = unsafe { core::slice::from_raw_parts(buf, count_usize) };

    match crate::extent::extent_write(
        &sb, &mut inode, offset, data_slice,
        &mut reader, &mut wb, &mut ab, &mut wi,
    ) {
        Ok(n) => {
            if !bytes_written.is_null() {
                unsafe { *bytes_written = n as u32; }
            }
            0
        }
        Err(e) => errno_from_error(e),
    }
}

/// Read directory entries from a directory at the given position.
///
/// Fills up to `max_entries` `ext4_dirent` structs from the directory
/// starting at position `*pos`. Updates `*pos` to continue from where
/// this call left off. Returns the number of entries filled in `count`.
///
/// # Safety
/// All callbacks must be valid and callable with the given `ctx`.
#[no_mangle]
pub unsafe extern "C" fn ext4_readdir(
    sbi: *const ext4_sb_info,
    ino: u32,
    pos: *mut u64,
    entries: *mut ext4_dirent,
    max_entries: u32,
    count: *mut u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || pos.is_null() || entries.is_null() || count.is_null() || read_block.is_none() {
        return 22; // EINVAL
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the directory inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd];
    let inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    let current_pos = unsafe { *pos };
    let file_size = inode.file_size();

    if current_pos >= file_size {
        unsafe { *count = 0; }
        return 0; // EOF
    }

    // Read a chunk of directory data starting at current_pos
    let read_size = core::cmp::min(block_size as u64, file_size - current_pos) as usize;
    let mut buf = vec![0u8; read_size];

    match crate::extent::extent_read(
        &sb, &inode, current_pos, &mut buf,
        |block: u64, block_buf: &mut [u8]| reader(block, block_buf),
    ) {
        Ok(n) => {
            if n == 0 {
                unsafe { *count = 0; }
                return 0;
            }
            buf.truncate(n);
        }
        Err(e) => return errno_from_error(e),
    }

    // Parse directory entries from the buffer, tracking position
    let data = &buf[..];
    let mut buf_pos: usize = 0;
    let entries_slice = unsafe { slice::from_raw_parts_mut(entries, max_entries as usize) };
    let mut num_entries: u32 = 0;

    while buf_pos + 8 <= data.len() {
        let entry_inode = u32::from_le_bytes([
            data[buf_pos], data[buf_pos + 1],
            data[buf_pos + 2], data[buf_pos + 3],
        ]);
        let rec_len = u16::from_le_bytes([
            data[buf_pos + 4], data[buf_pos + 5],
        ]) as usize;

        if rec_len < 8 || buf_pos + rec_len > data.len() {
            // Invalid entry or last entry in the block — stop
            break;
        }

        if entry_inode != 0 {
            if num_entries >= max_entries {
                break; // Buffer full
            }

            let name_len = data[buf_pos + 6] as usize;
            let file_type = data[buf_pos + 7];
            let actual_name_len = core::cmp::min(name_len, 255);

            let mut name_arr = [0i8; 255];
            for i in 0..actual_name_len {
                if buf_pos + 8 + i < data.len() {
                    name_arr[i] = data[buf_pos + 8 + i] as i8;
                }
            }

            entries_slice[num_entries as usize] = ext4_dirent {
                ino: entry_inode,
                file_type,
                name_len: actual_name_len as u8,
                name: name_arr,
            };

            num_entries += 1;
        }

        buf_pos += rec_len;
    }

    // Update pos to continue from where we left off
    unsafe { *pos = current_pos + buf_pos as u64; }
    unsafe { *count = num_entries; }

    0
}

/// Change ownership of a file (chown/chgrp).
///
/// Reads the inode, sets uid/gid, and writes it back.
/// `mode` is an in/out parameter — VFS passes the current mode,
/// and the FS can modify it if needed.
#[no_mangle]
pub unsafe extern "C" fn ext4_chown(
    sbi: *const ext4_sb_info,
    ino: u32,
    uid: u16,
    gid: u16,
    mode: *mut u16,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
) -> c_int {
    if sbi.is_null() || mode.is_null()
        || read_block.is_none() || write_block.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let mut inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Update uid, gid, and mode
    inode.i_uid = uid;
    inode.i_gid = gid;
    crate::inode::update_timestamps(&mut inode, 0, 0, 0);

    // Pre-compute inode table block info
    let index = crate::inode::inode_to_group_index(ino, &sb);
    let inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_offset = (index as u64 % inodes_per_block) * inode_size as u64;
    let inode_block_nr = inode_table_block + block_offset;

    // Write inode back
    let mut block_buf = vec![0u8; block_size];
    let ret = unsafe { read_fn(ctx, inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    crate::inode::serialize_inode(
        &mut block_buf[in_block_offset as usize..],
        &inode,
        &sb,
        Some(ino),
    );

    let ret = unsafe { write_fn(ctx, inode_block_nr, block_buf.as_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    // Return the new mode
    unsafe { *mode = inode.i_mode; }

    0
}

/// Change the mode of a file (chmod).
#[no_mangle]
pub unsafe extern "C" fn ext4_chmod(
    sbi: *const ext4_sb_info,
    ino: u32,
    mode: *mut u16,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
) -> c_int {
    if sbi.is_null() || mode.is_null()
        || read_block.is_none() || write_block.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let mut inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Preserve type bits, update permission bits
    let new_mode = (inode.i_mode & EXT4_S_IFMT) | (unsafe { *mode } & !EXT4_S_IFMT);
    inode.i_mode = new_mode;
    crate::inode::update_timestamps(&mut inode, 0, 0, 0);

    // Pre-compute inode table block info
    let index = crate::inode::inode_to_group_index(ino, &sb);
    let inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_offset = (index as u64 % inodes_per_block) * inode_size as u64;
    let inode_block_nr = inode_table_block + block_offset;

    // Write inode back
    let mut block_buf = vec![0u8; block_size];
    let ret = unsafe { read_fn(ctx, inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    crate::inode::serialize_inode(
        &mut block_buf[in_block_offset as usize..],
        &inode,
        &sb,
        Some(ino),
    );

    let ret = unsafe { write_fn(ctx, inode_block_nr, block_buf.as_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    // Return the new mode
    unsafe { *mode = inode.i_mode; }

    0
}

/// Update file timestamps (utime).
#[no_mangle]
pub unsafe extern "C" fn ext4_utime(
    sbi: *const ext4_sb_info,
    ino: u32,
    atime: u32,
    mtime: u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
) -> c_int {
    if sbi.is_null()
        || read_block.is_none() || write_block.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd.clone()];
    let mut inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    crate::inode::update_timestamps(&mut inode, mtime, mtime, atime);

    // Pre-compute inode table block info
    let index = crate::inode::inode_to_group_index(ino, &sb);
    let inode_table_block = gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_offset = (index as u64 % inodes_per_block) * inode_size as u64;
    let inode_block_nr = inode_table_block + block_offset;

    // Write inode back
    let mut block_buf = vec![0u8; block_size];
    let ret = unsafe { read_fn(ctx, inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    crate::inode::serialize_inode(
        &mut block_buf[in_block_offset as usize..],
        &inode,
        &sb,
        Some(ino),
    );

    let ret = unsafe { write_fn(ctx, inode_block_nr, block_buf.as_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    0
}

/// Create a device node (mknod).
#[no_mangle]
pub unsafe extern "C" fn ext4_mknod(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    mode: u16,
    uid: u16,
    gid: u16,
    rdev: u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
    alloc_inode: Option<ext4_alloc_inode_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null()
        || read_block.is_none() || write_block.is_none()
        || alloc_block.is_none() || alloc_inode.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let alloc_fn = alloc_block.unwrap();
    let alloc_ino_fn = alloc_inode.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let name_str = match unsafe { core::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read parent dir inode
    let parent_group = crate::inode::inode_to_group(dir_ino, &sb);
    let parent_gd = match crate::group_desc::read_group_descriptor(&sb, parent_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let parent_groups = [parent_gd.clone()];
    let mut parent_inode = match crate::block::read_inode(&sb, &parent_groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute parent inode table block info
    let parent_index = crate::inode::inode_to_group_index(dir_ino, &sb);
    let parent_inode_table_block = parent_gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let parent_block_offset = parent_index as u64 / inodes_per_block;
    let parent_in_block_offset = (parent_index as u64 % inodes_per_block) * inode_size as u64;
    let parent_inode_block_nr = parent_inode_table_block + parent_block_offset;

    // Allocate inode
    let new_ino = unsafe { alloc_ino_fn(ctx) };
    if new_ino == 0 { return 28; }

    // Create new inode with full mode (includes type bits from C)
    let mut new_inode = crate::inode::new_inode(mode, uid, gid);
    crate::inode::init_extent_tree(&mut new_inode);
    crate::inode::update_timestamps(&mut new_inode, 0, 0, 0);

    // For block/char devices, store rdev in i_block[0]
    if mode & EXT4_S_IFBLK != 0 || mode & EXT4_S_IFCHR != 0 {
        new_inode.i_block[0..4].copy_from_slice(&rdev.to_le_bytes());
        new_inode.i_flags = 0; // No extent tree for device nodes
        new_inode.i_block[4..6].copy_from_slice(&[0, 0]); // eh_magic = 0 = no extents
    }

    // Compute new inode table block info
    let new_group = crate::inode::inode_to_group(new_ino, &sb);
    let new_gd = match crate::group_desc::read_group_descriptor(&sb, new_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let new_index = crate::inode::inode_to_group_index(new_ino, &sb);
    let new_inode_table_block = new_gd.inode_table(&sb);
    let new_block_offset = new_index as u64 / inodes_per_block;
    let new_in_block_offset = (new_index as u64 % inodes_per_block) * inode_size as u64;
    let new_inode_block_nr = new_inode_table_block + new_block_offset;

    // Write new inode
    let mut inode_block_buf = vec![0u8; block_size];
    let ret = unsafe { read_fn(ctx, new_inode_block_nr, inode_block_buf.as_mut_ptr(), block_size as u32) };
    if ret != 0 { return 5; }
    crate::inode::serialize_inode(&mut inode_block_buf[new_in_block_offset as usize..], &new_inode, &sb, Some(new_ino));
    let ret = unsafe { write_fn(ctx, new_inode_block_nr, inode_block_buf.as_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    // Create closures for insert_in_dir
    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };
    let mut ab = move || -> Ext4Result<u64> {
        let phys = unsafe { alloc_fn(ctx) };
        if phys == 0 { Err(Ext4Error::NoSpace) } else { Ok(phys) }
    };
    let write_inode_sb = sb_from_sbi(sb_info);
    let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf2 = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, parent_inode_block_nr, block_buf2.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }
        crate::inode::serialize_inode(&mut block_buf2[parent_in_block_offset as usize..], updated_inode, &write_inode_sb, Some(dir_ino));
        let ret = unsafe { write_fn(ctx, parent_inode_block_nr, block_buf2.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    // Determine file_type from mode
    let file_type = if mode & EXT4_S_IFBLK != 0 { EXT4_FT_BLKDEV }
        else if mode & EXT4_S_IFCHR != 0 { EXT4_FT_CHRDEV }
        else if mode & EXT4_S_IFIFO != 0 { EXT4_FT_FIFO }
        else if mode & EXT4_S_IFSOCK != 0 { EXT4_FT_SOCK }
        else { EXT4_FT_UNKNOWN };

    match crate::dir::insert_in_dir(
        &sb, &mut parent_inode, dir_ino, name_str, file_type, new_ino,
        &mut reader, &mut wb, &mut ab, &mut wi,
    ) {
        Ok(()) => 0,
        Err(e) => errno_from_error(e),
    }
}

/// Create a symbolic link.
///
/// Fast symlinks (target <= 60 bytes) store the target in i_block.
/// Slow symlinks (target > 60 bytes) store the target in data blocks
/// via extent_insert.
#[no_mangle]
pub unsafe extern "C" fn ext4_symlink(
    sbi: *const ext4_sb_info,
    dir_ino: u32,
    name: *const c_char,
    target: *const c_char,
    uid: u16,
    gid: u16,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    write_block: Option<ext4_write_block_cb>,
    alloc_block: Option<ext4_alloc_block_cb>,
    alloc_inode: Option<ext4_alloc_inode_cb>,
) -> c_int {
    if sbi.is_null() || name.is_null() || target.is_null()
        || read_block.is_none() || write_block.is_none()
        || alloc_block.is_none() || alloc_inode.is_none()
    {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let write_fn = write_block.unwrap();
    let alloc_fn = alloc_block.unwrap();
    let alloc_ino_fn = alloc_inode.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();
    let inode_size = sb.inode_size() as usize;

    let name_str = match unsafe { core::ffi::CStr::from_ptr(name) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };
    let target_str = match unsafe { core::ffi::CStr::from_ptr(target) }.to_str() {
        Ok(s) => s,
        Err(_) => return 22,
    };

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read parent dir inode
    let parent_group = crate::inode::inode_to_group(dir_ino, &sb);
    let parent_gd = match crate::group_desc::read_group_descriptor(&sb, parent_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let parent_groups = [parent_gd.clone()];
    let mut parent_inode = match crate::block::read_inode(&sb, &parent_groups, dir_ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    // Pre-compute parent inode table block info
    let parent_index = crate::inode::inode_to_group_index(dir_ino, &sb);
    let parent_inode_table_block = parent_gd.inode_table(&sb);
    let inodes_per_block = block_size as u64 / inode_size as u64;
    let parent_block_offset = parent_index as u64 / inodes_per_block;
    let parent_in_block_offset = (parent_index as u64 % inodes_per_block) * inode_size as u64;
    let parent_inode_block_nr = parent_inode_table_block + parent_block_offset;

    // Allocate inode
    let new_ino = unsafe { alloc_ino_fn(ctx) };
    if new_ino == 0 { return 28; }

    // Create new inode for symlink
    let mut new_inode = crate::inode::new_inode(EXT4_S_IFLNK | 0o777, uid, gid);
    crate::inode::update_timestamps(&mut new_inode, 0, 0, 0);
    crate::inode::set_file_size(&mut new_inode, target_str.len() as u64);

    let target_bytes = target_str.as_bytes();
    let sectors_per_block = block_size as u64 / 512;

    if target_bytes.len() <= 60 {
        // Fast symlink: store target in i_block
        crate::inode::set_symlink_target(&mut new_inode, target_str);
    } else {
        // Slow symlink: alloc data block, write target, extent_insert
        crate::inode::init_extent_tree(&mut new_inode);
    }

    // Compute new inode table block info
    let new_group = crate::inode::inode_to_group(new_ino, &sb);
    let new_gd = match crate::group_desc::read_group_descriptor(&sb, new_group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let new_index = crate::inode::inode_to_group_index(new_ino, &sb);
    let new_inode_table_block = new_gd.inode_table(&sb);
    let new_block_offset = new_index as u64 / inodes_per_block;
    let new_in_block_offset = (new_index as u64 % inodes_per_block) * inode_size as u64;
    let new_inode_block_nr = new_inode_table_block + new_block_offset;

    // Write new inode initially
    let mut inode_block_buf = vec![0u8; block_size];
    let ret = unsafe { read_fn(ctx, new_inode_block_nr, inode_block_buf.as_mut_ptr(), block_size as u32) };
    if ret != 0 { return 5; }
    crate::inode::serialize_inode(&mut inode_block_buf[new_in_block_offset as usize..], &new_inode, &sb, Some(new_ino));
    let ret = unsafe { write_fn(ctx, new_inode_block_nr, inode_block_buf.as_ptr(), block_size as u32) };
    if ret != 0 { return 5; }

    // For slow symlinks, write data to a new block
    if target_bytes.len() > 60 {
        let data_block = unsafe { alloc_fn(ctx) };
        if data_block == 0 { return 28; }

        // Write symlink target to data block
        let data_buf = target_str.as_bytes();
        let ret = unsafe { write_fn(ctx, data_block, data_buf.as_ptr(), block_size as u32) };
        if ret != 0 { return 5; }

        // Re-read inode for extent_insert
        let mut buf2 = vec![0u8; block_size];
        let ret2 = unsafe { read_fn(ctx, new_inode_block_nr, buf2.as_mut_ptr(), block_size as u32) };
        if ret2 != 0 { return 5; }
        let mut inode_for_ext = match crate::inode::parse_inode(&buf2[new_in_block_offset as usize..], &sb) {
            Ok(inode) => inode,
            Err(e) => return errno_from_error(e),
        };

        crate::inode::set_file_size(&mut inode_for_ext, target_bytes.len() as u64);
        crate::inode::set_blocks_count(&mut inode_for_ext, sectors_per_block);
        crate::inode::update_timestamps(&mut inode_for_ext, 0, 0, 0);

        let write_inode_sb2 = sb_from_sbi(sb_info);
        let mut wi2 = move |updated: &Ext4Inode| -> Ext4Result<()> {
            let mut b3 = vec![0u8; block_size];
            let r = unsafe { read_fn(ctx, new_inode_block_nr, b3.as_mut_ptr(), block_size as u32) };
            if r != 0 { return Err(Ext4Error::IoError); }
            crate::inode::serialize_inode(&mut b3[new_in_block_offset as usize..], updated, &write_inode_sb2, Some(new_ino));
            let r = unsafe { write_fn(ctx, new_inode_block_nr, b3.as_ptr(), block_size as u32) };
            if r == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
        };

        if let Err(e) = crate::extent::extent_insert(
            &sb, &mut inode_for_ext, 0, data_block, 1, &mut wi2,
        ) {
            return errno_from_error(e);
        }
    }

    // Create closures for insert_in_dir
    let mut wb = move |block_nr: u64, data: &[u8]| -> Ext4Result<()> {
        let ret = unsafe { write_fn(ctx, block_nr, data.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };
    let mut ab = move || -> Ext4Result<u64> {
        let phys = unsafe { alloc_fn(ctx) };
        if phys == 0 { Err(Ext4Error::NoSpace) } else { Ok(phys) }
    };
    let write_inode_sb = sb_from_sbi(sb_info);
    let mut wi = move |updated_inode: &Ext4Inode| -> Ext4Result<()> {
        let mut block_buf = vec![0u8; block_size];
        let ret = unsafe { read_fn(ctx, parent_inode_block_nr, block_buf.as_mut_ptr(), block_size as u32) };
        if ret != 0 { return Err(Ext4Error::IoError); }
        crate::inode::serialize_inode(&mut block_buf[parent_in_block_offset as usize..], updated_inode, &write_inode_sb, Some(dir_ino));
        let ret = unsafe { write_fn(ctx, parent_inode_block_nr, block_buf.as_ptr(), block_size as u32) };
        if ret == 0 { Ok(()) } else { Err(Ext4Error::IoError) }
    };

    match crate::dir::insert_in_dir(
        &sb, &mut parent_inode, dir_ino, name_str, EXT4_FT_SYMLINK, new_ino,
        &mut reader, &mut wb, &mut ab, &mut wi,
    ) {
        Ok(()) => 0,
        Err(e) => errno_from_error(e),
    }
}

/// Read a symbolic link target.
///
/// For fast symlinks (target in i_block), copies the target directly.
/// For slow symlinks (target in data blocks), reads via extent_read.
#[no_mangle]
pub unsafe extern "C" fn ext4_readlink(
    sbi: *const ext4_sb_info,
    ino: u32,
    buf: *mut u8,
    buf_size: u32,
    bytes_read: *mut u32,
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
) -> c_int {
    if sbi.is_null() || buf.is_null() || bytes_read.is_null() || read_block.is_none() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };
    let read_fn = read_block.unwrap();
    let sb = sb_from_sbi(sb_info);
    let block_size = sb.block_size();

    let mut reader = unsafe { make_block_reader(ctx, read_fn, block_size) };

    // Read the inode
    let group = crate::inode::inode_to_group(ino, &sb);
    let gd = match crate::group_desc::read_group_descriptor(&sb, group, &mut reader) {
        Ok(gd) => gd,
        Err(e) => return errno_from_error(e),
    };
    let groups = [gd];
    let inode = match crate::block::read_inode(&sb, &groups, ino, &mut reader) {
        Ok(inode) => inode,
        Err(e) => return errno_from_error(e),
    };

    let file_size = inode.file_size() as usize;
    let copy_size = core::cmp::min(file_size, buf_size as usize);
    let buf_slice = unsafe { slice::from_raw_parts_mut(buf, copy_size) };

    if !inode.has_extents() {
        // Fast symlink: target is in i_block
        match crate::inode::get_symlink_target(&inode) {
            Some(target) => {
                let target_bytes = target.as_bytes();
                let n = core::cmp::min(target_bytes.len(), copy_size);
                buf_slice[..n].copy_from_slice(&target_bytes[..n]);
                unsafe { *bytes_read = n as u32; }
                0
            }
            None => {
                unsafe { *bytes_read = 0; }
                22 // EINVAL
            }
        }
    } else {
        // Slow symlink: read from data blocks via extent tree
        match crate::extent::extent_read(
            &sb, &inode, 0, buf_slice,
            |block: u64, block_buf: &mut [u8]| reader(block, block_buf),
        ) {
            Ok(n) => {
                unsafe { *bytes_read = n as u32; }
                0
            }
            Err(e) => errno_from_error(e),
        }
    }
}

/// Get filesystem statistics.
#[no_mangle]
pub unsafe extern "C" fn ext4_statvfs(
    sbi: *const ext4_sb_info,
    block_size: *mut u32,
    blocks_total: *mut u64,
    blocks_free: *mut u64,
    inodes_total: *mut u64,
    inodes_free: *mut u64,
) -> c_int {
    if sbi.is_null() {
        return 22;
    }

    let sb_info = unsafe { &*sbi };

    if !block_size.is_null() {
        unsafe { *block_size = sb_info.block_size; }
    }
    if !blocks_total.is_null() {
        unsafe { *blocks_total = sb_info.blocks_count; }
    }
    if !blocks_free.is_null() {
        unsafe { *blocks_free = sb_info.free_blocks_count; }
    }
    if !inodes_total.is_null() {
        unsafe { *inodes_total = sb_info.inodes_count; }
    }
    if !inodes_free.is_null() {
        unsafe { *inodes_free = sb_info.free_inodes_count; }
    }

    0
}
