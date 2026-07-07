//! # Virtio-net Driver — Core Driver Logic
//!
//! Implements the core driver operations: PCI probe, feature negotiation,
//! queue setup, packet send/receive, and interrupt handling.
//!
//! Architecture mirrors the C virtio_net driver at `minix/drivers/net/virtio_net/`.

#![allow(dead_code)]

use core::ffi::c_int;

use crate::device::{self, VirtioDevice, STATUS_DRV};
use crate::ffi;
use crate::net::{self, VirtioNetHdr};
use crate::queue::VirtQueue;

// ============================================================================
// Constants
// ============================================================================

/// Number of packet buffers to allocate.
/// Must be a power of 2, and <= the smallest queue size the device offers.
pub const BUF_PACKETS: u16 = 64;

/// Maximum ethernet packet size.
pub const MAX_PACK_SIZE: usize = 1514;

/// Size of the virtio-net header prepended to each packet.
pub const HDR_SIZE: usize = core::mem::size_of::<VirtioNetHdr>();

// ============================================================================
// Packet descriptor
// ============================================================================

/// A single packet buffer with its virtio-net header and data region.
pub struct Packet {
    pub vhdr: *mut VirtioNetHdr,
    pub vhdr_phys: u64,
    pub vdata: *mut u8,
    pub vdata_phys: u64,
    pub len: usize,
}

// ============================================================================
// Virtio-net device state
// ============================================================================

pub struct VirtioNet {
    pub dev: VirtioDevice,
    pub mac: [u8; 6],
    pub link_up: bool,
    pub features: u32,          // Negotiated features

    // Packet buffers
    pub packets: core::mem::ManuallyDrop<PacketPool>,

    // RX queue state
    pub in_rx: u16,             // Number of buffers currently in RX queue

    // Interrupt state
    pub spurious_intr: bool,
}

// ============================================================================
// Packet pool — owns the packet descriptors and contiguous memory
// ============================================================================

pub struct PacketPool {
    pub packets: *mut Packet,
    pub free_head: u16,
    pub free_count: u16,
    pub recv_head: u16,
    pub recv_tail: u16,
    pub recv_count: u16,
    pub next_free: *mut u16,    // Linked list of free packet indices

    // Contiguous memory
    pub data_vir: *mut u8,
    pub data_phys: u64,
    pub hdrs_vir: *mut VirtioNetHdr,
    pub hdrs_phys: u64,
    pub pool_size: u16,
}

impl PacketPool {
    /// Allocate packet buffers.
    pub fn allocate(count: u16) -> Option<Self> {
        let data_size = (count as usize) * MAX_PACK_SIZE;
        let hdrs_size = (count as usize) * HDR_SIZE;

        // Allocate contiguous memory
        let (data_vir, data_phys) = alloc_contig_pair(data_size)?;
        let (hdrs_vir, hdrs_phys) = alloc_contig_pair(hdrs_size)?;

        // Allocate packet array + free-list
        let pool_size = count as usize;
        let packet_size = core::mem::size_of::<Packet>() * pool_size;
        let free_list_size = core::mem::size_of::<u16>() * pool_size;
        // Use a single contiguous allocation for both
        let total_size = packet_size + free_list_size;
        let (pool_ptr, _pool_phys) = alloc_contig_pair(total_size)?;
        unsafe { core::ptr::write_bytes(pool_ptr, 0, total_size); }

        let packets = pool_ptr as *mut Packet;
        let next_free = unsafe { pool_ptr.add(packet_size) as *mut u16 };

        // Initialise free list: all packets are free initially
        for i in 0..count {
            let idx = i as usize;
            unsafe {
                let pkt = &mut *packets.add(idx);
                pkt.vhdr = (hdrs_vir as *mut u8).add(idx * HDR_SIZE) as *mut VirtioNetHdr;
                pkt.vhdr_phys = hdrs_phys + (idx * HDR_SIZE) as u64;
                pkt.vdata = (data_vir as *mut u8).add(idx * MAX_PACK_SIZE);
                pkt.vdata_phys = data_phys + (idx * MAX_PACK_SIZE) as u64;
                pkt.len = 0;

                // Link into free list
                *next_free.add(idx) = ((i + 1) % count) as u16;
            }
        }

        Some(PacketPool {
            packets,
            free_head: 0,
            free_count: count,
            recv_head: 0,
            recv_tail: 0,
            recv_count: 0,
            next_free,
            data_vir: data_vir as *mut u8,
            data_phys,
            hdrs_vir: hdrs_vir as *mut VirtioNetHdr,
            hdrs_phys,
            pool_size: count,
        })
    }

    /// Allocate a free packet from the pool.
    pub fn alloc(&mut self) -> Option<u16> {
        if self.free_count == 0 { return None; }
        let idx = self.free_head;
        self.free_head = unsafe { *self.next_free.add(idx as usize) };
        self.free_count -= 1;
        Some(idx)
    }

    /// Free a packet back to the pool.
    pub fn free(&mut self, idx: u16) {
        unsafe { *self.next_free.add(idx as usize) = self.free_head; }
        self.free_head = idx;
        self.free_count += 1;
    }

    /// Get a reference to a packet.
    pub fn get(&self, idx: u16) -> &Packet {
        unsafe { &*self.packets.add(idx as usize) }
    }

    /// Get a mutable reference to a packet.
    pub fn get_mut(&mut self, idx: u16) -> &mut Packet {
        unsafe { &mut *self.packets.add(idx as usize) }
    }

    /// Add a packet to the receive queue.
    pub fn push_recv(&mut self, idx: u16) {
        unsafe { *self.next_free.add(idx as usize) = 0xFFFF; } // not in free list
        if self.recv_count == 0 {
            self.recv_head = idx;
            self.recv_tail = idx;
        } else {
            unsafe { *self.next_free.add(self.recv_tail as usize) = idx; }
            self.recv_tail = idx;
        }
        self.recv_count += 1;
    }

    /// Pop a packet from the receive queue.
    pub fn pop_recv(&mut self) -> Option<u16> {
        if self.recv_count == 0 { return None; }
        let idx = self.recv_head;
        self.recv_head = unsafe { *self.next_free.add(idx as usize) };
        self.recv_count -= 1;
        if self.recv_count == 0 {
            self.recv_tail = 0;
        }
        // Mark as freeable
        unsafe { *self.next_free.add(idx as usize) = 0xFFFF; }
        Some(idx)
    }

    /// Free all resources.
    pub fn free_resources(&mut self) {
        let pool_size = self.pool_size as usize;
        let packet_size = core::mem::size_of::<Packet>() * pool_size;
        let free_list_size = core::mem::size_of::<u16>() * pool_size;
        let total_size = packet_size + free_list_size;

        free_contig_wrapper(self.packets as *mut core::ffi::c_void, total_size);
        free_contig_wrapper(self.data_vir as *mut core::ffi::c_void, pool_size * MAX_PACK_SIZE);
        free_contig_wrapper(self.hdrs_vir as *mut core::ffi::c_void, pool_size * HDR_SIZE);

        self.packets = core::ptr::null_mut();
        self.data_vir = core::ptr::null_mut();
        self.hdrs_vir = core::ptr::null_mut();
    }
}

// ============================================================================
// Contiguous memory allocation helpers
// ============================================================================

fn alloc_contig_pair(size: usize) -> Option<(*mut core::ffi::c_void, u64)> {
    #[cfg(target_os = "minix")]
    {
        extern "C" {
            fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut core::ffi::c_void;
        }
        unsafe {
            const AC_ALIGN4K: c_int = 1;
            let mut phys: u64 = 0;
            let ptr = alloc_contig(size, AC_ALIGN4K, &mut phys);
            if ptr.is_null() { None } else { Some((ptr, phys)) }
        }
    }
    #[cfg(not(target_os = "minix"))]
    {
        let _ = size;
        None
    }
}

fn free_contig_wrapper(addr: *mut core::ffi::c_void, size: usize) {
    #[cfg(target_os = "minix")]
    {
        extern "C" {
            fn free_contig(addr: *mut core::ffi::c_void, size: usize);
        }
        unsafe { free_contig(addr, size); }
    }
    #[cfg(not(target_os = "minix"))]
    {
        let _ = (addr, size);
    }
}

// ============================================================================
// VirtioNet implementation
// ============================================================================

impl VirtioNet {
    /// Probe and initialise a virtio-net device.
    pub fn probe_and_init(instance: c_int) -> Option<Self> {
        let (devind, mut dev) = VirtioDevice::probe(net::VIRTIO_NET_PCI_DEVICE_ID, instance)?;

        // Negotiate features
        let features = dev.negotiate_features(net::GUEST_FEATURES);
        let has_mac = (features >> net::VIRTIO_NET_F_MAC) & 1 != 0;
        let has_status = (features >> net::VIRTIO_NET_F_STATUS) & 1 != 0;

        // Read MAC address from config space
        let mut mac = [0u8; 6];
        if has_mac {
            net::read_mac(&dev, &mut mac);
        }

        // Read initial link status
        let link_up = if has_status {
            (net::read_link_status(&dev) & net::VIRTIO_NET_S_LINK_UP) != 0
        } else {
            true // assume up if no status feature
        };

        // We know how to drive it
        unsafe { dev.write8(device::DEV_STATUS_OFF, STATUS_DRV); }

        // Allocate 2 queues (RX + TX)
        if dev.alloc_queues(2).is_err() { return None; }

        // Allocate packet buffers
        let pool = PacketPool::allocate(BUF_PACKETS)?;

        let _ = devind; // Used only for probe, device is now configured via I/O ports

        Some(VirtioNet {
            dev,
            mac,
            link_up,
            features,
            packets: core::mem::ManuallyDrop::new(pool),
            in_rx: 0,
            spurious_intr: false,
        })
    }

    /// Refill the RX queue with free buffers.
    pub fn refill_rx_queue(&mut self) {
        let rx_q = unsafe { &mut *self.dev.queues.add(net::RX_Q) };
        let half = BUF_PACKETS / 2;

        while self.in_rx < half {
            let pkt_idx = match self.packets.alloc() {
                Some(idx) => idx,
                None => break,
            };

            let pkt = self.packets.get(pkt_idx);
            let desc_count: u16 = 2; // virtio-net header + data

            let head = match rx_q.alloc_descs(desc_count) {
                Some(h) => h,
                None => {
                    self.packets.free(pkt_idx);
                    break;
                }
            };

            // Descriptor 0: virtio-net header (device writes)
            let d0 = head;
            rx_q.set_desc(d0, pkt.vhdr_phys, HDR_SIZE as u32, true, true);

            // Descriptor 1: payload (device writes)
            let d1 = (d0 + 1) % rx_q.num;
            rx_q.set_desc(d1, pkt.vdata_phys, MAX_PACK_SIZE as u32, true, false);

            // Submit to available ring
            rx_q.submit(head, pkt_idx as usize);
            self.in_rx += 1;
        }
    }

    /// Process the used rings for both RX and TX queues.
    pub fn check_queues(&mut self) {
        let rx_q = unsafe { &mut *self.dev.queues.add(net::RX_Q) };

        // Collect received packets
        loop {
            match rx_q.collect() {
                Some((data, len)) => {
                    let pkt_idx = data as u16;
                    {
                        let pkt = self.packets.get_mut(pkt_idx);
                        pkt.len = len as usize;
                    }
                    self.packets.push_recv(pkt_idx);
                    self.in_rx -= 1;
                }
                None => break,
            }
        }

        // Collect completed TX (just free the buffer)
        let tx_q = unsafe { &mut *self.dev.queues.add(net::TX_Q) };
        loop {
            match tx_q.collect() {
                Some((data, _len)) => {
                    let pkt_idx = data as u16;
                    {
                        let pkt = self.packets.get_mut(pkt_idx);
                        // Zero out the header for safety
                        unsafe { core::ptr::write_bytes(pkt.vhdr as *mut u8, 0, HDR_SIZE); }
                        unsafe { core::ptr::write_bytes(pkt.vdata, 0, MAX_PACK_SIZE); }
                    }
                    self.packets.free(pkt_idx);
                }
                None => break,
            }
        }
    }

    /// Send a packet. Returns OK or SUSPEND if no free buffers.
    pub fn send(&mut self, data: *mut ffi::NetdriverData, size: usize) -> c_int {
        let pkt_idx = match self.packets.alloc() {
            Some(idx) => idx,
            None => return ffi::SUSPEND,
        };

        let pkt = self.packets.get(pkt_idx);
        if size > MAX_PACK_SIZE {
            self.packets.free(pkt_idx);
            return ffi::EINVAL;
        }

        // Copy packet data
        ffi::netdriver_copyin_ffi(data, 0, pkt.vdata as *const core::ffi::c_void, size);

        // Zero out the virtio-net header
        unsafe { core::ptr::write_bytes(pkt.vhdr as *mut u8, 0, HDR_SIZE); }

        let tx_q = unsafe { &mut *self.dev.queues.add(net::TX_Q) };
        let desc_count: u16 = 2; // header + data

        let head = match tx_q.alloc_descs(desc_count) {
            Some(h) => h,
            None => {
                self.packets.free(pkt_idx);
                return ffi::SUSPEND;
            }
        };

        // Descriptor 0: virtio-net header (device reads)
        let d0 = head;
        tx_q.set_desc(d0, pkt.vhdr_phys, HDR_SIZE as u32, false, true);

        // Descriptor 1: packet data (device reads)
        let d1 = (d0 + 1) % tx_q.num;
        tx_q.set_desc(d1, pkt.vdata_phys, size as u32, false, false);

        // Submit and kick
        tx_q.submit(head, pkt_idx as usize);
        unsafe { self.dev.kick(net::TX_Q as u16); }

        ffi::OK
    }

    /// Receive a packet. Returns size or SUSPEND if none available.
    pub fn recv(&mut self, data: *mut ffi::NetdriverData, max: usize) -> isize {
        let pkt_idx = match self.packets.pop_recv() {
            Some(idx) => idx,
            None => return ffi::SUSPEND as isize,
        };

        let pkt = self.packets.get(pkt_idx);

        // Strip the virtio-net header from the received length
        if pkt.len < HDR_SIZE {
            // Bogus packet — discard
            self.packets.free(pkt_idx);
            self.refill_rx_queue();
            return ffi::SUSPEND as isize;
        }

        let payload_len = pkt.len - HDR_SIZE;
        let copy_len = if payload_len > max { max } else { payload_len };

        // Copy out packet data (skip the virtio-net header)
        let payload_ptr = unsafe { (pkt.vdata as *mut u8).add(HDR_SIZE) };
        ffi::netdriver_copyout_ffi(data, 0, payload_ptr as *const core::ffi::c_void, copy_len);

        // Return buffer to free pool
        // (Don't zero the header/data here — the TX completion handler will do it)
        // For RX, just reset the header
        unsafe { core::ptr::write_bytes(pkt.vhdr as *mut u8, 0, HDR_SIZE); }
        self.packets.free(pkt_idx);

        // Ensure there are enough RX buffers
        self.refill_rx_queue();

        let ret_len = if payload_len < 60 { 60 } else { payload_len }; // min ethernet padding
        if copy_len < ret_len { ret_len as isize } else { copy_len as isize }
    }

    /// Handle an interrupt.
    pub fn handle_intr(&mut self) {
        if self.dev.had_irq() {
            self.check_queues();
        } else {
            if !self.spurious_intr {
                self.spurious_intr = true;
            }
        }

        // Notify the netdriver framework about pending work
        if self.packets.recv_count > 0 {
            ffi::netdriver_recv_ffi();
        }
        if self.packets.free_count > 0 {
            ffi::netdriver_send_ffi();
        }

        self.dev.irq_reenable();
        self.refill_rx_queue();
    }

    /// Stop the driver and clean up.
    pub fn stop(&mut self) {
        // Reset device
        self.dev.reset();

        // Free packet pool resources before dropping the ManuallyDrop wrapper.
        // ManuallyDrop prevents automatic Drop, so we must free manually.
        self.packets.free_resources();

        // Free queues and device
        self.dev.cleanup();
    }

    /// Get link status.
    pub fn get_link(&self) -> (u32, u32) {
        if self.link_up { (1, 0) } else { (0, 0) }
    }
}

impl Drop for VirtioNet {
    fn drop(&mut self) {
        self.dev.reset();
        self.dev.cleanup();
        // PacketPool is ManuallyDrop, so it won't be dropped automatically
    }
}
