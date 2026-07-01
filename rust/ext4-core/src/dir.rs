//! Directory entry reading, htree (indexed directory) support,
//! and CRC-32C directory block checksums (metadata_csum).
//!
//! When METADATA_CSUM is enabled, each directory leaf block has a 12-byte
//! `ext4_dir_entry_tail` appended (det_checksum at offset 8), and each
//! htree index node (dx_root/dx_node) has an 8-byte `dx_tail` appended
//! (dt_checksum at offset 4).
//!
//! Algorithm: CRC-32C(seed + dir_ino (4B LE) + i_generation (4B LE) +
//!            zeroed_block[..block_size - tail_size])

use crate::types::*;
use crate::journal::crc32c_le;

/// Minimum directory entry size.
const EXT4_DIR_ENTRY_MIN_SIZE: usize = 8;

/// Size of the ext4_dir_entry_tail appended to each leaf block.
/// Fields: det_reserved_zero1 (4B=0), det_rec_len (2B=12),
///         det_reserved_zero2 (1B=0), det_reserved_ft (1B=0xDE),
///         det_checksum (4B)
pub const EXT4_DIRENT_TAIL_SIZE: usize = 12;

/// Size of the dx_tail appended to each htree index node.
/// Fields: dt_reserved (4B), dt_checksum (4B)
pub const DX_TAIL_SIZE: usize = 8;

/// Offset of det_checksum within ext4_dir_entry_tail.
pub const EXT4_DIRENT_TAIL_CHECKSUM_OFF: usize = 8;

/// Offset of dt_checksum within dx_tail.
pub const DX_TAIL_CHECKSUM_OFF: usize = 4;

/// The reserved file_type value that identifies a dir_entry_tail.
pub const EXT4_DIRENT_TAIL_FT: u8 = 0xDE;

/// Compute the effective block size for checksum purposes (excludes the tail).
pub fn dir_block_csum_data_size(block_size: usize) -> usize {
    block_size - EXT4_DIRENT_TAIL_SIZE
}

/// Compute the CRC-32C checksum for a directory leaf block.
/// seed = csum_seed = crc32c_le(~0, s_uuid)
/// Algorithm: CRC-32C(seed + dir_ino (4B LE) + i_generation (4B LE) +
///            block[..block_size - EXT4_DIRENT_TAIL_SIZE])
pub fn compute_dir_leaf_csum(
    csum_seed: u32,
    dir_ino: u32,
    generation: u32,
    block: &[u8],
    block_size: usize,
) -> u32 {
    let data_size = dir_block_csum_data_size(block_size);
    let ino_le = dir_ino.to_le_bytes();
    let gen_le = generation.to_le_bytes();
    let mut crc = crc32c_le(csum_seed, &ino_le);
    crc = crc32c_le(crc, &gen_le);
    crc = crc32c_le(crc, &block[..data_size]);
    crc
}

/// Write the ext4_dir_entry_tail into a directory block.
pub fn write_dir_block_tail(block: &mut [u8], block_size: usize, checksum: u32) {
    let tail_off = block_size - EXT4_DIRENT_TAIL_SIZE;
    block[tail_off..tail_off + 4].copy_from_slice(&[0u8; 4]);
    let rec_len = EXT4_DIRENT_TAIL_SIZE as u16;
    block[tail_off + 4..tail_off + 6].copy_from_slice(&rec_len.to_le_bytes());
    block[tail_off + 6] = 0;
    block[tail_off + 7] = EXT4_DIRENT_TAIL_FT;
    block[tail_off + 8..tail_off + 12].copy_from_slice(&checksum.to_le_bytes());
}

/// Compute and write the CRC-32C checksum tail for a directory leaf block.
pub fn update_dir_leaf_csum(
    csum_seed: u32,
    dir_ino: u32,
    generation: u32,
    block: &mut [u8],
    block_size: usize,
) {
    if csum_seed == 0 { return; }
    let csum = compute_dir_leaf_csum(csum_seed, dir_ino, generation, block, block_size);
    write_dir_block_tail(block, block_size, csum);
}

/// Compute the CRC-32C checksum for an htree index node.
pub fn compute_dx_node_csum(csum_seed: u32, block: &[u8], block_size: usize) -> u32 {
    let data_size = block_size - DX_TAIL_SIZE;
    crc32c_le(csum_seed, &block[..data_size])
}

/// Write the dx_tail into an htree index node block.
pub fn write_dx_node_tail(block: &mut [u8], block_size: usize, checksum: u32) {
    let tail_off = block_size - DX_TAIL_SIZE;
    block[tail_off..tail_off + 4].copy_from_slice(&[0u8; 4]);
    block[tail_off + 4..tail_off + 8].copy_from_slice(&checksum.to_le_bytes());
}

/// Compute and write the CRC-32C checksum tail for an htree index node.
pub fn update_dx_node_csum(csum_seed: u32, block: &mut [u8], block_size: usize) {
    if csum_seed == 0 { return; }
    let csum = compute_dx_node_csum(csum_seed, block, block_size);
    write_dx_node_tail(block, block_size, csum);
}



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
/// If `csum_seed != 0`, appends a CRC-32C checksum tail (ext4_dir_entry_tail).
pub fn init_dir_block(block: &mut [u8], dir_ino: u32, parent_ino: u32,
                       csum_seed: u32) {
    let block_size = block.len();

    // Entry 0: "." -> dir_ino
    let dot_len = dirent_size(1); // "." is 1 byte
    block[0..4].copy_from_slice(&dir_ino.to_le_bytes());
    block[4..6].copy_from_slice(&(dot_len as u16).to_le_bytes());
    block[6] = 1; // name_len
    block[7] = EXT4_FT_DIR;
    block[8] = b'.';

    // Entry 1: ".." -> parent_ino, with rec_len covering the rest of the block
    // before the checksum tail (if any)
    let dotdot_rec_len = block_size - dot_len - EXT4_DIRENT_TAIL_SIZE;
    block[dot_len..dot_len + 4].copy_from_slice(&parent_ino.to_le_bytes());
    block[dot_len + 4..dot_len + 6].copy_from_slice(&(dotdot_rec_len as u16).to_le_bytes());
    block[dot_len + 6] = 2; // name_len
    block[dot_len + 7] = EXT4_FT_DIR;
    block[dot_len + 8] = b'.';
    block[dot_len + 9] = b'.';

    // Write checksum tail (generation = 0 for new directories)
    if csum_seed != 0 {
        let csum = compute_dir_leaf_csum(csum_seed, dir_ino, 0, block, block_size);
        write_dir_block_tail(block, block_size, csum);
    }
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

// ─── Htree write support ──────────────────────────────────────────

/// Offset of the dx_root_info in an htree root block.
/// After the fake dirent header (8 bytes: inode, rec_len, name_len, file_type),
/// the dx_root_info starts at offset 8.
pub const DX_ROOT_INFO_OFFSET: usize = 8;

/// Offset of the countlimit+entries in an htree root block.
/// dx_root_info is 8 bytes, so entries start at offset 16.
pub const DX_ROOT_ENTRIES_OFFSET: usize = 16;

/// Offset of the countlimit+entries in an htree internal node (dx_node).
/// A dx_node has a fake_dirent (8 bytes) followed by countlimit.
pub const DX_NODE_ENTRIES_OFFSET: usize = 8;

/// Parse the dx_root_info from an htree root block.
pub fn parse_dx_root_info(block: &[u8]) -> Ext4Result<Ext4DxRootInfo> {
    if block.len() < DX_ROOT_INFO_OFFSET + 8 {
        return Err(Ext4Error::InvalidExtentHeader);
    }
    let off = DX_ROOT_INFO_OFFSET;
    Ok(Ext4DxRootInfo {
        reserved_zero: u32::from_le_bytes([block[off], block[off+1], block[off+2], block[off+3]]),
        hash_version: block[off + 4],
        info_length: block[off + 5],
        indirect_levels: block[off + 6],
        unused_flags: block[off + 7],
    })
}

/// Get the entry count from an htree index block.
/// The countlimit is at `entries_off`, with count at entries_off+2.
pub fn dx_get_count(block: &[u8], entries_off: usize) -> u16 {
    if entries_off + 4 > block.len() { return 0; }
    u16::from_le_bytes([block[entries_off + 2], block[entries_off + 3]])
}

/// Get the entry limit from an htree index block.
pub fn dx_get_limit(block: &[u8], entries_off: usize) -> u16 {
    if entries_off + 4 > block.len() { return 0; }
    u16::from_le_bytes([block[entries_off], block[entries_off + 1]])
}

/// Compute the htree hash for a directory entry name.
pub fn htree_hash(sb: &Ext4Superblock, name: &str, hash_version: u8) -> u32 {
    let seed = sb.s_hash_seed;
    match hash_version {
        DX_HASH_LEGACY => half_md4_hash(name.as_bytes(), &seed),
        DX_HASH_HALF_MD4 => half_md4_hash(name.as_bytes(), &seed),
        DX_HASH_TEA => half_md4_hash(name.as_bytes(), &seed), // Simplified
        DX_HASH_LEGACY_UNSIGNED => half_md4_hash(name.as_bytes(), &seed),
        DX_HASH_HALF_MD4_UNSIGNED => half_md4_hash(name.as_bytes(), &seed),
        DX_HASH_TEA_UNSIGNED => half_md4_hash(name.as_bytes(), &seed),
        _ => half_md4_hash(name.as_bytes(), &seed),
    }
}

/// Walk the htree index to find the leaf block for a given name.
///
/// Returns the logical block number within the directory that should contain
/// the entry for `name`. Returns `Ok(None)` if the directory is not htree.
pub fn htree_find_leaf<FR>(
    sb: &Ext4Superblock,
    name: &str,
    mut read_block: FR,
) -> Ext4Result<Option<u64>>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let block_size = sb.block_size();
    let mut block = vec![0u8; block_size];
    read_block(0, &mut block)?;

    // Verify this is an htree directory
    let first_ino = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
    let first_rec_len = u16::from_le_bytes([block[4], block[5]]) as usize;
    if first_ino != 0 || first_rec_len != block_size {
        return Ok(None); // Not htree
    }

    let info = parse_dx_root_info(&block)?;
    let hash = htree_hash(sb, name, info.hash_version);

    let mut entries_off = DX_ROOT_ENTRIES_OFFSET;
    let mut current_level = info.indirect_levels;

    loop {
        let count = dx_get_count(&block, entries_off) as usize;
        if count == 0 {
            return Err(Ext4Error::NotFound);
        }

        // Real entries start at entries_off + 8 (entries[0] is the countlimit)
        let first_entry_off = entries_off + 8;

        // Binary search: find first entry with hash > target hash
        let mut lo = 0usize;
        let mut hi = count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let mid_off = first_entry_off + mid * 8;
            if mid_off + 8 > block.len() { break; }
            let entry_hash = u32::from_le_bytes([
                block[mid_off], block[mid_off+1],
                block[mid_off+2], block[mid_off+3],
            ]);
            if entry_hash <= hash {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        // Use the entry before the found one (or last if all hashes <= target)
        let idx = if lo > 0 { lo - 1 } else { 0 };
        let entry_off = first_entry_off + idx * 8;
        if entry_off + 8 > block.len() {
            return Err(Ext4Error::NotFound);
        }
        let _entry_hash = u32::from_le_bytes([
            block[entry_off], block[entry_off+1],
            block[entry_off+2], block[entry_off+3],
        ]);
        let entry_block = u32::from_le_bytes([
            block[entry_off+4], block[entry_off+5],
            block[entry_off+6], block[entry_off+7],
        ]);

        if current_level == 0 {
            // Leaf level — entry_block is the logical block number
            return Ok(Some(entry_block as u64));
        }

        // Intermediate index node — read the pointed block and descend
        read_block(entry_block as u64, &mut block)?;
        entries_off = DX_NODE_ENTRIES_OFFSET;
        current_level -= 1;
    }
}

/// Insert a new directory entry into an htree-indexed directory.
///
/// Hashes the name, finds the correct leaf block via htree,
/// and inserts the entry into that block.
///
/// Returns an error if the leaf block is full (caller should split
/// the leaf or expand the directory).
pub fn htree_insert_entry<FR, FW>(
    sb: &Ext4Superblock,
    dir_ino: u32,
    generation: u32,
    name: &str,
    file_type: u8,
    child_ino: u32,
    mut read_block: FR,
    mut write_block: FW,
) -> Ext4Result<bool>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    // Find the correct leaf block
    let leaf_block = match htree_find_leaf(sb, name, &mut read_block)? {
        Some(b) => b,
        None => return Err(Ext4Error::NotFound), // Not htree
    };

    // Read the leaf block and insert the entry
    let block_size = sb.block_size();
    let mut block = vec![0u8; block_size];
    read_block(leaf_block, &mut block)?;

    if insert_into_block(&mut block, child_ino, name, file_type) {
        if sb.has_metadata_csum() {
            update_dir_leaf_csum(
                crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid),
                dir_ino, generation, &mut block, block_size);
        }
        write_block(leaf_block, &block)?;
        Ok(true)
    } else {
        // Leaf is full — caller should split
        Ok(false)
    }
}

/// Remove a directory entry from an htree-indexed directory.
pub fn htree_remove_entry<FR, FW>(
    sb: &Ext4Superblock,
    dir_ino: u32,
    generation: u32,
    name: &str,
    mut read_block: FR,
    mut write_block: FW,
) -> Ext4Result<bool>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    let leaf_block = match htree_find_leaf(sb, name, &mut read_block)? {
        Some(b) => b,
        None => return Err(Ext4Error::NotFound),
    };

    let block_size = sb.block_size();
    let mut block = vec![0u8; block_size];
    read_block(leaf_block, &mut block)?;

    if remove_from_block(&mut block, name) {
        if sb.has_metadata_csum() {
            update_dir_leaf_csum(
                crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid),
                dir_ino, generation, &mut block, block_size);
        }
        write_block(leaf_block, &block)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Initialize a new htree directory root block.
///
/// In ext4 htree directories, the root block (block 0) contains BOTH
/// the `.` and `..` entries AND the htree index structures.
/// The `.` and `..` entries are stored as regular directory entries at
/// the beginning of block 0, and the htree index follows.
pub fn init_htree_dir<FW>(
    block: &mut [u8],
    dir_ino: u32,
    parent_ino: u32,
    hash_version: u8,
    csum_seed: u32,
    mut write_block: FW,
) -> Ext4Result<()>
where
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    let block_size = block.len();
    for byte in block.iter_mut() {
        *byte = 0;
    }

    // Entry 0: "." -> dir_ino (12 bytes)
    let dot_len = dirent_size(1);
    block[0..4].copy_from_slice(&dir_ino.to_le_bytes());
    block[4..6].copy_from_slice(&(dot_len as u16).to_le_bytes());
    block[6] = 1; // name_len
    block[7] = EXT4_FT_DIR;
    block[8] = b'.';

    // Entry 1: ".." -> parent_ino (12 bytes)
    let dot_dot_len = dirent_size(2);
    block[12..16].copy_from_slice(&parent_ino.to_le_bytes());
    block[16..18].copy_from_slice(&(dot_dot_len as u16).to_le_bytes());
    block[18] = 2; // name_len
    block[19] = EXT4_FT_DIR;
    block[20] = b'.';
    block[21] = b'.';

    // Entry 2: fake htree entry starting at offset 24
    // This entry has inode=0 and rec_len covering the rest of the block
    let htree_offset = 24usize;
    let htree_rec_len = block_size - htree_offset;
    block[htree_offset..htree_offset + 4].copy_from_slice(&0u32.to_le_bytes()); // inode=0
    block[htree_offset + 4..htree_offset + 6].copy_from_slice(&(htree_rec_len as u16).to_le_bytes());
    block[htree_offset + 6] = 0; // name_len
    block[htree_offset + 7] = 0; // file_type

    // dx_root_info at htree_offset + 8
    let info_off = htree_offset + 8;
    block[info_off..info_off + 4].copy_from_slice(&0u32.to_le_bytes()); // reserved_zero
    block[info_off + 4] = hash_version;
    block[info_off + 5] = 8; // info_length
    block[info_off + 6] = 0; // indirect_levels
    block[info_off + 7] = 0; // unused_flags

    // countlimit at info_off + 8 (which is htree_offset + 16)
    let entries_off = info_off + 8;
    let max_entries = ((block_size - entries_off - 4) / 8) as u16;
    block[entries_off..entries_off + 2].copy_from_slice(&max_entries.to_le_bytes()); // limit
    block[entries_off + 2..entries_off + 4].copy_from_slice(&1u16.to_le_bytes()); // count = 1

    // First real dx_entry at entries_off + 8: hash=0, block=0
    let dx_off = entries_off + 8;
    block[dx_off..dx_off + 4].copy_from_slice(&0u32.to_le_bytes()); // hash = 0
    block[dx_off + 4..dx_off + 8].copy_from_slice(&0u32.to_le_bytes()); // block = 0 (leaf)

    // Write dx_tail checksum
    if csum_seed != 0 {
        let csum = compute_dx_node_csum(csum_seed, block, block_size);
        write_dx_node_tail(block, block_size, csum);
    }

    write_block(0, block)?;
    Ok(())
}

/// Allocate and initialize a new empty directory block.
///
/// The new block contains a single empty entry with inode=0 and rec_len=block_size.
/// Returns the logical block number of the new block.
pub fn expand_dir<FR, FW, FA, FE>(
    sb: &Ext4Superblock,
    inode: &mut Ext4Inode,
    dir_ino: u32,
    mut read_block: FR,
    mut write_block: FW,
    mut alloc_block: FA,
    mut write_inode: FE,
) -> Ext4Result<u64>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    FA: FnMut() -> Ext4Result<u64>,
    FE: FnMut(&Ext4Inode) -> Ext4Result<()>,
{
    let block_size = sb.block_size() as u64;
    let file_size = inode.file_size();
    let current_blocks = file_size / block_size;

    // Allocate a physical block and insert it into the extent tree
    let phys_block = alloc_block()?;

    // Create an empty directory block with checksum tail reserved
    let mut block = vec![0u8; block_size as usize];
    let block_usize = block_size as usize;
    let emp_rec_len = (block_usize - EXT4_DIRENT_TAIL_SIZE) as u16;
    block[0..4].copy_from_slice(&0u32.to_le_bytes()); // inode=0
    block[4..6].copy_from_slice(&emp_rec_len.to_le_bytes()); // rec_len excludes tail
    
    // Compute checksum tail using actual dir_ino
    if sb.has_metadata_csum() {
        let csum_seed = crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid);
        let csum = compute_dir_leaf_csum(csum_seed, dir_ino, inode.i_generation, &block, block_usize);
        write_dir_block_tail(&mut block, block_usize, csum);
    }
    
    write_block(phys_block, &block)?;

    // Update file size BEFORE extent_insert, because extent_insert
    // calls write_inode internally and the inode must have the correct size.
    crate::inode::set_file_size(inode, file_size + block_size);

    // Extend the inode: add a new extent mapping (writes the inode via callback)
    crate::extent::extent_insert(
        sb, inode, current_blocks as u32, phys_block, 1, &mut write_inode,
    )?;

    Ok(current_blocks)
}

/// Generic directory insert: handles both linear and htree directories.
/// Inserts a new entry into a directory, expanding it if necessary.
pub fn insert_in_dir<FR, FW, FA, FE>(
    sb: &Ext4Superblock,
    inode: &mut Ext4Inode,
    dir_ino: u32,
    name: &str,
    file_type: u8,
    child_ino: u32,
    mut read_block: FR,
    mut write_block: FW,
    mut alloc_block: FA,
    mut write_inode: FE,
) -> Ext4Result<()>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    FA: FnMut() -> Ext4Result<u64>,
    FE: FnMut(&Ext4Inode) -> Ext4Result<()>,
{
    let block_size = sb.block_size();

    // Read block 0 to determine directory type
    let mut block = vec![0u8; block_size];
    read_block(0, &mut block)?;

    let first_ino = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
    let first_rec_len = u16::from_le_bytes([block[4], block[5]]) as usize;

    if first_ino == 0 && first_rec_len == block_size {
        // htree directory — use htree insert
        if htree_insert_entry(sb, dir_ino, inode.i_generation, name, file_type, child_ino, &mut read_block, &mut write_block)? {
            return Ok(());
        }
        // Leaf was full — need to split (not yet implemented)
        return Err(Ext4Error::ExtentTreeFull);
    }

    // Linear directory — try inserting in existing blocks
    let num_blocks = (inode.file_size() + block_size as u64 - 1) / block_size as u64;

    // Try each block
    for b in 0..num_blocks {
        read_block(b, &mut block)?;
        if insert_into_block(&mut block, child_ino, name, file_type) {
            if sb.has_metadata_csum() {
                update_dir_leaf_csum(
                    crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid),
                    dir_ino, inode.i_generation, &mut block, block_size);
            }
            write_block(b, &block)?;
            return Ok(());
        }
    }

    // Need to expand directory
    let new_block = expand_dir(sb, inode, dir_ino, &mut read_block, &mut write_block,
                               &mut alloc_block, &mut write_inode)?;
    read_block(new_block, &mut block)?;
    insert_into_block(&mut block, child_ino, name, file_type);
    if sb.has_metadata_csum() {
        update_dir_leaf_csum(
            crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid),
            dir_ino, inode.i_generation, &mut block, block_size);
    }
    write_block(new_block, &block)?;

    Ok(())
}

/// Generic directory remove: handles both linear and htree directories.
pub fn remove_in_dir<FR, FW>(
    sb: &Ext4Superblock,
    inode: &Ext4Inode,
    dir_ino: u32,
    name: &str,
    mut read_block: FR,
    mut write_block: FW,
) -> Ext4Result<bool>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    let block_size = sb.block_size();
    let num_blocks = (inode.file_size() + block_size as u64 - 1) / block_size as u64;

    let mut block = vec![0u8; block_size];

    // Check block 0 for htree signature
    read_block(0, &mut block)?;
    let first_ino = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
    let first_rec_len = u16::from_le_bytes([block[4], block[5]]) as usize;

    if first_ino == 0 && first_rec_len == block_size {
        // htree directory
        return htree_remove_entry(sb, dir_ino, inode.i_generation, name, &mut read_block, &mut write_block);
    }

    // Linear directory — scan all blocks
    for b in 0..num_blocks {
        read_block(b, &mut block)?;
        if remove_from_block(&mut block, name) {
            if sb.has_metadata_csum() {
                update_dir_leaf_csum(
                    crate::journal::crc32c_le(0xFFFFFFFF, &sb.s_uuid),
                    dir_ino, inode.i_generation, &mut block, block_size);
            }
            write_block(b, &block)?;
            return Ok(true);
        }
    }

    Ok(false)
}

// ─── Tests ───────────────────────────────────────────────────────────

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
