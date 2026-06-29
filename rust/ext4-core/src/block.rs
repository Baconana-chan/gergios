//! Block addressing utilities for ext4.
//!
//! Provides functions to locate block group data structures
//! (superblock backups, group descriptor tables, inode tables).

use crate::types::*;

/// Determine if a block group should have a backup superblock.
///
/// When SPARSE_SUPER is enabled, only groups 0, 1, and powers of 3, 5, 7
/// have backup superblocks.
pub fn has_superblock_backup(sb: &Ext4Superblock, group: u32) -> bool {
    if !sb.has_sparse_super() {
        return true; // All groups have backups
    }

    if group <= 1 {
        return true;
    }

    // Check powers of 3, 5, 7
    let mut n = group;
    while n % 3 == 0 { n /= 3; }
    if n == 1 { return true; }

    n = group;
    while n % 5 == 0 { n /= 5; }
    if n == 1 { return true; }

    n = group;
    while n % 7 == 0 { n /= 7; }
    if n == 1 { return true; }

    false
}

/// Calculate the byte offset of a block on the device.
pub fn block_to_byte(sb: &Ext4Superblock, block: u64) -> u64 {
    block * sb.block_size() as u64
}

/// Calculate the total number of blocks used by the group descriptor table.
pub fn gdt_blocks_count(sb: &Ext4Superblock) -> u32 {
    let groups = sb.block_groups_count();
    let desc_size = sb.desc_size() as u32;
    (groups * desc_size + sb.block_size() as u32 - 1) / sb.block_size() as u32
}

/// Read an inode from the block device.
pub fn read_inode<F>(
    sb: &Ext4Superblock,
    groups: &[Ext4GroupDesc],
    ino: u32,
    mut read_block: F,
) -> Ext4Result<Ext4Inode>
where
    F: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let group = crate::inode::inode_to_group(ino, sb) as usize;
    let index = crate::inode::inode_to_group_index(ino, sb);

    if group >= groups.len() {
        return Err(Ext4Error::NotFound);
    }

    let inode_table_block = groups[group].inode_table(sb);
    let inode_size = sb.inode_size() as u64;
    let block_size = sb.block_size() as u64;

    // Compute which block contains this inode
    let inodes_per_block = block_size / inode_size;
    let block_offset = index as u64 / inodes_per_block;
    let in_block_index = index as u64 % inodes_per_block;

    let block_nr = inode_table_block + block_offset;
    let mut buf = vec![0u8; block_size as usize];
    read_block(block_nr, &mut buf)?;

    let byte_offset = (in_block_index * inode_size) as usize;
    crate::inode::parse_inode(&buf[byte_offset..], sb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sb() -> Ext4Superblock {
        let mut data = vec![0u8; 1024];
        data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
        data[24..28].copy_from_slice(&(2u32).to_le_bytes());  // s_log_block_size
        data[32..36].copy_from_slice(&(32768u32).to_le_bytes()); // s_blocks_per_group
        data[40..44].copy_from_slice(&(8192u32).to_le_bytes()); // s_inodes_per_group
        data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
        data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
        data[76..80].copy_from_slice(&(1u32).to_le_bytes());   // s_rev_level
        data[88..90].copy_from_slice(&(256u16).to_le_bytes()); // s_inode_size
        data[286..288].copy_from_slice(&(64u16).to_le_bytes()); // s_desc_size
        crate::superblock::parse_superblock(&data).unwrap()
    }

    #[test]
    fn test_sparse_super() {
        let sb = make_sb();
        assert!(has_superblock_backup(&sb, 0));
        assert!(has_superblock_backup(&sb, 1));
        assert!(has_superblock_backup(&sb, 3));  // power of 3
        assert!(has_superblock_backup(&sb, 5));  // power of 5
        assert!(has_superblock_backup(&sb, 7));  // power of 7
        assert!(!has_superblock_backup(&sb, 2)); // not 0,1,3,5,7
        assert!(!has_superblock_backup(&sb, 4));
        assert!(!has_superblock_backup(&sb, 6));
        assert!(has_superblock_backup(&sb, 9));  // 3^2
        assert!(has_superblock_backup(&sb, 25)); // 5^2
        assert!(has_superblock_backup(&sb, 49)); // 7^2
    }
}
