//! # MMIO — Memory-Mapped I/O register access
//!
//! Safe wrappers for MINIX MMIO macros (`read32`, `write32`, `set32` from
//! `minix/include/minix/mmio.h`).
//!
//! The `MmioRegion` type represents a mapped MMIO region and provides
//! bounds-checked register access at arbitrary offsets.

use crate::DriverError;

/// A volatile memory cell for MMIO register access.
///
/// Wraps a single hardware register with volatile reads/writes.
/// This prevents the compiler from optimizing away access to
/// memory-mapped hardware registers.
#[derive(Debug)]
#[repr(transparent)]
pub struct VolatileCell<T: Copy>(core::cell::UnsafeCell<T>);

impl<T: Copy> VolatileCell<T> {
    /// Create a new VolatileCell pointing to the given pointer.
    ///
    /// # Safety
    /// `ptr` must point to a valid, aligned, MMIO-mapped hardware register.
    pub unsafe fn from_ptr(ptr: *mut T) -> &'static Self {
        unsafe { &*(ptr as *const Self) }
    }

    /// Perform a volatile read of the register.
    #[inline]
    pub fn read(&self) -> T {
        unsafe { core::ptr::read_volatile(self.0.get()) }
    }

    /// Perform a volatile write to the register.
    #[inline]
    pub fn write(&self, val: T) {
        unsafe { core::ptr::write_volatile(self.0.get(), val) }
    }

    /// Atomically set bits using a mask (read-modify-write).
    #[inline]
    pub fn set_bits(&self, mask: T, val: T)
    where
        T: core::ops::BitAnd<Output = T>
            + core::ops::BitOr<Output = T>
            + core::ops::Not<Output = T>,
    {
        let old = self.read();
        self.write((old & !mask) | (val & mask));
    }
}

/// A safe MMIO region with bounds-checked register access.
///
/// Represents a memory-mapped I/O region of a given size. All register
/// accesses are bounds-checked against the region size.
#[derive(Debug, Clone)]
pub struct MmioRegion {
    /// Base pointer of the MMIO region.
    base: *mut u8,
    /// Size of the region in bytes.
    size: usize,
}

// MMIO regions are safe to send between threads (hardware registers
// are inherently shared state).
unsafe impl Send for MmioRegion {}
unsafe impl Sync for MmioRegion {}

// Manual PartialEq for pointer comparison
impl PartialEq for MmioRegion {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.size == other.size
    }
}

impl MmioRegion {
    /// Create a new MMIO region from a base pointer and size.
    ///
    /// Returns `Err(DriverError::NullPointer)` if `base` is null.
    /// Returns `Err(DriverError::Misaligned)` if `base` is not aligned
    /// to the required alignment (4 bytes for 32-bit access).
    pub fn new(base: *mut u8, size: usize) -> Result<Self, DriverError> {
        if base.is_null() {
            return Err(DriverError::NullPointer);
        }
        if (base as usize) % 4 != 0 {
            return Err(DriverError::Misaligned);
        }
        Ok(Self { base, size })
    }

    /// Create a new MMIO region without alignment checks.
    ///
    /// Useful for devices with non-standard alignment requirements.
    pub fn new_unaligned(base: *mut u8, size: usize) -> Result<Self, DriverError> {
        if base.is_null() {
            return Err(DriverError::NullPointer);
        }
        Ok(Self { base, size })
    }

    /// Returns the base address of the MMIO region.
    pub fn base(&self) -> *mut u8 {
        self.base
    }

    /// Returns the size of the MMIO region in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Read a 32-bit register at the given byte offset.
    ///
    /// Returns `Err(DriverError::OutOfBounds)` if `offset + 4 > size`.
    pub fn read32(&self, offset: usize) -> Result<u32, DriverError> {
        if offset + 4 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) as *mut u32 };
        let cell = unsafe { VolatileCell::<u32>::from_ptr(ptr) };
        Ok(cell.read())
    }

    /// Write a 32-bit value to the register at the given byte offset.
    pub fn write32(&self, offset: usize, val: u32) -> Result<(), DriverError> {
        if offset + 4 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) as *mut u32 };
        let cell = unsafe { VolatileCell::<u32>::from_ptr(ptr) };
        cell.write(val);
        Ok(())
    }

    /// Set/clear 32-bit register bits using a mask (read-modify-write).
    pub fn set32(&self, offset: usize, mask: u32, val: u32) -> Result<(), DriverError> {
        if offset + 4 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) as *mut u32 };
        let cell = unsafe { VolatileCell::<u32>::from_ptr(ptr) };
        cell.set_bits(mask, val);
        Ok(())
    }

    /// Read a 16-bit register at the given byte offset.
    pub fn read16(&self, offset: usize) -> Result<u16, DriverError> {
        if offset + 2 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) as *mut u16 };
        let cell = unsafe { VolatileCell::<u16>::from_ptr(ptr) };
        Ok(cell.read())
    }

    /// Write a 16-bit value to the register at the given byte offset.
    pub fn write16(&self, offset: usize, val: u16) -> Result<(), DriverError> {
        if offset + 2 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) as *mut u16 };
        let cell = unsafe { VolatileCell::<u16>::from_ptr(ptr) };
        cell.write(val);
        Ok(())
    }

    /// Set/clear 16-bit register bits using a mask (read-modify-write).
    pub fn set16(&self, offset: usize, mask: u16, val: u16) -> Result<(), DriverError> {
        if offset + 2 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) as *mut u16 };
        let cell = unsafe { VolatileCell::<u16>::from_ptr(ptr) };
        cell.set_bits(mask, val);
        Ok(())
    }

    /// Read a 8-bit register at the given byte offset.
    pub fn read8(&self, offset: usize) -> Result<u8, DriverError> {
        if offset + 1 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) };
        let cell = unsafe { VolatileCell::<u8>::from_ptr(ptr) };
        Ok(cell.read())
    }

    /// Write an 8-bit value to the register at the given byte offset.
    pub fn write8(&self, offset: usize, val: u8) -> Result<(), DriverError> {
        if offset + 1 > self.size {
            return Err(DriverError::OutOfBounds);
        }
        let ptr = unsafe { self.base.add(offset) };
        let cell = unsafe { VolatileCell::<u8>::from_ptr(ptr) };
        cell.write(val);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free-standing helpers matching the C mmio.h API
// ---------------------------------------------------------------------------

/// Perform a volatile 32-bit read from a raw address (like `read32()` in C).
///
/// # Safety
/// `addr` must point to a valid, MMIO-mapped 32-bit register.
#[inline]
pub unsafe fn read32_raw(addr: usize) -> u32 {
    // SAFETY: caller guarantees addr points to a valid MMIO register
    let cell = unsafe { VolatileCell::<u32>::from_ptr(addr as *mut u32) };
    cell.read()
}

/// Perform a volatile 32-bit write to a raw address (like `write32()` in C).
///
/// # Safety
/// `addr` must point to a valid, MMIO-mapped 32-bit register.
#[inline]
pub unsafe fn write32_raw(addr: usize, val: u32) {
    // SAFETY: caller guarantees addr points to a valid MMIO register
    let cell = unsafe { VolatileCell::<u32>::from_ptr(addr as *mut u32) };
    cell.write(val);
}

/// Perform a volatile 32-bit set/clear at a raw address (like `set32()` in C).
///
/// # Safety
/// `addr` must point to a valid, MMIO-mapped 32-bit register.
#[inline]
pub unsafe fn set32_raw(addr: usize, mask: u32, val: u32) {
    // SAFETY: caller guarantees addr points to a valid MMIO register
    let cell = unsafe { VolatileCell::<u32>::from_ptr(addr as *mut u32) };
    cell.set_bits(mask, val);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_pointer_rejected() {
        assert_eq!(
            MmioRegion::new(core::ptr::null_mut(), 1024),
            Err(DriverError::NullPointer)
        );
    }

    #[test]
    fn bounds_checking() {
        let mut mem = [0u8; 16];
        let region = MmioRegion::new(mem.as_mut_ptr(), 16).unwrap();
        // Valid access at offset 12 (12 + 4 = 16, within bounds)
        assert!(region.read32(12).is_ok());
        // Invalid access at offset 13 (13 + 4 = 17 > 16)
        assert_eq!(region.read32(13), Err(DriverError::OutOfBounds));
        assert_eq!(region.read16(15), Err(DriverError::OutOfBounds));
        assert_eq!(region.read8(16), Err(DriverError::OutOfBounds));
    }

    #[test]
    fn write_and_read_back() {
        let mut mem = [0u8; 64];
        let region = MmioRegion::new(mem.as_mut_ptr(), 64).unwrap();
        region.write32(0, 0xDEAD_BEEF).unwrap();
        assert_eq!(region.read32(0).unwrap(), 0xDEAD_BEEF);
    }

    #[test]
    fn set32_mask() {
        let mut mem = [0u8; 64];
        let region = MmioRegion::new(mem.as_mut_ptr(), 64).unwrap();
        region.write32(0, 0x0000_FFFF).unwrap();
        // Set bit 16 using mask
        region.set32(0, 0x0001_0000, 0x0001_0000).unwrap();
        assert_eq!(region.read32(0).unwrap(), 0x0001_FFFF);
        // Clear bits 0-3 using mask
        region.set32(0, 0x0000_000F, 0x0000_0000).unwrap();
        assert_eq!(region.read32(0).unwrap(), 0x0001_FFF0);
    }

    #[test]
    fn read_write_16() {
        let mut mem = [0u8; 64];
        let region = MmioRegion::new(mem.as_mut_ptr(), 64).unwrap();
        region.write16(0, 0xABCD).unwrap();
        assert_eq!(region.read16(0).unwrap(), 0xABCD);
        region.set16(0, 0x00F0, 0x0000).unwrap();
        assert_eq!(region.read16(0).unwrap(), 0xAB0D);
    }

    #[test]
    fn read_write_8() {
        let mut mem = [0u8; 64];
        let region = MmioRegion::new(mem.as_mut_ptr(), 64).unwrap();
        region.write8(0, 0x42).unwrap();
        assert_eq!(region.read8(0).unwrap(), 0x42);
        region.write8(1, 0xFF).unwrap();
        assert_eq!(region.read8(0).unwrap(), 0x42);
        assert_eq!(region.read8(1).unwrap(), 0xFF);
    }

    #[test]
    fn volatile_cell_raw() {
        let mut val: u32 = 0;
        unsafe {
            let cell = VolatileCell::<u32>::from_ptr(&mut val as *mut u32);
            cell.write(42);
            assert_eq!(cell.read(), 42);
        }
        assert_eq!(val, 42);
    }

    #[test]
    fn set16_mask() {
        let mut mem = [0u8; 64];
        let region = MmioRegion::new(mem.as_mut_ptr(), 64).unwrap();
        region.write16(0, 0xFF00).unwrap();
        region.set16(0, 0x0F00, 0x0500).unwrap();
        assert_eq!(region.read16(0).unwrap(), 0xF500);
    }
}
