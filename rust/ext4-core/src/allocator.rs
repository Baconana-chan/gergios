//! Global allocator and panic handler for cross-compilation with `-Zbuild-std`.
//!
//! When building for `x86_64-unknown-minix` with `-Zbuild-std=core,alloc`,
//! the compiler requires both a `#[global_allocator]` and a `#[panic_handler]`
//! to exist somewhere in the dependency graph.
//!
//! This module provides:
//! - A `#[global_allocator]` that delegates to C's `malloc`/`free`/`realloc`
//! - A `#[panic_handler]` that calls `abort()`
//!
//! This module is compiled only when the `build-std` feature is enabled
//! (i.e., during cross-compilation for MINIX). For native builds, the
//! standard library provides both.

use core::alloc::{GlobalAlloc, Layout};

extern "C" {
    fn malloc(size: usize) -> *mut core::ffi::c_void;
    fn free(ptr: *mut core::ffi::c_void);
    fn realloc(ptr: *mut core::ffi::c_void, size: usize) -> *mut core::ffi::c_void;
}

/// Allocator that delegates to the C runtime's `malloc`/`free`/`realloc`.
///
/// This is appropriate for MINIX kernel drivers where the C host provides
/// heap memory management. The C `malloc` implementation (typically from
/// MINIX's `libc` or the kernel's own allocator) provides the backing memory.
pub struct MinixAllocator;

unsafe impl GlobalAlloc for MinixAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { malloc(layout.size()) };
        ptr as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { free(ptr as *mut core::ffi::c_void) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
        let ptr = unsafe { realloc(ptr as *mut core::ffi::c_void, new_size) };
        ptr as *mut u8
    }
}

#[global_allocator]
static ALLOCATOR: MinixAllocator = MinixAllocator;

/// Panic handler for `no_std` + `-Zbuild-std` builds.
///
/// The `"panic-strategy": "abort"` in the target spec should normally
/// eliminate the need for a panic handler, but `-Zbuild-std` sometimes
/// requires one explicitly. This handler loops with an abort hint.
#[cfg(target_os = "minix")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
