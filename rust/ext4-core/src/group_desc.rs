//! Group descriptor table parsing.
//!
//! The group descriptor table follows the superblock and contains metadata
//! about each block group: block/inode bitmaps, inode table location, etc.

use crate::types::*;

/// Parse all group descriptors from a raw buffer.
///
/// `data` must be at least `num_groups * desc_size` bytes long.
pub fn parse_group_descriptors(
    data: &[u8],
    num_groups: u32,
    desc_size: usize,
    sb: &Ext4Superblock,
) -> Ext4Result<Vec<Ext4GroupDesc>> {
    let needed = num_groups as usize * desc_size;
    if data.len() < needed {
        return Err(Ext4Error::IoError);
    }

    let mut groups = Vec::with_capacity(num_groups as usize);
    for i in 0..num_groups as usize {
        let off = i * desc_size;
        let gd = parse_group_desc(&data[off..], desc_size, sb)?;
        groups.push(gd);
    }
    Ok(groups)
}

/// Parse a single group descriptor at the given offset.
fn parse_group_desc(data: &[u8], desc_size: usize, _sb: &Ext4Superblock) -> Ext4Result<Ext4GroupDesc> {
    if data.len() < desc_size {
        return Err(Ext4Error::IoError);
    }

    let bg_block_bitmap_lo = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let bg_inode_bitmap_lo = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let bg_inode_table_lo = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let bg_free_blocks_count_lo = u16::from_le_bytes([data[12], data[13]]);
    let bg_free_inodes_count_lo = u16::from_le_bytes([data[14], data[15]]);
    let bg_used_dirs_count_lo = u16::from_le_bytes([data[16], data[17]]);
    let bg_flags = u16::from_le_bytes([data[18], data[19]]);
    let bg_exclude_bitmap_lo = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    let bg_block_bitmap_csum_lo = u16::from_le_bytes([data[24], data[25]]);
    let bg_inode_bitmap_csum_lo = u16::from_le_bytes([data[26], data[27]]);
    let bg_itable_unused_lo = u16::from_le_bytes([data[28], data[29]]);
    let bg_checksum = u16::from_le_bytes([data[30], data[31]]);

    // 64-bit fields (if desc_size >= 64)
    let (bg_block_bitmap_hi, bg_inode_bitmap_hi, bg_inode_table_hi,
         bg_free_blocks_count_hi, bg_free_inodes_count_hi,
         bg_used_dirs_count_hi, bg_itable_unused_hi,
         bg_exclude_bitmap_hi, bg_block_bitmap_csum_hi,
         bg_inode_bitmap_csum_hi, bg_reserved) = if desc_size >= 64 {
        (
            u32::from_le_bytes([data[32], data[33], data[34], data[35]]),
            u32::from_le_bytes([data[36], data[37], data[38], data[39]]),
            u32::from_le_bytes([data[40], data[41], data[42], data[43]]),
            u16::from_le_bytes([data[44], data[45]]),
            u16::from_le_bytes([data[46], data[47]]),
            u16::from_le_bytes([data[48], data[49]]),
            u16::from_le_bytes([data[50], data[51]]),
            u32::from_le_bytes([data[52], data[53], data[54], data[55]]),
            u16::from_le_bytes([data[56], data[57]]),
            u16::from_le_bytes([data[58], data[59]]),
            u32::from_le_bytes([data[60], data[61], data[62], data[63]]),
        )
    } else {
        (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
    };

    Ok(Ext4GroupDesc {
        bg_block_bitmap_lo,
        bg_inode_bitmap_lo,
        bg_inode_table_lo,
        bg_free_blocks_count_lo,
        bg_free_inodes_count_lo,
        bg_used_dirs_count_lo,
        bg_flags,
        bg_exclude_bitmap_lo,
        bg_block_bitmap_csum_lo,
        bg_inode_bitmap_csum_lo,
        bg_itable_unused_lo,
        bg_checksum,
        bg_block_bitmap_hi,
        bg_inode_bitmap_hi,
        bg_inode_table_hi,
        bg_free_blocks_count_hi,
        bg_free_inodes_count_hi,
        bg_used_dirs_count_hi,
        bg_itable_unused_hi,
        bg_exclude_bitmap_hi,
        bg_block_bitmap_csum_hi,
        bg_inode_bitmap_csum_hi,
        bg_reserved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sb() -> Ext4Superblock {
        let mut data = vec![0u8; 1024];
        data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
        data[24..28].copy_from_slice(&(2u32).to_le_bytes());  // s_log_block_size
        data[32..36].copy_from_slice(&(32768u32).to_le_bytes()); // s_blocks_per_group
        data[40..44].copy_from_slice(&(8192u32).to_le_bytes());  // s_inodes_per_group
        data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
        data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
        data[76..80].copy_from_slice(&(1u32).to_le_bytes());   // s_rev_level
        data[88..90].copy_from_slice(&(256u16).to_le_bytes()); // s_inode_size
        data[286..288].copy_from_slice(&(64u16).to_le_bytes()); // s_desc_size
        crate::superblock::parse_superblock(&data).unwrap()
    }

    #[test]
    fn test_parse_group_descriptors() {
        let sb = make_sb();
        let mut gdt_data = vec![0u8; 128]; // 2 groups × 64 bytes
        // Group 0: inode table at block 256
        gdt_data[8..12].copy_from_slice(&(256u32).to_le_bytes());
        // Group 1: inode table at block 256 + 256*8 = 2304
        gdt_data[64 + 8..64 + 12].copy_from_slice(&(2304u32).to_le_bytes());

        let groups = parse_group_descriptors(&gdt_data, 2, 64, &sb).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].inode_table(&sb), 256);
        assert_eq!(groups[1].inode_table(&sb), 2304);
    }

    #[test]
    fn test_32bit_desc() {
        let sb = make_sb();
        let mut gdt_data = vec![0u8; 64]; // 2 groups × 32 bytes (old format)
        gdt_data[8..12].copy_from_slice(&(128u32).to_le_bytes());

        let groups = parse_group_descriptors(&gdt_data[..64], 1, 32, &sb).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].inode_table(&sb), 128);
    }
}
