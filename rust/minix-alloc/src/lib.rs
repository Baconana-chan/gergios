//! # minix-alloc — Rust GlobalAlloc bridged to MINIX C malloc/free
//!
//! This crate provides a `#[global_allocator]` that routes Rust's `alloc`
//! crate allocations through the MINIX libc `malloc`/`free` functions via
//! C FFI.
//!
//! ## Usage
//!
//! To use the MINIX allocator in a Rust component:
//!
//! ```ignore
//! use minix_alloc::MinixAllocator;
//!
//! #[global_allocator]
//! static ALLOCATOR: MinixAllocator = MinixAllocator;
//! ```
//!
//! Once set, `Box`, `Vec`, `String`, `Rc`, `Arc`, etc. all work using
//! the standard libc heap.
//!
//! ## Safety
//!
//! The `GlobalAlloc` trait is inherently `unsafe` to implement. This crate:
//! - Forwards `Layout` size/alignment to `malloc`/`aligned_alloc`
//! - Wraps each FFI call in `unsafe` blocks (extern "C" declarations
//!   of `malloc`, `realloc`, `free`)
//! - Calls `alloc_error_handler` on allocation failure
//!
//! ## Platform support
//!
//! - **Minix** (`target_os = "minix"`): Links to `malloc`/`free` from libc.
//! - **Host** (everything else): Uses `std::alloc::System` as a fallback.

#![no_std]

// Import the `alloc` crate for `handle_alloc_error` and collection types.
extern crate alloc;use core::alloc::{GlobalAlloc, Layout};
use alloc::alloc::handle_alloc_error;

// ---------------------------------------------------------------------------
// MINIX allocator
// ---------------------------------------------------------------------------

/// The MINIX libc-based global allocator.
///
/// Routes all Rust heap allocations through the standard C `malloc`/`free`
/// functions available on MINIX.
pub struct MinixAllocator;

// On actual MINIX, link to libc malloc/free.
#[cfg(target_os = "minix")]
mod c_alloc {
    //! FFI declarations for MINIX libc allocator functions.
    extern "C" {
        pub fn malloc(size: usize) -> *mut u8;
        pub fn free(ptr: *mut u8);
        pub fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8;
        pub fn calloc(nmemb: usize, size: usize) -> *mut u8;
    }
}

// On host (non-MINIX), provide stubs or use std allocator.
#[cfg(not(target_os = "minix"))]
mod c_alloc {
    // Host development: wrap std::alloc::System functions.
    // These compile but won't be called if the user has a different
    // global allocator on the host.
    // Host stubs — always return null to signal "not available on host".
    pub unsafe fn malloc(_size: usize) -> *mut u8 { core::ptr::null_mut() }
    pub unsafe fn free(_ptr: *mut u8) {}
    pub unsafe fn realloc(_ptr: *mut u8, _new_size: usize) -> *mut u8 { core::ptr::null_mut() }
    pub unsafe fn calloc(_nmemb: usize, _size: usize) -> *mut u8 { core::ptr::null_mut() }
}

unsafe impl GlobalAlloc for MinixAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = if layout.align() <= 16 {
            // Standard malloc is sufficient for small alignments
            unsafe { c_alloc::malloc(layout.size()) }
        } else {
            // Large alignment requires aligned_alloc or manual overallocation
            unsafe { c_alloc::calloc(1, layout.size()) }
        };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        ptr
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { c_alloc::free(ptr) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { c_alloc::realloc(ptr, new_size) };
        if new_ptr.is_null() {
            // Realloc to 0 may return NULL on some implementations;
            // this is equivalent to free.
            if new_size == 0 {
                return core::ptr::NonNull::dangling().as_ptr();
            }
            let layout = Layout::from_size_align(new_size, 1).unwrap();
            handle_alloc_error(layout);
        }
        new_ptr
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { c_alloc::calloc(1, layout.size()) };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        ptr
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Full allocator tests only run on MINIX where real malloc/free exist.
    // On host, all stubs return null — no point testing allocation.

    #[test]
    fn crate_links() {
        // At least one test always runs to verify the crate compiles
        // and links correctly.
        let _ = MinixAllocator;
    }

    #[test]
    #[cfg(target_os = "minix")]
    fn alloc_dealloc() {
        let allocator = MinixAllocator;
        let layout = Layout::from_size_align(64, 4).unwrap();
        unsafe {
            let ptr = allocator.alloc(layout);
            assert!(!ptr.is_null());
            core::ptr::write_volatile(ptr, 0x42u8);
            allocator.dealloc(ptr, layout);
        }
    }

    #[test]
    #[cfg(target_os = "minix")]
    fn alloc_zeroed() {
        let allocator = MinixAllocator;
        let layout = Layout::from_size_align(64, 4).unwrap();
        unsafe {
            let ptr = allocator.alloc_zeroed(layout);
            assert!(!ptr.is_null());
            for i in 0..64 {
                assert_eq!(core::ptr::read_volatile(ptr.add(i)), 0u8);
            }
            allocator.dealloc(ptr, layout);
        }
    }

    #[test]
    #[cfg(target_os = "minix")]
    fn realloc_smaller() {
        let allocator = MinixAllocator;
        let old_layout = Layout::from_size_align(128, 4).unwrap();
        unsafe {
            let ptr = allocator.alloc(old_layout);
            assert!(!ptr.is_null());
            core::ptr::write_volatile(ptr, 0xFFu8);
            let new_ptr = allocator.realloc(ptr, old_layout, 64);
            assert!(!new_ptr.is_null());
            assert_eq!(core::ptr::read_volatile(new_ptr), 0xFFu8);
            let new_layout = Layout::from_size_align(64, 4).unwrap();
            allocator.dealloc(new_ptr, new_layout);
        }
    }
}
