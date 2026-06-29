//! Directory entry reading and htree (indexed directory) support.
//!
//! ext4 directories are either linear (linked list of entries) or indexed
//! using an htree (a hash-based B-tree). htree is enabled when the
//! RO_COMPAT_DIR_NLINK and INCOMPAT_FILETYPE features are present.

use crate::types::*;

/// Minimum directory entry size.
const EXT4_DIR_ENTRY_MIN_SIZE: usize = 8;

/// Hash version for htree.
const HTREE_HASH_HALF_MD4: u8 = 0;
// const HTREE_TEA: u8 = 1;
// const HTREE_LEGACY: u8 = 2;
// const HTREE_HALF_MD4_UNSIGNED: u8 = 3;
// const HTREE_TEA_UNSIGNED: u8 = 4;

/// Iterator over directory entries in a raw directory block.
pub struct DirEntryIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> DirEntryIter<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        DirEntryIter { data, pos: 0 }
    }
}

impl<'a> Iterator for DirEntryIter<'a> {
    type Item = Ext4DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.pos + EXT4_DIR_ENTRY_MIN_SIZE > self.data.len() {
                return None;
            }

            let inode = u32::from_le_bytes([
                self.data[self.pos], self.data[self.pos + 1],
                self.data[self.pos + 2], self.data[self.pos + 3],
            ]);
            let rec_len = u16::from_le_bytes([
                self.data[self.pos + 4], self.data[self.pos + 5],
            ]) as usize;

            if rec_len == 0 || self.pos + rec_len > self.data.len() {
                return None;
            }

            if inode == 0 {
                // Deleted entry, skip
                self.pos += rec_len;
                continue;
            }

            let name_len = self.data[self.pos + 6] as usize;
            let file_type = self.data[self.pos + 7];

            let actual_len = EXT4_DIR_ENTRY_MIN_SIZE + name_len;
            let mut name = [0u8; 255];
            let copy_len = core::cmp::min(name_len, 255);
            name[..copy_len].copy_from_slice(&self.data[self.pos + 8..self.pos + 8 + copy_len]);

            self.pos += rec_len;

            return Some(Ext4DirEntry {
                inode,
                rec_len: rec_len as u16,
                name_len: name_len as u8,
                file_type,
                name,
            });
        }
    }
}

/// Look up a name in a linear directory.
pub fn lookup_linear(data: &[u8], name: &str) -> Option<Ext4DirEntry> {
    for entry in DirEntryIter::new(data) {
        let entry_len = entry.name_len as usize;
        if entry_len == name.len() {
            let entry_name = &entry.name[..entry_len];
            if entry_name == name.as_bytes() {
                return Some(entry);
            }
        }
    }
    None
}

// ─── Directory Write Support ───────────────────────────────────────────

/// Minimum entry size for a directory entry (8 bytes header + 0 name).
pub const EXT4_DIR_ENTRY_HEADER_SIZE: usize = 8;

/// Check if a name is valid for insertion (no empty names, no reserved patterns).
pub fn is_valid_dirent_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && name.len() <= 255 && !name.contains('\0')
}

/// Compute how many bytes a directory entry of the given name length will occupy on disk.
/// Entries are 4-byte aligned.
pub fn dirent_size(name_len: u8) -> usize {
    let sz = EXT4_DIR_ENTRY_HEADER_SIZE + name_len as usize;
    // Round up to 4-byte alignment
    (sz + 3) & !3
}

/// Insert a new directory entry into a directory block.
///
/// Scans the block for an existing entry with enough rec_len padding to
/// accommodate the new entry. If successful, splits the padding and writes
/// the new entry.
///
/// Returns `true` if the entry was inserted, `false` if no space was found
/// (caller should extend the directory with a new block).
pub fn insert_into_block(block: &mut [u8], inode: u32, name: &str, file_type: u8) -> bool {
    let name_len = name.len() as u8;
    let needed = dirent_size(name_len);
    let mut pos = 0usize;

    while pos + EXT4_DIR_ENTRY_MIN_SIZE <= block.len() {
        let entry_inode = u32::from_le_bytes([
            block[pos], block[pos + 1], block[pos + 2], block[pos + 3],
        ]);
        let rec_len = u16::from_le_bytes([
            block[pos + 4], block[pos + 5],
        ]) as usize;

        if rec_len == 0 || pos + rec_len > block.len() {
            return false;
        }

        let entry_name_len = if entry_inode != 0 {
            block[pos + 6] as usize
        } else {
            0 // Deleted entry, name_len may be invalid
        };

        // Minimum space this entry actually needs
        let min_entry_size = if entry_inode != 0 {
            dirent_size(entry_name_len as u8)
        } else {
            EXT4_DIR_ENTRY_MIN_SIZE // Deleted entry could be as small as 8 bytes
        };

        // Available space = rec_len - what this entry actually needs
        let available = rec_len - min_entry_size;

        if available >= needed {
            // We can split this entry!
            // Shrink the current entry's rec_len to its minimum
            let new_rec_len = min_entry_size as u16;
            block[pos + 4..pos + 6].copy_from_slice(&new_rec_len.to_le_bytes());

            // Add the new entry after the current one
            let new_pos = pos + min_entry_size;
            let new_rec_len = available as u16;
            block[new_pos..new_pos + 4].copy_from_slice(&inode.to_le_bytes());
            block[new_pos + 4..new_pos + 6].copy_from_slice(&new_rec_len.to_le_bytes());
            block[new_pos + 6] = name_len;
            block[new_pos + 7] = file_type;
            block[new_pos + 8..new_pos + 8 + name.len()].copy_from_slice(name.as_bytes());
            return true;
        }

        pos += rec_len;
    }

    false
}

/// Remove a directory entry by name from a block.
/// Marks the entry as deleted (inode = 0) and merges its rec_len with
/// the previous entry's rec_len.
///
/// Returns `true` if the entry was found and removed.
pub fn remove_from_block(block: &mut [u8], name: &str) -> bool {
    let mut pos = 0usize;
    let mut prev_pos = 0usize;

    while pos + EXT4_DIR_ENTRY_MIN_SIZE <= block.len() {
        let entry_inode = u32::from_le_bytes([
            block[pos], block[pos + 1], block[pos + 2], block[pos + 3],
        ]);
        let rec_len = u16::from_le_bytes([
            block[pos + 4], block[pos + 5],
        ]) as usize;

        if rec_len == 0 || pos + rec_len > block.len() {
            return false;
        }

        if entry_inode != 0 {
            let entry_name_len = block[pos + 6] as usize;
            if entry_name_len == name.len() {
                let entry_name = &block[pos + 8..pos + 8 + entry_name_len];
                if entry_name == name.as_bytes() {
                    // Found! Mark as deleted and merge rec_len into previous entry
                    block[pos..pos + 4].copy_from_slice(&0u32.to_le_bytes()); // inode = 0

                    // Absorb this entry's rec_len into the previous entry
                    let merged_rec_len = pos - prev_pos + rec_len;
                    block[prev_pos + 4..prev_pos + 6]
                        .copy_from_slice(&(merged_rec_len as u16).to_le_bytes());
                    return true;
                }
            }
        }

        prev_pos = pos;
        pos += rec_len;
    }

    false
}

/// Initialize a fresh directory block with "." and ".." entries.
/// `parent_ino` is the inode number of the parent directory.
pub fn init_dir_block(block: &mut [u8], dir_ino: u32, parent_ino: u32) {
    let block_size = block.len();

    // Entry 0: "." -> dir_ino
    let dot_len = dirent_size(1); // "." is 1 byte
    block[0..4].copy_from_slice(&dir_ino.to_le_bytes());
    block[4..6].copy_from_slice(&(dot_len as u16).to_le_bytes());
    block[6] = 1; // name_len
    block[7] = EXT4_FT_DIR;
    block[8] = b'.';

    // Entry 1: ".." -> parent_ino, with rec_len covering the rest of the block
    let dotdot_rec_len = block_size - dot_len;
    block[dot_len..dot_len + 4].copy_from_slice(&parent_ino.to_le_bytes());
    block[dot_len + 4..dot_len + 6].copy_from_slice(&(dotdot_rec_len as u16).to_le_bytes());
    block[dot_len + 6] = 2; // name_len
    block[dot_len + 7] = EXT4_FT_DIR;
    block[dot_len + 8] = b'.';
    block[dot_len + 9] = b'.';
}

/// Compute the dx_root hash for htree directory indexing.
/// Uses the Half-MD4 hash (same as Linux ext4 dx_hash).
fn half_md4_hash(name: &[u8], seed: &[u32; 4]) -> u32 {
    let mut a = seed[0].wrapping_add(seed[1]);
    let mut b = seed[1];
    let mut c = seed[2];
    let mut d = seed[3];

    let mut buf = [0u8; 64];
    let len = name.len();

    // Copy name into buffer, zero-padded to 32 bytes
    let mut i = 0;
    while i < 32 {
        if i < len {
            buf[i] = name[i];
        } else {
            buf[i] = 0;
        }
        i += 1;
    }

    // Process in 4-byte chunks
    let mut words = [0u32; 8];
    for j in 0..8 {
        let off = j * 4;
        if off < 32 {
            words[j] = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        } else {
            words[j] = 0;
        }
    }

    // Round 1
    for j in 0..8 {
        let k = j;
        let f = (b & c) | (!b & d);
        a = a.wrapping_add(f).wrapping_add(words[k]);
        a = a.rotate_left(3);
        let tmp = d; d = c; c = b; b = a; a = tmp;
    }

    // Round 2
    for j in 0..8u32 {
        let k = (j.wrapping_mul(5).wrapping_add(1)) & 7;
        let g = (b & d) | (c & !d);
        a = a.wrapping_add(g).wrapping_add(words[k as usize]).wrapping_add(0x5A827999);
        a = a.rotate_left(5);
        let tmp = d; d = c; c = b; b = a; a = tmp;
    }

    // Round 3
    for j in 0..8u32 {
        let k = (j.wrapping_mul(3).wrapping_add(5)) & 7;
        let h = b ^ c ^ d;
        a = a.wrapping_add(h).wrapping_add(words[k as usize]).wrapping_add(0x6ED9EBA1);
        a = a.rotate_left(9);
        let tmp = d; d = c; c = b; b = a; a = tmp;
    }

    a.wrapping_add(b)
}

/// Look up a name in a directory, supporting both linear and htree formats.
pub fn lookup_in_dir<F>(
    sb: &Ext4Superblock,
    inode: &Ext4Inode,
    _dir_ino: u32,
    name: &str,
    mut read_block: F,
) -> Ext4Result<Option<Ext4DirEntry>>
where
    F: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let block_size = sb.block_size();

    // Read the first block (block 0) of the directory
    let mut block = vec![0u8; block_size];
    read_block(0, &mut block)?;

    // Check for htree: first entry should be a dx_root structure
    // dx_root has inode=0 and rec_len covers the entire block
    let first_ino = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
    let first_rec_len = u16::from_le_bytes([block[4], block[5]]) as usize;

    if first_ino == 0 && first_rec_len == block_size {
        // htree directory — the first entry is the dx_root
        // For now, fall back to linear scan of all blocks
        // (htree support will be added in a follow-up)
        let num_blocks = (inode.file_size() + block_size as u64 - 1) / block_size as u64;

        for b in 0..num_blocks {
            read_block(b, &mut block)?;
            if let Some(entry) = lookup_linear(&block, name) {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    } else {
        // Linear directory
        Ok(lookup_linear(&block, name))
    }
}

/// Convert a file_type byte to a mode_t-compatible value.
pub fn file_type_to_mode(file_type: u8) -> u16 {
    match file_type {
        EXT4_FT_REG_FILE => EXT4_S_IFREG,
        EXT4_FT_DIR => EXT4_S_IFDIR,
        EXT4_FT_SYMLINK => EXT4_S_IFLNK,
        EXT4_FT_CHRDEV => EXT4_S_IFCHR,
        EXT4_FT_BLKDEV => EXT4_S_IFBLK,
        EXT4_FT_FIFO => EXT4_S_IFIFO,
        EXT4_FT_SOCK => EXT4_S_IFSOCK,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_dir_lookup() {
        // Create a simple directory block
        let mut block = vec![0u8; 1024];

        // Entry 1: "." -> inode 2
        let mut off = 0usize;
        block[off..off + 4].copy_from_slice(&(2u32).to_le_bytes()); // inode
        block[off + 4..off + 6].copy_from_slice(&(12u16).to_le_bytes()); // rec_len = 12
        block[off + 6] = 1; // name_len
        block[off + 7] = EXT4_FT_DIR; // file_type
        block[off + 8] = b'.';

        // Entry 2: ".." -> inode 2
        off = 12;
        block[off..off + 4].copy_from_slice(&(2u32).to_le_bytes());
        block[off + 4..off + 6].copy_from_slice(&(12u16).to_le_bytes());
        block[off + 6] = 2;
        block[off + 7] = EXT4_FT_DIR;
        block[off + 8] = b'.';
        block[off + 9] = b'.';

        // Entry 3: "hello.txt" -> inode 100
        off = 24;
        block[off..off + 4].copy_from_slice(&(100u32).to_le_bytes());
        block[off + 4..off + 6].copy_from_slice(&(1000u16).to_le_bytes()); // rec_len to end
        block[off + 6] = 9;
        block[off + 7] = EXT4_FT_REG_FILE;
        block[off + 8..off + 17].copy_from_slice(b"hello.txt");

        let entry = lookup_linear(&block, "hello.txt").unwrap();
        assert_eq!(entry.inode, 100);
        assert_eq!(entry.file_type, EXT4_FT_REG_FILE);

        let entry2 = lookup_linear(&block, "missing").unwrap_or(
            Ext4DirEntry { inode: 0, rec_len: 0, name_len: 0, file_type: 0, name: [0u8; 255] }
        );
        assert_eq!(entry2.inode, 0);
    }

    #[test]
    fn test_half_md4_hash() {
        let seed = [0x12345678, 0x9ABCDEF0, 0x0FEDCBA9, 0x87654321];
        // Just verify it doesn't crash and returns a non-zero value
        let hash = half_md4_hash(b"test", &seed);
        assert_ne!(hash, 0);
    }
}
