//! mkfs.ext4 — Create an empty ext4 filesystem on a block device.
//!
//! Usage:
//!   mkfs.ext4 [-b block_size] [-i bytes_per_inode] [-L label] <device>
//!
//! Creates a single-block-group ext4 filesystem with:
//!   - 4096-byte blocks (default)
//!   - Extents + filetype + flex_bg + sparse_super features
//!   - Root directory (inode 2) with `.` and `..`
//!   - Lost+found directory (inode 11)
//!
//! Examples:
//!   mkfs.ext4 /dev/c0d0p0s0
//!   mkfs.ext4 -b 1024 /dev/ram0
//!   mkfs.ext4 -L "GergiOS-Root" /dev/vnd0

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ext4_core::*;

// ─── Defaults ────────────────────────────────────────────────────────

const DEFAULT_BLOCK_SIZE: u32 = 4096;
const DEFAULT_BYTES_PER_INODE: u32 = 16384; // One inode per 16KB
const DEFAULT_LABEL: &[u8] = b"GergiOS";

// ─── Help ────────────────────────────────────────────────────────────

fn usage() -> ! {
    eprintln!("Usage: mkfs.ext4 [-b block_size] [-i bytes_per_inode] [-L label] <device>");
    eprintln!();
    eprintln!("Create an empty ext4 filesystem on <device>.");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -b <size>    Block size in bytes (1024, 2048, 4096; default: 4096)");
    eprintln!("  -i <bytes>   Bytes per inode (default: 16384)");
    eprintln!("  -L <label>   Volume label (max 16 chars)");
    eprintln!("  -h           Show this help");
    std::process::exit(1);
}

// ─── Main ────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut block_size = DEFAULT_BLOCK_SIZE;
    let mut bytes_per_inode = DEFAULT_BYTES_PER_INODE;
    let mut label = DEFAULT_LABEL.to_vec();
    let mut device_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-b" => {
                i += 1;
                block_size = args[i].parse::<u32>()
                    .unwrap_or_else(|_| { eprintln!("Invalid block size"); usage() });
                if block_size != 1024 && block_size != 2048 && block_size != 4096 {
                    eprintln!("Block size must be 1024, 2048, or 4096");
                    std::process::exit(1);
                }
            }
            "-i" => {
                i += 1;
                bytes_per_inode = args[i].parse::<u32>()
                    .unwrap_or_else(|_| { eprintln!("Invalid bytes_per_inode"); usage() });
                if bytes_per_inode < 1024 {
                    eprintln!("bytes_per_inode must be >= 1024");
                    std::process::exit(1);
                }
            }
            "-L" => {
                i += 1;
                let mut lbl = args[i].as_bytes().to_vec();
                lbl.truncate(16);
                label = lbl;
            }
            "-h" => usage(),
            _ if device_path.is_none() => device_path = Some(args[i].clone()),
            _ => { eprintln!("Unexpected argument: {}", args[i]); usage() }
        }
        i += 1;
    }

    let dev = device_path.unwrap_or_else(|| {
        eprintln!("No device specified");
        usage()
    });

    // ─── Open device ─────────────────────────────────────────────
    let path = Path::new(&dev);
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap_or_else(|e| {
            eprintln!("Cannot open {}: {}", dev, e);
            std::process::exit(1);
        });

    // Determine device size
    let dev_size = file.seek(SeekFrom::End(0)).unwrap_or_else(|e| {
        eprintln!("Cannot seek on {}: {}", dev, e);
        std::process::exit(1);
    });
    file.seek(SeekFrom::Start(0)).unwrap();

    eprintln!("mkfs.ext4: Creating ext4 filesystem on {} ({})", dev, format_size(dev_size));
    eprintln!("  Block size: {}", block_size);
    eprintln!("  Device size: {}", format_size(dev_size));

    // ─── Compute geometry ───────────────────────────────────────
    let log_block_size = (block_size.trailing_zeros() - 10); // 1024=0, 2048=1, 4096=2
    let blocks_per_group = 32768u32;
    // Minimal mkfs: only initialize 1 block group. Clamp size for larger devices.
    let max_size = blocks_per_group as u64 * block_size as u64;
    if dev_size > max_size {
        eprintln!("WARNING: mkfs.ext4 (minimal) supports only 1 block group ({})",
                  format_size(max_size));
        eprintln!("  Device size {} exceeds limit. Only first {} will be initialized.",
                  format_size(dev_size), format_size(max_size));
    }
    let blocks_count = blocks_per_group.min((dev_size / block_size as u64) as u32);
    let groups_count = 1u32;

    let inodes_per_group = (blocks_count * block_size / bytes_per_inode).max(16);

    // Estimate overhead: SB(1) + GDT(ceil(groups*64/bs)) + bitmap(2 per group) + inode_table
    let gdt_blocks = ((groups_count as u64 * 64) + block_size as u64 - 1) / block_size as u64;
    let bitmap_blocks = 2u64; // block bitmap + inode bitmap (per group)
    let inode_table_blocks = (inodes_per_group as u64 * 256 + block_size as u64 - 1) / block_size as u64;
    let first_data_block = 1u32; // block 0 = SB + padding, block 1 = GDT start

    // For single group, first data block after all metadata
    let data_start_block = first_data_block + gdt_blocks as u32 + bitmap_blocks as u32 + inode_table_blocks as u32;

    eprintln!("  Groups: {}", groups_count);
    eprintln!("  Blocks per group: {}", blocks_per_group);
    eprintln!("  Inodes per group: {}", inodes_per_group);
    eprintln!("  First data block: {}", data_start_block);

    // ─── Generate UUID ──────────────────────────────────────────
    // Simple time-based UUID (good enough for mkfs)
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let uuid = simple_uuid(now.as_secs());

    // ─── Build superblock ────────────────────────────────────────
    let now32 = now.as_secs() as u32;
    let sb = Ext4Superblock {
        s_inodes_count: inodes_per_group * groups_count,
        s_blocks_count_lo: blocks_count,
        s_free_blocks_count_lo: blocks_count - data_start_block,
        s_free_inodes_count: inodes_per_group * groups_count - 12, // reserve 12 for system inodes
        s_first_data_block: if block_size == 1024 { 1 } else { 0 },
        s_log_block_size: log_block_size,
        s_log_cluster_size: log_block_size,
        s_blocks_per_group: blocks_per_group,
        s_clusters_per_group: blocks_per_group,
        s_inodes_per_group: inodes_per_group,
        s_mtime: 0,
        s_wtime: now32,
        s_mnt_count: 0,
        s_max_mnt_count: 65535, // Disable forced fsck
        s_magic: EXT4_SUPER_MAGIC,
        s_state: 1, // Clean
        s_errors: 1, // Continue on error
        s_minor_rev_level: 0,
        s_lastcheck: now32,
        s_checkinterval: 0, // No time-based check
        s_creator_os: 0, // Linux
        s_rev_level: 1, // Dynamic inode size
        s_def_resuid: 0,
        s_def_resgid: 0,
        s_first_ino: EXT4_GOOD_OLD_FIRST_INO,
        s_inode_size: 256,
        s_block_group_nr: 0,
        s_feature_compat: 0,
        s_feature_incompat: EXT4_FEATURE_INCOMPAT_FILETYPE
            | EXT4_FEATURE_INCOMPAT_EXTENTS
            | EXT4_FEATURE_INCOMPAT_FLEX_BG,
        s_feature_ro_compat: EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER
            | EXT4_FEATURE_RO_COMPAT_LARGE_FILE
            | EXT4_FEATURE_RO_COMPAT_GDT_CSUM
            | EXT4_FEATURE_RO_COMPAT_DIR_NLINK
            | EXT4_FEATURE_RO_COMPAT_EXTRA_ISIZE,
        s_uuid: uuid,
        s_volume_name: {
            let mut v = [0u8; 16];
            let len = label.len().min(16);
            v[..len].copy_from_slice(&label[..len]);
            v
        },
        s_last_mounted: [0u8; 64],
        s_algorithm_usage_bitmap: 0,
        s_prealloc_blocks: 0,
        s_prealloc_dir_blocks: 0,
        s_reserved_gdt_blocks: 0,
        s_journal_uuid: [0u8; 16],
        s_journal_inum: 0,
        s_journal_dev: 0,
        s_last_orphan: 0,
        s_hash_seed: [now.as_nanos() as u32, now.as_secs() as u32, !(now.as_nanos() as u32), !(now.as_secs() as u32)],
        s_def_hash_version: 2, // TEA
        s_jnl_backup_type: 0,
        s_desc_size: 64,
        s_default_mount_opts: 0,
        s_first_meta_bg: 0,
        s_mkfs_time: now32,
        s_jnl_blocks: [0u32; 17],
        s_blocks_count_hi: 0,
        s_inodes_count_hi: 0,
        s_free_blocks_count_hi: 0,
        s_free_inodes_count_hi: 0,
        s_minor_extra_isize: 32,
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

    // ─── Write superblock at offset 1024 ────────────────────────
    let mut sb_buf = vec![0u8; 1024];
    serialize_superblock(&sb, &mut sb_buf);
    file.seek(SeekFrom::Start(1024)).unwrap();
    file.write_all(&sb_buf).unwrap();

    // Zero out block 0 (first 1024 bytes = boot code, SB at 1024)
    file.seek(SeekFrom::Start(0)).unwrap();
    let zero_block = vec![0u8; block_size as usize];
    file.write_all(&zero_block).unwrap();

    // ─── Build group descriptor ─────────────────────────────────
    // Block allocation:
    //   block 0: boot code + SB backup + padding
    //   blocks 1..(1+GDT): group descriptor table
    //   block (1+GDT): block bitmap
    //   block (2+GDT): inode bitmap
    //   blocks (3+GDT)..(3+GDT+inode_table_blocks-1): inode table
    //   remaining: data blocks
    let gdt_start_block = first_data_block;
    let bitmap_block = gdt_start_block + gdt_blocks as u32;
    let inode_bitmap_block = bitmap_block + 1;
    let inode_table_block = inode_bitmap_block + 1;

    let gd = Ext4GroupDesc {
        bg_block_bitmap_lo: bitmap_block,
        bg_inode_bitmap_lo: inode_bitmap_block,
        bg_inode_table_lo: inode_table_block,
        bg_free_blocks_count_lo: (blocks_count - data_start_block) as u16,
        bg_free_inodes_count_lo: (inodes_per_group - 12) as u16,
        bg_used_dirs_count_lo: 2, // root dir + lost+found
        bg_flags: 0,
        bg_exclude_bitmap_lo: 0,
        bg_block_bitmap_csum_lo: 0,
        bg_inode_bitmap_csum_lo: 0,
        bg_itable_unused_lo: (inodes_per_group - 12) as u16,
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

    // ─── Write group descriptor table ──────────────────────────
    let gdt_size = 64usize;
    let gdt_buf_size = gdt_blocks as usize * block_size as usize;
    let mut gdt_buf = vec![0u8; gdt_buf_size];
    serialize_group_descriptors(&[gd.clone()], &mut gdt_buf, gdt_size);
    file.seek(SeekFrom::Start(gdt_start_block as u64 * block_size as u64)).unwrap();
    file.write_all(&gdt_buf).unwrap();

    // ─── Write block bitmap ────────────────────────────────────
    // Mark all metadata blocks as used, data blocks as free
    let mut block_bm = vec![0u8; block_size as usize];
    for b in 0..data_start_block {
        set_bitmap_bit(&mut block_bm, b as usize);
    }
    file.seek(SeekFrom::Start(bitmap_block as u64 * block_size as u64)).unwrap();
    file.write_all(&block_bm).unwrap();

    // ─── Write inode bitmap ────────────────────────────────────
    // Mark first 12 inodes as used (0=bad, 1=root, 2=EA, 3-10=reserved, 11=lost+found)
    let mut inode_bm = vec![0u8; block_size as usize];
    for ino in 0..12 {
        set_bitmap_bit(&mut inode_bm, ino);
    }
    // Also mark system inodes 1-10  (EXT4_GOOD_OLD_FIRST_INO = 11)
    file.seek(SeekFrom::Start(inode_bitmap_block as u64 * block_size as u64)).unwrap();
    file.write_all(&inode_bm).unwrap();

    // ─── Initialize inode table ────────────────────────────────
    let inode_table_size = inode_table_blocks as usize * block_size as usize;
    let mut inode_table = vec![0u8; inode_table_size];

    // Inode 0 is never used (zeroed)
    // Inode 1: bad blocks inode (unused)
    // Inode 2: root directory
    let mut root_inode = new_inode(EXT4_S_IFDIR | 0o755, 0, 0);
    root_inode.i_links_count = 2;
    root_inode.i_ctime = now32;
    root_inode.i_mtime = now32;
    root_inode.i_atime = now32;
    init_extent_tree(&mut root_inode);
    set_file_size(&mut root_inode, block_size as u64);
    // Root dir data block is at data_start_block
    // Add extent mapping: logical block 0 → physical block data_start_block
    let mut ext_header = root_inode.extent_header().unwrap();
    ext_header.eh_entries = 1;
    root_inode.i_block[0..2].copy_from_slice(&ext_header.eh_magic.to_le_bytes());
    root_inode.i_block[2..4].copy_from_slice(&ext_header.eh_entries.to_le_bytes());
    root_inode.i_block[4..6].copy_from_slice(&ext_header.eh_max.to_le_bytes());
    root_inode.i_block[6..8].copy_from_slice(&ext_header.eh_depth.to_le_bytes());
    // Extent entry at offset 12: block=0, len=1, start=data_start_block
    root_inode.i_block[12..16].copy_from_slice(&0u32.to_le_bytes()); // ee_block = 0
    root_inode.i_block[16..18].copy_from_slice(&1u16.to_le_bytes()); // ee_len = 1
    root_inode.i_block[18..20].copy_from_slice(&((data_start_block as u64 >> 32) as u16).to_le_bytes()); // ee_start_hi
    root_inode.i_block[20..24].copy_from_slice(&data_start_block.to_le_bytes()); // ee_start_lo
    // Blocks count: 1 block * 8 (512-byte sectors)
    set_blocks_count(&mut root_inode, 8);
    let root_inode_off = 2 * 256; // inode 2, 256-byte inodes
    serialize_inode(&mut inode_table[root_inode_off..], &root_inode, &sb, Some(2));

    // Inode 3-10: reserved, zeroed
    // Inode 11: lost+found (directory)
    let mut lf_inode = new_inode(EXT4_S_IFDIR | 0o755, 0, 0);
    lf_inode.i_links_count = 2;
    lf_inode.i_ctime = now32;
    lf_inode.i_mtime = now32;
    lf_inode.i_atime = now32;
    init_extent_tree(&mut lf_inode);
    set_file_size(&mut lf_inode, block_size as u64);
    let lf_data_block = data_start_block + 1;
    let mut lf_ext_header = lf_inode.extent_header().unwrap();
    lf_ext_header.eh_entries = 1;
    lf_inode.i_block[0..2].copy_from_slice(&lf_ext_header.eh_magic.to_le_bytes());
    lf_inode.i_block[2..4].copy_from_slice(&lf_ext_header.eh_entries.to_le_bytes());
    lf_inode.i_block[4..6].copy_from_slice(&lf_ext_header.eh_max.to_le_bytes());
    lf_inode.i_block[6..8].copy_from_slice(&lf_ext_header.eh_depth.to_le_bytes());
    lf_inode.i_block[12..16].copy_from_slice(&0u32.to_le_bytes());
    lf_inode.i_block[16..18].copy_from_slice(&1u16.to_le_bytes());
    lf_inode.i_block[18..20].copy_from_slice(&((lf_data_block as u64 >> 32) as u16).to_le_bytes());
    lf_inode.i_block[20..24].copy_from_slice(&lf_data_block.to_le_bytes());
    set_blocks_count(&mut lf_inode, 8);
    let lf_inode_off = 11 * 256;
    serialize_inode(&mut inode_table[lf_inode_off..], &lf_inode, &sb, Some(11));

    file.seek(SeekFrom::Start(inode_table_block as u64 * block_size as u64)).unwrap();
    file.write_all(&inode_table).unwrap();

    // ─── Write root directory data block ────────────────────────
    let mut root_data = vec![0u8; block_size as usize];
    init_dir_block(&mut root_data, EXT4_ROOT_INO, EXT4_ROOT_INO, 0);
    file.seek(SeekFrom::Start(data_start_block as u64 * block_size as u64)).unwrap();
    file.write_all(&root_data).unwrap();

    // ─── Write lost+found directory data block ──────────────────
    let mut lf_data = vec![0u8; block_size as usize];
    init_dir_block(&mut lf_data, 11, EXT4_ROOT_INO, 0);
    file.seek(SeekFrom::Start(lf_data_block as u64 * block_size as u64)).unwrap();
    file.write_all(&lf_data).unwrap();

    eprintln!("mkfs.ext4: Done.");
    eprintln!("  Inodes: {}, Blocks: {}, Root inode: {}",
              inodes_per_group * groups_count,
              blocks_count, EXT4_ROOT_INO);
    eprintln!("WARNING: mkfs.ext4 is minimal — no journal created.");
}

// ─── Helpers ─────────────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn simple_uuid(seed: u64) -> [u8; 16] {
    let mut u = [0u8; 16];
    let md5 = simple_md5(&seed.to_le_bytes());
    u.copy_from_slice(&md5[..16]);
    // Set version 4 (random UUID)
    u[6] = (u[6] & 0x0F) | 0x40;
    // Set variant 1 (RFC 4122)
    u[8] = (u[8] & 0x3F) | 0x80;
    u
}

fn simple_md5(data: &[u8]) -> [u8; 16] {
    // MD5 for UUID generation (simplified — just use a hash)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let h = hasher.finish();
    let mut result = [0u8; 16];
    result[..8].copy_from_slice(&h.to_le_bytes());
    // Mix in complement for the second half
    result[8..16].copy_from_slice(&(!h).to_le_bytes());
    result
}

fn set_bitmap_bit(bitmap: &mut [u8], bit: usize) {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    if byte < bitmap.len() {
        bitmap[byte] |= 1 << bit_in_byte;
    }
}
