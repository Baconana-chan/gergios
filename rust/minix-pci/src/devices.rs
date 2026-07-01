//! # PCI Device Table — Bus Scanning and Device Management
//!
//! Implements PCI bus scanning via legacy I/O ports (config mechanism #1),
//! maintains a device table with vendor/device IDs, BARs, and IRQ info.
//!
//! Supports:
//!   - Bus 0 scanning (host bridge + all devices)
//!   - PCI-to-PCI bridge secondary bus scanning
//!   - BAR probing (I/O and memory, 32/64-bit)
//!   - IRQ recording (PCI_INTERRUPT_LINE + ACPI)
//!   - Capability list walking

use crate::ffi;
use core::ffi::c_int;

// ============================================================================
// PCI Config Space Register Offsets
// ============================================================================

pub const PCI_VENDOR_ID: u8 = 0x00;
pub const PCI_DEVICE_ID: u8 = 0x02;
pub const PCI_COMMAND: u8 = 0x04;
pub const PCI_STATUS: u8 = 0x06;
pub const PCI_REVISION: u8 = 0x08;
pub const PCI_CLASS_CODE: u8 = 0x08;   // 3 bytes: base, sub, interface
pub const PCI_HEADER_TYPE: u8 = 0x0E;
pub const PCI_BAR_0: u8 = 0x10;
pub const PCI_SUBSYSTEM_VID: u8 = 0x2C;
pub const PCI_SUBSYSTEM_ID: u8 = 0x2E;
pub const PCI_CAPABILITIES: u8 = 0x34;
pub const PCI_INTERRUPT_LINE: u8 = 0x3C;
pub const PCI_INTERRUPT_PIN: u8 = 0x3D;

// PCI BAR types
pub const PCI_BAR_IO: u32 = 0x0000_0001;
pub const PCI_BAR_TYPE: u32 = 0x0000_0006;
pub const PCI_BAR_TYPE_32: u32 = 0x0000_0000;
pub const PCI_BAR_TYPE_64: u32 = 0x0000_0004;
pub const PCI_BAR_PREFETCH: u32 = 0x0000_0008;

// PCI header types
pub const PHT_NORMAL: u8 = 0x00;
pub const PHT_BRIDGE: u8 = 0x01;
pub const PHT_CARDBUS: u8 = 0x02;
pub const PHT_MULTIFUNC: u8 = 0x80;

// PCI class codes
pub const PCI_CLASS_STORAGE: u32 = 0x01;
pub const PCI_CLASS_NETWORK: u32 = 0x02;
pub const PCI_CLASS_BRIDGE: u32 = 0x06;

// Interface values
pub const PCI_T3_ISA: u32 = 0x060100;
pub const PCI_T3_PCI2PCI: u32 = 0x060400;
pub const PCI_T3_CARDBUS: u32 = 0x060700;
pub const PCI_T3_VGA: u32 = 0x030000;

// PCI bridge registers
pub const PPB_PRIMBN: u8 = 0x18;
pub const PPB_SECBN: u8 = 0x19;
pub const PPB_SUBORDBN: u8 = 0x1A;

// Max devices
pub const MAX_DEVICES: usize = 128;   // enough for typical systems

// Vendor ID indicating no device
const NO_VID: u16 = 0xFFFF;
const NO_DID: u16 = 0xFFFF;

// PCI standard config registers for the PCI-to-PCI bridge
const PPB_IOBASE: u8 = 0x1C;
const PPB_IOLIMIT: u8 = 0x1D;
const PPB_MEMBASE: u8 = 0x20;
const PPB_MEMLIMIT: u8 = 0x22;
const PPB_PFMEMBASE: u8 = 0x24;
const PPB_PFMEMLIMIT: u8 = 0x26;
const PPB_IOBASEU16: u8 = 0x30;
const PPB_IOLIMITU16: u8 = 0x32;
const PPB_SSTS: u8 = 0x1E;
const PPB_BRIDGECTRL: u8 = 0x3E;

// CardBus bridge registers
const CBB_MEMBASE_0: u8 = 0x1C;
const CBB_MEMLIMIT_0: u8 = 0x20;
const CBB_MEMBASE_1: u8 = 0x24;
const CBB_MEMLIMIT_1: u8 = 0x28;
const CBB_IOBASE_0: u8 = 0x2C;
const CBB_IOLIMIT_0: u8 = 0x30;
const CBB_SSTS: u8 = 0x16;

// ============================================================================
// PCI Device Descriptor
// ============================================================================

/// A BAR (Base Address Register) description.
#[derive(Debug, Clone, Copy)]
pub struct PciBar {
    pub base: u32,
    pub size: u32,
    pub is_io: bool,
    pub is_64bit: bool,
    pub prefetchable: bool,
}

/// A discovered PCI device.
#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub sub_vendor_id: u16,
    pub sub_device_id: u16,
    pub base_class: u8,
    pub sub_class: u8,
    pub interface: u8,
    pub header_type: u8,
    pub irq_line: u8,
    pub irq_pin: u8,
    pub bars: [Option<PciBar>; 6],
    pub bar_count: usize,
    pub in_use: bool,
    pub proc: c_int,  // MINIX endpoint that reserved this device
}

impl PciDevice {
    pub fn class_code(&self) -> u32 {
        (self.base_class as u32) << 16
            | (self.sub_class as u32) << 8
            | self.interface as u32
    }
}

// ============================================================================
// PCI Device Table
// ============================================================================

/// Holds all discovered PCI devices.
#[derive(Debug)]
pub struct PciDeviceTable {
    devices: [Option<PciDevice>; MAX_DEVICES],
    count: usize,
}

impl PciDeviceTable {
    pub fn new() -> Self {
        const INIT: Option<PciDevice> = None;
        PciDeviceTable {
            devices: [INIT; MAX_DEVICES],
            count: 0,
        }
    }

    pub fn count(&self) -> usize { self.count }

    /// Probe PCI bus 0 and all secondary buses (via bridges).
    pub fn probe_all(&mut self) -> usize {
        // Probe bus 0, devices 0-31, functions 0-7
        self.probe_bus(0);
        self.count
    }

    /// Probe a single bus for devices.
    fn probe_bus(&mut self, bus: u8) {
        let mut found_multifunc = [false; 32];

        for dev in 0..32 {
            let mut max_func = 1;

            for func in 0..max_func {
                // Read vendor/device ID
                let vid = self.read_config16(bus, dev, func, PCI_VENDOR_ID);
                let did = self.read_config16(bus, dev, func, PCI_DEVICE_ID);

                if vid == NO_VID && did == NO_DID {
                    if func == 0 { break; } // Nothing here
                    continue;
                }

                // If this is function 0 and it's multifunction, scan all 8 functions
                if func == 0 {
                    let header = self.read_config8(bus, dev, func, PCI_HEADER_TYPE);
                    if header & PHT_MULTIFUNC != 0 {
                        max_func = 8;
                        found_multifunc[dev as usize] = true;
                    }
                } else if !found_multifunc[dev as usize] {
                    // If a non-zero function exists but function 0 wasn't multifunction,
                    // still scan it (some devices lie)
                }

                // Check for duplicates
                if self.is_duplicate(bus, dev, func) {
                    continue;
                }

                // Read full config
                let header_type = self.read_config8(bus, dev, func, PCI_HEADER_TYPE) & 0x7F;
                let class_full = self.read_config32(bus, dev, func, PCI_CLASS_CODE);
                let base_class = ((class_full >> 24) & 0xFF) as u8;
                let sub_class = ((class_full >> 16) & 0xFF) as u8;
                let interface = ((class_full >> 8) & 0xFF) as u8;

                let sub_vid = self.read_config16(bus, dev, func, PCI_SUBSYSTEM_VID);
                let sub_did = self.read_config16(bus, dev, func, PCI_SUBSYSTEM_ID);

                let irq_line = self.read_config8(bus, dev, func, PCI_INTERRUPT_LINE);
                let irq_pin = self.read_config8(bus, dev, func, PCI_INTERRUPT_PIN);

                // Read BARs according to header type
                let mut bars = [None; 6];
                let bar_count = match header_type {
                    PHT_NORMAL => self.read_bars_normal(bus, dev, func, &mut bars),
                    PHT_BRIDGE => self.read_bars_bridge(bus, dev, func, &mut bars),
                    PHT_CARDBUS => self.read_bars_cardbus(bus, dev, func, &mut bars),
                    _ => 0,
                };

                let device = PciDevice {
                    bus, dev, func,
                    vendor_id: vid,
                    device_id: did,
                    sub_vendor_id: sub_vid,
                    sub_device_id: sub_did,
                    base_class, sub_class, interface,
                    header_type,
                    irq_line, irq_pin,
                    bars, bar_count,
                    in_use: false,
                    proc: 0,
                };

                self.add_device(device);

                // If this is a PCI-to-PCI bridge, probe the secondary bus
                let class_code = (base_class as u32) << 16
                    | (sub_class as u32) << 8 | interface as u32;
                if (header_type == PHT_BRIDGE || header_type == PHT_CARDBUS)
                    && (class_code == PCI_T3_PCI2PCI
                        || class_code == PCI_T3_CARDBUS)
                {
                    let sec_bus = self.read_config8(bus, dev, func, PPB_SECBN);
                    if sec_bus != 0 {
                        self.probe_bus(sec_bus);
                    }
                }
            }
        }
    }

    // ========================================================================
    // BAR Probing
    // ========================================================================

    /// Read standard BARs for normal (non-bridge) devices (6 BARs).
    fn read_bars_normal(&self, bus: u8, dev: u8, func: u8,
        bars: &mut [Option<PciBar>; 6]) -> usize {
        self.read_bars_range(bus, dev, func, PCI_BAR_0, 6, bars)
    }

    /// Read BARs for PCI-to-PCI bridges (2 BARs).
    fn read_bars_bridge(&self, bus: u8, dev: u8, func: u8,
        bars: &mut [Option<PciBar>; 6]) -> usize {
        self.read_bars_range(bus, dev, func, PCI_BAR_0, 2, bars)
    }

    /// Read BARs for CardBus bridges (1 BAR).
    fn read_bars_cardbus(&self, bus: u8, dev: u8, func: u8,
        bars: &mut [Option<PciBar>; 6]) -> usize {
        self.read_bars_range(bus, dev, func, PCI_BAR_0, 1, bars)
    }

    /// Read a range of BARs by probing their size.
    fn read_bars_range(&self, bus: u8, dev: u8, func: u8,
        start_offset: u8, count: usize, bars: &mut [Option<PciBar>; 6]) -> usize {
        let mut bar_count = 0;
        let mut bar_idx = 0;

        for i in 0..count {
            let offset = start_offset + (i as u8) * 4;
            let raw = self.read_config32(bus, dev, func, offset);

            if raw == 0 { continue; }

            if raw & PCI_BAR_IO != 0 {
                // I/O BAR — size probing via 0xFFFFFFFF write
                self.write_config32(bus, dev, func, offset, 0xFFFF_FFFF);
                let probe = self.read_config32(bus, dev, func, offset);
                self.write_config32(bus, dev, func, offset, raw);

                let base = raw & 0xFFFF_FFFC;  // I/O BAR mask
                let size = (!(probe & 0xFFFF_FFFC) + 1) & 0xFFFF;

                if bar_idx < 6 {
                    bars[bar_idx] = Some(PciBar {
                        base, size, is_io: true,
                        is_64bit: false, prefetchable: false,
                    });
                    bar_count += 1;
                }
                bar_idx += 1;
            } else {
                // Memory BAR
                let type_ = raw & PCI_BAR_TYPE;
                let prefetchable = raw & PCI_BAR_PREFETCH != 0;

                let is_64bit = type_ == PCI_BAR_TYPE_64;
                if is_64bit && i + 1 >= count {
                    // 64-bit BAR extends beyond range, skip
                    break;
                }

                // Save original, probe size
                self.write_config32(bus, dev, func, offset, 0xFFFF_FFFF);
                let probe = self.read_config32(bus, dev, func, offset);
                self.write_config32(bus, dev, func, offset, raw);

                let base = raw & 0xFFFF_FFF0;  // Memory BAR mask
                let size = !(probe & 0xFFFF_FFF0) + 1;

                if bar_idx < 6 {
                    bars[bar_idx] = Some(PciBar {
                        base, size, is_io: false,
                        is_64bit, prefetchable,
                    });
                    bar_count += 1;
                }
                bar_idx += 1;

                if is_64bit {
                    bar_idx += 1; // Skip the next DWORD (upper 32 bits)
                }
            }
        }
        bar_count
    }

    // ========================================================================
    // Config space access helpers via I/O ports
    // ========================================================================

    fn read_config8(&self, bus: u8, dev: u8, func: u8, offset: u8) -> u8 {
        unsafe { ffi::pci_read_config8(bus, dev, func, offset) }
    }

    fn read_config16(&self, bus: u8, dev: u8, func: u8, offset: u8) -> u16 {
        unsafe { ffi::pci_read_config16(bus, dev, func, offset) }
    }

    fn read_config32(&self, bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
        unsafe { ffi::pci_read_config32(bus, dev, func, offset) }
    }

    #[allow(dead_code)]
    fn write_config8(&self, bus: u8, dev: u8, func: u8, offset: u8, value: u8) {
        unsafe { ffi::pci_write_config8(bus, dev, func, offset, value) }
    }

    #[allow(dead_code)]
    fn write_config16(&self, bus: u8, dev: u8, func: u8, offset: u8, value: u16) {
        unsafe { ffi::pci_write_config16(bus, dev, func, offset, value) }
    }

    fn write_config32(&self, bus: u8, dev: u8, func: u8, offset: u8, value: u32) {
        unsafe { ffi::pci_write_config32(bus, dev, func, offset, value) }
    }

    // ========================================================================
    // Device table management
    // ========================================================================

    fn add_device(&mut self, dev: PciDevice) {
        if self.count >= MAX_DEVICES { return; }
        self.devices[self.count] = Some(dev);
        self.count += 1;
    }

    fn is_duplicate(&self, bus: u8, dev: u8, func: u8) -> bool {
        for i in 0..self.count {
            if let Some(ref d) = self.devices[i] {
                if d.bus == bus && d.dev == dev && d.func == func {
                    return true;
                }
            }
        }
        false
    }

    // ========================================================================
    // Public query API
    // ========================================================================

    /// Get reference to a device by index.
    pub fn get(&self, index: usize) -> Option<&PciDevice> {
        if index < self.count { self.devices[index].as_ref() } else { None }
    }

    /// Get mutable reference to a device by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut PciDevice> {
        if index < self.count { self.devices[index].as_mut() } else { None }
    }

    /// Find first device matching vendor/device ID.
    pub fn find_first(&self, vid: u16, did: u16) -> Option<(usize, &PciDevice)> {
        for i in 0..self.count {
            if let Some(ref d) = self.devices[i] {
                if d.vendor_id == vid && d.device_id == did {
                    return Some((i, d));
                }
            }
        }
        None
    }

    /// Return the first device (for IPC pci_first_dev).
    pub fn first_dev(&self) -> Option<(usize, u16, u16)> {
        for i in 0..self.count {
            if let Some(ref d) = self.devices[i] {
                return Some((i, d.vendor_id, d.device_id));
            }
        }
        None
    }

    /// Return the first device matching a visibility predicate (ACL-aware).
    pub fn first_dev_where<F>(&self, mut visible: F) -> Option<(usize, u16, u16)>
    where F: FnMut(usize, &PciDevice) -> bool {
        for i in 0..self.count {
            if let Some(ref d) = self.devices[i] {
                if visible(i, d) {
                    return Some((i, d.vendor_id, d.device_id));
                }
            }
        }
        None
    }

    /// Return the next device after `current` (for IPC pci_next_dev).
    pub fn next_dev(&self, current: usize) -> Option<(usize, u16, u16)> {
        for i in (current + 1)..self.count {
            if let Some(ref d) = self.devices[i] {
                return Some((i, d.vendor_id, d.device_id));
            }
        }
        None
    }

    /// Return the next device after `current` matching a visibility predicate.
    pub fn next_dev_where<F>(&self, current: usize, mut visible: F) -> Option<(usize, u16, u16)>
    where F: FnMut(usize, &PciDevice) -> bool {
        for i in (current + 1)..self.count {
            if let Some(ref d) = self.devices[i] {
                if visible(i, d) {
                    return Some((i, d.vendor_id, d.device_id));
                }
            }
        }
        None
    }

    /// Find device by BDF.
    pub fn find_by_bdf(&self, bus: u8, dev: u8, func: u8) -> Option<(usize, &PciDevice)> {
        for i in 0..self.count {
            if let Some(ref d) = self.devices[i] {
                if d.bus == bus && d.dev == dev && d.func == func {
                    return Some((i, d));
                }
            }
        }
        None
    }

    /// Reserve a device — tracks which endpoint reserved it.
    pub fn reserve(&mut self, index: usize, proc: c_int) -> Result<(), c_int> {
        if let Some(ref mut d) = self.devices[index] {
            if d.in_use && d.proc != proc {
                return Err(-16); // EBUSY
            }
            d.in_use = true;
            d.proc = proc;
            Ok(())
        } else {
            Err(-22) // EINVAL
        }
    }

    /// Release a device by index.
    pub fn release(&mut self, index: usize) {
        if let Some(ref mut d) = self.devices[index] {
            d.in_use = false;
            d.proc = 0;
        }
    }

    /// Release all devices held by a given endpoint.
    pub fn release_by_endpoint(&mut self, proc: c_int) {
        for i in 0..self.count {
            if let Some(ref mut d) = self.devices[i] {
                if d.in_use && d.proc == proc {
                    d.in_use = false;
                    d.proc = 0;
                }
            }
        }
    }

    #[cfg(test)]
    /// Add a test device (for unit tests only, skips I/O port access).
    pub fn add_for_test(&mut self, vid: u16, did: u16,
        bus: u8, dev: u8, class_code: u32, _devind: u32) {
        let dev = PciDevice {
            bus, dev, func: 0,
            vendor_id: vid, device_id: did,
            sub_vendor_id: 0, sub_device_id: 0,
            base_class: ((class_code >> 16) & 0xFF) as u8,
            sub_class: ((class_code >> 8) & 0xFF) as u8,
            interface: (class_code & 0xFF) as u8,
            header_type: PHT_NORMAL,
            irq_line: 0, irq_pin: 0,
            bars: [None; 6],
            bar_count: 0,
            in_use: false,
            proc: 0,
        };
        self.add_device(dev);
    }
}
