//! Extended attributes (xattrs) for ext4.
//!
//! Supports reading, writing, and removing extended attributes stored:
//! - In the inode's extra space (after i_extra_isize, before next inode)
//! - In an external block pointed to by `i_file_acl_lo`
//!
//! **Note**: EA_INODE (INCOMPAT_EA_INODE) feature is NOT supported.
//! Values must fit within a single block or the inode's extra space.

use crate::types::*;

// ─── Constants ────────────────────────────────────────────────────────

/// Magic number for xattr header (both ibody and external block).
pub const EXT4_XATTR_MAGIC: u32 = 0xEA020000;

/// Xattr name indices (prefixes).
pub const EXT4_XATTR_INDEX_USER: u8 = 1;
pub const EXT4_XATTR_INDEX_POSIX_ACL_ACCESS: u8 = 2;
pub const EXT4_XATTR_INDEX_POSIX_ACL_DEFAULT: u8 = 3;
pub const EXT4_XATTR_INDEX_TRUSTED: u8 = 4;
pub const EXT4_XATTR_INDEX_SECURITY: u8 = 6;
pub const EXT4_XATTR_INDEX_SYSTEM: u8 = 7;

/// Map name index → prefix string.
pub fn xattr_prefix(index: u8) -> &'static str {
    match index {
        EXT4_XATTR_INDEX_USER => "user.",
        EXT4_XATTR_INDEX_POSIX_ACL_ACCESS => "system.posix_acl_access",
        EXT4_XATTR_INDEX_POSIX_ACL_DEFAULT => "system.posix_acl_default",
        EXT4_XATTR_INDEX_TRUSTED => "trusted.",
        EXT4_XATTR_INDEX_SECURITY => "security.",
        EXT4_XATTR_INDEX_SYSTEM => "system.",
        _ => "",
    }
}

/// Try to match a full attribute name (e.g. "user.foo") to an index + short name.
pub fn match_xattr_name(full_name: &str) -> (u8, String) {
    for (idx, prefix) in &[
        (EXT4_XATTR_INDEX_USER, "user."),
        (EXT4_XATTR_INDEX_POSIX_ACL_ACCESS, "system.posix_acl_access"),
        (EXT4_XATTR_INDEX_POSIX_ACL_DEFAULT, "system.posix_acl_default"),
        (EXT4_XATTR_INDEX_TRUSTED, "trusted."),
        (EXT4_XATTR_INDEX_SECURITY, "security."),
    ] {
        if let Some(rest) = full_name.strip_prefix(prefix) {
            return (*idx, rest.to_string());
        }
    }
    (0, full_name.to_string())
}

// ─── On-disk structures ────────────────────────────────────────────────

/// External xattr block header (32 bytes at start of block).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Ext4XattrHeader {
    pub h_magic: u32,        // 0xEA020000
    pub h_refcount: u32,
    pub h_blocks: u32,
    pub h_hash: u32,
    pub h_checksum: u32,
    pub h_reserved: [u32; 3],
}

/// In-inode xattr header (4 bytes, after i_extra_isize).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Ext4XattrIbodyHeader {
    pub h_magic: u32,         // 0xEA020000
}

/// Xattr entry (variable length, minimum 16 bytes).
#[derive(Clone, Debug)]
pub struct Ext4XattrEntry {
    pub e_name_len: u8,
    pub e_name_index: u8,
    pub e_value_offs: u16,
    pub e_value_inum: u32,    // 0 if value is in same block
    pub e_value_size: u32,
    pub e_hash: u32,
    pub e_name: Vec<u8>,
}

/// A parsed xattr (name + value).
#[derive(Clone, Debug)]
pub struct Xattr {
    pub name: String,
    pub value: Vec<u8>,
}

/// Location where xattrs are stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XattrLocation {
    InInode,
    ExternalBlock,
}

// ─── Parsing helpers ───────────────────────────────────────────────────

/// Parse an xattr entry from a buffer at offset `off`.
/// Returns the entry and the entry size (including name).
fn parse_xattr_entry(data: &[u8], off: usize) -> Ext4Result<(Ext4XattrEntry, usize)> {
    if off + 16 > data.len() {
        return Err(Ext4Error::InvalidXattr);
    }

    let e_name_len = data[off];
    let e_name_index = data[off + 1];
    let e_value_offs = u16::from_le_bytes([data[off + 2], data[off + 3]]);
    let e_value_inum = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
    let e_value_size = u32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
    let e_hash = u32::from_le_bytes([data[off + 12], data[off + 13], data[off + 14], data[off + 15]]);

    // Name follows the 16-byte fixed part
    let name_start = off + 16;
    let name_end = name_start + e_name_len as usize;
    if name_end > data.len() {
        return Err(Ext4Error::InvalidXattr);
    }

    let e_name = data[name_start..name_end].to_vec();

    // Entry size = 16 + name_len, padded to 4 bytes
    let entry_size = ((16 + e_name_len as usize) + 3) & !3;

    Ok((Ext4XattrEntry {
        e_name_len,
        e_name_index,
        e_value_offs,
        e_value_inum,
        e_value_size,
        e_hash,
        e_name,
    }, entry_size))
}

/// Parse all xattr entries from an xattr data buffer.
/// `data` is either the in-inode area or the full external block.
/// `has_header` indicates whether the data starts with a header.
pub fn parse_xattrs(data: &[u8], has_header: bool) -> Ext4Result<Vec<Xattr>> {
    let mut xattrs = Vec::new();
    let data_len = data.len();

    let start_off = if has_header {
        if data.len() < 4 {
            return Err(Ext4Error::InvalidXattr);
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != EXT4_XATTR_MAGIC {
            return Err(Ext4Error::InvalidXattr);
        }
        // External block header is 32 bytes; ibody header is 4 bytes
        if data.len() >= 32 && u32::from_le_bytes([data[4], data[5], data[6], data[7]]) != 0 {
            // External block: header is 32 bytes
            32
        } else {
            // In-inode: header is 4 bytes
            4
        }
    } else {
        0
    };

    let mut off = start_off;

    loop {
        if off + 16 > data_len {
            break;
        }

        // Check for end marker: first 4 bytes all zero
        if data[off] == 0 && data[off + 1] == 0 && data[off + 2] == 0 && data[off + 3] == 0 {
            break;
        }

        let (entry, entry_size) = parse_xattr_entry(data, off)?;

        if entry.e_name_len == 0 {
            off += entry_size;
            continue;
        }

        // Extract value
        let value = if entry.e_value_inum != 0 {
            // EA_INODE not supported
            continue;
        } else if entry.e_value_offs != 0 && entry.e_value_size > 0 {
            let value_off = if has_header {
                // For external block, value_offs is relative to start of block
                entry.e_value_offs as usize
            } else {
                // For in-inode, value_offs is relative to start of first entry
                start_off + entry.e_value_offs as usize
            };

            if value_off + entry.e_value_size as usize > data_len {
                return Err(Ext4Error::InvalidXattr);
            }
            data[value_off..value_off + entry.e_value_size as usize].to_vec()
        } else {
            Vec::new()
        };

        // Reconstruct full name
        let prefix = xattr_prefix(entry.e_name_index);
        let name_str = String::from_utf8_lossy(&entry.e_name).to_string();
        let full_name = if prefix.is_empty() {
            name_str
        } else {
            format!("{}{}", prefix, name_str)
        };

        xattrs.push(Xattr {
            name: full_name,
            value,
        });

        off += entry_size;
    }

    Ok(xattrs)
}

/// Find a specific xattr by full name.
pub fn find_xattr<'a>(xattrs: &'a [Xattr], name: &str) -> Option<&'a Xattr> {
    xattrs.iter().find(|x| x.name == name)
}

// ─── Serialization helpers ──────────────────────────────────────────────

/// Serialize a single xattr entry into a buffer at offset `off`.
/// Returns the entry size (including padding).
fn serialize_xattr_entry(buf: &mut [u8], off: usize, index: u8, name: &[u8], value_offs: u16, value_size: u32) -> usize {
    if off + 16 > buf.len() {
        return 0;
    }

    buf[off] = name.len() as u8;
    buf[off + 1] = index;
    buf[off + 2..off + 4].copy_from_slice(&value_offs.to_le_bytes());
    buf[off + 4..off + 8].copy_from_slice(&0u32.to_le_bytes()); // e_value_inum = 0
    buf[off + 8..off + 12].copy_from_slice(&value_size.to_le_bytes());
    buf[off + 12..off + 16].copy_from_slice(&0u32.to_le_bytes()); // e_hash (kernel doesn't check for in-inode)

    // Write name
    let name_start = off + 16;
    let name_end = name_start + name.len();
    if name_end > buf.len() {
        return 0;
    }
    buf[name_start..name_end].copy_from_slice(name);

    // Pad to 4 bytes
    let entry_size = ((16 + name.len()) + 3) & !3;
    for i in (name_end)..(off + entry_size) {
        if i < buf.len() {
            buf[i] = 0;
        }
    }

    entry_size
}

// ─── In-inode xattr helpers ─────────────────────────────────────────────

/// Compute the offset and available space for in-inode xattrs.
/// Returns (start_offset, available_bytes) or None if inode has no space.
pub fn inode_xattr_space(inode: &Ext4Inode, sb: &Ext4Superblock) -> Option<(usize, usize)> {
    let inode_size = sb.inode_size();
    let extra_isize = inode.i_extra_isize as usize;

    // Standard fields are 128 bytes
    // After that: i_extra_isize bytes of extended fields
    // Then: xattrs start, until the end of the inode
    if inode_size <= 128 || extra_isize < 4 {
        return None;
    }

    let xattr_start = 128 + extra_isize;
    if xattr_start >= inode_size {
        return None;
    }

    let available = inode_size - xattr_start;
    Some((xattr_start, available))
}

// To read in-inode xattrs, use `parse_inode_xattrs(data, sb)` with the
// raw inode buffer (sb.inode_size() bytes). This function requires the raw
// inode data because xattrs are stored after the parsed inode fields (at
// offset 128 + i_extra_isize).
