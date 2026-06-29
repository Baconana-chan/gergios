//! Extent tree traversal.
//!
//! ext4 uses extent trees (a B-tree-like structure) instead of the traditional
//! indirect block maps of ext2/3. The extent tree root is stored in i_block[0..3]
//! of the inode (first 12 bytes).

use crate::types::*;

/// Traverse the extent tree to find the physical block for a given logical block.
///
/// `read_block` is a callback that reads a single filesystem block into `buf`.
/// Returns the physical block number, or `None` if the logical block is not mapped
/// (sparse file / hole).
pub fn extent_lookup<F>(
    sb: &Ext4Superblock,
    inode: &Ext4Inode,
    logical_block: u64,
    mut read_block: F,
) -> Ext4Result<Option<u64>>
where
    F: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let header = inode.extent_header()
        .ok_or(Ext4Error::NotAnExtentInode)?;

    if header.eh_magic != EXT4_EXTENT_MAGIC {
        return Err(Ext4Error::InvalidExtentHeader);
    }

    if header.eh_entries == 0 {
        return Ok(None); // Sparse file
    }

    if header.eh_depth == 0 {
        // Leaf node: extents are directly in the inode
        lookup_in_leaf(&inode.i_block[..], &header, logical_block)
    } else {
        // Index node: need to walk the tree
        let block_size = sb.block_size();
        let mut current_block: Vec<u8> = inode.i_block.to_vec();
        let mut current_header = header;
        let mut depth = header.eh_depth;

        loop {
            let idx = find_index(&current_block, &current_header, logical_block)?;
            let leaf_block = idx.leaf_block();

            // Read the next level block
            let mut buf = vec![0u8; block_size];
            read_block(leaf_block, &mut buf)?;

            let next_header = Ext4ExtentHeader {
                eh_magic: u16::from_le_bytes([buf[0], buf[1]]),
                eh_entries: u16::from_le_bytes([buf[2], buf[3]]),
                eh_max: u16::from_le_bytes([buf[4], buf[5]]),
                eh_depth: u16::from_le_bytes([buf[6], buf[7]]),
                eh_generation: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            };

            if next_header.eh_magic != EXT4_EXTENT_MAGIC {
                return Err(Ext4Error::InvalidExtentHeader);
            }

            depth -= 1;
            if depth == 0 {
                return lookup_in_leaf(&buf, &next_header, logical_block);
            }

            current_block = buf;
            current_header = next_header;
        }
    }
}

/// Find the extent containing the given logical block in a leaf node.
fn lookup_in_leaf(block: &[u8], header: &Ext4ExtentHeader, logical_block: u64) -> Ext4Result<Option<u64>> {
    let entries = header.eh_entries as usize;
    for i in 0..entries {
        let off = 12 + i * 12; // header is 12 bytes, each extent is 12 bytes
        if off + 12 > block.len() {
            break;
        }
        let extent = Ext4Extent {
            ee_block: u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]),
            ee_len: u16::from_le_bytes([block[off + 4], block[off + 5]]),
            ee_start_hi: u16::from_le_bytes([block[off + 6], block[off + 7]]),
            ee_start_lo: u32::from_le_bytes([block[off + 8], block[off + 9], block[off + 10], block[off + 11]]),
        };

        let start_block = extent.ee_block as u64;
        let end_block = start_block + extent.block_count() as u64;

        if logical_block >= start_block && logical_block < end_block {
            let phys = extent.start_block() + (logical_block - start_block);
            return Ok(Some(phys));
        }
    }
    Ok(None) // Sparse / not found
}

/// Find the index entry pointing to the subtree that should contain the logical block.
fn find_index(block: &[u8], header: &Ext4ExtentHeader, logical_block: u64) -> Ext4Result<Ext4ExtentIdx> {
    let entries = header.eh_entries as usize;
    for i in 0..entries {
        let off = 12 + i * 12;
        if off + 12 > block.len() {
            break;
        }
        let idx = Ext4ExtentIdx {
            ei_block: u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]),
            ei_leaf_lo: u32::from_le_bytes([block[off + 4], block[off + 5], block[off + 6], block[off + 7]]),
            ei_leaf_hi: u16::from_le_bytes([block[off + 8], block[off + 9]]),
            ei_unused: u16::from_le_bytes([block[off + 10], block[off + 11]]),
        };

        // Find the first index whose block > logical_block
        let next_block = if i + 1 < entries {
            let next_off = 12 + (i + 1) * 12;
            u32::from_le_bytes([block[next_off], block[next_off + 1], block[next_off + 2], block[next_off + 3]])
        } else {
            u32::MAX
        };

        if logical_block >= idx.ei_block as u64 && logical_block < next_block as u64 {
            return Ok(idx);
        }
    }
    // Fallback to last entry
    let off = 12 + (entries - 1) * 12;
    Ok(Ext4ExtentIdx {
        ei_block: u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]),
        ei_leaf_lo: u32::from_le_bytes([block[off + 4], block[off + 5], block[off + 6], block[off + 7]]),
        ei_leaf_hi: u16::from_le_bytes([block[off + 8], block[off + 9]]),
        ei_unused: u16::from_le_bytes([block[off + 10], block[off + 11]]),
    })
}

/// Read file data at a given offset using the extent tree.
///
/// Returns the number of bytes actually read.
pub fn extent_read<F>(
    sb: &Ext4Superblock,
    inode: &Ext4Inode,
    offset: u64,
    buf: &mut [u8],
    mut read_block: F,
) -> Ext4Result<usize>
where
    F: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let file_size = inode.file_size();
    if offset >= file_size {
        return Ok(0);
    }

    let block_size = sb.block_size() as u64;
    let end = core::cmp::min(offset + buf.len() as u64, file_size);
    let mut written = 0;

    let start_block = offset / block_size;
    let end_block = (end + block_size - 1) / block_size;

    for lb in start_block..end_block {
        let in_block_off = if lb == start_block { (offset % block_size) as usize } else { 0 };
        let in_block_end = if lb == end_block - 1 { ((end - 1) % block_size) as usize + 1 } else { block_size as usize };

        let phys = extent_lookup(sb, inode, lb, &mut read_block)?;

        let copy_start = written;
        let copy_len = in_block_end - in_block_off;

        if let Some(pbn) = phys {
            let mut block_buf = vec![0u8; block_size as usize];
            read_block(pbn, &mut block_buf)?;
            buf[copy_start..copy_start + copy_len]
                .copy_from_slice(&block_buf[in_block_off..in_block_end]);
        } else {
            // Sparse file: read zeros
            for i in copy_start..copy_start + copy_len {
                buf[i] = 0;
            }
        }
        written += copy_len;
    }

    Ok(written)
}

// ─── Extent tree write / modification ───────────────────────────────

/// Maximum number of extent entries in an inline extent tree (depth 0, 4 entries).
pub const EXT4_INLINE_EXTENTS_COUNT: usize = 4;

/// Maximum number of extent entries in an extent block.
pub const EXT4_EXTENT_BLOCK_ENTRIES: usize = 340; // (4096 - 12) / 12 ≈ 340

/// Serialize an extent header into a buffer at offset 0.
pub fn serialize_header(buf: &mut [u8], header: &Ext4ExtentHeader) {
    if buf.len() < 12 { return; }
    buf[0..2].copy_from_slice(&header.eh_magic.to_le_bytes());
    buf[2..4].copy_from_slice(&header.eh_entries.to_le_bytes());
    buf[4..6].copy_from_slice(&header.eh_max.to_le_bytes());
    buf[6..8].copy_from_slice(&header.eh_depth.to_le_bytes());
    buf[8..12].copy_from_slice(&header.eh_generation.to_le_bytes());
}

/// Serialize a single extent entry into a buffer at the given offset.
pub fn serialize_extent(buf: &mut [u8], off: usize, extent: &Ext4Extent) {
    if off + 12 > buf.len() { return; }
    buf[off..off + 4].copy_from_slice(&extent.ee_block.to_le_bytes());
    buf[off + 4..off + 6].copy_from_slice(&extent.ee_len.to_le_bytes());
    buf[off + 6..off + 8].copy_from_slice(&extent.ee_start_hi.to_le_bytes());
    buf[off + 8..off + 12].copy_from_slice(&extent.ee_start_lo.to_le_bytes());
}

/// Serialize an extent index entry into a buffer at the given offset.
pub fn serialize_idx(buf: &mut [u8], off: usize, idx: &Ext4ExtentIdx) {
    if off + 12 > buf.len() { return; }
    buf[off..off + 4].copy_from_slice(&idx.ei_block.to_le_bytes());
    buf[off + 4..off + 8].copy_from_slice(&idx.ei_leaf_lo.to_le_bytes());
    buf[off + 8..off + 10].copy_from_slice(&idx.ei_leaf_hi.to_le_bytes());
    buf[off + 10..off + 12].copy_from_slice(&idx.ei_unused.to_le_bytes());
}

/// Deserialize extents from a leaf node block into a Vec.
pub fn deserialize_extents(block: &[u8], header: &Ext4ExtentHeader) -> Ext4Result<Vec<Ext4Extent>> {
    let entries = header.eh_entries as usize;
    let mut extents = Vec::with_capacity(entries);
    for i in 0..entries {
        let off = 12 + i * 12;
        if off + 12 > block.len() { break; }
        extents.push(Ext4Extent {
            ee_block: u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]),
            ee_len: u16::from_le_bytes([block[off + 4], block[off + 5]]),
            ee_start_hi: u16::from_le_bytes([block[off + 6], block[off + 7]]),
            ee_start_lo: u32::from_le_bytes([block[off + 8], block[off + 9], block[off + 10], block[off + 11]]),
        });
    }
    Ok(extents)
}

/// Insert a new extent mapping into an inode's extent tree (depth 0 only).
///
/// Handles:
/// - Merging with predecessor extent (physically adjacent, extend right)
/// - Merging with successor extent (physically adjacent, extend left)
/// - Insertion between existing extents (sorted by logical block)
/// - **Splitting** existing extents when new extent overlaps partially:
///   - New starts before existing, ends inside → shorten existing on the right
///   - New starts inside existing, ends after → shorten existing on the left
///   - New completely covers existing → remove existing
///   - New completely inside existing → split into left + right parts
/// - Overflow detection: if inline extent count exceeds 4, returns ExtentTreeFull
///
/// `write_inode` is called after modifying the inode's i_block to persist changes.
pub fn extent_insert<F>(
    _sb: &Ext4Superblock,
    inode: &mut Ext4Inode,
    logical_block: u32,
    physical_block: u64,
    block_count: u16,
    write_inode: &mut F,
) -> Ext4Result<()>
where
    F: FnMut(&Ext4Inode) -> Ext4Result<()>,
{
    let header = inode.extent_header()
        .ok_or(Ext4Error::NotAnExtentInode)?;

    if header.eh_magic != EXT4_EXTENT_MAGIC {
        return Err(Ext4Error::InvalidExtentHeader);
    }

    if header.eh_depth != 0 {
        return Err(Ext4Error::ExtentTreeFull);
    }

    let mut extents = deserialize_extents(&inode.i_block[..], &header)?;

    let new_start = logical_block as u64;
    let new_end = new_start + block_count as u64;
    let new_phys = physical_block;
    let new_phys_end = new_phys + block_count as u64;

    // Phase 1: Handle overlaps — split or remove existing extents that intersect [new_start, new_end)
    // We process extents in reverse order so removals/shifts don't mess up indices
    let mut i = extents.len();
    while i > 0 {
        i -= 1;
        let ext = extents[i];
        let ext_start = ext.ee_block as u64;
        let ext_end = ext_start + ext.block_count() as u64;
        let ext_phys = ext.start_block();

        // Check for overlap
        if new_start < ext_end && new_end > ext_start {
            // Overlap detected! Remove the existing extent
            extents.remove(i);

            // Left non-overlapping part: [ext_start, new_start)
            if ext_start < new_start {
                let left_len = (new_start - ext_start) as u32;
                let left = Ext4Extent {
                    ee_block: ext.ee_block,
                    ee_len: left_len as u16,
                    ee_start_hi: ext.ee_start_hi,
                    ee_start_lo: ext.ee_start_lo,
                };
                extents.insert(i, left);
                // i now points to left part; right part (if any) goes after
            }

            // Right non-overlapping part: [new_end, ext_end)
            if new_end < ext_end {
                let right_len = (ext_end - new_end) as u32;
                // Physical block for right part = ext_phys + (new_end - ext_start)
                let right_phys = ext_phys + (new_end - ext_start);
                let right = Ext4Extent {
                    ee_block: new_end as u32,
                    ee_len: right_len as u16,
                    ee_start_hi: (right_phys >> 32) as u16,
                    ee_start_lo: right_phys as u32,
                };
                // Insert right part after left part (or at position i+1)
                let insert_at = if ext_start < new_start { i + 1 } else { i };
                extents.insert(insert_at, right);
            }
        }
    }

    // Phase 2: Check if the new extent can merge with adjacent extents
    let mut merged = false;

    for j in 0..extents.len() {
        let ext = extents[j];
        let ext_start = ext.ee_block as u64;
        let ext_block_count = ext.block_count() as u64;
        let ext_phys = ext.start_block();

        // Check merge-right: new extent starts right after existing, physically adjacent
        if ext_start + ext_block_count == new_start
            && ext_phys + ext_block_count == new_phys
        {
            extents[j].ee_len = (ext.block_count() + block_count as u32) as u16;
            merged = true;
            break;
        }

        // Check merge-left: new extent ends right before existing, physically adjacent
        if new_end == ext_start
            && new_phys_end == ext_phys
        {
            extents[j].ee_block = logical_block;
            extents[j].ee_start_lo = physical_block as u32;
            extents[j].ee_start_hi = (physical_block >> 32) as u16;
            extents[j].ee_len = (block_count as u32 + ext.block_count()) as u16;
            merged = true;
            break;
        }
    }

    // Phase 3: If not merged, find insertion position and insert
    if !merged {
        if extents.len() >= EXT4_INLINE_EXTENTS_COUNT {
            return Err(Ext4Error::ExtentTreeFull);
        }

        let mut insert_pos = extents.len();
        for j in 0..extents.len() {
            if logical_block > extents[j].ee_block {
                insert_pos = j + 1;
            }
        }

        let new = Ext4Extent {
            ee_block: logical_block,
            ee_len: block_count & 0x7FFF,
            ee_start_hi: (physical_block >> 32) as u16,
            ee_start_lo: physical_block as u32,
        };

        extents.insert(insert_pos, new);
    }

    // Serialize back to inode i_block
    let new_header = Ext4ExtentHeader {
        eh_magic: EXT4_EXTENT_MAGIC,
        eh_entries: extents.len() as u16,
        eh_max: header.eh_max,
        eh_depth: 0,
        eh_generation: 0,
    };

    let mut i_block = [0u8; 60];
    serialize_header(&mut i_block[..12], &new_header);
    for (i, extent) in extents.iter().enumerate() {
        serialize_extent(&mut i_block[..], 12 + i * 12, extent);
    }
    inode.i_block = i_block;

    write_inode(inode)?;
    Ok(())
}

/// Truncate (shrink) an inode's extent tree to a new size.
///
/// Removes all extent entries beyond `new_size` and shortens the
/// last extent that crosses the boundary. The freed blocks are
/// reported via the `free_blocks_cb` callback.
///
/// `write_inode` is called after modifying the inode.
pub fn extent_truncate<FF, FW>(
    _sb: &Ext4Superblock,
    inode: &mut Ext4Inode,
    new_size: u64,
    mut free_blocks_cb: FF,
    write_inode: &mut FW,
) -> Ext4Result<()>
where
    FF: FnMut(u64, u32) -> Ext4Result<()>,
    FW: FnMut(&Ext4Inode) -> Ext4Result<()>,
{
    let header = inode.extent_header()
        .ok_or(Ext4Error::NotAnExtentInode)?;

    if header.eh_magic != EXT4_EXTENT_MAGIC {
        return Err(Ext4Error::InvalidExtentHeader);
    }

    if header.eh_depth != 0 {
        return Err(Ext4Error::ExtentTreeFull);
    }

    let mut extents = deserialize_extents(&inode.i_block[..], &header)?;
    let block_size = _sb.block_size() as u64;
    let new_logical_blocks = (new_size + block_size - 1) / block_size;

    // Remove or shorten extents beyond new_size
    let mut i = 0;
    while i < extents.len() {
        let ext = extents[i];
        let ext_start = ext.ee_block as u64;
        let ext_end = ext_start + ext.block_count() as u64;

        if ext_start >= new_logical_blocks {
            // Extent is entirely beyond new size — remove it
            let phys = ext.start_block();
            free_blocks_cb(phys, ext.block_count())?;
            extents.remove(i);
            // Don't increment i, we removed the current element
        } else if ext_end > new_logical_blocks {
            // Extent crosses the boundary — shorten it
            let blocks_to_keep = (new_logical_blocks - ext_start) as u32;
            let blocks_to_free = ext.block_count() - blocks_to_keep;

            if blocks_to_free > 0 {
                let free_phys = ext.start_block() + blocks_to_keep as u64;
                free_blocks_cb(free_phys, blocks_to_free)?;
            }

            extents[i].ee_len = blocks_to_keep as u16;
            i += 1;
        } else {
            // Extent is entirely within bounds — keep it
            i += 1;
        }
    }

    // Serialize back to inode i_block
    let new_header = Ext4ExtentHeader {
        eh_magic: EXT4_EXTENT_MAGIC,
        eh_entries: extents.len() as u16,
        eh_max: header.eh_max,
        eh_depth: 0,
        eh_generation: 0,
    };

    let mut i_block = [0u8; 60];
    serialize_header(&mut i_block[..12], &new_header);
    for (i, extent) in extents.iter().enumerate() {
        serialize_extent(&mut i_block[..], 12 + i * 12, extent);
    }
    inode.i_block = i_block;

    // Update inode size
    crate::inode::set_file_size(inode, new_size);

    write_inode(inode)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sb() -> Ext4Superblock {
        let mut data = vec![0u8; 1024];
        data[56..58].copy_from_slice(&EXT4_SUPER_MAGIC.to_le_bytes());
        data[24..28].copy_from_slice(&(2u32).to_le_bytes());  // s_log_block_size -> block_size=4096
        data[96..100].copy_from_slice(&(EXT4_FEATURE_INCOMPAT_FILETYPE | EXT4_FEATURE_INCOMPAT_EXTENTS | EXT4_FEATURE_INCOMPAT_FLEX_BG).to_le_bytes());
        data[100..104].copy_from_slice(&EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER.to_le_bytes());
        data[76..80].copy_from_slice(&(1u32).to_le_bytes());   // s_rev_level
        data[88..90].copy_from_slice(&(256u16).to_le_bytes()); // s_inode_size
        crate::superblock::parse_superblock(&data).unwrap()
    }

    #[test]
    fn test_extent_lookup_inline() {
        let sb = make_sb();
        let mut raw_inode = vec![0u8; 256];
        raw_inode[32..36].copy_from_slice(&EXT4_EXTENTS_FL.to_le_bytes()); // i_flags

        // Extent tree root in i_block (bytes 40-99):
        // Header: depth=0, entries=1, magic=0xF30A
        raw_inode[40..42].copy_from_slice(&EXT4_EXTENT_MAGIC.to_le_bytes());
        raw_inode[42..44].copy_from_slice(&(1u16).to_le_bytes()); // eh_entries = 1
        raw_inode[44..46].copy_from_slice(&(4u16).to_le_bytes()); // eh_max = 4
        raw_inode[46..48].copy_from_slice(&(0u16).to_le_bytes()); // eh_depth = 0

        // Extent: logical block 0, length 1024 blocks, physical block 256
        raw_inode[52..56].copy_from_slice(&(0u32).to_le_bytes()); // ee_block = 0
        raw_inode[56..58].copy_from_slice(&(1024u16).to_le_bytes()); // ee_len = 1024
        raw_inode[58..60].copy_from_slice(&(0u16).to_le_bytes()); // ee_start_hi = 0
        raw_inode[60..64].copy_from_slice(&(256u32).to_le_bytes()); // ee_start_lo = 256

        let inode = crate::inode::parse_inode(&raw_inode, &sb).unwrap();

        let mut read_count = 0u32;
        let result = extent_lookup(&sb, &inode, 0, |_block, _buf| {
            read_count += 1;
            Err(Ext4Error::IoError) // Should not be called for depth=0
        }).unwrap();

        assert_eq!(result, Some(256));
        assert_eq!(read_count, 0);

        let result2 = extent_lookup(&sb, &inode, 500, |_block, _buf| {
            read_count += 1;
            Err(Ext4Error::IoError)
        }).unwrap();
        assert_eq!(result2, Some(256 + 500));

        let result3 = extent_lookup(&sb, &inode, 2048, |_block, _buf| {
            read_count += 1;
            Err(Ext4Error::IoError)
        }).unwrap();
        assert_eq!(result3, None); // Sparse
    }
}
