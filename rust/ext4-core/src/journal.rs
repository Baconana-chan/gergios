//! JBD2 (Journaling Block Device 2) — ext4 journal support.
//!
//! Provides parsing, recovery, and commit/checkpoint capabilities for the
//! ext4 journal, which is a write-ahead log used to ensure filesystem
//! consistency after crashes.
//!
//! **Byte order**: JBD2 uses **big-endian** (network byte order) for all
//! multi-byte fields, unlike ext4 which uses little-endian.
//!
//! **Journal location**: Either in a reserved inode (`s_journal_inum`, usually
//! inode 8) or on a separate device (`s_journal_dev`).
//!
//! **Recovery process**:
//! 1. Read the journal superblock
//! 2. Scan from `s_start` for committed transactions
//! 3. For each descriptor block, read the data blocks
//! 4. If a commit block follows, replay the transaction
//! 5. Skip revoked blocks
//!
//! **Commit process** (write path):
//! 1. Collect metadata blocks to journal
//! 2. Write descriptor block (header + tags for each block)
//! 3. Write the actual data blocks
//! 4. Write commit block (with optional checksum)
//! 5. Update journal superblock (bump sequence, advance start)
//!
//! **Checkpoint process**:
//! 1. Read committed descriptor blocks from the journal
//! 2. For each, read the data blocks and write them to final FS locations
//! 3. Advance s_start to reclaim journal space

use crate::types::*;

// ─── Constants ──────────────────────────────────────────────────────

/// JBD2 magic number (big-endian `0xC03B3998`).
pub const JBD2_MAGIC_NUMBER: u32 = 0xC03B3998;

/// Block type: descriptor — start of a transaction.
pub const JBD2_DESCRIPTOR_BLOCK: u32 = 1;
/// Block type: commit — marks successful transaction completion.
pub const JBD2_COMMIT_BLOCK: u32 = 2;
/// Block type: superblock (V1).
pub const JBD2_SUPERBLOCK_V1: u32 = 3;
/// Block type: superblock (V2).
pub const JBD2_SUPERBLOCK_V2: u32 = 4;
/// Block type: revoke — prevents replay of specific blocks.
pub const JBD2_REVOKE_BLOCK: u32 = 5;

/// Tag flags.
/// Data block was escaped (0x00 replaced with 0x20).
pub const JBD2_FLAG_ESCAPE: u16 = 0x0001;
/// Block's UUID matches the previous tag.
pub const JBD2_FLAG_SAME_UUID: u16 = 0x0002;
/// Block was deleted (should zero on replay).
pub const JBD2_FLAG_DELETED: u16 = 0x0004;
/// Last tag in this descriptor block.
pub const JBD2_FLAG_LAST_TAG: u16 = 0x0008;

/// Size of a journal block (always 1024 bytes).
pub const JBD2_BLOCK_SIZE: usize = 1024;

/// Default journal inode number (inode 8 on ext4).
pub const JBD2_DEFAULT_JOURNAL_INO: u32 = 8;

/// Default max journal block size (1024).
pub const JBD2_MIN_BLOCK_SIZE: usize = 1024;

// ─── Journal feature flags ────────────────────────────────────────

/// Compat feature: checksum (V1 CRC-32).
pub const JBD2_FEATURE_COMPAT_CHECKSUM: u32 = 0x00000001;

/// Incompat feature: checksum V2 (CRC-32C).
pub const JBD2_FEATURE_INCOMPAT_CSUM_V2: u32 = 0x00000001;
/// Incompat feature: checksum V3 (fixed-size tags + CRC-32C).
pub const JBD2_FEATURE_INCOMPAT_CSUM_V3: u32 = 0x00000002;

/// Commit block checksum type: CRC-32.
pub const JBD2_CRC32_CHKSUM: u8 = 1;
/// Commit block checksum type: CRC-32C.
pub const JBD2_CRC32C_CHKSUM: u8 = 2;

// ─── CRC-32 table-driven implementation ───────────────────────────

/// CRC-32 polynomial (reflected).
const CRC32_POLY: u32 = 0xEDB88320;
/// CRC-32C (Castagnoli) polynomial (reflected).
const CRC32C_POLY: u32 = 0x82F63B78;

/// Precomputed CRC-32 lookup table.
static CRC32_TABLE: [u32; 256] = make_crc_table(CRC32_POLY);
/// Precomputed CRC-32C lookup table.
static CRC32C_TABLE: [u32; 256] = make_crc_table(CRC32C_POLY);

/// Build a 256-entry CRC lookup table for the given reflected polynomial.
const fn make_crc_table(poly: u32) -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ poly;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Compute CRC-32 of `data` using the given table.
fn crc_with_table(table: &[u32; 256], data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ table[idx];
    }
    crc ^ 0xFFFFFFFF
}

/// Compute CRC-32C over `data` seeded with `~0`.
pub fn crc32c(data: &[u8]) -> u32 {
    crc_with_table(&CRC32C_TABLE, data)
}

/// Compute CRC-32 over `data` seeded with `~0`.
pub fn crc32(data: &[u8]) -> u32 {
    crc_with_table(&CRC32_TABLE, data)
}

/// Raw CRC-32C (no final XOR) — matches the kernel's `crc32c_le`.
/// This is the same as `crc32c_seeded` but named to match the kernel API.
/// Use this for ext4 metadata_csum computations.
pub fn crc32c_le(initial: u32, data: &[u8]) -> u32 {
    let mut crc = initial;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32C_TABLE[idx];
    }
    crc
}

/// CRC-32C with a specific initial value (like jbd2_chksum).
pub fn crc32c_seeded(initial: u32, data: &[u8]) -> u32 {
    let mut crc = initial;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32C_TABLE[idx];
    }
    crc
}

// ─── Helpers for big-endian reads ───────────────────────────────────

#[inline]
fn be_u32(data: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

#[inline]
fn be_u16(data: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([data[off], data[off + 1]])
}

#[inline]
fn be_i32(data: &[u8], off: usize) -> i32 {
    i32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

// ─── Helpers for big-endian writes ──────────────────────────────────

#[inline]
fn write_be_u32(data: &mut [u8], off: usize, val: u32) {
    data[off..off + 4].copy_from_slice(&val.to_be_bytes());
}

#[inline]
fn write_be_u16(data: &mut [u8], off: usize, val: u16) {
    data[off..off + 2].copy_from_slice(&val.to_be_bytes());
}

#[inline]
fn write_be_i32(data: &mut [u8], off: usize, val: i32) {
    data[off..off + 4].copy_from_slice(&val.to_be_bytes());
}

#[inline]
fn write_be_u64(data: &mut [u8], off: usize, val: u64) {
    data[off..off + 8].copy_from_slice(&val.to_be_bytes());
}

// ─── Types ──────────────────────────────────────────────────────────

/// JBD2 journal block header (12 bytes, common to all journal blocks).
/// Fields are big-endian.
#[derive(Clone, Copy, Debug)]
pub struct Jbd2Header {
    pub h_magic: u32,
    pub h_blocktype: u32,
    pub h_sequence: u32,
}

/// JBD2 journal superblock (1024 bytes, at journal offset 0).
#[derive(Clone, Debug)]
pub struct Jbd2Superblock {
    pub header: Jbd2Header,
    pub s_first: u32,
    pub s_maxlen: u32,
    pub s_sequence: u32,
    pub s_start: u32,
    pub s_errno: i32,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_blocksize: u32,
    pub s_nr_users: u32,
    pub s_max_transaction: u32,
    pub s_max_trans_data: u32,
    pub s_uuid: [u8; 16],
    pub s_commit_interval: u32,
}

/// Block tag — describes a single block within a descriptor block.
#[derive(Clone, Copy, Debug)]
pub struct Jbd2BlockTag {
    pub t_blocknr: u32,
    pub t_flags: u16,
    pub t_blocknr_high: u16,
}

impl Jbd2BlockTag {
    pub fn block_number(&self) -> u64 {
        (self.t_blocknr as u64) | ((self.t_blocknr_high as u64) << 32)
    }
}

/// Descriptor block — lists blocks in a transaction.
#[derive(Clone, Debug)]
pub struct Jbd2DescriptorBlock {
    pub header: Jbd2Header,
    pub tags: Vec<Jbd2BlockTag>,
}

/// Commit block — marks a transaction as committed.
///
/// With checksum features, the commit block contains additional fields:
/// - V1/V2 compat (JBD2_FEATURE_COMPAT_CHECKSUM): CRC-32 at offset 12
/// - V2/V3 (JBD2_FEATURE_INCOMPAT_CSUM_V2/V3): CRC-32C at offset 16
#[derive(Clone, Debug)]
pub struct Jbd2CommitBlock {
    pub header: Jbd2Header,
    pub chksum_type: u8,  // 0 = none, 1 = CRC-32, 2 = CRC-32C
    pub chksum_size: u8,  // 4 for CRC-32/CRC-32C
    pub chksum: u32,       // Stored checksum value (0 if not present)
    pub commit_sec: u64,   // Commit timestamp
    pub commit_nsec: u32,  // Commit nanoseconds
}

/// Revoke block — prevents replay of specific blocks.
#[derive(Clone, Debug)]
pub struct Jbd2RevokeBlock {
    pub header: Jbd2Header,
    pub r_count: u32,
    pub revoke_records: Vec<u64>,
}

/// A single journal transaction ready for replay.
#[derive(Clone, Debug)]
pub struct JournalTransaction {
    pub sequence: u32,
    pub blocks: Vec<JournalBlock>,
}

/// A single journaled block within a transaction.
#[derive(Clone, Debug)]
pub struct JournalBlock {
    pub dest_block: u64,
    pub data: Vec<u8>,
    pub flags: u16,
}

// ─── Journal state for write path ────────────────────────────────────

/// Runtime state for a journal that supports both commit and checkpoint.
///
/// Tracks the current journal superblock, write position, and a pending
/// transaction buffer. The caller manages the actual I/O callbacks.
#[derive(Clone, Debug)]
pub struct Journal {
    /// Parsed journal superblock (cached in-memory copy).
    pub sb: Jbd2Superblock,
    /// Number of blocks in the journal device.
    pub maxlen: u32,
    /// First usable journal block (from superblock).
    pub first: u32,
    /// Current write position in journal (logical block number).
    pub next_log_block: u32,
    /// Whether to use CSUM_V3 6-byte tags.
    pub csum_v3: bool,
    /// Whether to write checksummed commit blocks.
    pub csum_commit: bool,
}

impl Journal {
    /// Create a new journal state from a parsed superblock.
    pub fn new(sb: Jbd2Superblock) -> Self {
        let csum_v3 = sb.s_feature_incompat & JBD2_FEATURE_INCOMPAT_CSUM_V3 != 0;
        let csum_commit = csum_v3
            || (sb.s_feature_incompat & JBD2_FEATURE_INCOMPAT_CSUM_V2 != 0)
            || (sb.s_feature_compat & JBD2_FEATURE_COMPAT_CHECKSUM != 0);

        // Next write position starts at the end of the last committed transaction,
        // or at s_first if the journal is clean.
        let next_log_block = if sb.is_clean() {
            sb.s_first
        } else {
            // Start writing after the last known committed sequence
            // For simplicity, start at s_first + 1 (after superblock)
            sb.s_first
        };

        let first = sb.s_first;
        let maxlen = sb.s_maxlen;

        Journal {
            sb,
            maxlen,
            first,
            next_log_block,
            csum_v3,
            csum_commit,
        }
    }

    /// Compute the number of free blocks available in the journal.
    /// This is a conservative estimate — actual space depends on how many
    /// blocks are occupied by un-checkpointed transactions.
    pub fn free_blocks(&self) -> u32 {
        // The journal is a circular buffer:
        // if next_log_block >= s_start: free = maxlen - (next_log_block - s_start)
        // else: free = s_start - next_log_block
        // Reserve 1 block for the journal superblock (block 0)
        // and 2 blocks for the descriptor + commit overhead.
        let used = if self.next_log_block >= self.sb.s_start {
            self.next_log_block - self.sb.s_start
        } else {
            self.maxlen - (self.sb.s_start - self.next_log_block)
        };
        // Conservative: reserve superblock (block 0) + 2 for overhead
        let usable = self.maxlen.saturating_sub(3); // SB + descriptor + commit min
        usable.saturating_sub(used)
    }

    /// Check if there's enough space for a transaction with `num_data_blocks`.
    /// Total blocks needed = 1 (descriptor) + num_data_blocks + 1 (commit).
    pub fn has_space_for(&self, num_data_blocks: u32) -> bool {
        let needed = 1 + num_data_blocks + 1;
        self.free_blocks() >= needed
    }

    /// Advance the write position by `n` blocks (circular).
    pub fn advance(&mut self, n: u32) {
        self.next_log_block = (self.next_log_block + n) % self.maxlen;
    }
}

// ─── Serialization helpers (big-endian write) ────────────────────────

/// Serialize a JBD2 header into a buffer at the given offset.
fn serialize_header(data: &mut [u8], off: usize, blocktype: u32, sequence: u32) {
    write_be_u32(data, off, JBD2_MAGIC_NUMBER);
    write_be_u32(data, off + 4, blocktype);
    write_be_u32(data, off + 8, sequence);
}

/// Serialize a descriptor block into a pre-allocated 1024-byte buffer.
///
/// `tags` are the block tags (without UUIDs — uses SAME_UUID flag).
/// If `csum_v3`, uses 6-byte tags; otherwise 8-byte.
/// If `csum_v3`, writes a CRC-32C checksum tail in the last 4 bytes.
///
/// Returns the number of tags that fit (may be < tags.len() if too many).
pub fn serialize_descriptor_block(
    block: &mut [u8],
    sequence: u32,
    tags: &[(u64, u16)],
    csum_v3: bool,
) -> usize {
    if block.len() < JBD2_BLOCK_SIZE {
        return 0;
    }

    // Zero the entire block first
    for byte in block.iter_mut() { *byte = 0; }

    // Write header
    serialize_header(block, 0, JBD2_DESCRIPTOR_BLOCK, sequence);

    let tag_size: usize = if csum_v3 { 6 } else { 8 };
    let mut off = 12; // After header

    for (i, &(block_nr, flags)) in tags.iter().enumerate() {
        // Check if we have room for this tag + at least 4 bytes for checksum tail
        let tail_reserve = if csum_v3 { 4 } else { 0 };
        if off + tag_size + tail_reserve > JBD2_BLOCK_SIZE {
            // Need to make this the last tag and break
            // First, mark the previous tag (if any) as last
            if i > 0 {
                // Rewrite the previous tag's flags with LAST_TAG
                let prev_off = 12 + (i - 1) * tag_size;
                let prev_flags = u16::from_be_bytes([block[prev_off + 4], block[prev_off + 5]]);
                block[prev_off + 4..prev_off + 6].copy_from_slice(
                    &(prev_flags | JBD2_FLAG_LAST_TAG).to_be_bytes()
                );
            }
            break;
        }

        // Write tag
        let blocknr_lo = block_nr as u32;
        let blocknr_hi = (block_nr >> 32) as u16;
        write_be_u32(block, off, blocknr_lo);
        let mut tag_flags = flags;
        if !csum_v3 {
            write_be_u16(block, off + 6, blocknr_hi);
        } else {
            // CSUM_V3: no blocknr_high field
        }
        tag_flags |= JBD2_FLAG_SAME_UUID; // Use same UUID for all tags
        write_be_u16(block, off + 4, tag_flags);

        off += tag_size;

        // If this is the last tag, mark it
        if i == tags.len() - 1 || off + tag_size + tail_reserve > JBD2_BLOCK_SIZE {
            // Rewrite the flags with LAST_TAG set
            let flag_off = off - tag_size + 4;
            let current_flags = u16::from_be_bytes([block[flag_off], block[flag_off + 1]]);
            block[flag_off..flag_off + 2].copy_from_slice(
                &(current_flags | JBD2_FLAG_LAST_TAG).to_be_bytes()
            );
            let count = i + 1;

            // CSUM_V3: write checksum tail
            if csum_v3 {
                let tail_off = JBD2_BLOCK_SIZE - 4;
                let mut zeroed = block.to_vec();
                zeroed[tail_off..tail_off + 4].copy_from_slice(&[0u8; 4]);
                let csum = crc32c_seeded(0x00000000, &zeroed) ^ 0xFFFFFFFF;
                write_be_u32(block, tail_off, csum);
            }

            return count;
        }
    }

    // If we have tags but none were handled as last (shouldn't happen with >0 tags)
    if !tags.is_empty() {
        // Mark the last tag as last
        let flag_off = off - tag_size + 4;
        let current_flags = u16::from_be_bytes([block[flag_off], block[flag_off + 1]]);
        block[flag_off..flag_off + 2].copy_from_slice(
            &(current_flags | JBD2_FLAG_LAST_TAG).to_be_bytes()
        );
    }

    // CSUM_V3: write checksum tail even for empty descriptor
    if csum_v3 && !tags.is_empty() {
        let tail_off = JBD2_BLOCK_SIZE - 4;
        let mut zeroed = block.to_vec();
        zeroed[tail_off..tail_off + 4].copy_from_slice(&[0u8; 4]);
        let csum = crc32c_seeded(0x00000000, &zeroed) ^ 0xFFFFFFFF;
        write_be_u32(block, tail_off, csum);
    }

    tags.len()
}

/// Serialize a commit block into a pre-allocated 1024-byte buffer.
///
/// If `csum_v3` features are enabled, writes a V3 extended commit header
/// with CRC-32C checksum.
pub fn serialize_commit_block(
    block: &mut [u8],
    sequence: u32,
    csum_v3: bool,
) {
    if block.len() < JBD2_BLOCK_SIZE {
        return;
    }

    // Zero the entire block first
    for byte in block.iter_mut() { *byte = 0; }

    // Write header
    serialize_header(block, 0, JBD2_COMMIT_BLOCK, sequence);

    if csum_v3 {
        // V3 extended commit header:
        // offset 12: chksum_type (1 byte)
        // offset 13: chksum_size (1 byte)
        // offset 14-15: padding
        // offset 16-19: chksum (4 bytes, will compute)
        // offset 20-27: commit_sec (8 bytes)
        // offset 28-31: commit_nsec (4 bytes)
        block[12] = JBD2_CRC32C_CHKSUM;
        block[13] = 4; // chksum_size
        // Padding at 14-15 is already zero

        // Write timestamp
        // Use 0 for now — caller can set via set_commit_timestamp
        write_be_u64(block, 20, 0); // commit_sec
        write_be_u32(block, 28, 0); // commit_nsec

        // Compute CRC-32C over the block with checksum field zeroed
        let mut zeroed = block.to_vec();
        zeroed[16..20].copy_from_slice(&[0u8; 4]);
        let csum = crc32c_seeded(0x00000000, &zeroed) ^ 0xFFFFFFFF;
        write_be_u32(block, 16, csum);
    } else {
        // Basic commit block (just header, no checksum)
        // offset 12+: optional CRC-32 for V1/V2 compat
        // We skip that for simplicity — basic commit is just the header
    }
}

/// Set the commit timestamp in a serialized commit block.
pub fn set_commit_timestamp(block: &mut [u8], sec: u64, nsec: u32) {
    if block.len() < 32 {
        return;
    }
    // Only set if V3 extended header is present (chksum_type == 2 at offset 12)
    if block[12] == JBD2_CRC32C_CHKSUM && block[13] == 4 {
        write_be_u64(block, 20, sec);
        write_be_u32(block, 28, nsec);

        // Recompute checksum with new timestamp
        let mut zeroed = block.to_vec();
        zeroed[16..20].copy_from_slice(&[0u8; 4]);
        let csum = crc32c_seeded(0x00000000, &zeroed) ^ 0xFFFFFFFF;
        write_be_u32(block, 16, csum);
    }
}

/// Serialize a journal superblock into a pre-allocated 1024-byte buffer.
///
/// This is used when updating the journal superblock after a commit.
pub fn serialize_journal_superblock(sb: &Jbd2Superblock, data: &mut [u8]) {
    if data.len() < JBD2_BLOCK_SIZE {
        return;
    }
    // Zero the block
    for byte in data.iter_mut() { *byte = 0; }

    // Header
    serialize_header(data, 0, JBD2_SUPERBLOCK_V2, sb.header.h_sequence);

    // Superblock fields
    write_be_u32(data, 12, sb.s_blocksize);
    write_be_u32(data, 16, sb.s_maxlen);
    write_be_u32(data, 20, sb.s_first);
    write_be_u32(data, 24, sb.s_sequence);
    write_be_u32(data, 28, sb.s_start);
    write_be_i32(data, 32, sb.s_errno);
    write_be_u32(data, 36, sb.s_feature_compat);
    write_be_u32(data, 40, sb.s_feature_incompat);
    write_be_u32(data, 44, sb.s_feature_ro_compat);
    data[48..64].copy_from_slice(&sb.s_uuid);
    write_be_u32(data, 64, sb.s_nr_users);
    write_be_u32(data, 68, sb.s_max_transaction);
    write_be_u32(data, 72, sb.s_max_trans_data);
    write_be_u32(data, 76, sb.s_commit_interval);
}

// ─── Parsing ────────────────────────────────────────────────────────

/// Parse a JBD2 header from a buffer (12 bytes from `off`).
pub fn parse_jbd2_header(data: &[u8], off: usize) -> Ext4Result<Jbd2Header> {
    if off + 12 > data.len() {
        return Err(Ext4Error::IoError);
    }
    Ok(Jbd2Header {
        h_magic: be_u32(data, off),
        h_blocktype: be_u32(data, off + 4),
        h_sequence: be_u32(data, off + 8),
    })
}

/// Parse the journal superblock from a 1024-byte buffer.
pub fn parse_journal_superblock(data: &[u8]) -> Ext4Result<Jbd2Superblock> {
    if data.len() < JBD2_BLOCK_SIZE {
        return Err(Ext4Error::IoError);
    }

    let header = parse_jbd2_header(data, 0)?;

    if header.h_magic != JBD2_MAGIC_NUMBER {
        return Err(Ext4Error::InvalidMagic);
    }

    if header.h_blocktype != JBD2_SUPERBLOCK_V1 && header.h_blocktype != JBD2_SUPERBLOCK_V2 {
        return Err(Ext4Error::UnsupportedIncompat(header.h_blocktype));
    }

    Ok(Jbd2Superblock {
        header,
        s_blocksize: be_u32(data, 12),
        s_maxlen: be_u32(data, 16),
        s_first: be_u32(data, 20),
        s_sequence: be_u32(data, 24),
        s_start: be_u32(data, 28),
        s_errno: be_i32(data, 32),
        s_feature_compat: be_u32(data, 36),
        s_feature_incompat: be_u32(data, 40),
        s_feature_ro_compat: be_u32(data, 44),
        s_uuid: {
            let mut u = [0u8; 16];
            u.copy_from_slice(&data[48..64]);
            u
        },
        s_nr_users: be_u32(data, 64),
        s_max_transaction: be_u32(data, 68),
        s_max_trans_data: be_u32(data, 72),
        s_commit_interval: be_u32(data, 76),
    })
}

impl Jbd2Superblock {
    /// Check if the journal is clean (no recovery needed).
    /// Returns `true` if s_start == 0 (journal is empty/clean).
    pub fn is_clean(&self) -> bool {
        self.s_start == 0 || self.s_start == self.s_sequence
    }

    /// Convert a journal logical block number to a journal device block number.
    pub fn journal_block_to_dev_block(&self, log_block: u32) -> u64 {
        (self.s_first as u64) + (log_block as u64)
    }

    /// Get the readable journal info.
    pub fn info_string(&self) -> String {
        format!(
            "jbd2 journal: blocks={} first={} sequence={} start={} clean={} errno={}",
            self.s_maxlen,
            self.s_first,
            self.s_sequence,
            self.s_start,
            self.is_clean(),
            self.s_errno,
        )
    }
}

/// Parse a descriptor block from a buffer.
pub fn parse_descriptor_block(data: &[u8], header: &Jbd2Header, csum_v3: bool) -> Ext4Result<Jbd2DescriptorBlock> {
    if header.h_blocktype != JBD2_DESCRIPTOR_BLOCK {
        return Err(Ext4Error::InvalidExtentHeader);
    }

    let tag_size: usize = if csum_v3 { 6 } else { 8 };
    let mut tags = Vec::new();
    let mut off = 12;

    loop {
        if off + tag_size > data.len() {
            break;
        }

        let t_blocknr = be_u32(data, off);
        let t_flags = be_u16(data, off + 4);
        let t_blocknr_high = if csum_v3 {
            0
        } else {
            be_u16(data, off + 6)
        };

        tags.push(Jbd2BlockTag {
            t_blocknr,
            t_flags,
            t_blocknr_high,
        });

        off += tag_size;

        if t_flags & JBD2_FLAG_SAME_UUID == 0 {
            off += 16;
        }

        if t_flags & JBD2_FLAG_LAST_TAG != 0 {
            break;
        }
    }

    Ok(Jbd2DescriptorBlock {
        header: *header,
        tags,
    })
}

/// Validate the checksum tail of a descriptor block (CSUM_V2/V3).
pub fn validate_descriptor_checksum(block: &[u8], sb: &Jbd2Superblock) -> bool {
    let has_csum_v2or3 = (sb.s_feature_incompat & JBD2_FEATURE_INCOMPAT_CSUM_V2 != 0)
        || (sb.s_feature_incompat & JBD2_FEATURE_INCOMPAT_CSUM_V3 != 0);

    if !has_csum_v2or3 {
        return true;
    }

    if block.len() < 4 {
        return false;
    }

    let tail_off = block.len() - 4;
    let stored_csum = be_u32(block, tail_off);

    let mut data = block.to_vec();
    data[tail_off..tail_off + 4].copy_from_slice(&[0u8; 4]);

    let computed = crc32c_seeded(0x00000000, &data) ^ 0xFFFFFFFF;

    stored_csum == computed
}

/// Validate the checksum in a commit block.
pub fn validate_commit_checksum(block: &[u8], commit: &Jbd2CommitBlock, _sb: &Jbd2Superblock) -> bool {
    if commit.chksum_type == 0 || commit.chksum == 0 {
        return true;
    }

    if commit.chksum_type == JBD2_CRC32_CHKSUM {
        true
    } else if commit.chksum_type == JBD2_CRC32C_CHKSUM {
        if block.len() < 20 {
            return false;
        }

        let mut data = block.to_vec();
        data[16..20].copy_from_slice(&[0u8; 4]);

        let computed = crc32c_seeded(0x00000000, &data) ^ 0xFFFFFFFF;

        commit.chksum == computed
    } else {
        false
    }
}

/// Parse a commit block from a buffer.
pub fn parse_commit_block(data: &[u8], header: &Jbd2Header) -> Ext4Result<Jbd2CommitBlock> {
    if header.h_blocktype != JBD2_COMMIT_BLOCK {
        return Err(Ext4Error::InvalidExtentHeader);
    }

    let mut chksum_type = 0u8;
    let mut chksum_size = 0u8;
    let mut chksum = 0u32;
    let mut commit_sec = 0u64;
    let mut commit_nsec = 0u32;

    if data.len() >= 16 {
        let maybe_type = data[12];
        let maybe_size = data[13];

        if (maybe_type == JBD2_CRC32_CHKSUM || maybe_type == JBD2_CRC32C_CHKSUM)
            && maybe_size == 4
        {
            chksum_type = maybe_type;
            chksum_size = maybe_size;
            if data.len() >= 20 {
                chksum = be_u32(data, 16);
            }
            if data.len() >= 32 {
                commit_sec = u64::from_be_bytes([
                    data[20], data[21], data[22], data[23],
                    data[24], data[25], data[26], data[27],
                ]);
                commit_nsec = u32::from_be_bytes([
                    data[28], data[29], data[30], data[31],
                ]);
            }
        } else if data.len() >= 16 && maybe_type != 0 {
            chksum = be_u32(data, 12);
            if chksum != 0 {
                chksum_type = JBD2_CRC32_CHKSUM;
                chksum_size = 4;
            }
        }
    }

    Ok(Jbd2CommitBlock {
        header: *header,
        chksum_type,
        chksum_size,
        chksum,
        commit_sec,
        commit_nsec,
    })
}

/// Parse a revoke block from a buffer.
pub fn parse_revoke_block(data: &[u8], header: &Jbd2Header) -> Ext4Result<Jbd2RevokeBlock> {
    if header.h_blocktype != JBD2_REVOKE_BLOCK {
        return Err(Ext4Error::InvalidExtentHeader);
    }

    if data.len() < 16 {
        return Err(Ext4Error::IoError);
    }

    let r_count = be_u32(data, 12);
    let num_records = (r_count as usize - 12) / 8;
    let mut revoke_records = Vec::with_capacity(num_records);

    for i in 0..num_records {
        let off = 16 + i * 8;
        if off + 8 > data.len() {
            break;
        }
        let block = u64::from_be_bytes([
            data[off], data[off+1], data[off+2], data[off+3],
            data[off+4], data[off+5], data[off+6], data[off+7],
        ]);
        revoke_records.push(block);
    }

    Ok(Jbd2RevokeBlock {
        header: *header,
        r_count,
        revoke_records,
    })
}

// ─── Journal scanner (identifies transaction boundaries) ────────────

/// Result of scanning one journal block.
#[derive(Debug)]
pub enum ScanResult {
    /// Descriptor block found with its tags.
    Descriptor(Jbd2DescriptorBlock),
    /// Commit block found.
    Commit(Jbd2CommitBlock),
    /// Revoke block found.
    Revoke(Jbd2RevokeBlock),
    /// Invalid checksum — block was corrupt.
    InvalidChecksum,
    /// Not a journal block (superblock or unknown).
    Other,
    /// End of valid journal data.
    End,
}

/// Scan a journal block and identify its type.
pub fn scan_journal_block(
    data: &[u8],
    sequence: u32,
    csum_v3: bool,
    sb: Option<&Jbd2Superblock>,
) -> Ext4Result<ScanResult> {
    let header = parse_jbd2_header(data, 0)?;

    if header.h_magic != JBD2_MAGIC_NUMBER {
        return Ok(ScanResult::Other);
    }

    let seq_diff = if header.h_sequence >= sequence {
        header.h_sequence - sequence
    } else {
        sequence - header.h_sequence
    };

    if seq_diff > 256 && header.h_sequence > 0 {
        return Ok(ScanResult::End);
    }

    match header.h_blocktype {
        JBD2_DESCRIPTOR_BLOCK => {
            let desc = parse_descriptor_block(data, &header, csum_v3)?;
            if let Some(sb_ref) = sb {
                if !validate_descriptor_checksum(data, sb_ref) {
                    return Ok(ScanResult::InvalidChecksum);
                }
            }
            Ok(ScanResult::Descriptor(desc))
        }
        JBD2_COMMIT_BLOCK => {
            let commit = parse_commit_block(data, &header)?;
            if let Some(sb_ref) = sb {
                if !validate_commit_checksum(data, &commit, sb_ref) {
                    return Ok(ScanResult::InvalidChecksum);
                }
            }
            Ok(ScanResult::Commit(commit))
        }
        JBD2_REVOKE_BLOCK => {
            let revoke = parse_revoke_block(data, &header)?;
            Ok(ScanResult::Revoke(revoke))
        }
        JBD2_SUPERBLOCK_V1 | JBD2_SUPERBLOCK_V2 => Ok(ScanResult::Other),
        _ => Ok(ScanResult::Other),
    }
}

// ─── Journal recovery ───────────────────────────────────────────────

/// Configuration for journal recovery.
#[derive(Clone, Debug)]
pub struct RecoveryConfig {
    /// Number of blocks in the journal device.
    pub journal_blocks: u32,
    /// First usable journal block (from superblock).
    pub first_block: u32,
}

/// Recover (replay) committed transactions from the journal.
pub fn recover_journal<FR, FW>(
    sb: &Jbd2Superblock,
    config: &RecoveryConfig,
    mut read_journal_block: FR,
    mut write_fs_block: FW,
) -> Ext4Result<u32>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    if sb.is_clean() {
        return Ok(0);
    }

    let mut recovered_transactions = 0u32;
    let mut revoke_set: Vec<u64> = Vec::new();
    let mut pos = sb.s_start;

    let max_scan = config.journal_blocks;
    let first = config.first_block;

    struct CommittedTx {
        _seq: u32,
        descriptor_block_pos: u32,
        tags: Vec<Jbd2BlockTag>,
    }

    let mut transactions: Vec<CommittedTx> = Vec::new();
    let mut current_tags: Vec<Jbd2BlockTag> = Vec::new();
    let mut current_seq = 0u32;
    let mut in_transaction = false;
    let mut descriptor_pos = 0u32;
    let mut blocks_since_descriptor = 0u32;

    for _ in 0..max_scan {
        let dev_block = ((first + pos) % config.journal_blocks) as u64;
        let mut buf = vec![0u8; JBD2_BLOCK_SIZE];
        if read_journal_block(dev_block, &mut buf).is_err() {
            break;
        }

        let csum_v3 = sb.s_feature_incompat & JBD2_FEATURE_INCOMPAT_CSUM_V3 != 0;
        match scan_journal_block(&buf, sb.s_sequence, csum_v3, Some(sb))? {
            ScanResult::Descriptor(desc) => {
                if in_transaction {
                    current_tags.clear();
                }
                current_tags = desc.tags;
                current_seq = desc.header.h_sequence;
                descriptor_pos = pos;
                in_transaction = true;
                blocks_since_descriptor = 0;
            }
            ScanResult::Commit(commit) => {
                if in_transaction && commit.header.h_sequence == current_seq {
                    transactions.push(CommittedTx {
                        _seq: current_seq,
                        descriptor_block_pos: descriptor_pos,
                        tags: current_tags.clone(),
                    });
                    recovered_transactions += 1;
                }
                in_transaction = false;
                current_tags.clear();
            }
            ScanResult::Revoke(revoke) => {
                revoke_set.extend(revoke.revoke_records);
            }
            ScanResult::InvalidChecksum => {
                break;
            }
            ScanResult::Other | ScanResult::End => {
                if in_transaction {
                    blocks_since_descriptor += 1;
                    if blocks_since_descriptor > current_tags.len() as u32 + 2 {
                        in_transaction = false;
                        current_tags.clear();
                    }
                }
            }
        }

        pos += 1;
        if pos >= config.journal_blocks {
            pos = 0;
        }
        if pos == sb.s_start {
            break;
        }
    }

    for tx in &transactions {
        for (i, tag) in tx.tags.iter().enumerate() {
            let dest = tag.block_number();

            if revoke_set.contains(&dest) {
                continue;
            }

            let data_pos = (tx.descriptor_block_pos as u64 + 1 + i as u64) % config.journal_blocks as u64;
            let dev_block = ((first as u64 + data_pos) % config.journal_blocks as u64) as u64;

            let mut data_buf = vec![0u8; JBD2_BLOCK_SIZE];
            if read_journal_block(dev_block, &mut data_buf).is_err() {
                continue;
            }

            if tag.t_flags & JBD2_FLAG_DELETED != 0 {
                let zero_buf = vec![0u8; JBD2_BLOCK_SIZE];
                write_fs_block(dest, &zero_buf)?;
            } else {
                if tag.t_flags & JBD2_FLAG_ESCAPE != 0 {
                    unescape_block(&mut data_buf);
                }
                write_fs_block(dest, &data_buf)?;
            }
        }
    }

    Ok(recovered_transactions)
}

// ─── Journal commit — write path ─────────────────────────────────────

/// Commit a set of metadata blocks to the journal.
///
/// This is the core commit function: it serializes a descriptor block,
/// writes the data blocks, and appends a commit block.
///
/// Returns the new journal superblock after the commit, so the caller can
/// persist it.
///
/// # Arguments
///
/// * `journal` - Journal state (tracks write position, superblock, features)
/// * `blocks` - List of (dest_block, data, flags) to commit
/// * `write_journal_block` - Callback to write a raw block to the journal device
///
/// # Returns
///
/// The number of blocks written to the journal.
pub fn journal_commit<FW>(
    journal: &mut Journal,
    blocks: &[(u64, Vec<u8>, u16)],
    mut write_journal_block: FW,
) -> Ext4Result<u32>
where
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    if blocks.is_empty() {
        return Ok(0);
    }

    let maxlen = journal.maxlen;
    let first = journal.first;
    let sequence = journal.sb.s_sequence;

    // Check if we have space
    if !journal.has_space_for(blocks.len() as u32) {
        return Err(Ext4Error::NoSpace);
    }

    let mut pos = journal.next_log_block;

    // 1. Write descriptor block
    let mut desc_block = vec![0u8; JBD2_BLOCK_SIZE];
    let tags: Vec<(u64, u16)> = blocks.iter().map(|(dest, _, flags)| (*dest, *flags)).collect();
    serialize_descriptor_block(&mut desc_block, sequence, &tags, journal.csum_v3);
    let dev_block = (first + pos) % maxlen;
    write_journal_block(dev_block as u64, &desc_block)?;
    pos = (pos + 1) % maxlen;

    // 2. Write data blocks
    for (dest, data, flags) in blocks {
        let dev_block = (first + pos) % maxlen;
        let mut buf = data.clone();
        // Caller must set JBD2_FLAG_ESCAPE and zero the first 4 bytes if the
        // block starts with JBD2_MAGIC_NUMBER. We write the data as-is to the journal.
        // On checkpoint, if JBD2_FLAG_ESCAPE is set, unescape_block will restore the magic.
        write_journal_block(dev_block as u64, &buf)?;
        let _ = (dest, flags); // used in checkpoint
        pos = (pos + 1) % maxlen;
    }

    // 3. Write commit block
    let mut commit_block = vec![0u8; JBD2_BLOCK_SIZE];
    serialize_commit_block(&mut commit_block, sequence, journal.csum_v3);
    // Set timestamp
    let now_sec = 0u64; // TODO: get real time
    let now_nsec = 0u32;
    set_commit_timestamp(&mut commit_block, now_sec, now_nsec);
    let dev_block = (first + pos) % maxlen;
    write_journal_block(dev_block as u64, &commit_block)?;
    pos = (pos + 1) % maxlen;

    // 4. Update journal state
    journal.sb.s_sequence = sequence.wrapping_add(1);
    journal.sb.s_start = journal.next_log_block;
    journal.sb.s_errno = 0;
    journal.sb.header.h_sequence = sequence.wrapping_add(1);
    journal.next_log_block = pos;

    // 5. Write updated journal superblock (block 0 of journal device)
    let mut sb_block = vec![0u8; JBD2_BLOCK_SIZE];
    serialize_journal_superblock(&journal.sb, &mut sb_block);
    write_journal_block(0, &sb_block)?;

    let blocks_written = 1 + blocks.len() as u32 + 1;
    Ok(blocks_written)
}

// ─── Journal checkpoint ─────────────────────────────────────────────

/// Checkpoint committed transactions from the journal to their final
/// filesystem locations.
///
/// Scans the journal from `s_start`, reads descriptor blocks and their
/// associated data blocks, and writes the data blocks to their final
/// filesystem block addresses.
///
/// After successful checkpointing, updates `s_start` to reclaim journal
/// space. When all transactions are checkpointed, `s_start` equals
/// `s_sequence` and the journal is clean.
///
/// # Arguments
///
/// * `journal` - Journal state (tracks superblock, feature flags)
/// * `read_journal_block` - Callback to read a raw block from the journal device
/// * `write_fs_block` - Callback to write a block to its final FS location
///
/// # Returns
///
/// Number of blocks checkpointed (written from journal to their FS locations).
pub fn journal_checkpoint<FR, FW, FW2>(
    journal: &mut Journal,
    mut read_journal_block: FR,
    mut write_fs_block: FW,
    mut write_journal_block: FW2,
) -> Ext4Result<u32>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
    FW: FnMut(u64, &[u8]) -> Ext4Result<()>,
    FW2: FnMut(u64, &[u8]) -> Ext4Result<()>,
{
    if journal.sb.is_clean() {
        return Ok(0); // Nothing to checkpoint
    }

    let maxlen = journal.maxlen;
    let first = journal.first;
    let mut blocks_checkpointed = 0u32;
    let mut pos = journal.sb.s_start;
    let initial_start = journal.sb.s_start;

    struct CheckpointTx {
        descriptor_pos: u32,
        tags: Vec<Jbd2BlockTag>,
    }

    let mut transactions: Vec<CheckpointTx> = Vec::new();
    let mut current_tags: Vec<Jbd2BlockTag> = Vec::new();
    let mut current_seq = 0u32;
    let mut in_transaction = false;
    let mut descriptor_pos = 0u32;
    let mut blocks_since_descriptor = 0u32;

    // SCAN pass: find committed transactions
    for _ in 0..maxlen {
        let dev_block = ((first + pos) % maxlen) as u64;
        let mut buf = vec![0u8; JBD2_BLOCK_SIZE];
        if read_journal_block(dev_block, &mut buf).is_err() {
            break;
        }

        let csum_v3 = journal.sb.s_feature_incompat & JBD2_FEATURE_INCOMPAT_CSUM_V3 != 0;
        match scan_journal_block(&buf, journal.sb.s_sequence, csum_v3, Some(&journal.sb))? {
            ScanResult::Descriptor(desc) => {
                current_tags = desc.tags;
                current_seq = desc.header.h_sequence;
                descriptor_pos = pos;
                in_transaction = true;
                blocks_since_descriptor = 0;
            }
            ScanResult::Commit(commit) => {
                if in_transaction && commit.header.h_sequence == current_seq {
                    transactions.push(CheckpointTx {
                        descriptor_pos,
                        tags: current_tags.clone(),
                    });
                }
                in_transaction = false;
                current_tags.clear();
            }
            ScanResult::Revoke(_) => {
                // Skip revoke blocks during checkpoint
            }
            ScanResult::InvalidChecksum => {
                break;
            }
            ScanResult::Other | ScanResult::End => {
                if in_transaction {
                    blocks_since_descriptor += 1;
                    if blocks_since_descriptor > current_tags.len() as u32 + 2 {
                        in_transaction = false;
                        current_tags.clear();
                    }
                }
            }
        }

        pos = (pos + 1) % maxlen;
        if pos == initial_start {
            break;
        }
    }

    // REPLAY pass: write data blocks to their FS locations
    for tx in &transactions {
        for (i, tag) in tx.tags.iter().enumerate() {
            let dest = tag.block_number();

            // Compute the journal position of this data block
            let data_pos = (tx.descriptor_pos as u64 + 1 + i as u64) % maxlen as u64;
            let dev_block = ((first + data_pos as u32) % maxlen) as u64;

            let mut data_buf = vec![0u8; JBD2_BLOCK_SIZE];
            if read_journal_block(dev_block, &mut data_buf).is_err() {
                continue;
            }

            if tag.t_flags & JBD2_FLAG_DELETED != 0 {
                let zero_buf = vec![0u8; JBD2_BLOCK_SIZE];
                write_fs_block(dest, &zero_buf)?;
            } else {
                if tag.t_flags & JBD2_FLAG_ESCAPE != 0 {
                    unescape_block(&mut data_buf);
                }
                write_fs_block(dest, &data_buf)?;
            }
            blocks_checkpointed += 1;
        }
    }

    // After all transactions are checkpointed, mark journal as clean
    if !transactions.is_empty() {
        journal.sb.s_start = journal.sb.s_sequence;
    }

    // Persist the updated journal superblock (block 0 of journal device)
    let mut sb_block = vec![0u8; JBD2_BLOCK_SIZE];
    serialize_journal_superblock(&journal.sb, &mut sb_block);
    write_journal_block(0, &sb_block)?;

    Ok(blocks_checkpointed)
}

// ─── Escaped block handling ────────────────────────────────────────────

/// Unescape a data block read from the journal.
pub fn unescape_block(block: &mut [u8]) {
    if block.len() >= 4 {
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
    }
}

// ─── Journal info helper ────────────────────────────────────────────

/// Read and parse the journal superblock from the journal inode.
pub fn read_journal_superblock<F>(
    mut read_block: F,
) -> Ext4Result<Jbd2Superblock>
where
    F: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let mut buf = vec![0u8; JBD2_BLOCK_SIZE];
    read_block(0, &mut buf)?;
    parse_journal_superblock(&buf)
}

// ─── FFI-compatible helper functions ────────────────────────────────

/// Create a new Journal from a parsed superblock.
pub fn journal_new(sb: Jbd2Superblock) -> Journal {
    Journal::new(sb)
}

/// Start a new transaction by advancing the sequence number.
/// Call this before adding blocks and calling journal_commit.
pub fn journal_start_transaction(journal: &mut Journal) -> u32 {
    journal.sb.s_sequence = journal.sb.s_sequence.wrapping_add(1);
    journal.sb.header.h_sequence = journal.sb.s_sequence;
    journal.sb.s_sequence
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal valid journal superblock for commit checksum tests
fn make_test_sb() -> Jbd2Superblock {
    let mut data = vec![0u8; 1024];
    data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
    data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
    parse_journal_superblock(&data).unwrap()
}

fn make_journal_superblock() -> Vec<u8> {
        let mut sb = vec![0u8; 1024];
        sb[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb[8..12].copy_from_slice(&(42u32).to_be_bytes());
        sb[12..16].copy_from_slice(&(1024u32).to_be_bytes());
        sb[16..20].copy_from_slice(&(32768u32).to_be_bytes());
        sb[20..24].copy_from_slice(&(1u32).to_be_bytes());
        sb[24..28].copy_from_slice(&(42u32).to_be_bytes());
        sb[28..32].copy_from_slice(&(0u32).to_be_bytes());
        sb[32..36].copy_from_slice(&(0i32).to_be_bytes());
        sb
    }

    fn make_descriptor_block(sequence: u32, block_nr: u32, flags: u16, is_last: bool) -> Vec<u8> {
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_DESCRIPTOR_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&sequence.to_be_bytes());
        let mut tag_flags = flags;
        if is_last {
            tag_flags |= JBD2_FLAG_LAST_TAG;
        }
        block[12..16].copy_from_slice(&block_nr.to_be_bytes());
        block[16..18].copy_from_slice(&tag_flags.to_be_bytes());
        block
    }

    fn make_commit_block(sequence: u32) -> Vec<u8> {
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_COMMIT_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&sequence.to_be_bytes());
        block
    }

    fn make_revoke_block(sequence: u32, blocks: &[u64]) -> Vec<u8> {
        let r_count = 12 + blocks.len() * 8;
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_REVOKE_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&sequence.to_be_bytes());
        block[12..16].copy_from_slice(&(r_count as u32).to_be_bytes());
        for (i, b) in blocks.iter().enumerate() {
            let off = 16 + i * 8;
            block[off..off + 8].copy_from_slice(&b.to_be_bytes());
        }
        block
    }

    // ─── Journal struct tests ───────────────────────────────────────

    #[test]
    fn test_journal_new_clean() {
        let data = make_journal_superblock();
        let sb = parse_journal_superblock(&data).unwrap();
        let journal = Journal::new(sb);
        assert!(journal.sb.is_clean());
        assert_eq!(journal.next_log_block, journal.sb.s_first);
        assert!(journal.has_space_for(10));
    }

    #[test]
    fn test_journal_free_blocks() {
        let data = make_journal_superblock();
        let sb = parse_journal_superblock(&data).unwrap();
        let journal = Journal::new(sb);
        // maxlen = 32768, first = 1, start = 0 (clean)
        // free = maxlen - 3 (SB + overhead) = 32765
        assert!(journal.free_blocks() > 32760);
        assert!(journal.free_blocks() <= 32765);
    }

    // ─── Serialization tests ────────────────────────────────────────

    #[test]
    fn test_serialize_descriptor_block() {
        let mut block = vec![0u8; 1024];
        let tags = vec![(1000u64, 0u16), (2000u64, 0u16)];
        let count = serialize_descriptor_block(&mut block, 1, &tags, false);
        assert_eq!(count, 2);

        // Verify header
        let header = parse_jbd2_header(&block, 0).unwrap();
        assert_eq!(header.h_magic, JBD2_MAGIC_NUMBER);
        assert_eq!(header.h_blocktype, JBD2_DESCRIPTOR_BLOCK);
        assert_eq!(header.h_sequence, 1);

        // Verify tags
        let desc = parse_descriptor_block(&block, &header, false).unwrap();
        assert_eq!(desc.tags.len(), 2);
        assert_eq!(desc.tags[0].block_number(), 1000);
        assert_eq!(desc.tags[1].block_number(), 2000);
        assert!(desc.tags[1].t_flags & JBD2_FLAG_LAST_TAG != 0);
    }

    #[test]
    fn test_serialize_descriptor_block_csum_v3() {
        let mut block = vec![0u8; 1024];
        let tags = vec![(256u64, 0u16)];
        let count = serialize_descriptor_block(&mut block, 1, &tags, true);
        assert_eq!(count, 1);

        // Verify CSUM_V3 6-byte tags
        let header = parse_jbd2_header(&block, 0).unwrap();
        let desc = parse_descriptor_block(&block, &header, true).unwrap();
        assert_eq!(desc.tags.len(), 1);
        assert_eq!(desc.tags[0].block_number(), 256);

        // Verify checksum tail
        let tail_off = 1024 - 4;
        let stored_csum = be_u32(&block, tail_off);
        assert_ne!(stored_csum, 0);

        // Validate the checksum
        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb_data[40..44].copy_from_slice(&JBD2_FEATURE_INCOMPAT_CSUM_V3.to_be_bytes());
        let sb = parse_journal_superblock(&sb_data).unwrap();
        assert!(validate_descriptor_checksum(&block, &sb));
    }

    #[test]
    fn test_serialize_commit_block() {
        let mut block = vec![0u8; 1024];
        serialize_commit_block(&mut block, 1, false);

        let header = parse_jbd2_header(&block, 0).unwrap();
        assert_eq!(header.h_magic, JBD2_MAGIC_NUMBER);
        assert_eq!(header.h_blocktype, JBD2_COMMIT_BLOCK);
        assert_eq!(header.h_sequence, 1);
    }

    #[test]
    fn test_serialize_commit_block_csum_v3() {
        let mut block = vec![0u8; 1024];
        serialize_commit_block(&mut block, 1, true);

        let header = parse_jbd2_header(&block, 0).unwrap();
        assert_eq!(header.h_blocktype, JBD2_COMMIT_BLOCK);

        let commit = parse_commit_block(&block, &header).unwrap();
        assert_eq!(commit.chksum_type, JBD2_CRC32C_CHKSUM);
        assert_eq!(commit.chksum_size, 4);
        assert_ne!(commit.chksum, 0);

        // Validate the checksum (commit block validation doesn't need a real superblock)
        // Just check that the checksum is self-consistent
        assert!(validate_commit_checksum(&block, &commit, &make_test_sb()));
    }

    #[test]
    fn test_serialize_journal_superblock() {
        let data = make_journal_superblock();
        let sb = parse_journal_superblock(&data).unwrap();

        let mut serialized = vec![0u8; 1024];
        serialize_journal_superblock(&sb, &mut serialized);

        let parsed_back = parse_journal_superblock(&serialized).unwrap();
        assert_eq!(parsed_back.header.h_sequence, sb.header.h_sequence);
        assert_eq!(parsed_back.s_sequence, sb.s_sequence);
        assert_eq!(parsed_back.s_start, sb.s_start);
        assert_eq!(parsed_back.s_first, sb.s_first);
        assert_eq!(parsed_back.s_maxlen, sb.s_maxlen);
    }

    // ─── Commit and checkpoint integration tests ────────────────────

    #[test]
    fn test_journal_commit_basic() {
        // Create an in-memory journal (19 blocks: 1 SB + 18 usable)
        let mut journal_blocks = vec![vec![0u8; 1024]; 19];

        // Initialize superblock at block 0
        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb_data[8..12].copy_from_slice(&(1u32).to_be_bytes()); // h_sequence = 1
        sb_data[12..16].copy_from_slice(&(1024u32).to_be_bytes()); // s_blocksize
        sb_data[16..20].copy_from_slice(&(19u32).to_be_bytes()); // s_maxlen = 19
        sb_data[20..24].copy_from_slice(&(1u32).to_be_bytes()); // s_first = 1
        sb_data[24..28].copy_from_slice(&(1u32).to_be_bytes()); // s_sequence = 1
        sb_data[28..32].copy_from_slice(&(0u32).to_be_bytes()); // s_start = 0 (clean)
        journal_blocks[0] = sb_data;

        let sb = parse_journal_superblock(&journal_blocks[0]).unwrap();
        let mut journal = Journal::new(sb);
        journal.maxlen = 19;

        // Commit 2 metadata blocks
        let blocks = vec![
            (500u64, vec![0xAA; 1024], 0u16),
            (600u64, vec![0xBB; 1024], 0u16),
        ];

        let mut write_count = 0u32;
        let result = journal_commit(&mut journal, &blocks, |dev_block: u64, data: &[u8]| {
            let idx = dev_block as usize;
            if idx < journal_blocks.len() {
                journal_blocks[idx] = data.to_vec();
                write_count += 1;
            }
            Ok(())
        });
        assert!(result.is_ok());
        assert!(result.unwrap() >= 3); // desc + 2 data + commit

        // Verify journal state was updated
        assert_eq!(journal.sb.s_sequence, 2);
        assert_eq!(journal.sb.s_start, 1); // started at block 1

        // Verify the journal superblock was written at block 0
        let sb_check = parse_journal_superblock(&journal_blocks[0]).unwrap();
        assert_eq!(sb_check.s_sequence, 2);
        assert_eq!(sb_check.s_start, 1);

        // Verify a descriptor block exists at block 2 (dev_block = first + pos = 1 + 1 = 2)
        let desc_header = parse_jbd2_header(&journal_blocks[2], 0).unwrap();
        assert_eq!(desc_header.h_blocktype, JBD2_DESCRIPTOR_BLOCK);

        // With first=1, pos=1: SB(0), desc(2), data0(3), data1(4), commit(5)
        let commit_header = parse_jbd2_header(&journal_blocks[5], 0).unwrap();
        assert_eq!(commit_header.h_blocktype, JBD2_COMMIT_BLOCK);

        // Verify data block at block 3
        assert_eq!(journal_blocks[3][0], 0xAA);
        // Verify data block at block 4
        assert_eq!(journal_blocks[4][0], 0xBB);
    }

    #[test]
    fn test_journal_commit_and_checkpoint() {
        // Create an in-memory journal (19 blocks)
        let mut journal_blocks = vec![vec![0u8; 1024]; 19];

        // Initialize superblock
        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb_data[8..12].copy_from_slice(&(1u32).to_be_bytes());
        sb_data[12..16].copy_from_slice(&(1024u32).to_be_bytes());
        sb_data[16..20].copy_from_slice(&(19u32).to_be_bytes());
        sb_data[20..24].copy_from_slice(&(1u32).to_be_bytes());
        sb_data[24..28].copy_from_slice(&(1u32).to_be_bytes());
        sb_data[28..32].copy_from_slice(&(0u32).to_be_bytes());
        journal_blocks[0] = sb_data;

        let sb = parse_journal_superblock(&journal_blocks[0]).unwrap();
        let mut journal = Journal::new(sb);
        journal.maxlen = 19;

        // Also create a separate FS storage
        let mut fs_blocks: Vec<Vec<u8>> = vec![vec![0u8; 1024]; 1000];

        // Commit 1 metadata block: write block 42 to the journal
        let blocks = vec![
            (42u64, vec![0xCA; 1024], 0u16),
        ];

        journal_commit(&mut journal, &blocks, |dev_block: u64, data: &[u8]| {
            let idx = dev_block as usize;
            if idx < journal_blocks.len() {
                journal_blocks[idx] = data.to_vec();
            }
            Ok(())
        }).unwrap();

        // Verify that FS block 42 has NOT been modified yet (still zeros)
        assert_eq!(fs_blocks[42][0], 0x00);

        // Now checkpoint: write the journal contents to the actual FS blocks
        // Clone for the write closure to avoid borrow conflict
        let jb = journal_blocks.clone();
        journal_checkpoint(&mut journal,
            |dev_block: u64, buf: &mut [u8]| {
                let idx = dev_block as usize;
                if idx < jb.len() {
                    buf.copy_from_slice(&jb[idx]);
                    Ok(())
                } else {
                    Err(Ext4Error::IoError)
                }
            },
            |dest_block: u64, data: &[u8]| {
                let idx = dest_block as usize;
                if idx < fs_blocks.len() {
                    fs_blocks[idx] = data.to_vec();
                    Ok(())
                } else {
                    Err(Ext4Error::IoError)
                }
            },
            |dev_block: u64, data: &[u8]| {
                let idx = dev_block as usize;
                if idx < journal_blocks.len() {
                    journal_blocks[idx] = data.to_vec();
                }
                Ok(())
            }
        ).unwrap();

        // Verify that FS block 42 now contains the data from the journal
        assert_eq!(fs_blocks[42][0], 0xCA);

        // Verify journal is now clean (s_start == s_sequence after all checkpointed)
        assert_eq!(journal.sb.s_start, journal.sb.s_sequence);
    }

    #[test]
    fn test_journal_commit_escaping() {
        let mut journal_blocks = vec![vec![0u8; 1024]; 19];

        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb_data[8..12].copy_from_slice(&(1u32).to_be_bytes());
        sb_data[12..16].copy_from_slice(&(1024u32).to_be_bytes());
        sb_data[16..20].copy_from_slice(&(19u32).to_be_bytes());
        sb_data[20..24].copy_from_slice(&(1u32).to_be_bytes());
        sb_data[24..28].copy_from_slice(&(1u32).to_be_bytes());
        sb_data[28..32].copy_from_slice(&(0u32).to_be_bytes());
        journal_blocks[0] = sb_data;

        let sb = parse_journal_superblock(&journal_blocks[0]).unwrap();
        let mut journal = Journal::new(sb);
        journal.maxlen = 19;

        // Create a block that starts with JBD2_MAGIC_NUMBER (needs escaping)
        // The caller must zero the first 4 bytes AND set JBD2_FLAG_ESCAPE
        let mut data = vec![0u8; 1024];
        data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        data[4] = 0xFF;
        let mut escaped_data = data.clone();
        escaped_data[0..4].copy_from_slice(&[0u8; 4]); // caller zeroes the magic

        let blocks = vec![
            (100u64, escaped_data, JBD2_FLAG_ESCAPE),
        ];

        journal_commit(&mut journal, &blocks, |dev_block: u64, data: &[u8]| {
            let idx = dev_block as usize;
            if idx < journal_blocks.len() {
                journal_blocks[idx] = data.to_vec();
            }
            Ok(())
        }).unwrap();

        // The data block in the journal should have first 4 bytes zeroed (escaped)
        // With first=1, pos=1: SB(0), desc(2), data(3), commit(4)
        assert_eq!(journal_blocks[3][0..4], [0u8; 4]);
        assert_eq!(journal_blocks[3][4], 0xFF);

        // After checkpoint + unescape, the FS block should have MAGIC restored
        let mut fs_blocks = vec![vec![0u8; 1024]; 200];
        // Clone for the write closure to avoid borrow conflict
        let jb = journal_blocks.clone();
        journal_checkpoint(&mut journal,
            |dev_block, buf| {
                let idx = dev_block as usize;
                buf.copy_from_slice(&jb[idx]);
                Ok(())
            },
            |dest_block, data| {
                let idx = dest_block as usize;
                fs_blocks[idx] = data.to_vec();
                Ok(())
            },
            |dev_block: u64, data: &[u8]| {
                let idx = dev_block as usize;
                if idx < journal_blocks.len() {
                    journal_blocks[idx] = data.to_vec();
                }
                Ok(())
            }
        ).unwrap();

        assert_eq!(fs_blocks[100][0..4], JBD2_MAGIC_NUMBER.to_be_bytes());
        assert_eq!(fs_blocks[100][4], 0xFF);
    }

    #[test]
    fn test_serialize_descriptor_many_tags() {
        // Test that we can fit many tags in one descriptor block
        let mut block = vec![0u8; 1024];
        let mut tags = Vec::new();
        // With 8-byte tags + SAME_UUID, we can fit (1024 - 12) / 8 = 126 tags
        for i in 0..120 {
            tags.push((i as u64 * 1000, 0u16));
        }
        let count = serialize_descriptor_block(&mut block, 1, &tags, false);
        assert_eq!(count, 120);

        let header = parse_jbd2_header(&block, 0).unwrap();
        let desc = parse_descriptor_block(&block, &header, false).unwrap();
        assert_eq!(desc.tags.len(), 120);
        assert!(desc.tags[119].t_flags & JBD2_FLAG_LAST_TAG != 0);
    }

    #[test]
    fn test_serialize_descriptor_empty() {
        // Empty tags should still produce a valid descriptor
        let mut block = vec![0u8; 1024];
        let count = serialize_descriptor_block(&mut block, 1, &[], false);
        assert_eq!(count, 0);

        let header = parse_jbd2_header(&block, 0).unwrap();
        assert_eq!(header.h_blocktype, JBD2_DESCRIPTOR_BLOCK);
    }

    // ─── Existing tests (preserved) ─────────────────────────────────

    #[test]
    fn test_parse_journal_superblock() {
        let data = make_journal_superblock();
        let sb = parse_journal_superblock(&data).unwrap();
        assert_eq!(sb.header.h_magic, JBD2_MAGIC_NUMBER);
        assert_eq!(sb.header.h_blocktype, JBD2_SUPERBLOCK_V2);
        assert_eq!(sb.header.h_sequence, 42);
        assert_eq!(sb.s_sequence, 42);
        assert_eq!(sb.s_start, 0);
        assert_eq!(sb.s_blocksize, 1024);
        assert!(sb.is_clean());
    }

    #[test]
    fn test_parse_descriptor_block() {
        let block = make_descriptor_block(100, 256, 0, true);
        let header = parse_jbd2_header(&block, 0).unwrap();
        let desc = parse_descriptor_block(&block, &header, false).unwrap();
        assert_eq!(desc.header.h_sequence, 100);
        assert_eq!(desc.tags.len(), 1);
        assert_eq!(desc.tags[0].block_number(), 256);
    }

    #[test]
    fn test_parse_commit_block() {
        let block = make_commit_block(100);
        let header = parse_jbd2_header(&block, 0).unwrap();
        let commit = parse_commit_block(&block, &header).unwrap();
        assert_eq!(commit.header.h_sequence, 100);
    }

    #[test]
    fn test_parse_revoke_block() {
        let block = make_revoke_block(100, &[256, 512, 1024]);
        let header = parse_jbd2_header(&block, 0).unwrap();
        let revoke = parse_revoke_block(&block, &header).unwrap();
        assert_eq!(revoke.revoke_records.len(), 3);
        assert_eq!(revoke.revoke_records[0], 256);
        assert_eq!(revoke.revoke_records[1], 512);
        assert_eq!(revoke.revoke_records[2], 1024);
    }

    #[test]
    fn test_scan_descriptor_and_commit() {
        let mut journal = vec![vec![0u8; 1024]; 10];
        journal[0] = make_journal_superblock();
        journal[1] = make_descriptor_block(1, 1000, 0, true);
        journal[2][0] = 0xAA;
        journal[3] = make_commit_block(1);

        let result = scan_journal_block(&journal[1], 1, false, None).unwrap();
        match result {
            ScanResult::Descriptor(desc) => {
                assert_eq!(desc.tags[0].block_number(), 1000);
            }
            _ => panic!("Expected descriptor block"),
        }

        let result = scan_journal_block(&journal[3], 1, false, None).unwrap();
        match result {
            ScanResult::Commit(_) => {}
            _ => panic!("Expected commit block"),
        }
    }

    #[test]
    fn test_recover_clean_journal() {
        let data = make_journal_superblock();
        let sb = parse_journal_superblock(&data).unwrap();
        let config = RecoveryConfig {
            journal_blocks: sb.s_maxlen,
            first_block: sb.s_first,
        };

        let count = recover_journal(
            &sb,
            &config,
            |_block, _buf| Err(Ext4Error::IoError),
            |_block, _buf| Err(Ext4Error::IoError),
        ).unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_superblock_too_small() {
        assert!(parse_journal_superblock(&[0u8; 100]).is_err());
    }

    #[test]
    fn test_superblock_wrong_magic() {
        let mut data = vec![0u8; 1024];
        data[0..4].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());
        assert!(parse_journal_superblock(&data).is_err());
    }

    #[test]
    fn test_journal_info_string() {
        let data = make_journal_superblock();
        let sb = parse_journal_superblock(&data).unwrap();
        let info = sb.info_string();
        assert!(info.contains("jbd2"));
        assert!(info.contains("clean"));
    }

    // ─── Checksum tests ────────────────────────────────────────────

    #[test]
    fn test_crc32c_basic() {
        assert_eq!(crc32c(b""), 0x00000000);
        assert_eq!(crc32c(b"a"), 0xC1D04330);
        assert_eq!(crc32c(b"abc"), 0x364B3FB7);
        assert_eq!(crc32c(b"123456789"), 0xE3069283);
    }

    #[test]
    fn test_crc32c_jbd2_matches_standard() {
        let data = b"hello";
        let result = crc32c_seeded(0x00000000, data) ^ 0xFFFFFFFF;
        let mut full = vec![0xFFu8; 4];
        full.extend_from_slice(data);
        let expected = crc32c(&full);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_crc32_basic() {
        assert_eq!(crc32(b""), 0x00000000);
        assert_eq!(crc32(b"abc"), 0x352441C2);
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn test_descriptor_csum_v3_6byte_tags() {
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_DESCRIPTOR_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&(1u32).to_be_bytes());
        block[12..16].copy_from_slice(&(1000u32).to_be_bytes());
        block[16..18].copy_from_slice(&(JBD2_FLAG_LAST_TAG | JBD2_FLAG_SAME_UUID).to_be_bytes());

        let header = parse_jbd2_header(&block, 0).unwrap();
        let desc = parse_descriptor_block(&block, &header, true).unwrap();
        assert_eq!(desc.tags.len(), 1);
        assert_eq!(desc.tags[0].block_number(), 1000);
        assert_eq!(desc.tags[0].t_blocknr_high, 0);
    }

    #[test]
    fn test_descriptor_csum_validation_valid() {
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_DESCRIPTOR_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&(1u32).to_be_bytes());
        block[12..16].copy_from_slice(&(256u32).to_be_bytes());
        block[16..18].copy_from_slice(&(JBD2_FLAG_LAST_TAG | JBD2_FLAG_SAME_UUID).to_be_bytes());

        let tail_off = block.len() - 4;
        let mut zeroed = block.clone();
        zeroed[tail_off..tail_off + 4].copy_from_slice(&[0u8; 4]);
        let csum = crc32c_seeded(0x00000000, &zeroed) ^ 0xFFFFFFFF;
        block[tail_off..tail_off + 4].copy_from_slice(&csum.to_be_bytes());

        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb_data[40..44].copy_from_slice(&JBD2_FEATURE_INCOMPAT_CSUM_V2.to_be_bytes());
        let sb = parse_journal_superblock(&sb_data).unwrap();

        assert!(validate_descriptor_checksum(&block, &sb));
    }

    #[test]
    fn test_descriptor_csum_validation_invalid() {
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_DESCRIPTOR_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&(1u32).to_be_bytes());
        block[12..16].copy_from_slice(&(256u32).to_be_bytes());
        block[16..18].copy_from_slice(&(JBD2_FLAG_LAST_TAG | JBD2_FLAG_SAME_UUID).to_be_bytes());

        let tail_off = block.len() - 4;
        block[tail_off..tail_off + 4].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());

        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb_data[40..44].copy_from_slice(&JBD2_FEATURE_INCOMPAT_CSUM_V2.to_be_bytes());
        let sb = parse_journal_superblock(&sb_data).unwrap();

        assert!(!validate_descriptor_checksum(&block, &sb));
    }

    #[test]
    fn test_descriptor_csum_skipped_without_feature() {
        let block = vec![0u8; 1024];
        let mut sb_data = vec![0u8; 1024];
        sb_data[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb_data[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        let sb = parse_journal_superblock(&sb_data).unwrap();
        assert!(validate_descriptor_checksum(&block, &sb));
    }

    #[test]
    fn test_commit_csum_v3_validation() {
        let mut block = vec![0u8; 1024];
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_COMMIT_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&(1u32).to_be_bytes());

        block[12] = JBD2_CRC32C_CHKSUM;
        block[13] = 4;
        block[20..28].copy_from_slice(&1000u64.to_be_bytes());
        block[28..32].copy_from_slice(&500u32.to_be_bytes());

        let mut zeroed = block.clone();
        zeroed[16..20].copy_from_slice(&[0u8; 4]);
        let csum = crc32c_seeded(0x00000000, &zeroed) ^ 0xFFFFFFFF;
        block[16..20].copy_from_slice(&csum.to_be_bytes());

        let header = parse_jbd2_header(&block, 0).unwrap();
        let commit = parse_commit_block(&block, &header).unwrap();

        assert_eq!(commit.chksum_type, JBD2_CRC32C_CHKSUM);
        assert_eq!(commit.chksum_size, 4);
        assert_eq!(commit.chksum, csum);
        assert_eq!(commit.commit_sec, 1000);
        assert_eq!(commit.commit_nsec, 500);

        assert!(validate_commit_checksum(&block, &commit, &make_test_sb()));

        let mut bad_block = block.clone();
        bad_block[16..20].copy_from_slice(&0xDEADBEEFu32.to_be_bytes());
        let bad_header = parse_jbd2_header(&bad_block, 0).unwrap();
        let bad_commit = parse_commit_block(&bad_block, &bad_header).unwrap();
        assert!(!validate_commit_checksum(&bad_block, &bad_commit, &make_test_sb()));
    }
}
