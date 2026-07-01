//! # E1000 EEPROM Reading
//!
//! Supports two EEPROM access methods:
//! - EERD register (standard for 8254x/8257x/8258x)
//! - ICH8 flash memory (for ICH8-ICH10 integrated controllers)

use crate::ffi;
use crate::reg;
use crate::pci_ids::EepromType;

use core::ffi::c_uint;

// ============================================================================
// EERD register-based EEPROM read
// ============================================================================

/// Read a 16-bit word from EEPROM using the EERD register.
///
/// `regs` — pointer to MMIO registers
/// `reg` — EEPROM word address (0, 1, 2 for MAC address)
/// `done_bit` — bit to check for completion (device-specific)
/// `addr_off` — address field bit offset (device-specific)
fn eeprom_read_eerd(regs: *mut u8, reg: c_uint, done_bit: u32, addr_off: u32) -> u16 {
    // Request EEPROM read
    let cmd = (reg << addr_off) | reg::EERD_START;
    write_reg(regs, reg::EERD, cmd);

    // Wait until ready
    loop {
        let data = read_reg(regs, reg::EERD);
        if (data & done_bit) != 0 {
            return (data >> 16) as u16;
        }
    }
}

// ============================================================================
// ICH8 flash-based EEPROM read
// ============================================================================

/// ICH8 flash status register
#[repr(C)]
#[derive(Clone, Copy)]
struct Hsfsts(u16);

impl Hsfsts {
    fn flcdone(&self) -> bool { (self.0 & 0x0001) != 0 }
    fn flcerr(&self) -> bool { (self.0 & 0x0002) != 0 }
    fn dael(&self) -> bool { (self.0 & 0x0004) != 0 }
    fn flcinprog(&self) -> bool { (self.0 & 0x0020) != 0 }
    fn fldesvalid(&self) -> bool { (self.0 & 0x4000) != 0 }

    fn set_flcdone(&mut self) { self.0 |= 0x0001; }
    fn set_flcerr(&mut self) { self.0 |= 0x0002; }
    fn set_dael(&mut self) { self.0 |= 0x0004; }
}

/// ICH8 flash control register
#[repr(C)]
#[derive(Clone, Copy)]
struct Hsfctl(u16);

impl Hsfctl {
    fn set_flcgo(&mut self) { self.0 |= 0x0001; }
    fn set_flcycle(&mut self, cycle: u16) { self.0 = (self.0 & !0x0006) | ((cycle & 3) << 1); }
    fn set_fldbcount(&mut self, count: u16) { self.0 = (self.0 & !0x0300) | ((count & 3) << 8); }
}

/// Initialize ICH8 flash controller for a read cycle.
fn ich_init(flash: *mut u8) -> bool {
    let hsfsts_addr = flash.wrapping_add(reg::ICH_FLASH_HSFSTS as usize);
    let mut hsfsts = Hsfsts(unsafe { core::ptr::read_volatile(hsfsts_addr as *const u16) });

    // Check if flash descriptor is valid
    if !hsfsts.fldesvalid() {
        return false;
    }

    // Clear FCERR and DAEL by writing 1
    hsfsts.set_flcerr();
    hsfsts.set_dael();
    unsafe { core::ptr::write_volatile(hsfsts_addr as *mut u16, hsfsts.0); }

    // Wait if a cycle is in progress
    if hsfsts.flcinprog() {
        for _ in 0..reg::ICH_FLASH_READ_COMMAND_TIMEOUT {
            hsfsts = Hsfsts(unsafe { core::ptr::read_volatile(hsfsts_addr as *const u16) });
            if !hsfsts.flcinprog() {
                break;
            }
            ffi::udelay(16_000);
        }
        if hsfsts.flcinprog() {
            return false;
        }
    }

    // Set Flash Cycle Done
    hsfsts.set_flcdone();
    unsafe { core::ptr::write_volatile(hsfsts_addr as *mut u16, hsfsts.0); }
    true
}

/// Execute a single ICH8 flash read cycle.
fn ich_cycle(flash: *mut u8, timeout: u32) -> bool {
    // Start cycle by setting FLCGO
    let hsfctl_addr = flash.wrapping_add(reg::ICH_FLASH_HSFCTL as usize);
    let mut hsflctl = Hsfctl(unsafe { core::ptr::read_volatile(hsfctl_addr as *const u16) });
    hsflctl.set_flcgo();
    unsafe { core::ptr::write_volatile(hsfctl_addr as *mut u16, hsflctl.0); }

    // Wait for FDONE
    let hsfsts_addr = flash.wrapping_add(reg::ICH_FLASH_HSFSTS as usize);
    for _ in 0..timeout {
        let hsfsts = Hsfsts(unsafe { core::ptr::read_volatile(hsfsts_addr as *const u16) });
        if hsfsts.flcdone() {
            return !hsfsts.flcerr();
        }
        ffi::udelay(16_000);
    }
    false
}

/// Read a 16-bit word from ICH8 flash.
fn eeprom_read_ich(flash: *mut u8, flash_base: u32, reg: c_uint) -> u16 {
    let flash_linear = (reg::ICH_FLASH_LINEAR_ADDR_MASK & (reg * 2)) + flash_base;

    for _ in 0..reg::ICH_FLASH_CYCLE_REPEAT_COUNT {
        ffi::udelay(16_000);

        if !ich_init(flash) { continue; }

        // Configure cycle: read, 2 bytes
        let hsfctl_addr = flash.wrapping_add(reg::ICH_FLASH_HSFCTL as usize);
        let mut hsflctl = Hsfctl(unsafe { core::ptr::read_volatile(hsfctl_addr as *const u16) });
        hsflctl.set_fldbcount(1);   // 2 bytes
        hsflctl.set_flcycle(reg::ICH_CYCLE_READ);
        unsafe { core::ptr::write_volatile(hsfctl_addr as *mut u16, hsflctl.0); }

        // Set flash address
        let faddr_addr = flash.wrapping_add(reg::ICH_FLASH_FADDR as usize);
        unsafe { core::ptr::write_volatile(faddr_addr as *mut u32, flash_linear); }

        if !ich_cycle(flash, reg::ICH_FLASH_READ_COMMAND_TIMEOUT) {
            // Check for FCERR
            let hsfsts_addr = flash.wrapping_add(reg::ICH_FLASH_HSFSTS as usize);
            let hsfsts = Hsfsts(unsafe { core::ptr::read_volatile(hsfsts_addr as *const u16) });
            if !hsfsts.flcerr() {
                break; // timeout, not recoverable
            }
            continue; // retry
        }

        // Read data
        let fdata_addr = flash.wrapping_add(reg::ICH_FLASH_FDATA0 as usize);
        return unsafe { core::ptr::read_volatile(fdata_addr as *const u32) as u16 };
    }

    0
}

// ============================================================================
// Public API
// ============================================================================

/// Read a 16-bit word from EEPROM/flash.
///
/// * `eeprom_type` — EERD or ICH8 flash
/// * `regs` — MMIO register base
/// * `flash` — optional flash mapping (may be null)
/// * `flash_base` — flash sector-aligned base address
/// * `reg` — EEPROM word address (0, 1, 2 for MAC)
/// * `done_bit` — EERD completion bit mask
/// * `addr_off` — EERD address field shift
pub fn eeprom_read(
    eeprom_type: EepromType,
    regs: *mut u8,
    flash: *mut u8,
    flash_base: u32,
    reg: c_uint,
    done_bit: u32,
    addr_off: u32,
) -> u16 {
    match eeprom_type {
        EepromType::Eerd => eeprom_read_eerd(regs, reg, done_bit, addr_off),
        EepromType::Ich8 => eeprom_read_ich(flash, flash_base, reg),
    }
}

// ============================================================================
// MMIO register helpers
// ============================================================================

/// Read a 32-bit MMIO register.
#[inline]
pub fn read_reg(regs: *mut u8, offset: u32) -> u32 {
    unsafe { core::ptr::read_volatile(regs.wrapping_add(offset as usize) as *const u32) }
}

/// Write a 32-bit MMIO register.
#[inline]
pub fn write_reg(regs: *mut u8, offset: u32, value: u32) {
    unsafe { core::ptr::write_volatile(regs.wrapping_add(offset as usize) as *mut u32, value) }
}

/// Set bits in a register (read-modify-write).
#[inline]
pub fn set_reg(regs: *mut u8, offset: u32, bits: u32) {
    let val = read_reg(regs, offset);
    write_reg(regs, offset, val | bits);
}

/// Clear bits in a register (read-modify-write).
#[inline]
pub fn clear_reg(regs: *mut u8, offset: u32, bits: u32) {
    let val = read_reg(regs, offset);
    write_reg(regs, offset, val & !bits);
}
