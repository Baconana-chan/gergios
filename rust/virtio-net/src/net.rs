//! # Virtio-net Protocol Definitions
//!
//! Feature bits, config space layout, and virtio-net header structures
//! for the virtio-net device type (legacy, pre-1.0).

#![allow(dead_code)]

use crate::device::VirtioDevice;

// ============================================================================
// PCI Device ID
// ============================================================================

/// virtio-net transitional subsystem device ID (virtio type number)
/// PCI device ID at offset 0x02 is 0x1000, but subsystem device ID
/// at offset 0x2E is 0x0001 (type 1 = network controller).
/// The probe function reads the subsystem ID to identify the device type.
pub const VIRTIO_NET_PCI_DEVICE_ID: u16 = 0x0001;

// ============================================================================
// Feature bits
// ============================================================================

pub const VIRTIO_NET_F_CSUM: u32 = 0;       // Host handles pkts w/ partial csum
pub const VIRTIO_NET_F_GUEST_CSUM: u32 = 1; // Guest handles pkts w/ partial csum
pub const VIRTIO_NET_F_MAC: u32 = 5;        // Host has given MAC address
pub const VIRTIO_NET_F_GSO: u32 = 6;        // Host handles pkts w/ any GSO type
pub const VIRTIO_NET_F_GUEST_TSO4: u32 = 7; // Guest can handle TSOv4 in
pub const VIRTIO_NET_F_GUEST_TSO6: u32 = 8; // Guest can handle TSOv6 in
pub const VIRTIO_NET_F_GUEST_ECN: u32 = 9;  // Guest can handle TSO[6] w/ ECN
pub const VIRTIO_NET_F_GUEST_UFO: u32 = 10; // Guest can handle UFO in
pub const VIRTIO_NET_F_HOST_TSO4: u32 = 11; // Host can handle TSOv4 in
pub const VIRTIO_NET_F_HOST_TSO6: u32 = 12; // Host can handle TSOv6 in
pub const VIRTIO_NET_F_HOST_ECN: u32 = 13;  // Host can handle TSO[6] w/ ECN
pub const VIRTIO_NET_F_HOST_UFO: u32 = 14;  // Host can handle UFO in
pub const VIRTIO_NET_F_MRG_RXBUF: u32 = 15; // Host can merge receive buffers
pub const VIRTIO_NET_F_STATUS: u32 = 16;    // Config status field available
pub const VIRTIO_NET_F_CTRL_VQ: u32 = 17;   // Control channel available
pub const VIRTIO_NET_F_CTRL_RX: u32 = 18;   // Control RX mode support
pub const VIRTIO_NET_F_CTRL_VLAN: u32 = 19; // Control VLAN filtering
pub const VIRTIO_NET_F_GUEST_ANNOUNCE: u32 = 21; // Guest can announce device

// ============================================================================
// Status flags (from config space status field)
// ============================================================================

pub const VIRTIO_NET_S_LINK_UP: u16 = 1;  // Link is up
pub const VIRTIO_NET_S_ANNOUNCE: u16 = 2; // Announcement is needed

// ============================================================================
// Config space offsets (from DEV_SPECIFIC, i.e. 0x14 on legacy I/O BAR)
// ============================================================================

const CFG_MAC: u16 = 0;       // 6 bytes MAC address
const CFG_STATUS: u16 = 6;    // u16_t link status

// ============================================================================
// virtio-net header (prepended to each packet in virtqueue descriptors)
// ============================================================================

/// virtio-net header — MUST be the first element of the RX/TX scatter-gather list.
/// If you don't specify GSO or CSUM features, you can simply zero-initialise it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioNetHdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,      // Ethernet + IP + TCP/UDP headers
    pub gso_size: u16,     // Bytes to append per frame (MSS)
    pub csum_start: u16,   // Position to start checksumming from
    pub csum_offset: u16,  // Offset after csum_start to place checksum
}

pub const VIRTIO_NET_HDR_F_NEEDS_CSUM: u8 = 1;  // Use csum_start, csum_offset
pub const VIRTIO_NET_HDR_F_DATA_VALID: u8 = 2;   // Csum is valid

pub const VIRTIO_NET_HDR_GSO_NONE: u8 = 0;       // Not a GSO frame
pub const VIRTIO_NET_HDR_GSO_TCPV4: u8 = 1;      // GSO frame, IPv4 TCP (TSO)
pub const VIRTIO_NET_HDR_GSO_UDP: u8 = 3;        // GSO frame, IPv4 UDP (UFO)
pub const VIRTIO_NET_HDR_GSO_TCPV6: u8 = 4;      // GSO frame, IPv6 TCP
pub const VIRTIO_NET_HDR_GSO_ECN: u8 = 0x80;     // TCP has ECN set

/// virtio-net header with mergeable RX buffer support (if MRG_RXBUF negotiated)
#[repr(C)]
pub struct VirtioNetHdrMrgRxbuf {
    pub hdr: VirtioNetHdr,
    pub num_buffers: u16, // Number of merged RX buffers
}

// ============================================================================
// Feature negotiation
// ============================================================================

/// Features we want to negotiate with the host.
/// Keep it minimal for the pilot: MAC, status, CSUM offload.
pub const GUEST_FEATURES: u32 =
    (1 << VIRTIO_NET_F_MAC)
    | (1 << VIRTIO_NET_F_STATUS)
    | (1 << VIRTIO_NET_F_CSUM)
    | (1 << VIRTIO_NET_F_GUEST_CSUM);

// ============================================================================
// Queue indices
// ============================================================================

pub const RX_Q: usize = 0;
pub const TX_Q: usize = 1;
pub const CTRL_Q: usize = 2;

// ============================================================================
// Config space accessors
// ============================================================================

/// Read MAC address from device config space.
pub fn read_mac(dev: &VirtioDevice, mac: &mut [u8; 6]) {
    unsafe {
        mac[0] = dev.sread8(CFG_MAC);
        mac[1] = dev.sread8(CFG_MAC + 1);
        mac[2] = dev.sread8(CFG_MAC + 2);
        mac[3] = dev.sread8(CFG_MAC + 3);
        mac[4] = dev.sread8(CFG_MAC + 4);
        mac[5] = dev.sread8(CFG_MAC + 5);
    }
}

/// Read link status from device config space.
pub fn read_link_status(dev: &VirtioDevice) -> u16 {
    unsafe { dev.sread16(CFG_STATUS) }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_sizes() {
        assert_eq!(core::mem::size_of::<VirtioNetHdr>(), 10);
        assert_eq!(core::mem::size_of::<VirtioNetHdrMrgRxbuf>(), 12);
    }

    #[test]
    fn feature_constants() {
        assert_eq!(VIRTIO_NET_F_MAC, 5);
        assert_eq!(VIRTIO_NET_F_STATUS, 16);
        assert_eq!(VIRTIO_NET_F_CTRL_VQ, 17);
    }
}
