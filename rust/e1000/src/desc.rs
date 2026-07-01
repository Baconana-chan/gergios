//! # E1000 Descriptor Structures
//!
//! Legacy receive and transmit descriptor formats
//! for Intel PRO/1000 Gigabit Ethernet controllers.

// ============================================================================
// Receive Descriptor
// ============================================================================

/// Legacy receive descriptor (16 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RxDesc {
    /// Address of the receive data buffer (low 32 bits)
    pub buffer: u32,
    /// High 32 bits of buffer address (unused on 32-bit)
    pub buffer_hi: u32,
    /// Size of the received data
    pub length: u16,
    /// Packet checksum
    pub checksum: u16,
    /// Descriptor status
    pub status: u8,
    /// Descriptor errors
    pub errors: u8,
    /// VLAN / special info
    pub special: u16,
}

/// Receive Status bits
pub const RX_STATUS_DONE: u8 = 1 << 0;  // Descriptor done
pub const RX_STATUS_EOP: u8 = 1 << 1;  // End of packet
pub const RX_STATUS_PIF: u8 = 1 << 7;  // Passed in-exact filter

/// Receive Error bits
pub const RX_ERROR_CE: u8 = 1 << 0;   // CRC/Alignment error
pub const RX_ERROR_SEQ: u8 = 1 << 2;  // Sequence/Framing error
pub const RX_ERROR_CXE: u8 = 1 << 4;  // Carrier extension error
pub const RX_ERROR_RXE: u8 = 1 << 7;  // RX data error

// ============================================================================
// Transmit Descriptor
// ============================================================================

/// Legacy transmit descriptor (16 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TxDesc {
    /// Address of the transmit buffer (low 32 bits)
    pub buffer: u32,
    /// High 32 bits of buffer address (unused on 32-bit)
    pub buffer_hi: u32,
    /// Size of the data to transmit
    pub length: u16,
    /// Checksum offset
    pub cso: u8,
    /// Command field
    pub cmd: u8,
    /// Status field
    pub status: u8,
    /// Checksum start
    pub css: u8,
    /// VLAN / special info
    pub special: u16,
}

/// Transmit Command bits
pub const TX_CMD_EOP: u8 = 1 << 0;  // End of packet
pub const TX_CMD_FCS: u8 = 1 << 1;  // Insert FCS/CRC
pub const TX_CMD_RS: u8 = 1 << 3;   // Report status

/// Transmit Status bits
pub const TX_STATUS_DONE: u8 = 1 << 0;  // Descriptor done

// ============================================================================
// Descriptor Constants
// ============================================================================

/// Null pointer for TBD Array (legacy)
pub const TX_TBDA_NIL: u32 = 0xFFFFFFFF;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rx_desc_size() {
        assert_eq!(core::mem::size_of::<RxDesc>(), 16);
        assert_eq!(core::mem::align_of::<RxDesc>(), 4);
    }

    #[test]
    fn tx_desc_size() {
        assert_eq!(core::mem::size_of::<TxDesc>(), 16);
        assert_eq!(core::mem::align_of::<TxDesc>(), 4);
    }
}
