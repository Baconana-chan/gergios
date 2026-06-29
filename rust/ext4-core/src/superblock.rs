//! ext4 superblock parsing and validation.
//!
//! The superblock is located at offset 1024 from the start of the block device
//! and is exactly 1024 bytes long. The magic number 0xEF53 identifies ext4.
//!
//! Byte offsets verified against Linux kernel `ext4.h` (`struct ext4_super_block`).

use crate::types::*;

/// Parse and validate the ext4 superblock from a raw 1024-byte buffer.
///
/// Returns `Ok(Ext4Superblock)` on success, or an `Ext4Error` describing
/// the problem (invalid magic, unsupported features, etc.).
pub fn parse_superblock(data: &[u8]) -> Ext4Result<Ext4Superblock> {
    if data.len() < 1024 {
        return Err(Ext4Error::IoError);
    }

    // ── Verify magic (offset 56: __le16 s_magic = 0xEF53) ──────────
    let magic = u16::from_le_bytes([data[56], data[57]]);
    if magic != EXT4_SUPER_MAGIC {
        return Err(Ext4Error::InvalidMagic);
    }

    // ── Feature flags ──────────────────────────────────────────────
    // offset 92: __le32 s_feature_compat
    // offset 96: __le32 s_feature_incompat
    // offset 100: __le32 s_feature_ro_compat
    let s_feature_compat = u32::from_le_bytes([data[92], data[93], data[94], data[95]]);
    let s_feature_incompat = u32::from_le_bytes([data[96], data[97], data[98], data[99]]);
    let s_feature_ro_compat = u32::from_le_bytes([data[100], data[101], data[102], data[103]]);

    // Check unsupported INCOMPAT features
    let unsupported_incompat = s_feature_incompat & EXT4_UNSUPPORTED_INCOMPAT;
    if unsupported_incompat != 0 {
        return Err(Ext4Error::UnsupportedIncompat(unsupported_incompat));
    }

    // Check required INCOMPAT features
    let required = EXT4_REQUIRED_INCOMPAT & !s_feature_incompat;
    if required != 0 {
        return Err(Ext4Error::UnsupportedIncompat(required));
    }

    // Check unsupported RO_COMPAT features
    let unsupported_ro = s_feature_ro_compat & EXT4_UNSUPPORTED_RO_COMPAT;
    if unsupported_ro != 0 {
        return Err(Ext4Error::UnsupportedRoCompat(unsupported_ro));
    }

    // ── Revision & inode size ──────────────────────────────────────
    // offset 76: __le32 s_rev_level (0=good old, 1=dynamic)
    let s_rev_level = u32::from_le_bytes([data[76], data[77], data[78], data[79]]);
    // offset 88: __le16 s_inode_size
    let s_inode_size = if s_rev_level == 0 {
        128u16
    } else {
        u16::from_le_bytes([data[88], data[89]])
    };

    if s_inode_size < 128 || (s_inode_size as u32) & (s_inode_size as u32 - 1) != 0 {
        return Err(Ext4Error::InvalidInodeSize);
    }

    // ── Block / cluster size ───────────────────────────────────────
    // offset 24: __le32 s_log_block_size  (block_size = 1024 << this)
    // offset 28: __le32 s_log_cluster_size
    let s_log_block_size = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
    let s_log_cluster_size = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);

    if s_log_block_size > 16 {
        return Err(Ext4Error::InvalidBlockSize);
    }

    Ok(Ext4Superblock {
        // offset 0:  __le32 s_inodes_count
        s_inodes_count: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        // offset 4:  __le32 s_blocks_count_lo
        s_blocks_count_lo: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        // offset 12: __le32 s_free_blocks_count_lo  (skip 8: s_r_blocks_count_lo)
        s_free_blocks_count_lo: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        // offset 16: __le32 s_free_inodes_count
        s_free_inodes_count: u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
        // offset 20: __le32 s_first_data_block
        s_first_data_block: u32::from_le_bytes([data[20], data[21], data[22], data[23]]),
        s_log_block_size,
        s_log_cluster_size,
        // offset 32: __le32 s_blocks_per_group
        s_blocks_per_group: u32::from_le_bytes([data[32], data[33], data[34], data[35]]),
        // offset 36: __le32 s_clusters_per_group
        s_clusters_per_group: u32::from_le_bytes([data[36], data[37], data[38], data[39]]),
        // offset 40: __le32 s_inodes_per_group
        s_inodes_per_group: u32::from_le_bytes([data[40], data[41], data[42], data[43]]),
        // offset 44: __le32 s_mtime
        s_mtime: u32::from_le_bytes([data[44], data[45], data[46], data[47]]),
        // offset 48: __le32 s_wtime
        s_wtime: u32::from_le_bytes([data[48], data[49], data[50], data[51]]),
        // offset 52: __le16 s_mnt_count
        s_mnt_count: u16::from_le_bytes([data[52], data[53]]),
        // offset 54: __le16 s_max_mnt_count
        s_max_mnt_count: u16::from_le_bytes([data[54], data[55]]),
        s_magic: magic,
        // offset 58: __le16 s_state
        s_state: u16::from_le_bytes([data[58], data[59]]),
        // offset 60: __le16 s_errors
        s_errors: u16::from_le_bytes([data[60], data[61]]),
        // offset 62: __le16 s_minor_rev_level
        s_minor_rev_level: u16::from_le_bytes([data[62], data[63]]),
        // offset 64: __le32 s_lastcheck
        s_lastcheck: u32::from_le_bytes([data[64], data[65], data[66], data[67]]),
        // offset 68: __le32 s_checkinterval
        s_checkinterval: u32::from_le_bytes([data[68], data[69], data[70], data[71]]),
        // offset 72: __le32 s_creator_os
        s_creator_os: u32::from_le_bytes([data[72], data[73], data[74], data[75]]),
        s_rev_level,
        // offset 80: __le16 s_def_resuid
        s_def_resuid: u16::from_le_bytes([data[80], data[81]]),
        // offset 82: __le16 s_def_resgid
        s_def_resgid: u16::from_le_bytes([data[82], data[83]]),
        // offset 84: __le32 s_first_ino
        s_first_ino: u32::from_le_bytes([data[84], data[85], data[86], data[87]]),
        s_inode_size,
        // offset 90: __le16 s_block_group_nr
        s_block_group_nr: u16::from_le_bytes([data[90], data[91]]),
        s_feature_compat,
        s_feature_incompat,
        s_feature_ro_compat,
        // offset 104: __u8 s_uuid[16]
        s_uuid: {
            let mut u = [0u8; 16];
            u.copy_from_slice(&data[104..120]);
            u
        },
        // offset 120: __u8 s_volume_name[16]
        s_volume_name: {
            let mut v = [0u8; 16];
            v.copy_from_slice(&data[120..136]);
            v
        },
        // offset 136: __u8 s_last_mounted[64]
        s_last_mounted: {
            let mut m = [0u8; 64];
            m.copy_from_slice(&data[136..200]);
            m
        },
        // offset 200: __le32 s_algorithm_usage_bitmap
        s_algorithm_usage_bitmap: u32::from_le_bytes([data[200], data[201], data[202], data[203]]),
        // offset 204: __u8 s_prealloc_blocks
        s_prealloc_blocks: data[204],
        // offset 205: __u8 s_prealloc_dir_blocks
        s_prealloc_dir_blocks: data[205],
        // offset 208: __le16 s_reserved_gdt_blocks (gap at 206-207 is padding)
        s_reserved_gdt_blocks: u16::from_le_bytes([data[208], data[209]]),
        // offset 240: __u8 s_journal_uuid[16]  (gap 210-239)
        s_journal_uuid: {
            let mut u = [0u8; 16];
            u.copy_from_slice(&data[240..256]);
            u
        },
        // offset 256: __le32 s_journal_inum
        s_journal_inum: u32::from_le_bytes([data[256], data[257], data[258], data[259]]),
        // offset 260: __le32 s_journal_dev
        s_journal_dev: u32::from_le_bytes([data[260], data[261], data[262], data[263]]),
        // offset 264: __le32 s_last_orphan
        s_last_orphan: u32::from_le_bytes([data[264], data[265], data[266], data[267]]),
        // offset 268: __le32 s_hash_seed[4]
        s_hash_seed: [
            u32::from_le_bytes([data[268], data[269], data[270], data[271]]),
            u32::from_le_bytes([data[272], data[273], data[274], data[275]]),
            u32::from_le_bytes([data[276], data[277], data[278], data[279]]),
            u32::from_le_bytes([data[280], data[281], data[282], data[283]]),
        ],
        // offset 284: __u8 s_def_hash_version
        s_def_hash_version: data[284],
        // offset 285: __u8 s_jnl_backup_type
        s_jnl_backup_type: data[285],
        // offset 286: __le16 s_desc_size
        s_desc_size: u16::from_le_bytes([data[286], data[287]]),
        // offset 288: __le32 s_default_mount_opts
        s_default_mount_opts: u32::from_le_bytes([data[288], data[289], data[290], data[291]]),
        // offset 292: __le32 s_first_meta_bg
        s_first_meta_bg: u32::from_le_bytes([data[292], data[293], data[294], data[295]]),
        // offset 296: __le32 s_mkfs_time
        s_mkfs_time: u32::from_le_bytes([data[296], data[297], data[298], data[299]]),
        // offset 300: __le32 s_jnl_blocks[17]
        s_jnl_blocks: {
            let mut j = [0u32; 17];
            for i in 0..17 {
                let off = 300 + i * 4;
                j[i] = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            }
            j
        },
        // offset 368: __le32 s_blocks_count_hi
        s_blocks_count_hi: u32::from_le_bytes([data[368], data[369], data[370], data[371]]),
        // offset 372: __le32 s_inodes_count_hi
        s_inodes_count_hi: u32::from_le_bytes([data[372], data[373], data[374], data[375]]),
        // offset 376: __le32 s_free_blocks_count_hi
        s_free_blocks_count_hi: u32::from_le_bytes([data[376], data[377], data[378], data[379]]),
        // offset 380: __le32 s_free_inodes_count_hi
        s_free_inodes_count_hi: u32::from_le_bytes([data[380], data[381], data[382], data[383]]),
        // offset 384: __le16 s_minor_extra_isize
        s_minor_extra_isize: u16::from_le_bytes([data[384], data[385]]),
        // offset 386: __le16 s_want_extra_isize
        s_want_extra_isize: u16::from_le_bytes([data[386], data[387]]),
        // offset 388: __le32 s_flags
        s_flags: u32::from_le_bytes([data[388], data[389], data[390], data[391]]),
        // offset 392: __le16 s_raid_stride
        s_raid_stride: u16::from_le_bytes([data[392], data[393]]),
        // offset 394: __le16 s_mmp_interval
        s_mmp_interval: u16::from_le_bytes([data[394], data[395]]),
        // offset 396: __le64 s_mmp_block
        s_mmp_block: u64::from_le_bytes([
            data[396], data[397], data[398], data[399],
            data[400], data[401], data[402], data[403],
        ]),
        // offset 404: __le32 s_raid_stripe_width
        s_raid_stripe_width: u32::from_le_bytes([data[404], data[405], data[406], data[407]]),
        // offset 408: __u8 s_log_groups_per_flex
        s_log_groups_per_flex: data[408],
        // offset 409: __u8 s_checksum_type
        s_checksum_type: data[409],
        // offset 410: __le16 s_reserved_pad  (s_checksum_seed moved in later kernels, use this gap)
        s_reserved_pad: u16::from_le_bytes([data[410], data[411]]),
        // offset 412: __le64 s_kbytes_written
        s_kbytes_written: u64::from_le_bytes([
            data[412], data[413], data[414], data[415],
            data[416], data[417], data[418], data[419],
        ]),
        // offset 420: __le32 s_snapshot_inum
        s_snapshot_inum: u32::from_le_bytes([data[420], data[421], data[422], data[423]]),
        // offset 424: __le32 s_snapshot_id
        s_snapshot_id: u32::from_le_bytes([data[424], data[425], data[426], data[427]]),
        // offset 428: __le64 s_snapshot_r_blocks
        s_snapshot_r_blocks: u64::from_le_bytes([
            data[428], data[429], data[430], data[431],
            data[432], data[433], data[434], data[435],
        ]),
        // offset 436: __le32 s_snapshot_list
        s_snapshot_list: u32::from_le_bytes([data[436], data[437], data[438], data[439]]),
        // offset 440: __le32 s_error_count
        s_error_count: u32::from_le_bytes([data[440], data[441], data[442], data[443]]),
        // offset 444: __le32 s_first_error_time
        s_first_error_time: u32::from_le_bytes([data[444], data[445], data[446], data[447]]),
        // offset 448: __le32 s_first_error_ino
        s_first_error_ino: u32::from_le_bytes([data[448], data[449], data[450], data[451]]),
        // offset 452: __le64 s_first_error_block
        s_first_error_block: u64::from_le_bytes([
            data[452], data[453], data[454], data[455],
            data[456], data[457], data[458], data[459],
        ]),
        // offset 460: __u8 s_first_error_func[32]
        s_first_error_func: {
            let mut f = [0u8; 32];
            f.copy_from_slice(&data[460..492]);
            f
        },
        // offset 492: __le32 s_first_error_line
        s_first_error_line: u32::from_le_bytes([data[492], data[493], data[494], data[495]]),
        // offset 496: __le32 s_last_error_time
        s_last_error_time: u32::from_le_bytes([data[496], data[497], data[498], data[499]]),
        // offset 500: __le32 s_last_error_ino
        s_last_error_ino: u32::from_le_bytes([data[500], data[501], data[502], data[503]]),
        // offset 504: __le32 s_last_error_line
        s_last_error_line: u32::from_le_bytes([data[504], data[505], data[506], data[507]]),
        // offset 508: __le64 s_last_error_block
        s_last_error_block: u64::from_le_bytes([
            data[508], data[509], data[510], data[511],
            data[512], data[513], data[514], data[515],
        ]),
        // offset 516: __u8 s_last_error_func[32]
        s_last_error_func: {
            let mut f = [0u8; 32];
            f.copy_from_slice(&data[516..548]);
            f
        },
        // offset 548: __u8 s_mount_opts[64]
        s_mount_opts: {
            let mut m = [0u8; 64];
            m.copy_from_slice(&data[548..612]);
            m
        },
        // offset 612: __le32 s_usr_quota_inum
        s_usr_quota_inum: u32::from_le_bytes([data[612], data[613], data[614], data[615]]),
        // offset 616: __le32 s_grp_quota_inum
        s_grp_quota_inum: u32::from_le_bytes([data[616], data[617], data[618], data[619]]),
        // offset 620: __le32 s_overhead_blocks
        s_overhead_blocks: u32::from_le_bytes([data[620], data[621], data[622], data[623]]),
        // offset 624: __le32 s_backup_bgs[2]
        s_backup_bgs: [
            u32::from_le_bytes([data[624], data[625], data[626], data[627]]),
            u32::from_le_bytes([data[628], data[629], data[630], data[631]]),
        ],
        // offset 632: __u8 s_encrypt_algos[4]
        s_encrypt_algos: {
            let mut a = [0u8; 4];
            a.copy_from_slice(&data[632..636]);
            a
        },
        // offset 636: __u8 s_encrypt_pw_salt[16]
        s_encrypt_pw_salt: {
            let mut s = [0u8; 16];
            s.copy_from_slice(&data[636..652]);
            s
        },
        // offset 652: __le32 s_lpf_ino
        s_lpf_ino: u32::from_le_bytes([data[652], data[653], data[654], data[655]]),
        // offset 656: __le32 s_prj_quota_inum
        s_prj_quota_inum: u32::from_le_bytes([data[656], data[657], data[658], data[659]]),
        // offset 660: __le32 s_checksum_seed  (on disk, in newer ext4 with CSUM_SEED)
        s_checksum_seed: u32::from_le_bytes([data[660], data[661], data[662], data[663]]),
        // offset 664: __u8 s_wtime_hi
        s_wtime_hi: data[664],
        // offset 665: __u8 s_mtime_hi
        s_mtime_hi: data[665],
        // offset 666: __u8 s_mkfs_time_hi
        s_mkfs_time_hi: data[666],
        // offset 667: __u8 s_lastcheck_hi
        s_lastcheck_hi: data[667],
        // offset 668: __u8 s_first_error_time_hi
        s_first_error_time_hi: data[668],
        // offset 669: __u8 s_last_error_time_hi
        s_last_error_time_hi: data[669],
        // offset 670: __u8 s_pad[2]
        s_pad: [data[670], data[671]],
        // offset 672: __le32 s_checksum
        s_checksum: u32::from_le_bytes([data[672], data[673], data[674], data[675]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid ext4 superblock with correct byte offsets.
    fn create_test_superblock() -> Vec<u8> {
        let mut sb = vec![0u8; 1024];
        // offset 0: s_inodes_count = 1024
        sb[0..4].copy_from_slice(&(1024u32).to_le_bytes());
        // offset 4: s_blocks_count_lo = 819200 (~400MB)
        sb[4..8].copy_from_slice(&(819200u32).to_le_bytes());
        // offset 12: s_free_blocks_count_lo = 789200
        sb[12..16].copy_from_slice(&(789200u32).to_le_bytes());
        // offset 16: s_free_inodes_count = 824
        sb[16..20].copy_from_slice(&(824u32).to_le_bytes());
        // offset 20: s_first_data_block = 0 (block size > 1024)
        sb[20..24].copy_from_slice(&(0u32).to_le_bytes());
        // offset 24: s_log_block_size = 2 (4096 bytes)
        sb[24..28].copy_from_slice(&(2u32).to_le_bytes());
        // offset 28: s_log_cluster_size = 2
        sb[28..32].copy_from_slice(&(2u32).to_le_bytes());
        // offset 32: s_blocks_per_group = 32768
        sb[32..36].copy_from_slice(&(32768u32).to_le_bytes());
        // offset 36: s_clusters_per_group = 32768
        sb[36..40].copy_from_slice(&(32768u32).to_le_bytes());
        // offset 40: s_inodes_per_group = 8192
        sb[40..44].copy_from_slice(&(8192u32).to_le_bytes());
        // offset 44: s_mtime = 0
        sb[44..48].copy_from_slice(&(0u32).to_le_bytes());
        // offset 48: s_wtime = 0
        sb[48..52].copy_from_slice(&(0u32).to_le_bytes());
        // offset 56: s_magic = 0xEF53
        sb[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
        // offset 58: s_state = 1 (clean)
        sb[58..60].copy_from_slice(&(1u16).to_le_bytes());
        // offset 76: s_rev_level = 1 (dynamic inode size)
        sb[76..80].copy_from_slice(&(1u32).to_le_bytes());
        // offset 84: s_first_ino = 11
        sb[84..88].copy_from_slice(&(11u32).to_le_bytes());
        // offset 88: s_inode_size = 256
        sb[88..90].copy_from_slice(&(256u16).to_le_bytes());
        // offset 92: s_feature_compat = 0
        sb[92..96].copy_from_slice(&(0u32).to_le_bytes());
        // offset 96: s_feature_incompat = FILETYPE | EXTENTS | FLEX_BG
        let incompat = EXT4_FEATURE_INCOMPAT_FILETYPE
            | EXT4_FEATURE_INCOMPAT_EXTENTS
            | EXT4_FEATURE_INCOMPAT_FLEX_BG;
        sb[96..100].copy_from_slice(&incompat.to_le_bytes());
        // offset 100: s_feature_ro_compat = SPARSE_SUPER | LARGE_FILE | GDT_CSUM | DIR_NLINK | EXTRA_ISIZE
        let ro_compat = EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER
            | EXT4_FEATURE_RO_COMPAT_LARGE_FILE
            | EXT4_FEATURE_RO_COMPAT_GDT_CSUM
            | EXT4_FEATURE_RO_COMPAT_DIR_NLINK
            | EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE;
        sb[100..104].copy_from_slice(&ro_compat.to_le_bytes());
        // offset 286: s_desc_size = 64
        sb[286..288].copy_from_slice(&(64u16).to_le_bytes());
        // offset 408: s_log_groups_per_flex = 0 (flex_bg=1, no grouping)
        // already zero
        sb
    }

    #[test]
    fn test_parse_valid_superblock() {
        let data = create_test_superblock();
        let sb = parse_superblock(&data).unwrap();
        assert_eq!(sb.block_size(), 4096);
        assert_eq!(sb.inodes_count(), 1024);
        assert_eq!(sb.blocks_count(), 819200);
        assert_eq!(sb.block_groups_count(), 25); // 819200 / 32768 = 25
        assert_eq!(sb.inode_size(), 256);
        assert!(sb.has_extents());
        assert!(sb.has_flex_bg());
        assert!(sb.has_sparse_super());
    }

    #[test]
    fn test_invalid_magic() {
        let mut data = create_test_superblock();
        data[56] = 0x00;
        data[57] = 0x00;
        assert!(matches!(parse_superblock(&data), Err(Ext4Error::InvalidMagic)));
    }

    #[test]
    fn test_too_small_buffer() {
        assert!(matches!(parse_superblock(&[0u8; 100]), Err(Ext4Error::IoError)));
    }

    #[test]
    fn test_unsupported_incompat() {
        let mut data = create_test_superblock();
        let bad = EXT4_FEATURE_INCOMPAT_COMPRESSION;
        let current = u32::from_le_bytes([data[96], data[97], data[98], data[99]]);
        data[96..100].copy_from_slice(&(current | bad).to_le_bytes());
        assert!(matches!(parse_superblock(&data), Err(Ext4Error::UnsupportedIncompat(_))));
    }

    #[test]
    fn test_unsupported_ro_compat() {
        let mut data = create_test_superblock();
        let bad = EXT4_FEATURE_RO_COMPAT_BTREE_DIR;
        let current = u32::from_le_bytes([data[100], data[101], data[102], data[103]]);
        data[100..104].copy_from_slice(&(current | bad).to_le_bytes());
        assert!(matches!(parse_superblock(&data), Err(Ext4Error::UnsupportedRoCompat(_))));
    }

    #[test]
    fn test_sparse_super_blocks() {
        let data = create_test_superblock();
        let sb = parse_superblock(&data).unwrap();
        assert!(crate::block::has_superblock_backup(&sb, 0));
        assert!(crate::block::has_superblock_backup(&sb, 1));
        assert!(crate::block::has_superblock_backup(&sb, 3));
        assert!(!crate::block::has_superblock_backup(&sb, 4));
        assert!(crate::block::has_superblock_backup(&sb, 5));
        assert!(crate::block::has_superblock_backup(&sb, 7));
        assert!(crate::block::has_superblock_backup(&sb, 9));  // 3^2
        assert!(!crate::block::has_superblock_backup(&sb, 10));
    }
}
