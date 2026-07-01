//! Shared ext4 types, constants, and feature flags.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::fmt;

// ─── Superblock magic ────────────────────────────────────────────────

pub const EXT4_SUPER_MAGIC: u16 = 0xEF53;

// ─── Superblock offset ───────────────────────────────────────────────

/// Superblock is always at offset 1024 from the start of the block device.
pub const EXT4_SB_OFFSET: u64 = 1024;

// ─── Inode sizes ─────────────────────────────────────────────────────

pub const EXT4_GOOD_OLD_INODE_SIZE: u16 = 128;
pub const EXT4_DYNAMIC_INODE_SIZE: u16 = 256;

// ─── Feature flags: INCOMPAT ─────────────────────────────────────────

pub const EXT4_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
pub const EXT4_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
pub const EXT4_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
pub const EXT4_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
pub const EXT4_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;
pub const EXT4_FEATURE_INCOMPAT_EXTENTS: u32 = 0x0040;
pub const EXT4_FEATURE_INCOMPAT_64BIT: u32 = 0x0080;
pub const EXT4_FEATURE_INCOMPAT_MMP: u32 = 0x0100;
pub const EXT4_FEATURE_INCOMPAT_FLEX_BG: u32 = 0x0200;
pub const EXT4_FEATURE_INCOMPAT_EA_INODE: u32 = 0x0400;
pub const EXT4_FEATURE_INCOMPAT_DIRDATA: u32 = 0x1000;
pub const EXT4_FEATURE_INCOMPAT_CSUM_SEED: u32 = 0x2000;
pub const EXT4_FEATURE_INCOMPAT_LARGEDIR: u32 = 0x4000;
pub const EXT4_FEATURE_INCOMPAT_INLINE_DATA: u32 = 0x8000;
pub const EXT4_FEATURE_INCOMPAT_ENCRYPT: u32 = 0x10000;

/// Features we require (must be present).
pub const EXT4_REQUIRED_INCOMPAT: u32 = EXT4_FEATURE_INCOMPAT_FILETYPE
    | EXT4_FEATURE_INCOMPAT_EXTENTS;

/// Features we cannot handle (must NOT be present).
pub const EXT4_UNSUPPORTED_INCOMPAT: u32 = EXT4_FEATURE_INCOMPAT_COMPRESSION
    | EXT4_FEATURE_INCOMPAT_JOURNAL_DEV
    | EXT4_FEATURE_INCOMPAT_META_BG
    | EXT4_FEATURE_INCOMPAT_MMP
    | EXT4_FEATURE_INCOMPAT_EA_INODE
    | EXT4_FEATURE_INCOMPAT_DIRDATA
    | EXT4_FEATURE_INCOMPAT_CSUM_SEED
    | EXT4_FEATURE_INCOMPAT_LARGEDIR
    | EXT4_FEATURE_INCOMPAT_INLINE_DATA
    | EXT4_FEATURE_INCOMPAT_ENCRYPT;

// ─── Feature flags: RO_COMPAT ────────────────────────────────────────

pub const EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
pub const EXT4_FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
pub const EXT4_FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;
pub const EXT4_FEATURE_RO_COMPAT_HUGE_FILE: u32 = 0x0008;
pub const EXT4_FEATURE_RO_COMPAT_GDT_CSUM: u32 = 0x0010;
pub const EXT4_FEATURE_RO_COMPAT_DIR_NLINK: u32 = 0x0020;
pub const EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE: u32 = 0x0040;
pub const EXT4_FEATURE_RO_COMPAT_QUOTA: u32 = 0x0100;
pub const EXT4_FEATURE_RO_COMPAT_BIGALLOC: u32 = 0x0200;
pub const EXT4_FEATURE_RO_COMPAT_METADATA_CSUM: u32 = 0x0400;
pub const EXT4_FEATURE_RO_COMPAT_READONLY: u32 = 0x0800;
pub const EXT4_FEATURE_RO_COMPAT_PROJECT: u32 = 0x2000;

/// Read-only compat features we support.
pub const EXT4_SUPPORTED_RO_COMPAT: u32 = EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER
    | EXT4_FEATURE_RO_COMPAT_LARGE_FILE
    | EXT4_FEATURE_RO_COMPAT_HUGE_FILE
    | EXT4_FEATURE_RO_COMPAT_GDT_CSUM
    | EXT4_FEATURE_RO_COMPAT_DIR_NLINK
    | EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE
    | EXT4_FEATURE_RO_COMPAT_QUOTA
    | EXT4_FEATURE_RO_COMPAT_METADATA_CSUM
    | EXT4_FEATURE_RO_COMPAT_PROJECT;

pub const EXT4_UNSUPPORTED_RO_COMPAT: u32 = EXT4_FEATURE_RO_COMPAT_BTREE_DIR
    | EXT4_FEATURE_RO_COMPAT_BIGALLOC
    | EXT4_FEATURE_RO_COMPAT_READONLY;

// ─── Superblock struct ───────────────────────────────────────────────

/// Parsed ext4 superblock (on-disk format at offset 1024).
/// Only the fields needed for read-only access are included.
#[derive(Clone, Debug)]
pub struct Ext4Superblock {
    pub s_inodes_count: u32,
    pub s_blocks_count_lo: u32,
    pub s_free_blocks_count_lo: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_cluster_size: u32,
    pub s_blocks_per_group: u32,
    pub s_clusters_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    pub s_volume_name: [u8; 16],
    pub s_last_mounted: [u8; 64],
    pub s_algorithm_usage_bitmap: u32,
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub s_reserved_gdt_blocks: u16,
    pub s_journal_uuid: [u8; 16],
    pub s_journal_inum: u32,
    pub s_journal_dev: u32,
    pub s_last_orphan: u32,
    pub s_hash_seed: [u32; 4],
    pub s_def_hash_version: u8,
    pub s_jnl_backup_type: u8,
    pub s_desc_size: u16,
    pub s_default_mount_opts: u32,
    pub s_first_meta_bg: u32,
    pub s_mkfs_time: u32,
    pub s_jnl_blocks: [u32; 17],
    pub s_blocks_count_hi: u32,
    pub s_inodes_count_hi: u32,
    pub s_free_blocks_count_hi: u32,
    pub s_free_inodes_count_hi: u32,
    pub s_minor_extra_isize: u16,
    pub s_want_extra_isize: u16,
    pub s_flags: u32,
    pub s_raid_stride: u16,
    pub s_mmp_interval: u16,
    pub s_mmp_block: u64,
    pub s_raid_stripe_width: u32,
    pub s_log_groups_per_flex: u8,
    pub s_checksum_type: u8,
    pub s_reserved_pad: u16,
    pub s_kbytes_written: u64,
    pub s_snapshot_inum: u32,
    pub s_snapshot_id: u32,
    pub s_snapshot_r_blocks: u64,
    pub s_snapshot_list: u32,
    pub s_error_count: u32,
    pub s_first_error_time: u32,
    pub s_first_error_ino: u32,
    pub s_first_error_block: u64,
    pub s_first_error_func: [u8; 32],
    pub s_first_error_line: u32,
    pub s_last_error_time: u32,
    pub s_last_error_ino: u32,
    pub s_last_error_line: u32,
    pub s_last_error_block: u64,
    pub s_last_error_func: [u8; 32],
    pub s_mount_opts: [u8; 64],
    pub s_usr_quota_inum: u32,
    pub s_grp_quota_inum: u32,
    pub s_overhead_blocks: u32,
    pub s_backup_bgs: [u32; 2],
    pub s_encrypt_algos: [u8; 4],
    pub s_encrypt_pw_salt: [u8; 16],
    pub s_lpf_ino: u32,
    pub s_prj_quota_inum: u32,
    pub s_checksum_seed: u32,
    pub s_wtime_hi: u8,
    pub s_mtime_hi: u8,
    pub s_mkfs_time_hi: u8,
    pub s_lastcheck_hi: u8,
    pub s_first_error_time_hi: u8,
    pub s_last_error_time_hi: u8,
    pub s_pad: [u8; 2],
    pub s_checksum: u32,
}

impl Ext4Superblock {
    /// Block size in bytes = 1024 << s_log_block_size.
    pub fn block_size(&self) -> usize {
        (1024usize) << self.s_log_block_size
    }

    /// Cluster size in bytes = 1024 << s_log_cluster_size.
    pub fn cluster_size(&self) -> usize {
        (1024usize) << self.s_log_cluster_size
    }

    /// Total number of blocks (64-bit).
    pub fn blocks_count(&self) -> u64 {
        (self.s_blocks_count_lo as u64) | ((self.s_blocks_count_hi as u64) << 32)
    }

    /// Total number of inodes (64-bit).
    pub fn inodes_count(&self) -> u64 {
        (self.s_inodes_count as u64) | ((self.s_inodes_count_hi as u64) << 32)
    }

    /// Number of block groups.
    pub fn block_groups_count(&self) -> u32 {
        let blocks = self.blocks_count();
        let bg = blocks / self.s_blocks_per_group as u64;
        if blocks % self.s_blocks_per_group as u64 != 0 {
            (bg + 1) as u32
        } else {
            bg as u32
        }
    }

    /// Size of group descriptor in bytes (at least 32, typically 64 for 64-bit).
    pub fn desc_size(&self) -> usize {
        if self.s_desc_size != 0 {
            self.s_desc_size as usize
        } else {
            32
        }
    }

    /// Whether the INCOMPAT_EXTENTS feature is enabled.
    pub fn has_extents(&self) -> bool {
        self.s_feature_incompat & EXT4_FEATURE_INCOMPAT_EXTENTS != 0
    }

    /// Whether the INCOMPAT_64BIT feature is enabled.
    pub fn has_64bit(&self) -> bool {
        self.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0
    }

    /// Whether the INCOMPAT_FLEX_BG feature is enabled.
    pub fn has_flex_bg(&self) -> bool {
        self.s_feature_incompat & EXT4_FEATURE_INCOMPAT_FLEX_BG != 0
    }

    /// Whether the RO_COMPAT_SPARSE_SUPER feature is enabled.
    pub fn has_sparse_super(&self) -> bool {
        self.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER != 0
    }

    /// Whether the RO_COMPAT_METADATA_CSUM feature is enabled.
    pub fn has_metadata_csum(&self) -> bool {
        self.s_feature_ro_compat & EXT4_FEATURE_RO_COMPAT_METADATA_CSUM != 0
    }

    /// Number of block groups in a flex_bg group.
    pub fn flex_bg_size(&self) -> u32 {
        if self.has_flex_bg() && self.s_log_groups_per_flex != 0 {
            1u32 << self.s_log_groups_per_flex
        } else {
            1
        }
    }

    /// Inode size in bytes (128 or 256).
    pub fn inode_size(&self) -> usize {
        if self.s_rev_level == 0 {
            EXT4_GOOD_OLD_INODE_SIZE as usize
        } else {
            self.s_inode_size as usize
        }
    }
}

impl fmt::Display for Ext4Superblock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ext4 superblock:")?;
        writeln!(f, "  Blocks: {}", self.blocks_count())?;
        writeln!(f, "  Inodes: {}", self.inodes_count())?;
        writeln!(f, "  Block size: {}", self.block_size())?;
        writeln!(f, "  Block groups: {}", self.block_groups_count())?;
        writeln!(f, "  Blocks per group: {}", self.s_blocks_per_group)?;
        writeln!(f, "  Inodes per group: {}", self.s_inodes_per_group)?;
        writeln!(f, "  Inode size: {}", self.inode_size())?;
        writeln!(f, "  Volume: {:?}", self.volume_name_str())?;
        writeln!(f, "  UUID: {:02x?}", self.s_uuid)?;
        writeln!(f, "  INCOMPAT features: 0x{:08x}", self.s_feature_incompat)?;
        writeln!(f, "  RO_COMPAT features: 0x{:08x}", self.s_feature_ro_compat)?;
        write!(f, "  State: {}", if self.s_state == 1 { "clean" } else { "error" })
    }
}

impl Ext4Superblock {
    fn volume_name_str(&self) -> &str {
        let end = self.s_volume_name.iter().position(|&b| b == 0).unwrap_or(16);
        core::str::from_utf8(&self.s_volume_name[..end]).unwrap_or("")
    }
}

// ─── Group descriptor ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Ext4GroupDesc {
    pub bg_block_bitmap_lo: u32,
    pub bg_inode_bitmap_lo: u32,
    pub bg_inode_table_lo: u32,
    pub bg_free_blocks_count_lo: u16,
    pub bg_free_inodes_count_lo: u16,
    pub bg_used_dirs_count_lo: u16,
    pub bg_flags: u16,
    pub bg_exclude_bitmap_lo: u32,
    pub bg_block_bitmap_csum_lo: u16,
    pub bg_inode_bitmap_csum_lo: u16,
    pub bg_itable_unused_lo: u16,
    pub bg_checksum: u16,
    // 64-bit fields (only valid if INCOMPAT_64BIT)
    pub bg_block_bitmap_hi: u32,
    pub bg_inode_bitmap_hi: u32,
    pub bg_inode_table_hi: u32,
    pub bg_free_blocks_count_hi: u16,
    pub bg_free_inodes_count_hi: u16,
    pub bg_used_dirs_count_hi: u16,
    pub bg_itable_unused_hi: u16,
    pub bg_exclude_bitmap_hi: u32,
    pub bg_block_bitmap_csum_hi: u16,
    pub bg_inode_bitmap_csum_hi: u16,
    pub bg_reserved: u32,
}

impl Ext4GroupDesc {
    pub fn block_bitmap(&self, sb: &Ext4Superblock) -> u64 {
        let lo = self.bg_block_bitmap_lo as u64;
        let hi = if sb.has_64bit() { self.bg_block_bitmap_hi as u64 } else { 0 };
        lo | (hi << 32)
    }

    pub fn inode_bitmap(&self, sb: &Ext4Superblock) -> u64 {
        let lo = self.bg_inode_bitmap_lo as u64;
        let hi = if sb.has_64bit() { self.bg_inode_bitmap_hi as u64 } else { 0 };
        lo | (hi << 32)
    }

    pub fn inode_table(&self, sb: &Ext4Superblock) -> u64 {
        let lo = self.bg_inode_table_lo as u64;
        let hi = if sb.has_64bit() { self.bg_inode_table_hi as u64 } else { 0 };
        lo | (hi << 32)
    }
}

// ─── Extent tree ─────────────────────────────────────────────────────

pub const EXT4_EXTENT_MAGIC: u16 = 0xF30A;

/// Extent tree header.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Ext4ExtentHeader {
    pub eh_magic: u16,
    pub eh_entries: u16,
    pub eh_max: u16,
    pub eh_depth: u16,
    pub eh_generation: u32,
}

/// Extent tree leaf node.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Ext4Extent {
    pub ee_block: u32,
    pub ee_len: u16,
    pub ee_start_hi: u16,
    pub ee_start_lo: u32,
}

impl Ext4Extent {
    pub fn start_block(&self) -> u64 {
        (self.ee_start_lo as u64) | ((self.ee_start_hi as u64) << 32)
    }

    pub fn block_count(&self) -> u32 {
        if self.ee_len & 0x8000 != 0 {
            // Initialized extent: ee_len is negative of actual length
            -(self.ee_len as i16) as u32
        } else {
            self.ee_len as u32
        }
    }

    pub fn is_uninit(&self) -> bool {
        self.ee_len & 0x8000 != 0
    }
}

/// Extent tree index (internal node).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Ext4ExtentIdx {
    pub ei_block: u32,
    pub ei_leaf_lo: u32,
    pub ei_leaf_hi: u16,
    pub ei_unused: u16,
}

impl Ext4ExtentIdx {
    pub fn leaf_block(&self) -> u64 {
        (self.ei_leaf_lo as u64) | ((self.ei_leaf_hi as u64) << 32)
    }
}

// ─── Inode ───────────────────────────────────────────────────────────

pub const EXT4_ROOT_INO: u32 = 2;
pub const EXT4_GOOD_OLD_FIRST_INO: u32 = 11;

/// Inode types/modes.
pub const EXT4_S_IFMT: u16 = 0xF000;
pub const EXT4_S_IFSOCK: u16 = 0xC000;
pub const EXT4_S_IFLNK: u16 = 0xA000;
pub const EXT4_S_IFREG: u16 = 0x8000;
pub const EXT4_S_IFBLK: u16 = 0x6000;
pub const EXT4_S_IFDIR: u16 = 0x4000;
pub const EXT4_S_IFCHR: u16 = 0x2000;
pub const EXT4_S_IFIFO: u16 = 0x1000;

pub const EXT4_FT_UNKNOWN: u8 = 0;
pub const EXT4_FT_REG_FILE: u8 = 1;
pub const EXT4_FT_DIR: u8 = 2;
pub const EXT4_FT_CHRDEV: u8 = 3;
pub const EXT4_FT_BLKDEV: u8 = 4;
pub const EXT4_FT_FIFO: u8 = 5;
pub const EXT4_FT_SOCK: u8 = 6;
pub const EXT4_FT_SYMLINK: u8 = 7;

/// Size of the 12 direct + 1 indirect + 1 double indirect + 1 triple indirect block pointers.
/// For ext4 with extents, i_block[0..3] holds the extent tree root.
pub const EXT4_N_BLOCKS: usize = 15;

/// Inode flags.
pub const EXT4_EXTENTS_FL: u32 = 0x00080000;
pub const EXT4_INLINE_DATA_FL: u32 = 0x10000000;
pub const EXT4_PROJINHERIT_FL: u32 = 0x20000000;

/// Parsed ext4 inode (256-byte on-disk).
#[derive(Clone, Debug)]
pub struct Ext4Inode {
    pub i_mode: u16,
    pub i_uid: u16,
    pub i_size_lo: u32,
    pub i_atime: u32,
    pub i_ctime: u32,
    pub i_mtime: u32,
    pub i_dtime: u32,
    pub i_gid: u16,
    pub i_links_count: u16,
    pub i_blocks_lo: u32,
    pub i_flags: u32,
    pub i_osd1: u32,
    pub i_block: [u8; 60],     // 60 bytes = 15 × 4-byte block pointers (or extent tree root)
    pub i_generation: u32,
    pub i_file_acl_lo: u32,
    pub i_size_hi: u32,
    pub i_obso_faddr: u32,
    pub i_osd2: [u8; 12],
    pub i_blocks_hi: u32,
    pub i_extra_isize: u16,
    pub i_checksum_hi: u16,
    pub i_ctime_extra: u32,
    pub i_mtime_extra: u32,
    pub i_atime_extra: u32,
    pub i_crtime: u32,
    pub i_crtime_extra: u32,
    pub i_version_hi: u32,
    pub i_projid: u32,
}

impl Ext4Inode {
    pub fn file_size(&self) -> u64 {
        (self.i_size_lo as u64) | ((self.i_size_hi as u64) << 32)
    }

    pub fn is_dir(&self) -> bool {
        self.i_mode & EXT4_S_IFMT == EXT4_S_IFDIR
    }

    pub fn is_reg(&self) -> bool {
        self.i_mode & EXT4_S_IFMT == EXT4_S_IFREG
    }

    pub fn is_lnk(&self) -> bool {
        self.i_mode & EXT4_S_IFMT == EXT4_S_IFLNK
    }

    pub fn has_extents(&self) -> bool {
        self.i_flags & EXT4_EXTENTS_FL != 0
    }

    /// Total number of 512-byte blocks allocated to this inode (64-bit).
    pub fn blocks_count(&self) -> u64 {
        (self.i_blocks_lo as u64) | ((self.i_blocks_hi as u64) << 32)
    }

    /// Get extent header from inode i_block (first 12 bytes).
    pub fn extent_header(&self) -> Option<Ext4ExtentHeader> {
        if !self.has_extents() {
            return None;
        }
        let bytes = &self.i_block[..12];
        Some(Ext4ExtentHeader {
            eh_magic: u16::from_le_bytes([bytes[0], bytes[1]]),
            eh_entries: u16::from_le_bytes([bytes[2], bytes[3]]),
            eh_max: u16::from_le_bytes([bytes[4], bytes[5]]),
            eh_depth: u16::from_le_bytes([bytes[6], bytes[7]]),
            eh_generation: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        })
    }
}

// ─── Directory entry ─────────────────────────────────────────────────

/// Directory entry (on-disk, variable length).
#[derive(Clone, Debug)]
pub struct Ext4DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    /// Name bytes (not null-terminated on disk, length = name_len).
    pub name: [u8; 255],
}

// ─── Htree (dx) types ────────────────────────────────────────────────

/// Maximum filename length in ext4.
pub const EXT4_NAME_LEN: usize = 255;

/// Hash version for htree directory indexing.
pub const DX_HASH_LEGACY: u8 = 0;
pub const DX_HASH_HALF_MD4: u8 = 1;
pub const DX_HASH_TEA: u8 = 2;
pub const DX_HASH_LEGACY_UNSIGNED: u8 = 3;
pub const DX_HASH_HALF_MD4_UNSIGNED: u8 = 4;
pub const DX_HASH_TEA_UNSIGNED: u8 = 5;

/// Dx root info block (starts at offset 8 within the fake directory entry
/// of an htree directory root block).
#[derive(Clone, Copy, Debug)]
pub struct Ext4DxRootInfo {
    pub reserved_zero: u32,
    pub hash_version: u8,
    pub info_length: u8,       // Always 8 for version 0
    pub indirect_levels: u8,   // 0 = 2-level tree, 1 = 3-level tree
    pub unused_flags: u8,
}

/// Htree index entry (dx_entry) — maps a hash value to a directory leaf block.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Ext4DxEntry {
    pub hash: u32,
    pub block: u32,  // Logical block number within the directory
}

// ─── Error type ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Ext4Error {
    InvalidMagic,
    UnsupportedIncompat(u32),
    UnsupportedRoCompat(u32),
    InvalidBlockSize,
    InvalidInodeSize,
    ChecksumMismatch,
    IoError,
    NotFound,
    InvalidExtentHeader,
    NotAnExtentInode,
    ReadOnly,
    NoSpace,
    ExtentTreeFull,
    InvalidJournal,
    JournalCorrupt,
    InvalidXattr,
}

impl fmt::Display for Ext4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ext4Error::InvalidMagic => write!(f, "invalid ext4 superblock magic"),
            Ext4Error::UnsupportedIncompat(flags) => {
                write!(f, "unsupported INCOMPAT features: 0x{:08x}", flags)
            }
            Ext4Error::UnsupportedRoCompat(flags) => {
                write!(f, "unsupported RO_COMPAT features: 0x{:08x}", flags)
            }
            Ext4Error::InvalidBlockSize => write!(f, "invalid block size"),
            Ext4Error::InvalidInodeSize => write!(f, "invalid inode size"),
            Ext4Error::ChecksumMismatch => write!(f, "checksum mismatch"),
            Ext4Error::IoError => write!(f, "I/O error"),
            Ext4Error::NotFound => write!(f, "not found"),
            Ext4Error::InvalidExtentHeader => write!(f, "invalid extent header"),
            Ext4Error::NotAnExtentInode => write!(f, "inode does not use extents"),
            Ext4Error::ReadOnly => write!(f, "read-only filesystem"),
            Ext4Error::NoSpace => write!(f, "no space left"),
            Ext4Error::ExtentTreeFull => write!(f, "extent tree is full, index node needed"),
            Ext4Error::InvalidJournal => write!(f, "invalid journal"),
            Ext4Error::JournalCorrupt => write!(f, "corrupt journal"),
            Ext4Error::InvalidXattr => write!(f, "invalid extended attribute"),
        }
    }
}

pub type Ext4Result<T> = Result<T, Ext4Error>;
