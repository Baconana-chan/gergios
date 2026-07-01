//! # ATA Command Helpers
//!
//! Higher-level ATA command wrappers: IDENTIFY DEVICE, READ/WRITE DMA EXT,
//! FLUSH CACHE, SET FEATURES (write cache control), and ATAPI helpers.
//!
//! These build on top of the port-level command execution infrastructure.

#![allow(dead_code)]

use crate::ffi;
use crate::registers::{
    self, ata_cmd, ata_id, fis as fis_reg, is_fpdma_cmd, port_flags, PortState, ATA_SECTOR_SIZE,
    MAX_PRDS,
};
use crate::hba::HbaRef;
use crate::port::{H2dFis, Prd, Port, PortBuffers};

// ============================================================================
// ATAPI packet sizes
// ============================================================================

pub const ATAPI_PACKET_SIZE: usize = 16;
pub const ATAPI_REQUEST_SENSE_LEN: usize = 18;
pub const ATAPI_READ_CAPACITY_LEN: usize = 8;

// ============================================================================
// ATAPI commands
// ============================================================================

pub const ATAPI_CMD_TEST_UNIT: u8 = 0x00;
pub const ATAPI_CMD_REQUEST_SENSE: u8 = 0x03;
pub const ATAPI_SENSE_UNIT_ATT: u8 = 6;
pub const ATAPI_CMD_START_STOP: u8 = 0x1B;
pub const ATAPI_START_STOP_EJECT: u8 = 0x02;
pub const ATAPI_START_STOP_LOAD: u8 = 0x03;
pub const ATAPI_CMD_READ_CAPACITY: u8 = 0x25;
pub const ATAPI_CMD_READ: u8 = 0xA8;
pub const ATAPI_CMD_WRITE: u8 = 0xAA;

// ============================================================================
// Command Result
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdResult {
    Failure,
    Success,
}

// ============================================================================
// Low-level command setup
// ============================================================================

/// Set up a H2D FIS in a command table.
pub fn setup_fis(ct: &mut [u8], fis: &H2dFis) -> usize {
    let size = core::mem::size_of::<H2dFis>();
    let raw = unsafe {
        core::slice::from_raw_parts(fis as *const H2dFis as *const u8, size)
    };
    ct[..size].copy_from_slice(raw);
    size
}

/// Set up PRD entries in a command table.
pub fn setup_prdt(ct: &mut [u8], prds: &[Prd]) {
    let prd_offset = 0x80; // AHCI_CT_PRDT_OFF
    let prd_bytes = unsafe {
        core::slice::from_raw_parts(
            prds.as_ptr() as *const u8,
            prds.len() * core::mem::size_of::<Prd>(),
        )
    };
    ct[prd_offset..prd_offset + prd_bytes.len()].copy_from_slice(prd_bytes);
}

/// Command list entry = 32 bytes = 8 dwords.
const CL_ENTRY_DWORDS: usize = 8;

/// Set up a command list entry.
pub fn setup_cl_entry(cl: &mut [u32], cmd_tag: u32, ct_phys: u64, prdtl: u32, write: bool) {
    let index = cmd_tag as usize * CL_ENTRY_DWORDS;
    let dw = &mut cl[index..index + 8];
    dw[0] = (prdtl << 16) | ((write as u32) << 6) | (5 << 0); // CFL=5 dwords
    dw[2] = ct_phys as u32;
    dw[3] = (ct_phys >> 32) as u32;
    // dw[1], dw[4..7] are zero (byte count, reserved)
}

// ============================================================================
// ATA IDENTIFY
// ============================================================================

/// Execute IDENTIFY DEVICE or IDENTIFY PACKET DEVICE.
pub fn ata_identify(hba: &HbaRef, port_idx: usize, port: &mut Port, blocking: bool) -> CmdResult {
    let mut fis = H2dFis {
        fis_type: registers::fis::TYPE_H2D,
        flags: fis_reg::H2D_FLAGS_C,
        cmd: if (port.flags & port_flags::ATAPI) != 0 {
            ata_cmd::IDENTIFY_PACKET
        } else {
            ata_cmd::IDENTIFY
        },
        feat: 0,
        lba_lo: 0,
        lba_mid: 0,
        lba_hi: 0,
        dev: 0,
        lba_lo_exp: 0,
        lba_mid_exp: 0,
        lba_hi_exp: 0,
        feat_exp: 0,
        sec_cnt: 0,
        sec_cnt_exp: 0,
        resv: 0,
        ctl: 0,
        resv2: [0; 4],
    };

    let prd = Prd {
        dba: port.bufs.tmp_phys as u32,
        resv: 0,
        resv2: 0,
        size: registers::ATA_ID_SIZE as u32 - 1,
    };

    // Set up command table
    let ct_virt = port.bufs.ct_virt[0];
    unsafe { core::ptr::write_bytes(ct_virt, 0, registers::CT_SIZE) };
    let ct_slice = unsafe { core::slice::from_raw_parts_mut(ct_virt, registers::CT_SIZE) };
    setup_fis(ct_slice, &fis);
    setup_prdt(ct_slice, &[prd]);

    // Set up command list
    let cl_slice = unsafe {
        core::slice::from_raw_parts_mut(port.bufs.cl_virt, registers::CL_SIZE / 4)
    };
    setup_cl_entry(cl_slice, 0, port.bufs.ct_phys[0], 1, false);

    // Issue command (set SACT for NCQ)
    if (port.flags & port_flags::HAS_NCQ) != 0 {
        hba.port_write32(port_idx, registers::port::SACT, 1 << 0);
    }
    hba.port_issue_cmd(port_idx, 0);

    if blocking {
        wait_for_cmd(hba, port_idx, port, 0, 10_000)
    } else {
        CmdResult::Success
    }
}

/// Wait for a command to complete (spin with timeout).
fn wait_for_cmd(
    hba: &HbaRef,
    port_idx: usize,
    port: &Port,
    cmd_tag: u32,
    timeout_ms: u64,
) -> CmdResult {
    let timeout = timeout_ms * 1000; // microseconds
    let mut waited = 0u32;
    loop {
        // Check CI (non-NCQ) or SACT (NCQ) register
        let mask = 1 << cmd_tag;
        let done = if (port.flags & port_flags::NCQ_MODE) != 0 {
            (hba.port_read32(port_idx, registers::port::SACT) & mask) == 0
        } else {
            (hba.port_read32(port_idx, registers::port::CI) & mask) == 0
        };

        if done {
            // Check for errors
            let tfd = hba.port_read32(port_idx, registers::port::TFD);
            if tfd & (registers::port::TFD_ERR | registers::port::TFD_DF) != 0 {
                return CmdResult::Failure;
            }
            return CmdResult::Success;
        }

        if waited >= timeout as u32 {
            return CmdResult::Failure;
        }
        ffi::udelay(100);
        waited += 100;
    }
}

// ============================================================================
// ATA IDENTIFY data parsing
// ============================================================================

/// Parse IDENTIFY data and update port state. Returns true if device is usable.
pub fn ata_parse_identify(port: &mut Port, id_buf: &[u16]) -> bool {
    let gcap = id_buf[ata_id::GCAP];
    let cap = id_buf[ata_id::CAP];
    let sup1 = id_buf[ata_id::SUP1];

    if (port.flags & port_flags::ATAPI) != 0 {
        // ATAPI device check
        if (gcap & 0xC000) != ata_id::GCAP_ATAPI || (gcap & ata_id::GCAP_REMOVABLE) == 0 {
            return false;
        }
        if (cap & ata_id::CAP_DMA) == 0 {
            // Check DMADIR
            let dmadir = id_buf[ata_id::DMADIR];
            if (dmadir & (ata_id::DMADIR_DMADIR | ata_id::DMADIR_DMA)) !=
                (ata_id::DMADIR_DMADIR | ata_id::DMADIR_DMA) {
                return false;
            }
            port.flags |= port_flags::USE_DMADIR;
        }
    } else {
        // ATA device check
        if (gcap & 0x8000) != ata_id::GCAP_ATA || (gcap & ata_id::GCAP_REMOVABLE) != 0 {
            return false;
        }
        if (cap & (ata_id::CAP_LBA | ata_id::CAP_DMA)) != (ata_id::CAP_LBA | ata_id::CAP_DMA) {
            return false;
        }
        if (sup1 & ata_id::SUP1_VALID_MASK) != ata_id::SUP1_VALID ||
            (sup1 & ata_id::SUP1_FLUSH) == 0 ||
            (sup1 & ata_id::SUP1_LBA48) == 0 {
            return false;
        }
    }

    // Read LBA count
    port.lba_count = (id_buf[ata_id::LBA3] as u64) << 48
        | (id_buf[ata_id::LBA2] as u64) << 32
        | (id_buf[ata_id::LBA1] as u64) << 16
        | id_buf[ata_id::LBA0] as u64;

    // Check for long logical sectors
    let plss = id_buf[ata_id::PLSS];
    if (plss & ata_id::PLSS_VALID_MASK) == ata_id::PLSS_VALID &&
        (plss & ata_id::PLSS_LLS) != 0 {
        port.sector_size = ((id_buf[ata_id::LSS1] as u32) << 16 | id_buf[ata_id::LSS0] as u32) << 1;
    } else {
        port.sector_size = ATA_SECTOR_SIZE as u32;
    }

    if port.sector_size < ATA_SECTOR_SIZE as u32 {
        return false;
    }

    // NCQ support
    if id_buf[ata_id::SATA_CAP] & ata_id::SATA_CAP_NCQ != 0 {
        port.flags |= port_flags::HAS_NCQ;
        port.queue_depth = ((id_buf[ata_id::QDEPTH] & ata_id::QDEPTH_MASK) + 1) as u32;
    }

    // Write cache and flush support
    port.flags |= port_flags::HAS_MEDIUM | port_flags::HAS_FLUSH;
    if (sup1 & ata_id::SUP1_VALID_MASK) == ata_id::SUP1_VALID {
        if id_buf[ata_id::SUP0] & ata_id::SUP0_WCACHE != 0 {
            port.flags |= port_flags::HAS_WCACHE;
        }
    }

    // FUA support
    if (id_buf[ata_id::ENA2] & ata_id::ENA2_VALID_MASK) == ata_id::ENA2_VALID &&
        (id_buf[ata_id::ENA2] & ata_id::ENA2_FUA) != 0 {
        port.flags |= port_flags::HAS_FUA;
    }

    true
}

// ============================================================================
// ATA data transfer (READ/WRITE DMA EXT)
// ============================================================================

/// Perform an ATA data transfer.
pub fn ata_transfer(
    hba: &HbaRef,
    port_idx: usize,
    port: &mut Port,
    start_lba: u64,
    count: u32,
    write: bool,
    force: bool,
    prds: &[Prd],
) -> CmdResult {
    let fis = H2dFis {
        fis_type: registers::fis::TYPE_H2D,
        flags: fis_reg::H2D_FLAGS_C,
        cmd: if force && (port.flags & port_flags::HAS_FUA) != 0 && write {
            ata_cmd::WRITE_DMA_FUA_EXT
        } else if write {
            ata_cmd::WRITE_DMA_EXT
        } else {
            ata_cmd::READ_DMA_EXT
        },
        feat: 0,
        lba_lo: (start_lba & 0xFF) as u8,
        lba_mid: ((start_lba >> 8) & 0xFF) as u8,
        lba_hi: ((start_lba >> 16) & 0xFF) as u8,
        dev: fis_reg::DEV_LBA,
        lba_lo_exp: ((start_lba >> 24) & 0xFF) as u8,
        lba_mid_exp: ((start_lba >> 32) & 0xFF) as u8,
        lba_hi_exp: ((start_lba >> 40) & 0xFF) as u8,
        feat_exp: 0,
        sec_cnt: (count & 0xFF) as u8,
        sec_cnt_exp: ((count >> 8) & 0xFF) as u8,
        resv: 0,
        ctl: 0,
        resv2: [0; 4],
    };

    // Set up command table
    let ct_virt = port.bufs.ct_virt[0];
    unsafe { core::ptr::write_bytes(ct_virt, 0, registers::CT_SIZE) };
    let ct_slice = unsafe { core::slice::from_raw_parts_mut(ct_virt, registers::CT_SIZE) };
    setup_fis(ct_slice, &fis);
    setup_prdt(ct_slice, prds);

    // Set up command list
    let cl_slice = unsafe {
        core::slice::from_raw_parts_mut(port.bufs.cl_virt, registers::CL_SIZE / 4)
    };
    setup_cl_entry(cl_slice, 0, port.bufs.ct_phys[0], prds.len() as u32, write);

    // Issue command (set SACT for NCQ)
    if (port.flags & port_flags::HAS_NCQ) != 0 {
        hba.port_write32(port_idx, registers::port::SACT, 1 << 0);
    }
    hba.port_issue_cmd(port_idx, 0);
    wait_for_cmd(hba, port_idx, port, 0, 30_000) // 30s timeout for I/O
}

// ============================================================================
// ATA FLUSH CACHE
// ============================================================================

/// Execute FLUSH CACHE.
pub fn ata_flush(hba: &HbaRef, port_idx: usize, port: &mut Port) -> CmdResult {
    let fis = H2dFis {
        fis_type: registers::fis::TYPE_H2D,
        flags: fis_reg::H2D_FLAGS_C,
        cmd: ata_cmd::FLUSH_CACHE,
        feat: 0,
        lba_lo: 0,
        lba_mid: 0,
        lba_hi: 0,
        dev: 0,
        lba_lo_exp: 0,
        lba_mid_exp: 0,
        lba_hi_exp: 0,
        feat_exp: 0,
        sec_cnt: 0,
        sec_cnt_exp: 0,
        resv: 0,
        ctl: 0,
        resv2: [0; 4],
    };

    let ct_virt = port.bufs.ct_virt[0];
    unsafe { core::ptr::write_bytes(ct_virt, 0, registers::CT_SIZE) };
    let ct_slice = unsafe { core::slice::from_raw_parts_mut(ct_virt, registers::CT_SIZE) };
    setup_fis(ct_slice, &fis);

    let cl_slice = unsafe {
        core::slice::from_raw_parts_mut(port.bufs.cl_virt, registers::CL_SIZE / 4)
    };
    setup_cl_entry(cl_slice, 0, port.bufs.ct_phys[0], 0, false);

    // Issue command
    if (port.flags & port_flags::HAS_NCQ) != 0 {
        hba.port_write32(port_idx, registers::port::SACT, 1 << 0);
    }
    hba.port_issue_cmd(port_idx, 0);
    wait_for_cmd(hba, port_idx, port, 0, 60_000) // 60s timeout for flush
}

// ============================================================================
// ATA SET FEATURES (write cache control)
// ============================================================================

/// Enable or disable the write cache.
pub fn ata_set_wcache(hba: &HbaRef, port_idx: usize, port: &mut Port, enable: bool) -> CmdResult {
    let fis = H2dFis {
        fis_type: registers::fis::TYPE_H2D,
        flags: fis_reg::H2D_FLAGS_C,
        cmd: ata_cmd::SET_FEATURES,
        feat: if enable { 0x02 } else { 0x82 }, // SF_EN_WCACHE / SF_DI_WCACHE
        lba_lo: 0,
        lba_mid: 0,
        lba_hi: 0,
        dev: 0,
        lba_lo_exp: 0,
        lba_mid_exp: 0,
        lba_hi_exp: 0,
        feat_exp: 0,
        sec_cnt: 0,
        sec_cnt_exp: 0,
        resv: 0,
        ctl: 0,
        resv2: [0; 4],
    };

    let ct_virt = port.bufs.ct_virt[0];
    unsafe { core::ptr::write_bytes(ct_virt, 0, registers::CT_SIZE) };
    let ct_slice = unsafe { core::slice::from_raw_parts_mut(ct_virt, registers::CT_SIZE) };
    setup_fis(ct_slice, &fis);

    let cl_slice = unsafe {
        core::slice::from_raw_parts_mut(port.bufs.cl_virt, registers::CL_SIZE / 4)
    };
    setup_cl_entry(cl_slice, 0, port.bufs.ct_phys[0], 0, false);

    // Issue command
    if (port.flags & port_flags::HAS_NCQ) != 0 {
        hba.port_write32(port_idx, registers::port::SACT, 1 << 0);
    }
    hba.port_issue_cmd(port_idx, 0);
    wait_for_cmd(hba, port_idx, port, 0, 10_000)
}
