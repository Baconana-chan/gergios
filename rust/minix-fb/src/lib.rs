// minix-fb — Minimal framebuffer API for GergiOS.
//
// Wraps the MINIX /dev/fb device with ioctl(), mmap(), and
// basic 2D drawing operations for games and graphical applications.
//
// # Quick start
//
// ```no_run
// use minix_fb::Framebuffer;
//
// let mut fb = Framebuffer::open().expect("failed to open /dev/fb");
// fb.clear(0x000000); // black
// fb.fill_rect(10, 10, 100, 100, 0xFF0000); // red rect
// fb.flip(); // double-buffer swap (if virtual height > visible height)
// ```

use std::io;
use std::os::unix::io::RawFd;
use std::ptr;

const FB_DEVICE: &str = "/dev/fb";

// ===========================================================================
// C-compatible struct types (matching MINIX <minix/fb.h>)
// ===========================================================================

/// Color bitfield description (matching MINIX `struct fb_bitfield`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FbBitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

/// Fixed screen info (matching MINIX `struct fb_fix_screeninfo`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FbFixScreeninfo {
    pub id: [u8; 16],
    pub xpanstep: u16,
    pub ypanstep: u16,
    pub ywrapstep: u16,
    pub _pad0: u16,         // padding to 4-byte align line_length
    pub line_length: u32,
    pub _pad1: u32,         // padding to 8-byte align mmio_start
    pub mmio_start: u64,     // phys_bytes = unsigned long (8 bytes on x86_64)
    pub mmio_len: u64,       // size_t = 8 bytes on x86_64
    pub reserved: [u16; 15],
}

/// Variable screen info (matching MINIX `struct fb_var_screeninfo`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FbVarScreeninfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub bits_per_pixel: u32,
    pub red: FbBitfield,
    pub green: FbBitfield,
    pub blue: FbBitfield,
    pub transp: FbBitfield,
    pub reserved: [u16; 10],
}

// ===========================================================================
// ioctl request codes
// ===========================================================================
//
// MINIX/NetBSD uses the BSD _IOR/_IOW convention:
//   IOC_OUT = 0x40000000  (_IOR = read)
//   IOC_IN  = 0x80000000  (_IOW = write)
//   code = dir | (type << 8) | num | (sizeof(struct) << 16)

const fn ioc_ior(group: u8, num: u8, size: usize) -> libc::c_ulong {
    (0x40000000u64 | ((group as u64) << 8) | (num as u64) | ((size as u64) << 16)) as libc::c_ulong
}

const fn ioc_iow(group: u8, num: u8, size: usize) -> libc::c_ulong {
    (0x80000000u64 | ((group as u64) << 8) | (num as u64) | ((size as u64) << 16)) as libc::c_ulong
}

const FBIOGET_VSCREENINFO: libc::c_ulong =
    ioc_ior(b'V', 1, core::mem::size_of::<FbVarScreeninfo>());
const FBIOPUT_VSCREENINFO: libc::c_ulong =
    ioc_iow(b'V', 2, core::mem::size_of::<FbVarScreeninfo>());
const FBIOGET_FSCREENINFO: libc::c_ulong =
    ioc_ior(b'V', 3, core::mem::size_of::<FbFixScreeninfo>());
const FBIOPAN_DISPLAY: libc::c_ulong =
    ioc_iow(b'V', 4, core::mem::size_of::<FbVarScreeninfo>());

// ===========================================================================
// Framebuffer
// ===========================================================================

/// A handle to the framebuffer device with mapped memory for direct pixel access.
pub struct Framebuffer {
    fd: RawFd,
    fix: FbFixScreeninfo,
    var: FbVarScreeninfo,
    buffer: &'static mut [u8],
    /// Current back buffer index for double buffering (0 or 1).
    back_buffer: u32,
}

// SAFETY: The framebuffer is safe to send between threads since all operations
// are sequential and there's no concurrent access to the mapped buffer.
unsafe impl Send for Framebuffer {}

impl Framebuffer {
    /// Open `/dev/fb`, query screen info, and mmap the framebuffer memory.
    pub fn open() -> io::Result<Self> {
        // Open the framebuffer device
        let fd = unsafe {
            let fd = libc::open(
                FB_DEVICE.as_ptr() as *const libc::c_char,
                libc::O_RDWR | libc::O_CLOEXEC,
            );
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            fd
        };

        // Get fixed screen info
        let mut fix: FbFixScreeninfo = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            libc::ioctl(
                fd,
                FBIOGET_FSCREENINFO,
                &mut fix as *mut FbFixScreeninfo,
            )
        };
        if ret < 0 {
            unsafe { libc::close(fd) };
            return Err(io::Error::last_os_error());
        }

        // Get variable screen info
        let mut var: FbVarScreeninfo = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            libc::ioctl(
                fd,
                FBIOGET_VSCREENINFO,
                &mut var as *mut FbVarScreeninfo,
            )
        };
        if ret < 0 {
            unsafe { libc::close(fd) };
            return Err(io::Error::last_os_error());
        }

        // Calculate framebuffer size
        let fb_size = (fix.line_length as u64) * (var.yres_virtual as u64) as usize;

        // Map framebuffer memory
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                fb_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            unsafe { libc::close(fd) };
            return Err(io::Error::last_os_error());
        }

        let buffer = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, fb_size) };

        Ok(Framebuffer {
            fd,
            fix,
            var,
            buffer,
            back_buffer: 0,
        })
    }

    // =======================================================================
    // Info getters
    // =======================================================================

    /// Visible width in pixels.
    pub fn width(&self) -> u32 {
        self.var.xres
    }

    /// Visible height in pixels.
    pub fn height(&self) -> u32 {
        self.var.yres
    }

    /// Virtual width in pixels (may be larger than visible for double-buffering).
    pub fn virtual_width(&self) -> u32 {
        self.var.xres_virtual
    }

    /// Virtual height in pixels (may be larger than visible for double-buffering).
    pub fn virtual_height(&self) -> u32 {
        self.var.yres_virtual
    }

    /// Bits per pixel (e.g., 32 for true color).
    pub fn bpp(&self) -> u8 {
        self.var.bits_per_pixel as u8
    }

    /// Bytes per pixel (rounded up).
    pub fn bytes_per_pixel(&self) -> usize {
        ((self.var.bits_per_pixel + 7) / 8) as usize
    }

    /// Line length in bytes (stride).
    pub fn line_length(&self) -> u32 {
        self.fix.line_length
    }

    /// Reference to the raw framebuffer buffer.
    pub fn buffer(&self) -> &[u8] {
        self.buffer
    }

    /// Mutable reference to the raw framebuffer buffer.
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        self.buffer
    }

    /// Get the current variable screen info.
    pub fn var_screeninfo(&self) -> &FbVarScreeninfo {
        &self.var
    }

    /// Get the fixed screen info.
    pub fn fix_screeninfo(&self) -> &FbFixScreeninfo {
        &self.fix
    }

    // =======================================================================
    // Pixel operations
    // =======================================================================

    /// Pack RGB (8-bit each) into a pixel value suitable for the current mode.
    ///
    /// For 32-bit modes, this packs as 0x00RRGGBB (assuming red at offset 16,
    /// green at offset 8, blue at offset 0, which is the most common layout).
    pub fn rgb(&self, r: u8, g: u8, b: u8) -> u32 {
        if self.var.bits_per_pixel == 32 || self.var.bits_per_pixel == 24 {
            // Common default: RGB888, red at bit 16, green at 8, blue at 0
            let r_shift = if self.var.red.offset < 32 { self.var.red.offset } else { 16 };
            let g_shift = if self.var.green.offset < 32 { self.var.green.offset } else { 8 };
            let b_shift = if self.var.blue.offset < 32 { self.var.blue.offset } else { 0 };
            ((r as u32) << r_shift) | ((g as u32) << g_shift) | ((b as u32) << b_shift)
        } else if self.var.bits_per_pixel == 16 {
            // RGB565
            ((r as u32 >> 3) << 11) | ((g as u32 >> 2) << 5) | (b as u32 >> 3)
        } else {
            // Fallback: pack into lowest bits
            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
        }
    }

    /// Set a pixel at (x, y) to the given packed color.
    pub fn pixel(&mut self, x: u32, y: u32, color: u32) {
        if x >= self.var.xres || y >= self.var.yres {
            return;
        }
        let bpp = self.bytes_per_pixel();
        let offset = (y as usize) * (self.fix.line_length as usize) + (x as usize) * bpp;
        let buf = &mut self.buffer[offset..offset + bpp];

        match bpp {
            4 => {
                // 32-bit: write as u32
                let bytes = color.to_ne_bytes();
                buf.copy_from_slice(&bytes);
            }
            3 => {
                // 24-bit: write as 3 bytes
                buf[0] = color as u8;
                buf[1] = (color >> 8) as u8;
                buf[2] = (color >> 16) as u8;
            }
            2 => {
                // 16-bit: write as u16
                let bytes = (color as u16).to_ne_bytes();
                buf.copy_from_slice(&bytes);
            }
            1 => {
                // 8-bit: write as byte
                buf[0] = color as u8;
            }
            _ => {}
        }
    }

    /// Fill a rectangle with a packed color.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let x_end = (x + w).min(self.var.xres);
        let y_end = (y + h).min(self.var.yres);
        let bpp = self.bytes_per_pixel();

        for row in y..y_end {
            let offset = (row as usize) * (self.fix.line_length as usize) + (x as usize) * bpp;
            let row_bytes = ((x_end - x) as usize) * bpp;

            // Fill the first pixel, then replicate across the row
            match bpp {
                4 => {
                    let color_bytes = color.to_ne_bytes();
                    let buf = &mut self.buffer[offset..offset + row_bytes];
                    for chunk in buf.chunks_exact_mut(4) {
                        chunk.copy_from_slice(&color_bytes);
                    }
                }
                3 => {
                    let buf = &mut self.buffer[offset..offset + row_bytes];
                    let mut i = 0;
                    while i + 3 <= row_bytes {
                        buf[i] = color as u8;
                        buf[i + 1] = (color >> 8) as u8;
                        buf[i + 2] = (color >> 16) as u8;
                        i += 3;
                    }
                }
                2 => {
                    let color_bytes = (color as u16).to_ne_bytes();
                    let buf = &mut self.buffer[offset..offset + row_bytes];
                    for chunk in buf.chunks_exact_mut(2) {
                        chunk.copy_from_slice(&color_bytes);
                    }
                }
                1 => {
                    self.buffer[offset..offset + row_bytes].fill(color as u8);
                }
                _ => {}
            }
        }
    }

    /// Clear the entire visible screen to a packed color.
    pub fn clear(&mut self, color: u32) {
        self.fill_rect(0, 0, self.var.xres, self.var.yres, color);
    }

    // =======================================================================
    // Blit (copy rectangle)
    // =======================================================================

    /// Copy a rectangle of pixels from (sx, sy) to (dx, dy) with given size.
    ///
    /// Source and destination may overlap — the copy is direction-aware.
    pub fn blit(&mut self, sx: u32, sy: u32, w: u32, h: u32, dx: u32, dy: u32) {
        let bpp = self.bytes_per_pixel();
        let stride = self.fix.line_length as usize;

        let src_row_s = |row: u32| -> usize { (row as usize) * stride + (sx as usize) * bpp };
        let dst_row_s = |row: u32| -> usize { (row as usize) * stride + (dx as usize) * bpp };
        let copy_bytes = (w as usize) * bpp;

        // Handle overlapping regions by copying in correct direction
        if dy > sy || (dy == sy && dx >= sx) {
            // Copy top-to-bottom (or left-to-right)
            for i in 0..h {
                let src_off = src_row_s(sy + i);
                let dst_off = dst_row_s(dy + i);
                let src = self.buffer[src_off..src_off + copy_bytes].as_ptr();
                let dst = self.buffer[dst_off..dst_off + copy_bytes].as_mut_ptr();
                unsafe {
                    ptr::copy(src, dst, copy_bytes);
                }
            }
        } else {
            // Copy bottom-to-top to avoid overwriting
            for i in (0..h).rev() {
                let src_off = src_row_s(sy + i);
                let dst_off = dst_row_s(dy + i);
                let src = self.buffer[src_off..src_off + copy_bytes].as_ptr();
                let dst = self.buffer[dst_off..dst_off + copy_bytes].as_mut_ptr();
                unsafe {
                    ptr::copy(src, dst, copy_bytes);
                }
            }
        }
    }

    /// Blit from an external pixel buffer into the framebuffer.
    ///
    /// The source buffer should be tightly packed (no extra stride) with
    /// `w * h * bytes_per_pixel` bytes.
    pub fn blit_from(&mut self, src: &[u8], sx: u32, sy: u32, w: u32, h: u32, dx: u32, dy: u32) {
        let bpp = self.bytes_per_pixel();
        let stride = self.fix.line_length as usize;
        let src_stride = (w as usize) * bpp;

        for i in 0..h {
            let src_off = ((sy + i) as usize) * src_stride + (sx as usize) * bpp;
            let dst_off = ((dy + i) as usize) * stride + (dx as usize) * bpp;
            let copy_bytes = (w as usize) * bpp;

            if src_off + copy_bytes <= src.len() && dst_off + copy_bytes <= self.buffer.len() {
                let src_slice = &src[src_off..src_off + copy_bytes];
                let dst_slice = &mut self.buffer[dst_off..dst_off + copy_bytes];
                dst_slice.copy_from_slice(src_slice);
            }
        }
    }

    // =======================================================================
    // Double buffering (pan display / flip)
    // =======================================================================

    /// Set up double buffering by configuring virtual height = 2 × visible height.
    ///
    /// Must be called once before using `flip()`.
    pub fn enable_double_buffer(&mut self) -> io::Result<()> {
        if self.var.yres_virtual < self.var.yres * 2 {
            self.var.yres_virtual = self.var.yres * 2;

            let ret = unsafe {
                libc::ioctl(
                    self.fd,
                    FBIOPUT_VSCREENINFO,
                    &self.var as *const FbVarScreeninfo,
                )
            };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }

            // Re-query to get updated values
            let ret = unsafe {
                libc::ioctl(
                    self.fd,
                    FBIOGET_VSCREENINFO,
                    &mut self.var as *mut FbVarScreeninfo,
                )
            };
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        // Ensure the buffer length covers both pages (re-mmap if needed)
        let new_size = (self.fix.line_length as u64 * self.var.yres_virtual as u64) as usize;
        if new_size > self.buffer.len() {
            // Unmap old buffer, mmap new larger one
            unsafe {
                libc::munmap(
                    self.buffer.as_mut_ptr() as *mut libc::c_void,
                    self.buffer.len(),
                );
            }
            let new_ptr = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    new_size,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    self.fd,
                    0,
                )
            };
            if new_ptr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            self.buffer = unsafe { std::slice::from_raw_parts_mut(new_ptr as *mut u8, new_size) };
        }

        self.back_buffer = 0;
        Ok(())
    }

    /// Swap the visible and back buffers (pan display).
    ///
    /// Requires `enable_double_buffer()` to have been called first.
    /// Swaps between front buffer (offset 0) and back buffer (offset yres).
    pub fn flip(&mut self) -> io::Result<()> {
        self.back_buffer ^= 1;
        let yoffset = if self.back_buffer > 0 { self.var.yres } else { 0 };

        let mut pan = self.var;
        pan.yoffset = yoffset;

        let ret = unsafe {
            libc::ioctl(self.fd, FBIOPAN_DISPLAY, &pan as *const FbVarScreeninfo)
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        self.var.yoffset = yoffset;
        Ok(())
    }

    /// Get a mutable slice to the current back buffer for drawing.
    ///
    /// Returns `None` if double buffering is not enabled.
    pub fn back_buffer(&mut self) -> Option<&mut [u8]> {
        if self.var.yres_virtual < self.var.yres * 2 {
            return None;
        }
        let offset = if self.back_buffer > 0 {
            (self.var.yres as usize) * (self.fix.line_length as usize)
        } else {
            0
        };
        let size = (self.var.yres as usize) * (self.fix.line_length as usize);
        Some(&mut self.buffer[offset..offset + size])
    }

    /// Get a mutable slice to the current front buffer.
    pub fn front_buffer(&self) -> &[u8] {
        let offset = if self.var.yoffset > 0 {
            (self.var.yoffset as usize) * (self.fix.line_length as usize)
        } else {
            0
        };
        let size = (self.var.yres as usize) * (self.fix.line_length as usize);
        &self.buffer[offset..offset + size]
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            // Unmap framebuffer memory
            libc::munmap(
                self.buffer.as_mut_ptr() as *mut libc::c_void,
                self.buffer.len(),
            );
            // Close the device
            libc::close(self.fd);
        }
    }
}
