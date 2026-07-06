//! # USB Mass Storage Class (Bulk-Only Transport)
//!
//! Implements the USB Mass Storage Class Bulk-Only Transport (BOT) protocol:
//! - CBW (Command Block Wrapper) / CSW (Command Status Wrapper)
//! - SCSI command set: READ10, WRITE10, INQUIRY, READ CAPACITY(10), TEST UNIT READY
//! - Bulk endpoint discovery from config descriptor
//!
//! ## BOT Protocol Flow
//!
//! 1. Send CBW on bulk OUT endpoint (31 bytes)
//! 2. Transfer data on bulk IN/OUT (optional, depends on direction)
//! 3. Receive CSW on bulk IN endpoint (13 bytes)
//! 4. Verify CSW signature + status

use crate::ffi;
use crate::ring::{RingMem, RING_SIZE};
use crate::registers::{
    self, EndpointDescriptor, InterfaceDescriptor, usb_descriptor,
    usb_class, usb_xfer_type, Cbw, Csw, ScsiSenseData,
    build_read10_cdb, build_write10_cdb, build_read_capacity10_cdb,
    build_inquiry_cdb, build_test_unit_ready_cdb, build_request_sense_cdb,
    ReadCapacity10,
};
use crate::xhci::XhciController;

/// Maximum number of USB Mass Storage devices we can track.
pub const MAX_MSC_DEVICES: usize = 4;

/// Detected USB Mass Storage device state.
/// NOTE: Intentionally does NOT derive Clone because RingMem (DMA buffer)
/// does not implement Clone. Each MscDevice uniquely owns its DMA buffers.
pub struct MscDevice {
    /// xHCI device slot ID.
    pub slot_id: u8,
    /// Bulk OUT endpoint number (host→device).
    pub ep_out: u8,
    /// Bulk IN endpoint number (device→host).
    pub ep_in: u8,
    /// Max LUN (from Get Max LUN).
    pub max_lun: u8,
    /// Block size in bytes (from READ CAPACITY).
    pub block_size: u32,
    /// Number of blocks (from READ CAPACITY, must be > 0).
    pub block_count: u64,
    /// Whether the device is initialized and ready.
    pub ready: bool,
    /// DMA buffer for CBW (31 bytes).
    pub cbw_buf: Option<RingMem>,
    /// DMA buffer for CSW (13 bytes).
    pub csw_buf: Option<RingMem>,
    /// DMA data buffer for BOT bulk data transfers (256KB max).
    pub data_buf: Option<RingMem>,
    /// Last SCSI sense data from automatic Request Sense on error.
    pub last_sense: ScsiSenseData,
}

impl MscDevice {
    pub fn new() -> Self {
        Self {
            slot_id: 0,
            ep_out: 0,
            ep_in: 0,
            max_lun: 0,
            block_size: 512,
            block_count: 0,
            ready: false,
            cbw_buf: None,
            csw_buf: None,
            data_buf: None,
            last_sense: ScsiSenseData::zeroed(),
        }
    }

    /// Allocate DMA buffers for CBW, CSW, and data (256KB).
    fn alloc_buffers(&mut self) -> bool {
        self.cbw_buf = RingMem::alloc(64); // 64 bytes (31 + padding)
        self.csw_buf = RingMem::alloc(64); // 64 bytes (13 + padding)
        self.data_buf = RingMem::alloc(262144); // 256KB for bulk data (up to 2× MAX_TRB_DATA_LEN)
        self.cbw_buf.is_some() && self.csw_buf.is_some() && self.data_buf.is_some()
    }

    fn free_buffers(&mut self) {
        if let Some(b) = &mut self.cbw_buf { b.free(); self.cbw_buf = None; }
        if let Some(b) = &mut self.csw_buf { b.free(); self.csw_buf = None; }
        if let Some(b) = &mut self.data_buf { b.free(); self.data_buf = None; }
    }
}

// ============================================================================
// Config Descriptor Parsing — Find Mass Storage Bulk Endpoints
// ============================================================================

/// Result from scanning a config descriptor for a Mass Storage interface.
pub struct MscInterfaceInfo {
    pub interface_number: u8,
    pub ep_out: u8,   // endpoint number
    pub ep_in: u8,    // endpoint number
    pub mps_out: u16, // max packet size for OUT
    pub mps_in: u16,  // max packet size for IN
}

/// Scan a USB configuration descriptor buffer for Mass Storage interface + bulk endpoints.
/// `buf` — full config descriptor data (as read by get_config_descriptor).
/// Returns the first MSC interface found, or None.
pub fn find_msc_interface(buf: &[u8]) -> Option<MscInterfaceInfo> {
    if buf.len() < 9 { return None; }
    let total_len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
    let data_len = core::cmp::min(buf.len(), total_len);
    if data_len < 9 { return None; }

    let mut i = 0;
    let mut found_msc = false;
    let mut iface_num = 0u8;
    let mut ep_out = 0u8;
    let mut ep_in = 0u8;
    let mut mps_out = 0u16;
    let mut mps_in = 0u16;

    while i + 1 < data_len {
        let len = buf[i] as usize;
        let desc_type = buf[i + 1];
        if len < 2 { break; }

        match desc_type {
            t if t == usb_descriptor::CONFIGURATION => {
                // Skip config descriptor header
            }
            t if t == usb_descriptor::INTERFACE => {
                if i + 9 <= data_len {
                    let iface = match InterfaceDescriptor::parse(&buf[i..]) {
                        Some(iface) => iface,
                        None => break,
                    };
                    // Check if this is Mass Storage class
                    if iface.bInterfaceClass == usb_class::MASS_STORAGE {
                        found_msc = true;
                        iface_num = iface.bInterfaceNumber;
                    } else {
                        found_msc = false;
                    }
                }
            }
            t if t == usb_descriptor::ENDPOINT => {
                if found_msc && i + 7 <= data_len {
                    let ep = match EndpointDescriptor::parse(&buf[i..]) {
                        Some(ep) => ep,
                        None => break,
                    };
                    let ep_num = ep.endpoint_number();
                    let mps = ep.max_packet_size();
                    if ep.transfer_type() == usb_xfer_type::BULK {
                        if ep.is_in() && ep_in == 0 {
                            ep_in = ep_num;
                            mps_in = mps;
                        } else if !ep.is_in() && ep_out == 0 {
                            ep_out = ep_num;
                            mps_out = mps;
                        }
                    }
                }
            }
            _ => {}
        }

        if len == 0 { break; }
        i += len;
    }

    if found_msc && ep_out > 0 && ep_in > 0 {
        Some(MscInterfaceInfo { interface_number: iface_num, ep_out, ep_in, mps_out, mps_in })
    } else {
        None
    }
}

// ============================================================================
// BOT Transport Layer
// ============================================================================

/// Execute a BOT command: send CBW → data (if any) → receive CSW.
///
/// `tag` — unique command tag.
/// `dir_in` — true for data IN (device→host), false for data OUT.
/// `data_buf_phys` — physical address of data buffer (0 if no data).
/// `data_len` — data transfer length.
/// `cdb` — SCSI command descriptor block bytes.
///
/// Returns true if CSW indicates success and signature matches.
pub fn bot_transport(xhc: &mut XhciController, slot_id: u8,
    ep_out: u8, ep_in: u8,
    tag: u32, lun: u8, dir_in: bool,
    data_buf_phys: u64, data_len: u32,
    cdb: &[u8]
) -> bool {
    // Find the device's DMA buffers
    let dev_idx = match find_msc_device(xhc, slot_id) {
        Some(idx) => idx,
        None => return false,
    };
    let cbw_phys: u64;
    let csw_virt: *mut u8;
    let csw_phys: u64;
    {
        let dev = &xhc.msc_devices[dev_idx];
        cbw_phys = match &dev.cbw_buf { Some(b) => b.phys, None => return false };
        csw_virt = match &dev.csw_buf { Some(b) => b.virt, None => return false };
        csw_phys = match &dev.csw_buf { Some(b) => b.phys, None => return false };
    }

    // Build and write CBW
    let cbw = Cbw::new(tag, lun, dir_in, data_len, cdb);
    let cbw_bytes = cbw.as_bytes();
    // Copy CBW bytes into DMA buffer
    let cbw_buf = &xhc.msc_devices[dev_idx].cbw_buf;
    if let Some(b) = cbw_buf {
        unsafe { core::ptr::copy_nonoverlapping(cbw_bytes.as_ptr(), b.virt, 31); }
    }

    // Send CBW on bulk OUT endpoint
    if !xhc.queue_bulk_transfer(slot_id, ep_out, false, cbw_phys, 31, true) {
        return false;
    }
    if !xhc.poll_transfer_event(5_000_000) {
        return false;
    }

    // Data phase (if any)
    if data_len > 0 {
        if !xhc.queue_bulk_transfer(slot_id, if dir_in { ep_in } else { ep_out }, dir_in,
            data_buf_phys, data_len, true) {
            return false;
        }
        if !xhc.poll_transfer_event(30_000_000) { // 30s timeout for bulk data
            return false;
        }
    }

    // Receive CSW on bulk IN endpoint
    if !xhc.queue_bulk_transfer(slot_id, ep_in, true, csw_phys, 13, true) {
        return false;
    }
    if !xhc.poll_transfer_event(5_000_000) {
        return false;
    }

    // Parse CSW
    let csw_data = unsafe { core::slice::from_raw_parts(csw_virt as *const u8, 13) };
    match Csw::parse(csw_data) {
        Some(csw) => {
            if csw.is_ok() {
                true
            } else {
                // CSW indicates failure — auto-sense to capture error details
                let _ = auto_sense(xhc, dev_idx);
                false
            }
        }
        None => false,
    }
}

/// Find the index of an MSC device by slot_id.
fn find_msc_device(xhc: &XhciController, slot_id: u8) -> Option<usize> {
    for i in 0..MAX_MSC_DEVICES {
        if xhc.msc_devices[i].slot_id == slot_id && xhc.msc_devices[i].ready {
            return Some(i);
        }
    }
    None
}

// ============================================================================
// SCSI Commands via BOT
// ============================================================================

/// SCSI INQUIRY — get device identification.
/// Returns true on success, data is in the data buffer at `buf_phys`.
pub fn scsi_inquiry(xhc: &mut XhciController, dev_idx: usize,
    data_buf_phys: u64, alloc_len: u16
) -> bool {
    let dev = &xhc.msc_devices[dev_idx];
    let cdb = build_inquiry_cdb(0, alloc_len);
    bot_transport(xhc, dev.slot_id, dev.ep_out, dev.ep_in,
        1, 0, true, data_buf_phys, alloc_len as u32, &cdb)
}

/// SCSI TEST UNIT READY — check if device is ready.
pub fn scsi_test_unit_ready(xhc: &mut XhciController, dev_idx: usize) -> bool {
    let dev = &xhc.msc_devices[dev_idx];
    let cdb = build_test_unit_ready_cdb();
    bot_transport(xhc, dev.slot_id, dev.ep_out, dev.ep_in,
        2, 0, false, 0, 0, &cdb)
}

/// SCSI READ CAPACITY(10) — get device capacity.
/// Returns ReadCapacity10 on success, None on failure.
pub fn scsi_read_capacity10(xhc: &mut XhciController, dev_idx: usize,
    buf_phys: u64, buf_virt: *mut u8
) -> Option<ReadCapacity10> {
    let dev = &xhc.msc_devices[dev_idx];
    let cdb = build_read_capacity10_cdb();
    if !bot_transport(xhc, dev.slot_id, dev.ep_out, dev.ep_in,
        3, 0, true, buf_phys, 8, &cdb)
    {
        return None;
    }
    let data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, 8) };
    ReadCapacity10::parse(data)
}

/// SCSI READ10 — read `num_blocks` blocks starting at `lba`.
/// Data is written to the buffer at `data_phys`.
pub fn scsi_read10(xhc: &mut XhciController, dev_idx: usize,
    lba: u32, num_blocks: u16, data_phys: u64, data_len: u32
) -> bool {
    let dev = &xhc.msc_devices[dev_idx];
    let cdb = build_read10_cdb(lba, num_blocks);
    bot_transport(xhc, dev.slot_id, dev.ep_out, dev.ep_in,
        4, 0, true, data_phys, data_len, &cdb)
}

/// SCSI WRITE10 — write `num_blocks` blocks starting at `lba`.
/// Data is read from the buffer at `data_phys`.
pub fn scsi_write10(xhc: &mut XhciController, dev_idx: usize,
    lba: u32, num_blocks: u16, data_phys: u64, data_len: u32
) -> bool {
    let dev = &xhc.msc_devices[dev_idx];
    let cdb = build_write10_cdb(lba, num_blocks);
    bot_transport(xhc, dev.slot_id, dev.ep_out, dev.ep_in,
        5, 0, false, data_phys, data_len, &cdb)
}

// ============================================================================
// SCSI Request Sense + Auto-sense (Autoparam)
// ============================================================================

/// Issue SCSI REQUEST SENSE to get error details after a failed command.
/// Stores sense data in the device's `last_sense` field.
/// Returns true if sense data was retrieved successfully.
pub fn scsi_request_sense(xhc: &mut XhciController, dev_idx: usize,
    data_phys: u64, data_virt: *mut u8
) -> bool {
    if dev_idx >= MAX_MSC_DEVICES || !xhc.msc_devices[dev_idx].ready {
        return false;
    }
    let dev = &xhc.msc_devices[dev_idx];
    let cdb = build_request_sense_cdb(18);
    if !bot_transport(xhc, dev.slot_id, dev.ep_out, dev.ep_in,
        6, 0, true, data_phys, 18, &cdb)
    {
        return false;
    }
    // Parse sense data from the data buffer
    let data = unsafe { core::slice::from_raw_parts(data_virt as *const u8, 18) };
    match ScsiSenseData::parse(data) {
        Some(sense) => {
            xhc.msc_devices[dev_idx].last_sense = sense;
            true
        }
        None => false,
    }
}

/// Auto-sense helper: called when a SCSI command fails.
/// Issues REQUEST SENSE and logs the sense key + ASC/ASCQ via ffi::print.
/// Returns true if sense was retrieved (regardless of log level).
pub fn auto_sense(xhc: &mut XhciController, dev_idx: usize) -> bool {
    // Use the device's data buffer for sense data (small — 18 bytes)
    let data_buf = match &xhc.msc_devices[dev_idx].data_buf {
        Some(buf) => buf,
        None => return false,
    };

    if !scsi_request_sense(xhc, dev_idx, data_buf.phys, data_buf.virt) {
        return false;
    }

    let sense = xhc.msc_devices[dev_idx].last_sense;
    if xhc.verbose >= 1 {
        // Log: "MSC: sense_key=SK ASC=XX ASCQ=XX"
        let msg1 = b"MSC: sense_key=";
        let msg2 = b" ASC=";
        let msg3 = b" ASCQ=";
        let sk = sense.sense_key_val();
        let asc_val = sense.asc;
        let ascq_val = sense.ascq;
        log_sense(msg1, sk, msg2, asc_val, msg3, ascq_val);
    }
    true
}

/// Format and print sense info via ffi::print.
fn log_sense(prefix: &[u8], sk: u8, mid1: &[u8], asc_val: u8,
    mid2: &[u8], ascq_val: u8)
{
    let mut buf = [0u8; 40];
    let mut pos = 0;

    // Copy prefix
    buf[pos..pos+prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();

    // SK hex
    buf[pos] = hex_char(sk >> 4); pos += 1;
    buf[pos] = hex_char(sk & 0x0F); pos += 1;

    // mid1
    buf[pos..pos+mid1.len()].copy_from_slice(mid1);
    pos += mid1.len();

    // ASC hex
    buf[pos] = hex_char(asc_val >> 4); pos += 1;
    buf[pos] = hex_char(asc_val & 0x0F); pos += 1;

    // mid2
    buf[pos..pos+mid2.len()].copy_from_slice(mid2);
    pos += mid2.len();

    // ASCQ hex
    buf[pos] = hex_char(ascq_val >> 4); pos += 1;
    buf[pos] = hex_char(ascq_val & 0x0F); pos += 1;

    // Null-terminate for ffi::print
    buf[pos] = 0;
    ffi::print(&buf[..=pos]);
}

fn hex_char(v: u8) -> u8 {
    let nib = v & 0x0F;
    if nib < 10 { b'0' + nib } else { b'A' + nib - 10 }
}

// ============================================================================
// Device Initialization
// ============================================================================

/// Probe and initialize a USB Mass Storage device on a given slot.
/// Returns the device index if successful.
///
/// Steps:
/// 1. Setup EP0 transfer ring
/// 2. Get device descriptor + config descriptor
/// 3. Find MSC bulk endpoints
/// 4. Configure bulk endpoints with transfer rings
/// 5. Get Max LUN
/// 6. Issue TEST UNIT READY, READ CAPACITY10
/// 7. Store device state
pub fn probe_msc_device(xhc: &mut XhciController, slot_id: u8,
    config_buf_virt: *mut u8, config_buf_phys: u64, config_buf_size: usize
) -> Option<usize> {
    // Find a free slot in the MSC devices array
    let dev_idx = (0..MAX_MSC_DEVICES).find(|&i| !xhc.msc_devices[i].ready)?;

    if xhc.verbose >= 1 {
        ffi::print(b"xHCI: probing USB Mass Storage\0");
    }

    // Step 1: Setup EP0 transfer ring
    if !xhc.setup_ep0_transfer_ring(slot_id) {
        if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC EP0 ring failed\0"); }
        return None;
    }

    // Step 2: Get config descriptor
    if !xhc.get_config_descriptor(slot_id, 0, config_buf_virt, config_buf_phys, config_buf_size) {
        if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC config descriptor failed\0"); }
        return None;
    }

    // Step 3: Find MSC bulk endpoints
    let config_data = unsafe {
        core::slice::from_raw_parts(config_buf_virt as *const u8, config_buf_size)
    };
    let msc_info = match find_msc_interface(config_data) {
        Some(info) => info,
        None => {
            if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC interface not found\0"); }
            return None;
        }
    };

    // Step 4: Configure bulk endpoints
    let dci_out = XhciController::ep_num_to_dci(msc_info.ep_out, false);
    let dci_in = XhciController::ep_num_to_dci(msc_info.ep_in, true);

    if !xhc.configure_endpoint(slot_id, dci_out, 2 /* Bulk OUT */,
        msc_info.mps_out, 3, 65535) {
        if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC bulk OUT config failed\0"); }
        return None;
    }
    if !xhc.configure_endpoint(slot_id, dci_in, 6 /* Bulk IN */,
        msc_info.mps_in, 3, 65535) {
        if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC bulk IN config failed\0"); }
        return None;
    }

    // Register device
    xhc.msc_devices[dev_idx].slot_id = slot_id;
    xhc.msc_devices[dev_idx].ep_out = msc_info.ep_out;
    xhc.msc_devices[dev_idx].ep_in = msc_info.ep_in;
    xhc.msc_devices[dev_idx].ready = false;

    // Allocate DMA buffers
    if !xhc.msc_devices[dev_idx].alloc_buffers() {
        return None;
    }

    // Step 5 & 6: Initialize (TEST UNIT READY + READ CAPACITY)
    // Try TEST UNIT READY a few times (device might need time to spin up)
    let mut tur_ok = false;
    for _ in 0..10 {
        if scsi_test_unit_ready(xhc, dev_idx) {
            tur_ok = true;
            break;
        }
        ffi::udelay(200_000); // 200ms between attempts
    }
    if !tur_ok {
        if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC not ready\0"); }
    }

    // Read capacity
    let mut cap_buf = match RingMem::alloc(64) {
        Some(b) => b,
        None => return None,
    };

    let capacity = scsi_read_capacity10(xhc, dev_idx, cap_buf.phys, cap_buf.virt);
    let (block_size, block_count) = match capacity {
        Some(cap) => {
            if cap.block_size < 512 {
                (512u32, cap.total_blocks() * (cap.block_size as u64) / 512)
            } else {
                (cap.block_size, cap.total_blocks())
            }
        }
        None => {
            // Default: 512-byte blocks, unknown count
            cap_buf.free();
            xhc.msc_devices[dev_idx].ready = true;
            xhc.msc_devices[dev_idx].block_size = 512;
            xhc.msc_devices[dev_idx].block_count = 0;
            if xhc.verbose >= 1 { ffi::print(b"xHCI: MSC no capacity data\0"); }
            return Some(dev_idx);
        }
    };
    cap_buf.free();

    xhc.msc_devices[dev_idx].block_size = block_size;
    xhc.msc_devices[dev_idx].block_count = block_count;
    xhc.msc_devices[dev_idx].ready = true;

    if xhc.verbose >= 1 {
        ffi::print(b"xHCI: USB Mass Storage ready\0");
    }
    Some(dev_idx)
}

/// Read blocks from a USB Mass Storage device.
/// `lba` — logical block address (in 512-byte sectors).
/// `count` — number of 512-byte sectors to read.
/// `data_phys` — physical address of the DMA buffer.
/// Returns number of bytes read, or -1 on error.
pub fn msc_read(xhc: &mut XhciController, dev_idx: usize,
    lba: u64, count: usize, data_phys: u64
) -> isize {
    if dev_idx >= MAX_MSC_DEVICES || !xhc.msc_devices[dev_idx].ready {
        return -1;
    }
    let dev = &xhc.msc_devices[dev_idx];
    let block_size = dev.block_size;

    // Convert from 512-byte sectors to native block size
    let native_lba = if block_size >= 512 {
        (lba * 512 / block_size as u64) as u32
    } else {
        lba as u32 * (512 / block_size) as u32
    };

    let native_blocks = if block_size >= 512 {
        (count * 512 / block_size as usize) as u16
    } else {
        (count * 512 / block_size as usize) as u16
    };

    let data_len = (native_blocks as u32) * block_size;

    if !scsi_read10(xhc, dev_idx, native_lba, native_blocks, data_phys, data_len) {
        return -1;
    }
    data_len as isize
}

/// Write blocks to a USB Mass Storage device.
/// `lba` — logical block address (in 512-byte sectors).
/// `count` — number of 512-byte sectors to write.
/// `data_phys` — physical address of the DMA buffer.
/// Returns number of bytes written, or -1 on error.
pub fn msc_write(xhc: &mut XhciController, dev_idx: usize,
    lba: u64, count: usize, data_phys: u64
) -> isize {
    if dev_idx >= MAX_MSC_DEVICES || !xhc.msc_devices[dev_idx].ready {
        return -1;
    }
    let dev = &xhc.msc_devices[dev_idx];
    let block_size = dev.block_size;

    let native_lba = if block_size >= 512 {
        (lba * 512 / block_size as u64) as u32
    } else {
        lba as u32 * (512 / block_size) as u32
    };

    let native_blocks = if block_size >= 512 {
        (count * 512 / block_size as usize) as u16
    } else {
        (count * 512 / block_size as usize) as u16
    };

    let data_len = (native_blocks as u32) * block_size;

    if !scsi_write10(xhc, dev_idx, native_lba, native_blocks, data_phys, data_len) {
        return -1;
    }
    data_len as isize
}
