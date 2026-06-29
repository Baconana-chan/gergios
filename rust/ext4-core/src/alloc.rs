//! Block allocator for ext4.
//!
//! Manages block allocation and deallocation: finds free blocks,
//! updates block bitmaps, and adjusts group descriptor free counts.

use crate::types::*;

/// Block allocator — finds and manages free blocks.
///
/// Holds mutable references to the group descriptor table and
/// uses callbacks to read/write block bitmaps.
pub struct BlockAllocator<'a> {
    sb: &'a Ext4Superblock,
    groups: &'a mut [Ext4GroupDesc],
    block_size: usize,
}

impl<'a> BlockAllocator<'a> {
    pub fn new(sb: &'a Ext4Superblock, groups: &'a mut [Ext4GroupDesc]) -> Self {
        let block_size = sb.block_size();
        BlockAllocator { sb, groups, block_size }
    }

    /// Allocate `count` consecutive blocks, searching from a preferred group.
    ///
    /// Returns the physical block number of the first allocated block.
    /// `read_block` / `write_block` are used to update block bitmaps on disk.
    pub fn allocate_blocks<FR, FW>(
        &mut self,
        count: u32,
        prefer_group: u32,
        mut read_block: FR,
        mut write_block: FW,
    ) -> Ext4Result<u64>
    where
        FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
        FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    {
        let num_groups = self.sb.block_groups_count();

        // Search starting from preferred group, wrapping around
        for gi in 0..num_groups {
            let group = (prefer_group + gi) % num_groups;
            if let Some(block) = self.try_allocate_in_group(group, count, &mut read_block, &mut write_block)? {
                return Ok(block);
            }
        }

        Err(Ext4Error::NoSpace)
    }

    /// Try to allocate `count` blocks within a single block group.
    fn try_allocate_in_group<FR, FW>(
        &mut self,
        group: u32,
        count: u32,
        read_block: &mut FR,
        write_block: &mut FW,
    ) -> Ext4Result<Option<u64>>
    where
        FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
        FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    {
        let gd = &mut self.groups[group as usize];
        let free_blocks = gd.free_blocks_count(self.sb);
        if (free_blocks as u32) < count {
            return Ok(None);
        }

        let bitmap_block = gd.block_bitmap(self.sb);
        let mut bitmap = vec![0u8; self.block_size];
        read_block(bitmap_block, &mut bitmap)?;

        let blocks_per_group = self.sb.s_blocks_per_group;
        let group_start = group as u64 * blocks_per_group as u64;

        if let Some(bit) = find_consecutive_bits(&bitmap, count) {
            // Mark bits as used
            for i in 0..count {
                set_bit(&mut bitmap, (bit + i) as usize);
            }

            // Write updated bitmap back
            write_block(bitmap_block, &bitmap)?;

            // Update group descriptor
            gd.set_free_blocks_count(self.sb, free_blocks - count as u16);

            let first_block = group_start + bit as u64;
            Ok(Some(first_block))
        } else {
            Ok(None) // No contiguous space in this group
        }
    }

    /// Free `count` blocks starting at `block_nr`.
    pub fn free_blocks<FR, FW>(
        &mut self,
        block_nr: u64,
        count: u32,
        mut read_block: FR,
        mut write_block: FW,
    ) -> Ext4Result<()>
    where
        FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
        FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    {
        let blocks_per_group = self.sb.s_blocks_per_group as u64;
        let group = (block_nr / blocks_per_group) as u32;
        let in_group_offset = (block_nr % blocks_per_group) as usize;

        if group >= self.sb.block_groups_count() {
            return Err(Ext4Error::NotFound);
        }

        let gd = &mut self.groups[group as usize];
        let bitmap_block = gd.block_bitmap(self.sb);
        let mut bitmap = vec![0u8; self.block_size];
        read_block(bitmap_block, &mut bitmap)?;

        for i in 0..count as usize {
            clear_bit(&mut bitmap, in_group_offset + i);
        }

        write_block(bitmap_block, &bitmap)?;

        let free = gd.free_blocks_count(self.sb);
        gd.set_free_blocks_count(self.sb, free + count as u16);

        Ok(())
    }
}

// ─── Bitmap helpers ──────────────────────────────────────────────────

/// Find `count` consecutive zero bits in the bitmap.
fn find_consecutive_bits(bitmap: &[u8], count: u32) -> Option<u32> {
    let total_bits = bitmap.len() * 8;
    let mut run_start = 0;
    let mut run_len = 0;

    for bit in 0..total_bits {
        if is_bit_set(bitmap, bit) {
            run_len = 0;
            run_start = bit + 1;
        } else {
            run_len += 1;
            if run_len >= count as usize {
                return Some(run_start as u32);
            }
        }
    }
    None
}

fn is_bit_set(bitmap: &[u8], bit: usize) -> bool {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    byte < bitmap.len() && (bitmap[byte] & (1 << bit_in_byte)) != 0
}

fn set_bit(bitmap: &mut [u8], bit: usize) {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    if byte < bitmap.len() {
        bitmap[byte] |= 1 << bit_in_byte;
    }
}

fn clear_bit(bitmap: &mut [u8], bit: usize) {
    let byte = bit / 8;
    let bit_in_byte = bit % 8;
    if byte < bitmap.len() {
        bitmap[byte] &= !(1 << bit_in_byte);
    }
}

// ─── Group descriptor helpers ────────────────────────────────────────

impl Ext4GroupDesc {
    pub fn free_blocks_count(&self, sb: &Ext4Superblock) -> u16 {
        let hi = if sb.has_64bit() { self.bg_free_blocks_count_hi } else { 0 };
        self.bg_free_blocks_count_lo.saturating_add(hi)
    }

    pub fn set_free_blocks_count(&mut self, sb: &Ext4Superblock, count: u16) {
        self.bg_free_blocks_count_lo = count;
        if sb.has_64bit() {
            self.bg_free_blocks_count_hi = 0;
        }
    }

    pub fn free_inodes_count(&self, sb: &Ext4Superblock) -> u16 {
        let hi = if sb.has_64bit() { self.bg_free_inodes_count_hi } else { 0 };
        self.bg_free_inodes_count_lo.saturating_add(hi)
    }

    pub fn set_free_inodes_count(&mut self, sb: &Ext4Superblock, count: u16) {
        self.bg_free_inodes_count_lo = count;
        if sb.has_64bit() {
            self.bg_free_inodes_count_hi = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_consecutive_bits_empty() {
        let bitmap = vec![0xFFu8; 16]; // All used
        assert!(find_consecutive_bits(&bitmap, 1).is_none());
    }

    #[test]
    fn test_find_consecutive_bits_found() {
        let mut bitmap = vec![0xFFu8; 16];
        bitmap[4] = 0x00; // Bytes 32-39 are free
        // Bits 32-39 are free (byte index 4, bits 0-7)
        let result = find_consecutive_bits(&bitmap, 4);
        assert_eq!(result, Some(32));
    }

    #[test]
    fn test_find_consecutive_bits_cross_byte() {
        let mut bitmap = vec![0xFFu8; 16];
        bitmap[3] = 0x80; // Bit 31 is set (byte 3, bit 7)
        bitmap[4] = 0x01; // Bit 32 is set (byte 4, bit 0)
        // Gap at bit 31-32... actually
        // byte 3 = 0x80 = bit 31 set, bits 24-30 free
        // byte 4 = 0x01 = bit 32 set, bits 33-39 free
        // So free runs: 24-30 (7 bits), 33-39 (7 bits)
        // Let me adjust: make byte 3 = 0, byte 4 = 0, byte 5 = 0xFF
        bitmap[3] = 0x00;
        bitmap[4] = 0x00;
        let result = find_consecutive_bits(&bitmap, 16);
        assert_eq!(result, Some(24)); // Bits 24-39 = 16 free bits
    }

    #[test]
    fn test_bit_ops() {
        let mut bitmap = vec![0u8; 4];
        set_bit(&mut bitmap, 0);
        assert_eq!(bitmap[0], 0x01);
        set_bit(&mut bitmap, 7);
        assert_eq!(bitmap[0], 0x81);
        assert!(is_bit_set(&bitmap, 0));
        assert!(is_bit_set(&bitmap, 7));
        assert!(!is_bit_set(&bitmap, 1));

        clear_bit(&mut bitmap, 0);
        assert!(!is_bit_set(&bitmap, 0));
        assert_eq!(bitmap[0], 0x80);
    }
}
