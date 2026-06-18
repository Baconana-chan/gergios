//! # Port I/O — x86 port-mapped I/O via MINIX syscalls
//!
//! Safe wrappers for the MINIX `sys_inb`/`sys_outb` (and word/long variants)
//! system calls. These provide type-safe port I/O with error handling.
//!
//! ## MINIX FFI
//!
//! On MINIX, port I/O goes through kernel syscalls:
//! - `sys_outb(port, val)` — write byte to port
//! - `sys_inb(port, &val)` — read byte from port
//! - `sys_outw`/`sys_inw` — word (16-bit) variants
//! - `sys_outl`/`sys_inl` — long (32-bit) variants
//!
//! On non-MINIX hosts, these are stubs that return `Err(IoError)`.

use crate::DriverError;

#[cfg(target_os = "minix")]
mod ffi {
    use crate::DriverError;

    extern "C" {
        fn sys_outb(port: u16, val: u8) -> i32;
        fn sys_inb(port: u16, val: *mut u8) -> i32;
        fn sys_outw(port: u16, val: u16) -> i32;
        fn sys_inw(port: u16, val: *mut u16) -> i32;
        fn sys_outl(port: u16, val: u32) -> i32;
        fn sys_inl(port: u16, val: *mut u32) -> i32;
    }

    pub fn outb(port: u16, val: u8) -> Result<(), DriverError> {
        let r = unsafe { sys_outb(port, val) };
        if r == 0 { Ok(()) } else { Err(DriverError::IoError) }
    }

    pub fn inb(port: u16) -> Result<u8, DriverError> {
        let mut val: u8 = 0;
        let r = unsafe { sys_inb(port, &mut val as *mut u8) };
        if r == 0 { Ok(val) } else { Err(DriverError::IoError) }
    }

    pub fn outw(port: u16, val: u16) -> Result<(), DriverError> {
        let r = unsafe { sys_outw(port, val) };
        if r == 0 { Ok(()) } else { Err(DriverError::IoError) }
    }

    pub fn inw(port: u16) -> Result<u16, DriverError> {
        let mut val: u16 = 0;
        let r = unsafe { sys_inw(port, &mut val as *mut u16) };
        if r == 0 { Ok(val) } else { Err(DriverError::IoError) }
    }

    pub fn outl(port: u16, val: u32) -> Result<(), DriverError> {
        let r = unsafe { sys_outl(port, val) };
        if r == 0 { Ok(()) } else { Err(DriverError::IoError) }
    }

    pub fn inl(port: u16) -> Result<u32, DriverError> {
        let mut val: u32 = 0;
        let r = unsafe { sys_inl(port, &mut val as *mut u32) };
        if r == 0 { Ok(val) } else { Err(DriverError::IoError) }
    }
}

#[cfg(not(target_os = "minix"))]
mod ffi {
    use crate::DriverError;

    // Stubs for host development
    pub fn outb(_port: u16, _val: u8) -> Result<(), DriverError> {
        Err(DriverError::IoError)
    }
    pub fn inb(_port: u16) -> Result<u8, DriverError> {
        Err(DriverError::IoError)
    }
    pub fn outw(_port: u16, _val: u16) -> Result<(), DriverError> {
        Err(DriverError::IoError)
    }
    pub fn inw(_port: u16) -> Result<u16, DriverError> {
        Err(DriverError::IoError)
    }
    pub fn outl(_port: u16, _val: u32) -> Result<(), DriverError> {
        Err(DriverError::IoError)
    }
    pub fn inl(_port: u16) -> Result<u32, DriverError> {
        Err(DriverError::IoError)
    }
}

/// Write a byte to an I/O port.
#[inline]
pub fn outb(port: u16, val: u8) -> Result<(), DriverError> {
    ffi::outb(port, val)
}

/// Read a byte from an I/O port.
#[inline]
pub fn inb(port: u16) -> Result<u8, DriverError> {
    ffi::inb(port)
}

/// Write a 16-bit word to an I/O port.
#[inline]
pub fn outw(port: u16, val: u16) -> Result<(), DriverError> {
    ffi::outw(port, val)
}

/// Read a 16-bit word from an I/O port.
#[inline]
pub fn inw(port: u16) -> Result<u16, DriverError> {
    ffi::inw(port)
}

/// Write a 32-bit long to an I/O port.
#[inline]
pub fn outl(port: u16, val: u32) -> Result<(), DriverError> {
    ffi::outl(port, val)
}

/// Read a 32-bit long from an I/O port.
#[inline]
pub fn inl(port: u16) -> Result<u32, DriverError> {
    ffi::inl(port)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_io_stubs() {
        // On non-MINIX hosts, all port I/O returns Err(IoError)
        assert_eq!(outb(0x80, 0), Err(DriverError::IoError));
        assert_eq!(inb(0x80), Err(DriverError::IoError));
        assert_eq!(outw(0x80, 0), Err(DriverError::IoError));
        assert_eq!(inw(0x80), Err(DriverError::IoError));
        assert_eq!(outl(0x80, 0), Err(DriverError::IoError));
        assert_eq!(inl(0x80), Err(DriverError::IoError));
    }
}
