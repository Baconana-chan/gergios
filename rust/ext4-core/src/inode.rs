//! Inode parsing.
//!
//! ext4 inodes are 256 bytes on disk. The inode table is an array of inodes
//! in each block group, pointed to by the group descriptor.

use crate::types::*;

/// Serialize an Ext4Inode into a raw buffer (for writing back to disk).
/// `data` must be at least `sb.inode_size()` bytes.
pub fn serialize_inode(data: &mut [u8], inode: &Ext4Inode, sb: &Ext4Superblock) {
    let isize = sb.inode_size();
    if data.len() < isize { return; }

    data[0..2].copy_from_slice(&inode.i_mode.to_le_bytes());
    data[2..4].copy_from_slice(&inode.i_uid.to_le_bytes());
    data[4..8].copy_from_slice(&inode.i_size_lo.to_le_bytes());
    data[8..12].copy_from_slice(&inode.i_atime.to_le_bytes());
    data[12..16].copy_from_slice(&inode.i_ctime.to_le_bytes());
    data[16..20].copy_from_slice(&inode.i_mtime.to_le_bytes());
    data[20..24].copy_from_slice(&inode.i_dtime.to_le_bytes());
    data[24..26].copy_from_slice(&inode.i_gid.to_le_bytes());
    data[26..28].copy_from_slice(&inode.i_links_count.to_le_bytes());
    data[28..32].copy_from_slice(&inode.i_blocks_lo.to_le_bytes());
    data[32..36].copy_from_slice(&inode.i_flags.to_le_bytes());
    data[36..40].copy_from_slice(&inode.i_osd1.to_le_bytes());
    // i_block: 60 bytes at offset 40
    data[40..100].copy_from_slice(&inode.i_block);
    data[100..104].copy_from_slice(&inode.i_generation.to_le_bytes());
    data[104..108].copy_from_slice(&inode.i_file_acl_lo.to_le_bytes());
    data[108..112].copy_from_slice(&inode.i_size_hi.to_le_bytes());
    data[112..116].copy_from_slice(&inode.i_obso_faddr.to_le_bytes());
    data[116..120].copy_from_slice(&inode.i_blocks_hi.to_le_bytes()); // i_osd2[0..4] = i_blocks_hi
    data[120..128].copy_from_slice(&inode.i_osd2[4..12]);

    // Extra inode fields (if inode size > 128)
    if isize > 128 {
        data[128..130].copy_from_slice(&inode.i_extra_isize.to_le_bytes());
        data[130..132].copy_from_slice(&inode.i_checksum_hi.to_le_bytes());
        data[132..136].copy_from_slice(&inode.i_ctime_extra.to_le_bytes());
        data[136..140].copy_from_slice(&inode.i_mtime_extra.to_le_bytes());
        data[140..144].copy_from_slice(&inode.i_atime_extra.to_le_bytes());
        data[144..148].copy_from_slice(&inode.i_crtime.to_le_bytes());
        data[148..152].copy_from_slice(&inode.i_crtime_extra.to_le_bytes());
        data[152..156].copy_from_slice(&inode.i_version_hi.to_le_bytes());
        data[156..160].copy_from_slice(&inode.i_projid.to_le_bytes());
    }
}

/// Create a new inode structure with default values.
pub fn new_inode(mode: u16, uid: u16, gid: u16) -> Ext4Inode {
    let now = 0u32; // Will be set by caller to current time
    Ext4Inode {
        i_mode: mode,
        i_uid: uid,
        i_size_lo: 0,
        i_atime: now,
        i_ctime: now,
        i_mtime: now,
        i_dtime: 0,
        i_gid: gid,
        i_links_count: 1,
        i_blocks_lo: 0,
        i_flags: EXT4_EXTENTS_FL,
        i_osd1: 0,
        i_block: [0u8; 60],
        i_generation: 0,
        i_file_acl_lo: 0,
        i_size_hi: 0,
        i_obso_faddr: 0,
        i_osd2: [0u8; 12],
        i_blocks_hi: 0,
        i_extra_isize: 32, // Default extra isize for 256-byte inodes
        i_checksum_hi: 0,
        i_ctime_extra: 0,
        i_mtime_extra: 0,
        i_atime_extra: 0,
        i_crtime: 0,
        i_crtime_extra: 0,
        i_version_hi: 0,
        i_projid: 0,
    }
}

/// Initialize an inode's extent tree header (empty, 4 max entries, depth 0).
pub fn init_extent_tree(inode: &mut Ext4Inode) {
    let header = Ext4ExtentHeader {
        eh_magic: EXT4_EXTENT_MAGIC,
        eh_entries: 0,
        eh_max: 4,
        eh_depth: 0,
        eh_generation: 0,
    };
    inode.i_block[0..2].copy_from_slice(&header.eh_magic.to_le_bytes());
    inode.i_block[2..4].copy_from_slice(&header.eh_entries.to_le_bytes());
    inode.i_block[4..6].copy_from_slice(&header.eh_max.to_le_bytes());
    inode.i_block[6..8].copy_from_slice(&header.eh_depth.to_le_bytes());
    inode.i_block[8..12].copy_from_slice(&header.eh_generation.to_le_bytes());
}

/// Set the file size in the inode.
pub fn set_file_size(inode: &mut Ext4Inode, size: u64) {
    inode.i_size_lo = size as u32;
    inode.i_size_hi = (size >> 32) as u32;
}

/// Set the block count in the inode (in 512-byte sectors, rounded up).
pub fn set_blocks_count(inode: &mut Ext4Inode, count: u64) {
    inode.i_blocks_lo = count as u32;
    inode.i_blocks_hi = (count >> 32) as u32;
}

/// Update timestamps on an inode.
pub fn update_timestamps(inode: &mut Ext4Inode, ctime: u32, mtime: u32, atime: u32) {
    inode.i_ctime = ctime;
    inode.i_mtime = mtime;
    inode.i_atime = atime;
}

/// Update timestamps with nanosecond precision.
pub fn update_timestamps_ns(inode: &mut Ext4Inode, ctime: u32, mtime: u32, atime: u32,
                             ctime_ns: u32, mtime_ns: u32, atime_ns: u32) {
    inode.i_ctime = ctime;
    inode.i_mtime = mtime;
    inode.i_atime = atime;
    // Extra fields: lower 2 bits = extra seconds, upper 30 bits = ns / 100
    // Store ns << 2 in the upper portion, preserving extra seconds in lower 2 bits
    inode.i_ctime_extra = (ctime_ns << 2) | (inode.i_ctime_extra & 0x3);
    inode.i_mtime_extra = (mtime_ns << 2) | (inode.i_mtime_extra & 0x3);
    inode.i_atime_extra = (atime_ns << 2) | (inode.i_atime_extra & 0x3);
}

/// Set the symlink target in an inode (fast symlink — target stored in i_block).
/// Fast symlinks store the target path in i_block (up to 60 bytes).
/// The inode must NOT have EXT4_EXTENTS_FL set.
pub fn set_symlink_target(inode: &mut Ext4Inode, target: &str) -> bool {
    let bytes = target.as_bytes();
    if bytes.len() > 60 {
        return false; // Target too long for fast symlink
    }
    // Clear EXT4_EXTENTS_FL since we're using i_block for symlink target
    inode.i_flags &= !EXT4_EXTENTS_FL;
    // Store target in i_block
    let mut i_block = [0u8; 60];
    i_block[..bytes.len()].copy_from_slice(bytes);
    inode.i_block = i_block;
    true
}

/// Get the symlink target from an inode (fast symlink).
/// Returns None if the inode is not a symlink or uses extents.
pub fn get_symlink_target(inode: &Ext4Inode) -> Option<String> {
    if !inode.is_lnk() {
        return None;
    }
    if inode.has_extents() {
        return None; // Slow symlink (stored in data blocks, read via extent_read)
    }
    let len = inode.file_size() as usize;
    if len > 60 || len == 0 {
        return None;
    }
    let bytes = &inode.i_block[..len];
    // Find null terminator if present
    let actual_len = bytes.iter().position(|&b| b == 0).unwrap_or(len);
    core::str::from_utf8(&bytes[..actual_len]).ok().map(|s| s.to_string())
}

/// Create the standard ext4 reserved inodes (used during mkfs).
pub fn new_reserved_inode(ino: u32) -> Ext4Inode {
    let mut inode = new_inode(0, 0, 0);
    inode.i_links_count = if ino <= EXT4_ROOT_INO { 2 } else { 0 };
    inode.i_mode = if ino == EXT4_ROOT_INO {
        EXT4_S_IFDIR | 0o755
    } else {
        0
    };
    inode
}

/// Parse a single inode from a raw inode table buffer.
///
/// `data` must be at least `sb.inode_size()` bytes.
pub fn parse_inode(data: &[u8], sb: &Ext4Superblock) -> Ext4Result<Ext4Inode> {
    let isize = sb.inode_size();
    if data.len() < isize {
        return Err(Ext4Error::IoError);
    }

    let i_extra_isize = if isize > EXT4_GOOD_OLD_INODE_SIZE as usize {
        u16::from_le_bytes([data[128], data[129]])
    } else {
        0
    };

    Ok(Ext4Inode {
        i_mode: u16::from_le_bytes([data[0], data[1]]),
        i_uid: u16::from_le_bytes([data[2], data[3]]),
        i_size_lo: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        i_atime: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
        i_ctime: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        i_mtime: u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
        i_dtime: u32::from_le_bytes([data[20], data[21], data[22], data[23]]),
        i_gid: u16::from_le_bytes([data[24], data[25]]),
        i_links_count: u16::from_le_bytes([data[26], data[27]]),
        i_blocks_lo: u32::from_le_bytes([data[28], data[29], data[30], data[31]]),
        i_flags: u32::from_le_bytes([data[32], data[33], data[34], data[35]]),
        i_osd1: u32::from_le_bytes([data[36], data[37], data[38], data[39]]),
        i_block: {
            let mut b = [0u8; 60];
            b.copy_from_slice(&data[40..100]);
            b
        },
        i_generation: u32::from_le_bytes([data[100], data[101], data[102], data[103]]),
        i_file_acl_lo: u32::from_le_bytes([data[104], data[105], data[106], data[107]]),
        i_size_hi: u32::from_le_bytes([data[108], data[109], data[110], data[111]]),
        i_obso_faddr: u32::from_le_bytes([data[112], data[113], data[114], data[115]]),
        i_osd2: {
            let mut o = [0u8; 12];
            o.copy_from_slice(&data[116..128]);
            o
        },
        i_blocks_hi: u32::from_le_bytes([data[116], data[117], data[118], data[119]]),
        i_extra_isize,
        i_checksum_hi: if i_extra_isize >= 4 {
            u16::from_le_bytes([data[130], data[131]])
        } else {
            0
        },
        i_ctime_extra: if i_extra_isize >= 8 {
            u32::from_le_bytes([data[132], data[133], data[134], data[135]])
        } else {
            0
        },
        i_mtime_extra: if i_extra_isize >= 12 {
            u32::from_le_bytes([data[136], data[137], data[138], data[139]])
        } else {
            0
        },
        i_atime_extra: if i_extra_isize >= 16 {
            u32::from_le_bytes([data[140], data[141], data[142], data[143]])
        } else {
            0
        },
        i_crtime: if i_extra_isize >= 20 {
            u32::from_le_bytes([data[144], data[145], data[146], data[147]])
        } else {
            0
        },
        i_crtime_extra: if i_extra_isize >= 24 {
            u32::from_le_bytes([data[148], data[149], data[150], data[151]])
        } else {
            0
        },
        i_version_hi: if i_extra_isize >= 28 {
            u32::from_le_bytes([data[152], data[153], data[154], data[155]])
        } else {
            0
        },
        i_projid: if i_extra_isize >= 32 {
            u32::from_le_bytes([data[156], data[157], data[158], data[159]])
        } else {
            0
        },
    })
}

/// Compute the block group number for a given inode number.
pub fn inode_to_group(ino: u32, sb: &Ext4Superblock) -> u32 {
    (ino - 1) / sb.s_inodes_per_group
}

/// Compute the index within the inode table for a given inode number.
pub fn inode_to_group_index(ino: u32, sb: &Ext4Superblock) -> u32 {
    (ino - 1) % sb.s_inodes_per_group
}

/// Increment the link count on an inode (link/unlink/rename operation).
pub fn inode_link(inode: &mut Ext4Inode) {
    inode.i_links_count = inode.i_links_count.saturating_add(1);
}

/// Decrement the link count on an inode.
/// Returns `true` if the inode should be freed (links_count reached 0).
pub fn inode_unlink(inode: &mut Ext4Inode) -> bool {
    if inode.i_links_count > 0 {
        inode.i_links_count -= 1;
    }
    inode.i_links_count == 0
}

/// Mark an inode as deleted (set dtime, clear mode/flags).
pub fn mark_inode_deleted(inode: &mut Ext4Inode, delete_time: u32) {
    inode.i_dtime = delete_time;
    inode.i_links_count = 0;
    inode.i_mode = 0;
    inode.i_flags = 0;
}

/// Free all data blocks allocated to an inode, then free the inode itself.
///
/// This is the complete cleanup for an unlinked inode (links_count == 0):
/// 1. Truncate the extent tree to 0 — frees all data blocks via `free_blocks_cb`
/// 2. Clear the inode in the bitmap via `free_inode_cb`
/// 3. Mark the in-memory inode as deleted (`mark_inode_deleted`)
///
/// **Note**: The caller is responsible for persisting the freed inode to the
/// inode table block. After calling this function:
/// 1. The `extent_truncate` result is in-memory only (pass `|_| Ok(())` callback —
///    the truncated inode doesn't need persisting since it will be zeroed out)
/// 2. Use `serialize_inode()` + write the inode table block to zero the freed inode
/// 3. The inode bitmap has been updated by `free_inode_cb`
pub fn free_inode_data<FF, FI>(
    sb: &Ext4Superblock,
    inode: &mut Ext4Inode,
    ino: u32,
    mut free_blocks_cb: FF,
    mut free_inode_cb: FI,
) -> Ext4Result<()>
where
    FF: FnMut(u64, u32) -> Ext4Result<()>,
    FI: FnMut(u32) -> Ext4Result<()>,
{
    // Step 1: Truncate extent tree to free all data blocks
    // We pass a no-op write_inode since we're going to zero the inode table entry anyway.
    if inode.has_extents() {
        crate::extent::extent_truncate(
            sb, inode, 0, &mut free_blocks_cb, &mut |_| Ok(()),
        )?;
    }

    // Step 2: Clear the inode bitmap
    free_inode_cb(ino)?;

    // Step 3: Mark in-memory inode as deleted
    mark_inode_deleted(inode, 0);

    Ok(())
}

/// Compute the byte offset of an inode within its inode table block.
pub fn inode_table_offset(ino: u32, sb: &Ext4Superblock) -> u64 {
    let group = inode_to_group(ino, sb);
    let index = inode_to_group_index(ino, sb);
    (group as u64 * sb.s_inodes_per_group as u64 + index as u64) * sb.inode_size() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sb() -> Ext4Superblock {
        let mut data = vec![0u8; 1024];
        data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
        data[24..28].copy_from_slice(&(2u32).to_le_bytes());  // s_log_block_size
        data[40..44].copy_from_slice(&(8192u32).to_le_bytes()); // s_inodes_per_group
        data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
        data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
        data[76..80].copy_from_slice(&(1u32).to_le_bytes());   // s_rev_level
        data[88..90].copy_from_slice(&(256u16).to_le_bytes()); // s_inode_size
        data[84..88].copy_from_slice(&(11u32).to_le_bytes());  // s_first_ino
        crate::superblock::parse_superblock(&data).unwrap()
    }

    #[test]
    fn test_parse_inode() {
        let sb = make_sb();
        let mut raw = vec![0u8; 256];

        // mode = directory + 0755
        raw[0..2].copy_from_slice(&(EXT4_S_IFDIR | 0o755).to_le_bytes());
        // uid = 1000
        raw[2..4].copy_from_slice(&(1000u16).to_le_bytes());
        // size = 4096
        raw[4..8].copy_from_slice(&(4096u32).to_le_bytes());
        // links_count = 2
        raw[26..28].copy_from_slice(&(2u16).to_le_bytes());

        let inode = parse_inode(&raw, &sb).unwrap();
        assert!(inode.is_dir());
        assert_eq!(inode.file_size(), 4096);
        assert_eq!(inode.i_uid, 1000);
        assert_eq!(inode.i_links_count, 2);
    }

    #[test]
    fn test_inode_group_mapping() {
        let sb = make_sb(); // 8192 inodes per group
        assert_eq!(inode_to_group(1, &sb), 0);  // inode 1 = group 0
        assert_eq!(inode_to_group(8192, &sb), 0); // inode 8192 = group 0 (1-indexed)
        assert_eq!(inode_to_group(8193, &sb), 1); // inode 8193 = group 1
        assert_eq!(inode_to_group_index(2, &sb), 1); // inode 2 = index 1
        assert_eq!(inode_to_group_index(1, &sb), 0);
    }
}
