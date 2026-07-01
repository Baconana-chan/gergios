//! POSIX Access Control Lists (ACLs) for ext4.
//!
//! POSIX ACLs are stored as extended attributes:
//! - `system.posix_acl_access` — file access ACL
//! - `system.posix_acl_default` — directory default ACL
//!
//! The on-disk format is a reduced version of the kernel's internal ACL
//! format, with version number set to `EXT4_ACL_VERSION` (0x02000000).

use crate::types::*;

// ─── Constants ─────────────────────────────────────────────────────────

/// POSIX ACL version (on-disk).
pub const EXT4_ACL_VERSION: u32 = 0x02000000;

/// ACL tag types.
pub const ACL_USER_OBJ: u16 = 0x01;
pub const ACL_USER: u16 = 0x02;
pub const ACL_GROUP_OBJ: u16 = 0x04;
pub const ACL_GROUP: u16 = 0x08;
pub const ACL_MASK: u16 = 0x10;
pub const ACL_OTHER: u16 = 0x20;

/// ACL permissions.
pub const ACL_READ: u16 = 0x04;
pub const ACL_WRITE: u16 = 0x02;
pub const ACL_EXECUTE: u16 = 0x01;

// ─── On-disk structures ────────────────────────────────────────────────

/// POSIX ACL entry on disk (8 bytes).
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Ext4AclEntry {
    pub tag: u16,
    pub perms: u16,
    pub id: u32,
}

/// POSIX ACL on-disk header + entries.
#[derive(Clone, Debug)]
pub struct Ext4Acl {
    pub entries: Vec<Ext4AclEntry>,
}

// ─── Parsing ────────────────────────────────────────────────────────────

/// Parse POSIX ACL entries from xattr value data.
/// Returns the ACL or an error.
pub fn parse_acl(data: &[u8]) -> Ext4Result<Ext4Acl> {
    if data.len() < 4 {
        return Err(Ext4Error::InvalidXattr);
    }

    let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if version != EXT4_ACL_VERSION {
        return Err(Ext4Error::InvalidXattr);
    }

    let remaining = &data[4..];
    let count = remaining.len() / 8;
    let mut entries = Vec::with_capacity(count);

    for i in 0..count {
        let off = i * 8;
        if off + 8 > remaining.len() {
            break;
        }
        entries.push(Ext4AclEntry {
            tag: u16::from_le_bytes([remaining[off], remaining[off + 1]]),
            perms: u16::from_le_bytes([remaining[off + 2], remaining[off + 3]]),
            id: u32::from_le_bytes([
                remaining[off + 4], remaining[off + 5],
                remaining[off + 6], remaining[off + 7],
            ]),
        });
    }

    Ok(Ext4Acl { entries })
}

/// Serialize ACL entries into a buffer (xattr value format).
pub fn serialize_acl(acl: &Ext4Acl, buf: &mut [u8]) -> Ext4Result<usize> {
    if buf.len() < 4 + acl.entries.len() * 8 {
        return Err(Ext4Error::NoSpace);
    }

    // Write version
    buf[0..4].copy_from_slice(&EXT4_ACL_VERSION.to_le_bytes());

    // Write entries
    for (i, entry) in acl.entries.iter().enumerate() {
        let off = 4 + i * 8;
        buf[off..off + 2].copy_from_slice(&entry.tag.to_le_bytes());
        buf[off + 2..off + 4].copy_from_slice(&entry.perms.to_le_bytes());
        buf[off + 4..off + 8].copy_from_slice(&entry.id.to_le_bytes());
    }

    Ok(4 + acl.entries.len() * 8)
}

/// Convert POSIX file mode + ACL to effective permissions for a given UID/GID.
///
/// This is a simplified permission check. For a full implementation, see
/// the POSIX ACL specification.
pub fn acl_permissions(acl: &Ext4Acl, uid: u32, gid: u32, target_uid: u32, target_gid: u32) -> u16 {
    let mut effective_perms = 0u16;

    for entry in &acl.entries {
        match entry.tag {
            ACL_USER_OBJ => {
                if target_uid == uid {
                    effective_perms = entry.perms;
                }
            }
            ACL_USER => {
                if entry.id == target_uid {
                    effective_perms = entry.perms;
                }
            }
            ACL_GROUP_OBJ => {
                if target_gid == gid {
                    // If ACL_MASK exists, group perms are masked
                    effective_perms = entry.perms;
                }
            }
            ACL_GROUP => {
                if entry.id == target_gid {
                    effective_perms = entry.perms;
                }
            }
            ACL_MASK => {
                // Mask applies to named user, group obj, named group entries
                // We handle this at the end
            }
            ACL_OTHER => {
                // If no other entry matched, use this as fallback
                if effective_perms == 0 {
                    effective_perms = entry.perms;
                }
            }
            _ => {}
        }
    }

    // Apply mask to non-owner, non-other entries
    let has_mask = acl.entries.iter().any(|e| e.tag == ACL_MASK);
    if has_mask {
        if let Some(mask) = acl.entries.iter().find(|e| e.tag == ACL_MASK) {
            // If effective perms came from a named user or named group or group obj
            let from_named = acl.entries.iter().any(|e| {
                (e.tag == ACL_USER && e.id == target_uid)
                    || (e.tag == ACL_GROUP && e.id == target_gid)
                    || (e.tag == ACL_GROUP_OBJ && !(target_uid == uid))
            });
            if from_named {
                effective_perms &= mask.perms;
            }
        }
    }

    effective_perms
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_acl_valid() {
        let mut data = vec![0u8; 4 + 3 * 8];
        data[0..4].copy_from_slice(&EXT4_ACL_VERSION.to_le_bytes());

        // User_obj: rwx
        data[4..6].copy_from_slice(&ACL_USER_OBJ.to_le_bytes());
        data[6..8].copy_from_slice(&(ACL_READ | ACL_WRITE | ACL_EXECUTE).to_le_bytes());
        // Group_obj: r-x
        data[12..14].copy_from_slice(&ACL_GROUP_OBJ.to_le_bytes());
        data[14..16].copy_from_slice(&(ACL_READ | ACL_EXECUTE).to_le_bytes());
        // Other: r--
        data[20..22].copy_from_slice(&ACL_OTHER.to_le_bytes());
        data[22..24].copy_from_slice(&ACL_READ.to_le_bytes());

        let acl = parse_acl(&data).unwrap();
        assert_eq!(acl.entries.len(), 3);
        assert_eq!(acl.entries[0].tag, ACL_USER_OBJ);
        assert_eq!(acl.entries[0].perms, ACL_READ | ACL_WRITE | ACL_EXECUTE);
    }

    #[test]
    fn test_parse_acl_invalid_version() {
        let mut data = vec![0u8; 8];
        data[0..4].copy_from_slice(&0xDEADu32.to_le_bytes());
        assert!(parse_acl(&data).is_err());
    }

    #[test]
    fn test_parse_acl_too_short() {
        assert!(parse_acl(&[0u8; 3]).is_err());
    }

    #[test]
    fn test_serialize_acl_roundtrip() {
        let acl = Ext4Acl {
            entries: vec![
                Ext4AclEntry { tag: ACL_USER_OBJ, perms: ACL_READ | ACL_WRITE | ACL_EXECUTE, id: 0 },
                Ext4AclEntry { tag: ACL_GROUP_OBJ, perms: ACL_READ | ACL_EXECUTE, id: 0 },
                Ext4AclEntry { tag: ACL_OTHER, perms: ACL_READ, id: 0 },
            ],
        };

        let mut buf = vec![0u8; 4 + 3 * 8];
        let written = serialize_acl(&acl, &mut buf).unwrap();
        assert_eq!(written, 4 + 3 * 8);

        let parsed = parse_acl(&buf).unwrap();
        assert_eq!(parsed.entries.len(), 3);
        assert_eq!(parsed.entries[0].tag, ACL_USER_OBJ);
        assert_eq!(parsed.entries[0].perms, ACL_READ | ACL_WRITE | ACL_EXECUTE);
    }
}
