//! JBD2 (Journaling Block Device 2) — ext4 journal support.
//!
//! Provides parsing and recovery for the ext4 journal, which is a write-ahead
//! log used to ensure filesystem consistency after crashes.
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
#[derive(Clone, Debug)]
pub struct Jbd2CommitBlock {
    pub header: Jbd2Header,
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
///
/// Layout (from Linux kernel `include/linux/jbd2.h`):
///   0x00: h_magic (4 bytes)
///   0x04: h_blocktype (4 bytes)
///   0x08: h_sequence (4 bytes)
///   0x0C: s_blocksize (4 bytes)
///   0x10: s_maxlen (4 bytes)
///   0x14: s_first (4 bytes)
///   0x18: s_sequence (4 bytes)
///   0x1C: s_start (4 bytes)
///   0x20: s_errno (4 bytes)
///   0x24: s_feature_compat (4 bytes)
///   0x28: s_feature_incompat (4 bytes)
///   0x2C: s_feature_ro_compat (4 bytes)
///   0x30: s_uuid (16 bytes)
///   0x40: s_nr_users (4 bytes)
///   0x44: s_max_transaction (4 bytes)
///   0x48: s_max_trans_data (4 bytes)
///   0x4C: s_commit_interval (4 bytes)
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

    // Superblock fields start at offset 12 (0x0C), after the 12-byte header
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
///
/// Tags are stored in **forward order** starting at offset 12 (after the 12-byte header).
/// Each tag is followed by a 16-byte UUID when `JBD2_FLAG_SAME_UUID` is NOT set.
/// The last tag has `JBD2_FLAG_LAST_TAG` set.
///
/// Layout per entry (without CSUM_V2/CSUM_V3):
/// - Tag is always 8 bytes: blocknr(4) + flags(2) + blocknr_high(2)
/// - `JBD2_FLAG_SAME_UUID` set:   8 bytes total — no UUID (reuse journal superblock UUID)
/// - `JBD2_FLAG_SAME_UUID` clear: 24 bytes total — tag(8) + UUID(16) per entry
pub fn parse_descriptor_block(data: &[u8], header: &Jbd2Header) -> Ext4Result<Jbd2DescriptorBlock> {
    if header.h_blocktype != JBD2_DESCRIPTOR_BLOCK {
        return Err(Ext4Error::InvalidExtentHeader); // Wrong block type
    }

    let mut tags = Vec::new();
    let mut off = 12; // Skip header

    loop {
        if off + 8 > data.len() {
            break;
        }

        // Always read 8 bytes: blocknr(4) + flags(2) + blocknr_high(2)
        let t_blocknr = be_u32(data, off);
        let t_flags = be_u16(data, off + 4);
        let t_blocknr_high = be_u16(data, off + 6);

        tags.push(Jbd2BlockTag {
            t_blocknr,
            t_flags,
            t_blocknr_high,
        });

        off += 8; // Base tag size: always 8 bytes for non-CSUM_V2 format

        // When SAME_UUID is NOT set, each tag is followed by a 16-byte UUID
        if t_flags & JBD2_FLAG_SAME_UUID == 0 {
            off += 16; // Skip UUID
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

/// Parse a commit block from a buffer.
pub fn parse_commit_block(data: &[u8], header: &Jbd2Header) -> Ext4Result<Jbd2CommitBlock> {
    if header.h_blocktype != JBD2_COMMIT_BLOCK {
        return Err(Ext4Error::InvalidExtentHeader);
    }
    let _ = data; // Commit block data may contain optional checksum
    Ok(Jbd2CommitBlock { header: *header })
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
    /// Not a journal block (superblock or unknown).
    Other,
    /// End of valid journal data.
    End,
}

/// Scan a journal block and identify its type.
pub fn scan_journal_block(data: &[u8], sequence: u32) -> Ext4Result<ScanResult> {
    let header = parse_jbd2_header(data, 0)?;

    if header.h_magic != JBD2_MAGIC_NUMBER {
        return Ok(ScanResult::Other);
    }

    // Check sequence: it should be wrapping but for now check it's reasonable
    // (within 256 of the expected sequence, or smaller if wrapping)
    let seq_diff = if header.h_sequence >= sequence {
        header.h_sequence - sequence
    } else {
        sequence - header.h_sequence
    };

    if seq_diff > 256 && header.h_sequence > 0 {
        // Sequence too far off — probably not journal data
        return Ok(ScanResult::End);
    }

    match header.h_blocktype {
        JBD2_DESCRIPTOR_BLOCK => {
            let desc = parse_descriptor_block(data, &header)?;
            Ok(ScanResult::Descriptor(desc))
        }
        JBD2_COMMIT_BLOCK => {
            let commit = parse_commit_block(data, &header)?;
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
///
/// `read_journal_block` reads a raw block from the journal device at the
/// given absolute block number.
/// `write_fs_block` writes a replayed metadata block to its final
/// location on the filesystem.
///
/// The recovery uses a three-pass approach:
/// 1. SCAN: scan forward from `s_start`, collect descriptor tags + matching commits
/// 2. REVOKE: collect revoke records (done during SCAN)
/// 3. REPLAY: re-read descriptor blocks, read their data blocks, apply unescape/zero,
///    and write to final destinations (skipping revoked blocks)
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
        return Ok(0); // Nothing to recover
    }

    let mut recovered_transactions = 0u32;
    let mut revoke_set: Vec<u64> = Vec::new();
    let mut pos = sb.s_start;

    // SCAN + REVOKE pass: identify committed transactions and collect revokes
    let max_scan = config.journal_blocks;
    let first = config.first_block;

    // Collect: (transaction_pos_in_journal, descriptor_pos, tags)
    // transaction_pos tracks where in the journal the descriptor was found
    // so we can find data blocks during REPLAY
    struct CommittedTx {
        _seq: u32,
        descriptor_block_pos: u32,  // position in journal of descriptor
        tags: Vec<Jbd2BlockTag>,
    }

    let mut transactions: Vec<CommittedTx> = Vec::new();
    let mut current_tags: Vec<Jbd2BlockTag> = Vec::new();
    let mut current_seq = 0u32;
    let mut in_transaction = false;
    let mut descriptor_pos = 0u32;
    let mut blocks_since_descriptor = 0u32;

    for _ in 0..max_scan {
        let dev_block = first as u64 + pos as u64;
        let mut buf = vec![0u8; JBD2_BLOCK_SIZE];
        if read_journal_block(dev_block, &mut buf).is_err() {
            break;
        }

        match scan_journal_block(&buf, sb.s_sequence)? {
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

    // REPLAY pass: for each committed transaction, read data blocks
    // Data blocks immediately follow the descriptor block in the journal.
    // tag[0]'s data = descriptor_pos + 1, tag[1]'s data = descriptor_pos + 2, etc.
    for tx in &transactions {
        for (i, tag) in tx.tags.iter().enumerate() {
            let dest = tag.block_number();

            // Skip revoked blocks
            if revoke_set.contains(&dest) {
                continue;
            }

            // Compute the journal position of this data block
            let data_pos = (tx.descriptor_block_pos as u64 + 1 + i as u64) % config.journal_blocks as u64;
            let dev_block = first as u64 + data_pos;

            let mut data_buf = vec![0u8; JBD2_BLOCK_SIZE];
            if read_journal_block(dev_block, &mut data_buf).is_err() {
                continue;
            }

            if tag.t_flags & JBD2_FLAG_DELETED != 0 {
                // Write zeros
                let zero_buf = vec![0u8; JBD2_BLOCK_SIZE];
                write_fs_block(dest, &zero_buf)?;
            } else {
                // Apply unescape if needed
                if tag.t_flags & JBD2_FLAG_ESCAPE != 0 {
                    unescape_block(&mut data_buf);
                }
                // Write block to final destination
                write_fs_block(dest, &data_buf)?;
            }
        }
    }

    Ok(recovered_transactions)
}

// ─── Journal info helper ────────────────────────────────────────────

// ─── Escaped block handling ────────────────────────────────────────────

/// Unescape a data block read from the journal.
///
/// When `JBD2_FLAG_ESCAPE` is set on a tag, the data block's first 4 bytes
/// were replaced with `0x00000000` before writing to the journal (to avoid
/// the block looking like a journal header with `JBD2_MAGIC_NUMBER`).
/// During replay, we restore those 4 bytes to `JBD2_MAGIC_NUMBER`.
pub fn unescape_block(block: &mut [u8]) {
    if block.len() >= 4 {
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
    }
}

// ─── Journal info helper ────────────────────────────────────────────

/// Read and parse the journal superblock from the journal inode.
///
/// `read_journal_block` reads a raw block from the journal device at the given
/// absolute block number.
pub fn read_journal_superblock<F>(
    mut read_block: F,
) -> Ext4Result<Jbd2Superblock>
where
    F: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    let mut buf = vec![0u8; JBD2_BLOCK_SIZE];
    // Journal superblock is at block 0 of the journal
    read_block(0, &mut buf)?;
    parse_journal_superblock(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_journal_superblock() -> Vec<u8> {
        let mut sb = vec![0u8; 1024];
        // Header at offset 0: big-endian
        sb[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        sb[4..8].copy_from_slice(&JBD2_SUPERBLOCK_V2.to_be_bytes());
        sb[8..12].copy_from_slice(&(42u32).to_be_bytes()); // h_sequence = 42
        // Superblock fields after 12-byte header:
        sb[12..16].copy_from_slice(&(1024u32).to_be_bytes());  // s_blocksize = 1024
        sb[16..20].copy_from_slice(&(32768u32).to_be_bytes()); // s_maxlen = 32768
        sb[20..24].copy_from_slice(&(1u32).to_be_bytes());     // s_first = 1
        sb[24..28].copy_from_slice(&(42u32).to_be_bytes());    // s_sequence = 42
        sb[28..32].copy_from_slice(&(0u32).to_be_bytes());     // s_start = 0 (clean)
        sb[32..36].copy_from_slice(&(0i32).to_be_bytes());     // s_errno = 0
        sb
    }

    fn make_descriptor_block(sequence: u32, block_nr: u32, flags: u16, is_last: bool) -> Vec<u8> {
        let mut block = vec![0u8; 1024];
        // Header
        block[0..4].copy_from_slice(&JBD2_MAGIC_NUMBER.to_be_bytes());
        block[4..8].copy_from_slice(&JBD2_DESCRIPTOR_BLOCK.to_be_bytes());
        block[8..12].copy_from_slice(&sequence.to_be_bytes());

        // One block tag at offset 12
        let mut tag_flags = flags;
        if is_last {
            tag_flags |= JBD2_FLAG_LAST_TAG;
        }
        block[12..16].copy_from_slice(&block_nr.to_be_bytes()); // t_blocknr
        block[16..18].copy_from_slice(&tag_flags.to_be_bytes()); // t_flags
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
        let desc = parse_descriptor_block(&block, &header).unwrap();
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
        // Simulate a journal with a descriptor + data + commit
        let mut journal = vec![vec![0u8; 1024]; 10]; // 10 journal blocks

        // Block 0: superblock
        journal[0] = make_journal_superblock();

        // Block 1: descriptor (sequence 1, block 1000)
        journal[1] = make_descriptor_block(1, 1000, 0, true);

        // Block 2: data block (for block 1000)
        journal[2][0] = 0xAA;

        // Block 3: commit (sequence 1)
        journal[3] = make_commit_block(1);

        // Scan
        let result = scan_journal_block(&journal[1], 1).unwrap();
        match result {
            ScanResult::Descriptor(desc) => {
                assert_eq!(desc.tags[0].block_number(), 1000);
            }
            _ => panic!("Expected descriptor block"),
        }

        let result = scan_journal_block(&journal[3], 1).unwrap();
        match result {
            ScanResult::Commit(_) => {} // OK
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
            |_block, _buf| Err(Ext4Error::IoError), // Should not be called
            |_block, _buf| Err(Ext4Error::IoError),
        ).unwrap();

        assert_eq!(count, 0); // Clean journal = no recovery
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
}
