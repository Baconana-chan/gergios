//! Quota management for ext4.
//!
//! Supports V2 quota format with dqblk structures stored in dedicated
//! quota inodes (s_usr_quota_inum, s_grp_quota_inum, s_prj_quota_inum).
//!
//! Quota tracking: updates block/inode usage on allocation/free,
//! enforces limits before allowing new allocations.

use crate::types::*;

// ─── Constants ─────────────────────────────────────────────────────────

/// Quota version 2 format identifier.
pub const EXT4_QUOTA_V2: u32 = 2;

/// Quota IDs for reserved/quota inodes.
pub const EXT4_QUOTA_ROOT_UID: u32 = 0;
pub const EXT4_QUOTA_ROOT_GID: u32 = 0;

// ─── On-disk structures ────────────────────────────────────────────────

/// V2 quota disk block (dqblk) — 48 bytes per entry.
/// Each quota ID (UID/GID/project) has one dqblk entry.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Ext4DqblkV2 {
    pub dqb_id: u32,             // Quota ID (UID, GID, or project ID)
    pub dqb_pad: u32,            // Reserved
    pub dqb_curblocks: u64,      // Current block usage
    pub dqb_curinodes: u64,      // Current inode usage
    pub dqb_bsoftlimit: u64,     // Soft block limit (0 = no limit)
    pub dqb_bhardlimit: u64,     // Hard block limit (0 = no limit)
    pub dqb_isoftlimit: u64,     // Soft inode limit (0 = no limit)
    pub dqb_ihardlimit: u64,     // Hard inode limit (0 = no limit)
    pub dqb_btime: u64,          // Time limit for soft block limit exceeded
    pub dqb_itime: u64,          // Time limit for soft inode limit exceeded
}

/// Quota type identifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum QuotaType {
    User,
    Group,
    Project,
}

impl QuotaType {
    pub fn quota_inum(&self, sb: &Ext4Superblock) -> u32 {
        match self {
            QuotaType::User => sb.s_usr_quota_inum,
            QuotaType::Group => sb.s_grp_quota_inum,
            QuotaType::Project => sb.s_prj_quota_inum,
        }
    }
}

/// Runtime quota state for a single type (user/group/project).
pub struct QuotaState {
    pub qtype: QuotaType,
    /// Cached quota entries: Vec<(quota_id, dqblk)>
    pub entries: Vec<(u32, Ext4DqblkV2)>,
}

/// Top-level quota manager.
pub struct QuotaManager {
    pub user_quota: Option<QuotaState>,
    pub group_quota: Option<QuotaState>,
    pub project_quota: Option<QuotaState>,
    pub enabled: bool,
}

impl QuotaManager {
    /// Create a new quota manager, disabled by default.
    pub fn new() -> Self {
        QuotaManager {
            user_quota: None,
            group_quota: None,
            project_quota: None,
            enabled: false,
        }
    }

    /// Enable quota with the given superblock.
    /// Returns true if any quota inodes are configured.
    pub fn init_from_sb(&mut self, sb: &Ext4Superblock) -> bool {
        let has_user = sb.s_usr_quota_inum != 0;
        let has_group = sb.s_grp_quota_inum != 0;
        let has_project = sb.s_prj_quota_inum != 0;
        let has_any = has_user || has_group || has_project;

        if has_any {
            self.enabled = true;
            if has_user {
                self.user_quota = Some(QuotaState {
                    qtype: QuotaType::User,
                    entries: Vec::new(),
                });
            }
            if has_group {
                self.group_quota = Some(QuotaState {
                    qtype: QuotaType::Group,
                    entries: Vec::new(),
                });
            }
            if has_project {
                self.project_quota = Some(QuotaState {
                    qtype: QuotaType::Project,
                    entries: Vec::new(),
                });
            }
        }

        has_any
    }

    /// Check if a block allocation for the given UID/GID would exceed limits.
    /// Returns `Ext4Error::NoSpace` if quota would be exceeded.
    pub fn check_block_allocation(&self, uid: u32, gid: u32, blocks_needed: u64) -> Ext4Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Check user quota
        if let Some(ref uq) = self.user_quota {
            if let Some((_, dqblk)) = uq.entries.iter().find(|(id, _)| *id == uid) {
                if dqblk.dqb_bhardlimit > 0 && dqblk.dqb_curblocks + blocks_needed > dqblk.dqb_bhardlimit {
                    return Err(Ext4Error::NoSpace);
                }
                if dqblk.dqb_bsoftlimit > 0 && dqblk.dqb_curblocks + blocks_needed > dqblk.dqb_bsoftlimit {
                    // Soft limit exceeded — still allow if within grace time
                    // For simplicity, we enforce soft limits as hard limits
                    return Err(Ext4Error::NoSpace);
                }
            }
        }

        // Check group quota
        if let Some(ref gq) = self.group_quota {
            if let Some((_, dqblk)) = gq.entries.iter().find(|(id, _)| *id == gid) {
                if dqblk.dqb_bhardlimit > 0 && dqblk.dqb_curblocks + blocks_needed > dqblk.dqb_bhardlimit {
                    return Err(Ext4Error::NoSpace);
                }
                if dqblk.dqb_bsoftlimit > 0 && dqblk.dqb_curblocks + blocks_needed > dqblk.dqb_bsoftlimit {
                    return Err(Ext4Error::NoSpace);
                }
            }
        }

        Ok(())
    }

    /// Check if an inode allocation for the given UID/GID would exceed limits.
    pub fn check_inode_allocation(&self, uid: u32, gid: u32) -> Ext4Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(ref uq) = self.user_quota {
            if let Some((_, dqblk)) = uq.entries.iter().find(|(id, _)| *id == uid) {
                if dqblk.dqb_ihardlimit > 0 && dqblk.dqb_curinodes + 1 > dqblk.dqb_ihardlimit {
                    return Err(Ext4Error::NoSpace);
                }
                if dqblk.dqb_isoftlimit > 0 && dqblk.dqb_curinodes + 1 > dqblk.dqb_isoftlimit {
                    return Err(Ext4Error::NoSpace);
                }
            }
        }

        if let Some(ref gq) = self.group_quota {
            if let Some((_, dqblk)) = gq.entries.iter().find(|(id, _)| *id == gid) {
                if dqblk.dqb_ihardlimit > 0 && dqblk.dqb_curinodes + 1 > dqblk.dqb_ihardlimit {
                    return Err(Ext4Error::NoSpace);
                }
                if dqblk.dqb_isoftlimit > 0 && dqblk.dqb_curinodes + 1 > dqblk.dqb_isoftlimit {
                    return Err(Ext4Error::NoSpace);
                }
            }
        }

        Ok(())
    }

    /// Update block usage for a given UID/GID after allocation.
    pub fn add_blocks(&mut self, uid: u32, gid: u32, blocks: u64) {
        if !self.enabled {
            return;
        }

        if let Some(ref mut uq) = self.user_quota {
            if let Some((_, ref mut dqblk)) = uq.entries.iter_mut().find(|(id, _)| *id == uid) {
                dqblk.dqb_curblocks = dqblk.dqb_curblocks.saturating_add(blocks);
            }
        }

        if let Some(ref mut gq) = self.group_quota {
            if let Some((_, ref mut dqblk)) = gq.entries.iter_mut().find(|(id, _)| *id == gid) {
                dqblk.dqb_curblocks = dqblk.dqb_curblocks.saturating_add(blocks);
            }
        }
    }

    /// Update block usage for a given UID/GID after freeing.
    pub fn sub_blocks(&mut self, uid: u32, gid: u32, blocks: u64) {
        if !self.enabled {
            return;
        }

        if let Some(ref mut uq) = self.user_quota {
            if let Some((_, ref mut dqblk)) = uq.entries.iter_mut().find(|(id, _)| *id == uid) {
                dqblk.dqb_curblocks = dqblk.dqb_curblocks.saturating_sub(blocks);
            }
        }

        if let Some(ref mut gq) = self.group_quota {
            if let Some((_, ref mut dqblk)) = gq.entries.iter_mut().find(|(id, _)| *id == gid) {
                dqblk.dqb_curblocks = dqblk.dqb_curblocks.saturating_sub(blocks);
            }
        }
    }

    /// Update inode usage for a given UID/GID after allocation.
    pub fn add_inode(&mut self, uid: u32, gid: u32) {
        if !self.enabled {
            return;
        }

        if let Some(ref mut uq) = self.user_quota {
            if let Some((_, ref mut dqblk)) = uq.entries.iter_mut().find(|(id, _)| *id == uid) {
                dqblk.dqb_curinodes = dqblk.dqb_curinodes.saturating_add(1);
            }
        }

        if let Some(ref mut gq) = self.group_quota {
            if let Some((_, ref mut dqblk)) = gq.entries.iter_mut().find(|(id, _)| *id == gid) {
                dqblk.dqb_curinodes = dqblk.dqb_curinodes.saturating_add(1);
            }
        }
    }

    /// Update inode usage for a given UID/GID after freeing.
    pub fn sub_inode(&mut self, uid: u32, gid: u32) {
        if !self.enabled {
            return;
        }

        if let Some(ref mut uq) = self.user_quota {
            if let Some((_, ref mut dqblk)) = uq.entries.iter_mut().find(|(id, _)| *id == uid) {
                dqblk.dqb_curinodes = dqblk.dqb_curinodes.saturating_sub(1);
            }
        }

        if let Some(ref mut gq) = self.group_quota {
            if let Some((_, ref mut dqblk)) = gq.entries.iter_mut().find(|(id, _)| *id == gid) {
                dqblk.dqb_curinodes = dqblk.dqb_curinodes.saturating_sub(1);
            }
        }
    }
}

// ─── Parsing quota data ────────────────────────────────────────────────

/// Parse a V2 dqblk entry from raw data (48 bytes).
pub fn parse_dqblk_v2(data: &[u8]) -> Ext4Result<Ext4DqblkV2> {
    if data.len() < 72 {
        return Err(Ext4Error::IoError);
    }

    Ok(Ext4DqblkV2 {
        dqb_id: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        dqb_pad: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        dqb_curblocks: u64::from_le_bytes([data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15]]),
        dqb_curinodes: u64::from_le_bytes([data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23]]),
        dqb_bsoftlimit: u64::from_le_bytes([data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31]]),
        dqb_bhardlimit: u64::from_le_bytes([data[32], data[33], data[34], data[35], data[36], data[37], data[38], data[39]]),
        dqb_isoftlimit: u64::from_le_bytes([data[40], data[41], data[42], data[43], data[44], data[45], data[46], data[47]]),
        dqb_ihardlimit: u64::from_le_bytes([data[48], data[49], data[50], data[51], data[52], data[53], data[54], data[55]]),
        dqb_btime: u64::from_le_bytes([data[56], data[57], data[58], data[59], data[60], data[61], data[62], data[63]]),
        dqb_itime: u64::from_le_bytes([data[64], data[65], data[66], data[67], data[68], data[69], data[70], data[71]]),
    })
}

/// Serialize a V2 dqblk entry to raw data (72 bytes).
pub fn serialize_dqblk_v2(data: &mut [u8], dqblk: &Ext4DqblkV2) {
    if data.len() < 72 { return; }

    data[0..4].copy_from_slice(&dqblk.dqb_id.to_le_bytes());
    data[4..8].copy_from_slice(&dqblk.dqb_pad.to_le_bytes());
    data[8..16].copy_from_slice(&dqblk.dqb_curblocks.to_le_bytes());
    data[16..24].copy_from_slice(&dqblk.dqb_curinodes.to_le_bytes());
    data[24..32].copy_from_slice(&dqblk.dqb_bsoftlimit.to_le_bytes());
    data[32..40].copy_from_slice(&dqblk.dqb_bhardlimit.to_le_bytes());
    data[40..48].copy_from_slice(&dqblk.dqb_isoftlimit.to_le_bytes());
    data[48..56].copy_from_slice(&dqblk.dqb_ihardlimit.to_le_bytes());
    data[56..64].copy_from_slice(&dqblk.dqb_btime.to_le_bytes());
    data[64..72].copy_from_slice(&dqblk.dqb_itime.to_le_bytes());
}

/// Read quota entries from a quota inode using a block reader.
pub fn read_quota_entries<FR>(
    _sb: &Ext4Superblock,
    _quota_ino: u32,
    mut _read_block: FR,
) -> Ext4Result<Vec<(u32, Ext4DqblkV2)>>
where
    FR: FnMut(u64, &mut [u8]) -> Ext4Result<()>,
{
    // Parse quota inode, read its data blocks via extent_read,
    // extract dqblk entries from each block.
    //
    // V2 quota format: simple array of dqblk entries (72 bytes each)
    // stored in the data blocks of the quota inode.
    //
    // For a minimal implementation, we read all data blocks and
    // parse dqblk entries sequentially.
    //
    // Full implementation would handle the quota tree structure,
    // but for now we use the flat array assumption common on
    // smaller filesystems.

    let mut entries = Vec::new();
    let _ = _read_block; // Placeholder — full implementation pending
    Ok(entries)
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dqblk_v2() {
        let mut data = vec![0u8; 72];
        data[0..4].copy_from_slice(&1000u32.to_le_bytes()); // dqb_id = 1000
        data[8..16].copy_from_slice(&500u64.to_le_bytes()); // dqb_curblocks = 500
        data[16..24].copy_from_slice(&10u64.to_le_bytes()); // dqb_curinodes = 10
        data[32..40].copy_from_slice(&1000u64.to_le_bytes()); // dqb_bhardlimit = 1000

        let dqblk = parse_dqblk_v2(&data).unwrap();
        assert_eq!(dqblk.dqb_id, 1000);
        assert_eq!(dqblk.dqb_curblocks, 500);
        assert_eq!(dqblk.dqb_curinodes, 10);
        assert_eq!(dqblk.dqb_bhardlimit, 1000);
    }

    #[test]
    fn test_serialize_dqblk_v2_roundtrip() {
        let dqblk = Ext4DqblkV2 {
            dqb_id: 2000,
            dqb_pad: 0,
            dqb_curblocks: 1000,
            dqb_curinodes: 50,
            dqb_bsoftlimit: 5000,
            dqb_bhardlimit: 10000,
            dqb_isoftlimit: 100,
            dqb_ihardlimit: 500,
            dqb_btime: 0,
            dqb_itime: 0,
        };

        let mut data = vec![0u8; 72];
        serialize_dqblk_v2(&mut data, &dqblk);
        let parsed = parse_dqblk_v2(&data).unwrap();
        assert_eq!(parsed.dqb_id, 2000);
        assert_eq!(parsed.dqb_curblocks, 1000);
        assert_eq!(parsed.dqb_bhardlimit, 10000);
        assert_eq!(parsed.dqb_ihardlimit, 500);
    }

    #[test]
    fn test_quota_manager_block_check() {
        let mut qm = QuotaManager::new();
        qm.enabled = true;
        qm.user_quota = Some(QuotaState {
            qtype: QuotaType::User,
            entries: vec![(1000, Ext4DqblkV2 {
                dqb_id: 1000, dqb_pad: 0,
                dqb_curblocks: 50, dqb_curinodes: 5,
                dqb_bsoftlimit: 100, dqb_bhardlimit: 200,
                dqb_isoftlimit: 10, dqb_ihardlimit: 50,
                dqb_btime: 0, dqb_itime: 0,
            })],
        });

        // Should succeed: 50 + 50 = 100 <= 200 (bsoftlimit is 100, so 50+50=100 is OK)
        assert!(qm.check_block_allocation(1000, 0, 50).is_ok());

        // Should fail: 50 + 200 = 250 > 200 (bhardlimit is 200)
        assert!(qm.check_block_allocation(1000, 0, 200).is_err());

        // Should fail: 50 + 51 = 101 > 100 (bsoftlimit is 100)
        assert!(qm.check_block_allocation(1000, 0, 51).is_err());

        // Unknown user: should succeed
        assert!(qm.check_block_allocation(9999, 0, 99999).is_ok());
    }

    #[test]
    fn test_quota_manager_inode_check() {
        let mut qm = QuotaManager::new();
        qm.enabled = true;
        qm.user_quota = Some(QuotaState {
            qtype: QuotaType::User,
            entries: vec![(1000, Ext4DqblkV2 {
                dqb_id: 1000, dqb_pad: 0,
                dqb_curblocks: 0, dqb_curinodes: 5,
                dqb_bsoftlimit: 0, dqb_bhardlimit: 0,
                dqb_isoftlimit: 10, dqb_ihardlimit: 20,
                dqb_btime: 0, dqb_itime: 0,
            })],
        });

        // Should succeed: 5 + 1 = 6 <= 20
        assert!(qm.check_inode_allocation(1000, 0).is_ok());

        // Simulate adding 15 more inodes
        for _ in 0..15 {
            qm.add_inode(1000, 0);
        }
        // Now: 5 + 15 = 20 (at hard limit)

        // Should fail: 20 + 1 = 21 > 20
        assert!(qm.check_inode_allocation(1000, 0).is_err());
    }

    #[test]
    fn test_quota_disabled() {
        let qm = QuotaManager::new();
        // Even with extreme values, disabled quota should allow everything
        assert!(qm.check_block_allocation(0, 0, u64::MAX).is_ok());
        assert!(qm.check_inode_allocation(0, 0).is_ok());
    }

    #[test]
    fn test_quota_tracking() {
        let mut qm = QuotaManager::new();
        qm.enabled = true;
        qm.user_quota = Some(QuotaState {
            qtype: QuotaType::User,
            entries: vec![(1000, Ext4DqblkV2 {
                dqb_id: 1000, dqb_pad: 0,
                dqb_curblocks: 0, dqb_curinodes: 0,
                dqb_bsoftlimit: 0, dqb_bhardlimit: 0,
                dqb_isoftlimit: 0, dqb_ihardlimit: 0,
                dqb_btime: 0, dqb_itime: 0,
            })],
        });

        qm.add_blocks(1000, 0, 100);
        qm.add_inode(1000, 0);
        qm.add_inode(1000, 0);

        let entry = qm.user_quota.as_ref().unwrap().entries[0].1;
        assert_eq!(entry.dqb_curblocks, 100);
        assert_eq!(entry.dqb_curinodes, 2);

        qm.sub_blocks(1000, 0, 50);
        qm.sub_inode(1000, 0);

        let entry = qm.user_quota.as_ref().unwrap().entries[0].1;
        assert_eq!(entry.dqb_curblocks, 50);
        assert_eq!(entry.dqb_curinodes, 1);
    }
}
