//! # Virtio-blk Protocol
//!
//! Implements the virtio-blk device type: request/response format, config space
//! access, feature flags, and I/O operations (read, write, flush).

use crate::ffi;
use crate::virtio::{self, VirtQueue, VirtioDevice, VRING_DESC_F_WRITE};
use core::ffi::c_int;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

// ============================================================================
// Feature bits
// ============================================================================

pub const VIRTIO_BLK_F_BARRIER: u32 = 0;
pub const VIRTIO_BLK_F_SIZE_MAX: u32 = 1;
pub const VIRTIO_BLK_F_SEG_MAX: u32 = 2;
pub const VIRTIO_BLK_F_GEOMETRY: u32 = 4;
pub const VIRTIO_BLK_F_RO: u32 = 5;
pub const VIRTIO_BLK_F_BLK_SIZE: u32 = 6;
pub const VIRTIO_BLK_F_FLUSH: u32 = 9;
pub const VIRTIO_BLK_F_TOPOLOGY: u32 = 10;
pub const VIRTIO_BLK_F_MQ: u32 = 17;  // Multi-queue (bit 17)

// ============================================================================
// Config space offsets (from VIRTIO_DEV_SPECIFIC_OFF)
// ============================================================================

const CFG_CAPACITY_LO: u16 = 0;  // u64_t capacity (low 32 bits)
const CFG_CAPACITY_HI: u16 = 4;  // u64_t capacity (high 32 bits)
const CFG_SIZE_MAX: u16 = 8;     // u32_t
const CFG_SEG_MAX: u16 = 12;     // u32_t
const CFG_CYLINDERS: u16 = 16;   // u16_t geometry.cylinders
const CFG_HEADS: u16 = 18;       // u8_t geometry.heads
const CFG_SECTORS: u16 = 19;     // u8_t geometry.sectors
const CFG_BLK_SIZE: u16 = 20;    // u32_t blk_size
const CFG_NUM_QUEUES: u16 = 24;  // u16_t num_queues (if VIRTIO_BLK_F_MQ)

// ============================================================================
// Request header
// ============================================================================

/// virtio-blk request type
pub const VIRTIO_BLK_T_IN: u32 = 0;
pub const VIRTIO_BLK_T_OUT: u32 = 1;
pub const VIRTIO_BLK_T_FLUSH: u32 = 4;

/// BARRIER flag — OR'd into type_ field to enforce ordering
pub const VIRTIO_BLK_T_BARRIER: u32 = 0x8000_0000;

/// virtio-blk response status
pub const VIRTIO_BLK_S_OK: u8 = 0;
pub const VIRTIO_BLK_S_IOERR: u8 = 1;
pub const VIRTIO_BLK_S_UNSUPP: u8 = 2;

/// virtio-blk request header (16 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioBlkOutHdr {
    pub type_: u32,
    pub ioprio: u32,
    pub sector: u64,
}

/// virtio-blk response (single status byte)
pub type VirtioBlkStatus = u8;

// ============================================================================
// I/O vector support
// ============================================================================

/// Maximum number of iovec entries in a single try_transfer call.
pub const NR_IOREQS: usize = 8;

// ============================================================================
// Per-thread request buffers
// ============================================================================

/// Fixed number of threads for the pilot.
pub const MAX_THREADS: usize = 4;
pub const MAX_QUEUES: usize = 4;

/// Per-thread buffers for one I/O request.
pub struct ThreadBufs {
    pub hdr: *mut VirtioBlkOutHdr,
    pub hdr_phys: u64,
    pub status: *mut VirtioBlkStatus,
    pub status_phys: u64,
    pub data: *mut u8,
    pub data_phys: u64,
    pub data_size: usize,
}

impl ThreadBufs {
    /// Allocate per-thread buffers for `MAX_THREADS` threads.
    /// Each gets: header, status, and data buffer.
    pub fn allocate(data_sz: usize) -> Option<[ThreadBufs; MAX_THREADS]> {
        let alloc_one = || -> Option<ThreadBufs> {
            let hdr = ffi::alloc_contig_ffi(core::mem::size_of::<VirtioBlkOutHdr>())?;
            let status = ffi::alloc_contig_ffi(1)?;
            let data = ffi::alloc_contig_ffi(data_sz)?;
            Some(ThreadBufs {
                hdr: hdr.0 as *mut VirtioBlkOutHdr,
                hdr_phys: hdr.1,
                status: status.0 as *mut VirtioBlkStatus,
                status_phys: status.1,
                data: data.0 as *mut u8,
                data_phys: data.1,
                data_size: data_sz,
            })
        };

        Some([
            alloc_one()?,
            alloc_one()?,
            alloc_one()?,
            alloc_one()?,
        ])
    }

    pub fn free(bufs: &mut [ThreadBufs]) {
        for b in bufs.iter_mut() {
            unsafe {
                ffi::platform::free_contig(b.hdr as *mut core::ffi::c_void,
                    core::mem::size_of::<VirtioBlkOutHdr>());
                ffi::platform::free_contig(b.status as *mut core::ffi::c_void, 1);
                ffi::platform::free_contig(b.data as *mut core::ffi::c_void, b.data_size);
            }
        }
    }
}

// ============================================================================
// Completion slot (event-driven interrupt model)
// ============================================================================

/// A single completion event slot.
///
/// The interrupt handler writes status+bytes, then releases `done`.
/// The waiting thread acquires `done`, reads the result, then clears the flag.
/// This is a proper event-driven model: the interrupt handler publishes events
/// through shared atomic state, and threads consume them without relying on
/// sleep/wakeup for synchronization (sleep/wakeup is just a parking mechanism).
pub struct CompletionSlot {
    done: AtomicBool,
    status: AtomicU8,
    bytes: AtomicU32,
}

impl CompletionSlot {
    pub const fn new() -> Self {
        CompletionSlot {
            done: AtomicBool::new(false),
            status: AtomicU8::new(0),
            bytes: AtomicU32::new(0),
        }
    }

    /// Publish a completion event from interrupt context.
    /// `status` and `bytes` are released-before `done`.
    pub fn set_done(&self, status: u8, bytes: u32) {
        self.status.store(status, Ordering::Relaxed);
        self.bytes.store(bytes, Ordering::Relaxed);
        self.done.store(true, Ordering::Release);
    }

    /// Wait for a completion event and consume it.
    /// Returns (status, bytes).
    pub fn wait_and_clear(&self) -> (u8, u32) {
        loop {
            // Fast path: check if already completed (event-driven)
            if self.done.load(Ordering::Acquire) {
                let status = self.status.load(Ordering::Relaxed);
                let bytes = self.bytes.load(Ordering::Relaxed);
                self.done.store(false, Ordering::Relaxed);
                return (status, bytes);
            }
            // Park the thread until the interrupt handler publishes an event
            ffi::blockdriver_sleep();
        }
    }
}

// ============================================================================
// Virtio-blk device
// ============================================================================

/// Disk geometry (from VIRTIO_BLK_F_GEOMETRY config space)
pub struct DiskGeometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

pub struct VirtioBlk {
    pub dev: VirtioDevice,
    pub capacity: u64,         // in 512-byte sectors
    pub blk_size: u32,         // logical block size (usually 512)
    pub read_only: bool,
    pub flush_support: bool,
    pub barrier_support: bool,
    pub features: u32,         // negotiated features
    pub thread_bufs: [ThreadBufs; MAX_THREADS],
    pub completions: [CompletionSlot; MAX_THREADS],
    pub geometry: Option<DiskGeometry>,
    pub num_threads: usize,
    pub num_queues: u16,       // Number of virtqueues (1 if !MQ)
}

impl VirtioBlk {
    /// Probe and initialize a virtio-blk device.
    pub fn probe_and_init(instance: c_int, _num_threads: usize, data_size: usize)
        -> Option<Self>
    {
        let (devind, mut dev) = VirtioDevice::probe(0x0002, instance)?;

        // Negotiate features
        let guest_bits = (1 << VIRTIO_BLK_F_BARRIER)
            | (1 << VIRTIO_BLK_F_SEG_MAX)
            | (1 << VIRTIO_BLK_F_GEOMETRY)
            | (1 << VIRTIO_BLK_F_RO)
            | (1 << VIRTIO_BLK_F_BLK_SIZE)
            | (1 << VIRTIO_BLK_F_FLUSH)
            | (1 << VIRTIO_BLK_F_TOPOLOGY)
            | (1 << VIRTIO_BLK_F_MQ)
            | (1 << virtio::F_INDIRECT_DESC);

        let host_bits = dev.negotiate_features(guest_bits);
        let _ = devind;

        let ro = (host_bits >> VIRTIO_BLK_F_RO) & 1 != 0;
        let flush = (host_bits >> VIRTIO_BLK_F_FLUSH) & 1 != 0;
        let barrier = (host_bits >> VIRTIO_BLK_F_BARRIER) & 1 != 0;
        let mq = (host_bits >> VIRTIO_BLK_F_MQ) & 1 != 0;

        // Read config
        let capacity = unsafe {
            let lo = dev.sread32(CFG_CAPACITY_LO) as u64;
            let hi = dev.sread32(CFG_CAPACITY_HI) as u64;
            (hi << 32) | lo
        };

        let blk_size = if (host_bits >> VIRTIO_BLK_F_BLK_SIZE) & 1 != 0 {
            unsafe { dev.sread32(CFG_BLK_SIZE) }
        } else {
            512
        };

        // Read geometry from config (if supported)
        let geometry = if (host_bits >> VIRTIO_BLK_F_GEOMETRY) & 1 != 0 {
            Some(DiskGeometry {
                cylinders: unsafe { dev.sread16(CFG_CYLINDERS) },
                heads: unsafe { dev.sread8(CFG_HEADS) },
                sectors: unsafe { dev.sread8(CFG_SECTORS) },
            })
        } else {
            None
        };

        // Determine number of queues from config (MQ or single)
        let num_queues = if mq {
            let nq = unsafe { dev.sread16(CFG_NUM_QUEUES) };
            if nq == 0 { 1 } else { nq.min(MAX_QUEUES as u16) }
        } else {
            1
        };

        // We know how to drive it
        unsafe { dev.write_status(virtio::STATUS_DRV); }

        // Allocate queues
        if dev.alloc_queues(num_queues as usize).is_err() { return None; }

        // Allocate indirect descriptor table for each queue
        for i in 0..num_queues as usize {
            let q = unsafe { &mut *dev.queues.add(i) };
            if q.alloc_indirect(32).is_err() { return None; }
        }

        // Allocate per-thread request buffers
        let bufs = ThreadBufs::allocate(data_size)?;

        // Try MSI-X — if available, use per-queue vectors
        if dev.setup_msix(devind) {
            // MSI-X: register per-queue IRQ handler is already done in setup_msix
            // No need for legacy irq_setup/irqenable
        }

        // All good — ready
        // For legacy mode, ready() registers IRQ. For MSI-X, skip legacy IRQ setup.
        if !dev.msix_available {
            if dev.ready().is_err() { return None; }
        } else {
            // Signal DRV_OK without legacy IRQ setup
            unsafe { dev.write_status(virtio::STATUS_DRV_OK); }
        }

        Some(VirtioBlk {
            dev,
            capacity,
            blk_size: if blk_size == 0 { 512 } else { blk_size },
            read_only: ro,
            flush_support: flush,
            barrier_support: barrier,
            features: host_bits,
            thread_bufs: bufs,
            completions: [
                CompletionSlot::new(),
                CompletionSlot::new(),
                CompletionSlot::new(),
                CompletionSlot::new(),
            ],
            geometry,
            num_threads: MAX_THREADS,
            num_queues,
        })
    }

    /// Map a thread ID to a queue ID for per-queue operations.
    #[inline]
    pub fn queue_for_tid(&self, tid: usize) -> usize {
        if self.num_queues > 1 {
            tid % self.num_queues as usize
        } else {
            0
        }
    }

    /// Perform a read or write operation.
    pub fn transfer(&mut self, write: bool, sector: u64, count: u32) -> isize {
        let tid = ffi::blockdriver_get_tid() as usize;
        if tid >= self.num_threads { return ffi::platform::EINVAL as isize; }

        let qid = self.queue_for_tid(tid);
        let bufs = &self.thread_bufs[tid];
        let blk_size = self.blk_size;

        // Use raw pointer to avoid borrow conflict with self access below
        let q = unsafe { &mut *self.dev.queues.add(qid) };

        // Prepare the request header
        unsafe {
            (*bufs.hdr).type_ = if write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };
            (*bufs.hdr).ioprio = 0;
            (*bufs.hdr).sector = sector;
        }

        let need_data = count > 0;
        let desc_count: u16 = if need_data { 3 } else { 2 };

        let head = match q.alloc_descs(desc_count) {
            Some(h) => h,
            None => return ffi::platform::EBUSY as isize,
        };

        let hdr_phys = bufs.hdr_phys;
        let data_phys = bufs.data_phys;
        let data_size = (count as usize) * (blk_size as usize);
        let status_phys = bufs.status_phys;

        let d0 = head;
        if need_data {
            let d1 = (d0 + 1) % q.num;
            let d2 = (d1 + 1) % q.num;
            q.set_desc(d0, hdr_phys, core::mem::size_of::<VirtioBlkOutHdr>() as u32, false, true);
            q.set_desc(d1, data_phys, data_size as u32, !write, true);
            q.set_desc(d2, status_phys, 1, true, false);
        } else {
            let d2 = (d0 + 1) % q.num;
            q.set_desc(d0, hdr_phys, core::mem::size_of::<VirtioBlkOutHdr>() as u32, false, true);
            q.set_desc(d2, status_phys, 1, true, false);
        }

        // Submit to available ring
        q.submit(head, tid);

        unsafe { self.dev.kick(qid as u16); }

        // Wait for completion event
        let (status, _bytes) = self.completions[tid].wait_and_clear();

        if status == VIRTIO_BLK_S_OK {
            data_size as isize
        } else {
            ffi::platform::EIO as isize
        }
    }

    /// Flush the device cache.
    pub fn flush(&mut self) -> c_int {
        if !self.flush_support {
            return ffi::platform::EOPNOTSUPP;
        }

        let tid = ffi::blockdriver_get_tid() as usize;
        if tid >= self.num_threads { return ffi::platform::EINVAL; }

        let qid = self.queue_for_tid(tid);
        let bufs = &self.thread_bufs[tid];
        let barrier_support = self.barrier_support;

        // Use raw pointer to avoid borrow conflict
        let q = unsafe { &mut *self.dev.queues.add(qid) };

        // Flush request — with optional BARRIER flag
        unsafe {
            let mut type_ = VIRTIO_BLK_T_FLUSH;
            if barrier_support {
                type_ |= VIRTIO_BLK_T_BARRIER;
            }
            (*bufs.hdr).type_ = type_;
            (*bufs.hdr).ioprio = 0;
            (*bufs.hdr).sector = 0;
        }

        let head = match q.alloc_descs(2) {
            Some(h) => h,
            None => return ffi::platform::EBUSY,
        };

        let d0 = head;
        let d1 = (d0 + 1) % q.num;

        q.set_desc(d0, bufs.hdr_phys, core::mem::size_of::<VirtioBlkOutHdr>() as u32, false, true);
        q.set_desc(d1, bufs.status_phys, 1, true, false);

        q.submit(head, tid);

        unsafe { self.dev.kick(qid as u16); }

        // Wait for completion event
        let (status, _bytes) = self.completions[tid].wait_and_clear();

        if status == VIRTIO_BLK_S_OK { 0 } else { ffi::platform::EIO }
    }

    /// Perform a multi-sector I/O using caller-provided grant buffers.
    ///
    /// Uses `sys_vumap` to translate caller grants to physical addresses,
    /// then builds a single descriptor chain: header → data_bufs... → status.
    ///
    /// Returns bytes transferred on success, negative errno on failure.
    pub fn try_transfer(
        &mut self,
        write: bool,
        sector: u64,
        endpt: c_int,
        grants: &[c_int],
        sizes: &[usize],
    ) -> isize {
        let tid = ffi::blockdriver_get_tid() as usize;
        if tid >= self.num_threads { return ffi::platform::EINVAL as isize; }

        let cnt = grants.len();
        if cnt == 0 || cnt > NR_IOREQS || cnt != sizes.len() {
            return ffi::platform::EINVAL as isize;
        }

        let qid = self.queue_for_tid(tid);
        let bufs = &self.thread_bufs[tid];

        // Use raw pointer to avoid borrow conflict with completions below
        let q = unsafe { &mut *self.dev.queues.add(qid) };

        // Prepare request header
        unsafe {
            (*bufs.hdr).type_ = if write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };
            (*bufs.hdr).ioprio = 0;
            (*bufs.hdr).sector = sector;
        }

        // Build vumap_vir input array from grants
        let mut vvec: [ffi::VumapVir; NR_IOREQS] = [
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
            ffi::VumapVir { vv_addr: 0, vv_size: 0 },
        ];

        let mut total_size: usize = 0;
        for i in 0..cnt {
            vvec[i].vv_addr = grants[i] as u64;
            vvec[i].vv_size = sizes[i] as u64;
            total_size += sizes[i];
        }

        // Output physical vector: [header..data..status]
        // phys[0] is header, phys[1..ndata] is data, phys[ndata+1] is status
        let mut pvec: [ffi::VumapPhys; NR_IOREQS + 2] = [
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
            ffi::VumapPhys { vp_addr: 0, vp_size: 0 },
        ];

        let mut pcount: c_int = (NR_IOREQS + 2) as c_int;

        // access: VUA_READ(1) for write (device reads data), VUA_WRITE(2) for read (device writes data)
        let access = if write { ffi::VUA_READ } else { ffi::VUA_WRITE };

        let r = unsafe {
            ffi::sys_vumap_ffi(
                endpt,
                &vvec as *const _ as *const ffi::VumapVir,
                cnt as c_int,
                0,
                access,
                &mut pvec[1] as *mut _,
                &mut pcount,
            )
        };
        if r != ffi::platform::OK { return r as isize; }

        let ndata = pcount as usize;
        let desc_count: u16 = 1 + ndata as u16 + 1; // header + data + status

        let head = match q.alloc_descs(desc_count) {
            Some(h) => h,
            None => return ffi::platform::EBUSY as isize,
        };

        // Build descriptor chain
        let mut prev = head;

        // d0: header — device reads this (not write)
        q.set_desc(
            prev,
            bufs.hdr_phys,
            core::mem::size_of::<VirtioBlkOutHdr>() as u32,
            false,
            true, // always has next
        );

        // data descriptors — one per physical region from sys_vumap
        for i in 0..ndata {
            let idx = (prev + 1) % q.num;
            let phys = &pvec[1 + i];
            let dev_write = (phys.vp_addr & 1) != 0; // low bit = writable flag
            let clean_addr = phys.vp_addr & !1;
            let is_last_data = i == ndata - 1;

            q.set_desc(
                idx,
                clean_addr,
                phys.vp_size as u32,
                dev_write,
                true, // status follows after all data
            );
            prev = idx;
        }

        // status descriptor — device always writes to it
        let status_idx = (prev + 1) % q.num;
        q.set_desc(status_idx, bufs.status_phys, 1, true, false);

        // Submit and kick
        q.submit(head, tid);

        unsafe { self.dev.kick(qid as u16); }

        // Wait for completion event
        let (status, _bytes) = self.completions[tid].wait_and_clear();

        if status == VIRTIO_BLK_S_OK {
            total_size as isize
        } else {
            ffi::platform::EIO as isize
        }
    }

    /// Process the used rings on interrupt.
    /// Publishes completion events through the atomic CompletionSlot.
    /// Polls all queues to support multi-queue operation.
    pub fn handle_interrupt(&mut self) {
        let nq = self.num_queues as usize;
        for qid in 0..nq {
            let q = unsafe { &mut *self.dev.queues.add(qid) };
            loop {
                match q.collect() {
                    Some((tid, _len)) => {
                        if (tid as usize) < MAX_THREADS {
                            // Read status from the per-thread status buffer
                            let status = unsafe { *self.thread_bufs[tid as usize].status };
                            // Publish completion event
                            self.completions[tid as usize].set_done(status, _len);
                            // Wake the waiting thread
                            ffi::blockdriver_wakeup(tid as c_int);
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

impl Drop for VirtioBlk {
    fn drop(&mut self) {
        ThreadBufs::free(&mut self.thread_bufs);
        self.dev.reset();
        self.dev.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_layout() {
        assert_eq!(core::mem::size_of::<VirtioBlkOutHdr>(), 16);
        assert_eq!(core::mem::align_of::<VirtioBlkOutHdr>(), 8);
    }

    #[test]
    fn status_constants() {
        assert_eq!(VIRTIO_BLK_S_OK, 0);
        assert_eq!(VIRTIO_BLK_S_IOERR, 1);
        assert_eq!(VIRTIO_BLK_S_UNSUPP, 2);
    }
}
