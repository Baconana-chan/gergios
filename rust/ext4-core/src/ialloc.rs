//! Inode allocator for ext4.
//!
//! Manages inode allocation and deallocation: finds free inodes,
//! updates inode bitmaps, and adjusts group descriptor free counts.

use crate::types::*;
use crate::inode::inode_to_group;

/// Inode allocator — finds and manages free inodes.
///
/// Holds mutable references to the group descriptor table and
/// uses callbacks to read/write inode bitmaps.
pub struct InodeAllocator<'a> {
    sb: &'a Ext4Superblock,
    groups: &'a mut [Ext4GroupDesc],
}

impl<'a> InodeAllocator<'a> {
    pub fn new(sb: &'a Ext4Superblock, groups: &'a mut [Ext4GroupDesc]) -> Self {
        InodeAllocator { sb, groups }
    }

    /// Allocate a free inode, searching from a preferred group.
    ///
    /// Returns the inode number of the newly allocated inode.
    /// `read_block` / `write_block` are used to update inode bitmaps on disk.
    pub fn allocate_inode<FR, FW>(
        &mut self,
        prefer_group: u32,
        mut read_block: FR,
        mut write_block: FW,
    ) -> Ext4Result<u32>
    where
        FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
        FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    {
        let num_groups = self.sb.block_groups_count();

        // Search starting from preferred group, wrapping around
        for gi in 0..num_groups {
            let group = (prefer_group + gi) % num_groups;
            if let Some(ino) = self.try_allocate_in_group(group, &mut read_block, &mut write_block)? {
                return Ok(ino);
            }
        }

        Err(Ext4Error::NoSpace)
    }

    /// Try to allocate an inode within a single block group.
    fn try_allocate_in_group<FR, FW>(
        &mut self,
        group: u32,
        read_block: &mut FR,
        write_block: &mut FW,
    ) -> Ext4Result<Option<u32>>
    where
        FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
        FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    {
        let gd = &self.groups[group as usize];
        let free_inodes = gd.free_inodes_count(self.sb);
        if free_inodes == 0 {
            return Ok(None);
        }

        let bitmap_block = gd.inode_bitmap(self.sb);
        let block_size = self.sb.block_size();
        let mut bitmap = vec![0u8; block_size];
        read_block(bitmap_block, &mut bitmap)?;

        let inodes_per_group = self.sb.s_inodes_per_group;
        let first_ino = group * inodes_per_group + 1;

        // Find first free inode in this group
        for i in 0..inodes_per_group {
            if !is_inode_bit_set(&bitmap, i as usize) {
                // Mark as used
                set_inode_bit(&mut bitmap, i as usize);
                write_block(bitmap_block, &bitmap)?;

                let gd = &mut self.groups[group as usize];
                gd.set_free_inodes_count(self.sb, free_inodes - 1);

                let ino = first_ino + i;
                return Ok(Some(ino));
            }
        }

        Ok(None)
    }

    /// Free an inode (clear bitmap, update group descriptor free count).
    pub fn free_inode<FR, FW>(
        &mut self,
        ino: u32,
        mut read_block: FR,
        mut write_block: FW,
    ) -> Ext4Result<()>
    where
        FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
        FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    {
        let group = inode_to_group(ino, self.sb);
        let index = crate::inode::inode_to_group_index(ino, self.sb);

        if group >= self.sb.block_groups_count() {
            return Err(Ext4Error::NotFound);
        }

        let gd = &mut self.groups[group as usize];
        let bitmap_block = gd.inode_bitmap(self.sb);
        let block_size = self.sb.block_size();
        let mut bitmap = vec![0u8; block_size];
        read_block(bitmap_block, &mut bitmap)?;

        clear_inode_bit(&mut bitmap, index as usize);
        write_block(bitmap_block, &bitmap)?;

        let free = gd.free_inodes_count(self.sb);
        gd.set_free_inodes_count(self.sb, free + 1);

        Ok(())
    }
}

// ─── Bitmap helpers ──────────────────────────────────────────────────

/// Check if an inode bit is set in the bitmap.
fn is_inode_bit_set(bitmap: &[u8], bit: usize) -> bool {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    byte < bitmap.len() && (bitmap[byte] & (1 << bit_in_byte)) != 0
}

/// Set an inode bit in the bitmap.
fn set_inode_bit(bitmap: &mut [u8], bit: usize) {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    if byte < bitmap.len() {
        bitmap[byte] |= 1 << bit_in_byte;
    }
}

/// Clear an inode bit in the bitmap.
fn clear_inode_bit(bitmap: &mut [u8], bit: usize) {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    if byte < bitmap.len() {
        bitmap[byte] &= !(1 << bit_in_byte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inode_bit_ops() {
        let mut bitmap = vec![0u8; 4];
        set_inode_bit(&mut bitmap, 0);
        assert_eq!(bitmap[0], 0x01);
        set_inode_bit(&mut bitmap, 7);
        assert_eq!(bitmap[0], 0x81);
        assert!(is_inode_bit_set(&bitmap, 0));
        assert!(is_inode_bit_set(&bitmap, 7));
        assert!(!is_inode_bit_set(&bitmap, 1));

        clear_inode_bit(&mut bitmap, 0);
        assert!(!is_inode_bit_set(&bitmap, 0));
        assert_eq!(bitmap[0], 0x80);
    }
}
