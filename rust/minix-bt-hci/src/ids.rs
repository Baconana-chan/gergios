//! # Bluetooth Device ID Table
//!
//! Known USB Bluetooth adapter vendor/product IDs for which the HCI transport
//! has been tested or has known quirks/workarounds.
//!
//! Format: (vendor_id, product_id, name, quirks_bitmask)

#![allow(dead_code)]

/// Quirk flags for specific BT adapters.
#[repr(u16)]
pub enum BtQuirk {
    /// No special handling needed.
    None              = 0x0000,
    /// Requires a special baud rate change after firmware download (Broadcom).
    BcmBaudChange     = 0x0001,
    /// Requires loading firmware before first command (Broadcom/Intel).
    NeedFirmware      = 0x0002,
    /// Has broken isochronous support (claim SCO but doesn't work).
    BrokenSco         = 0x0004,
    /// Requires USB reset after firmware load (MediaTek).
    NeedResetAfterFw  = 0x0008,
    /// Supports Bluetooth 5.2+ ISO data.
    HasIsoData        = 0x0010,
    /// Requires Write_SSP_Debug_Mode to enable all features (Intel).
    IntelDebug        = 0x0020,
    /// Has RTL specific HCI extension commands (Realtek).
    RtlExtensions     = 0x0040,
    /// Fake/Jerry adapter — limited functionality.
    FakeAdapter       = 0x0080,
}

/// Entry in the BT device ID table.
#[derive(Clone, Copy)]
pub struct BtDeviceEntry {
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: &'static str,
    pub quirks: u16,
}

/// Known Bluetooth adapter IDs.
pub const BT_DEVICE_TABLE: &[BtDeviceEntry] = &[
    // =========================================================================
    // Broadcom (BCM)
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x0A5C, product_id: 0x21E8, name: "BCM20702A1", quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0A5C, product_id: 0x21EC, name: "BCM20702A0", quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0A5C, product_id: 0x216F, name: "BCM43142A0", quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0A5C, product_id: 0x21D7, name: "BCM4330",    quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0A5C, product_id: 0x22BE, name: "BCM2076B1",  quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0A5C, product_id: 0x21F1, name: "BCM2045A0",  quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0489, product_id: 0xE07A, name: "BCM4324B3",  quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0489, product_id: 0xE0CD, name: "BCM4350C5",  quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x413C, product_id: 0x8143, name: "DW1560 (BCM20702)", quirks: BtQuirk::BcmBaudChange as u16 | BtQuirk::NeedFirmware as u16 },

    // =========================================================================
    // Intel
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0025, name: "Intel Wireless-AC 7260", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0026, name: "Intel Wireless-AC 3160", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0A2A, name: "Intel Wireless-AC 8260", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0A2B, name: "Intel Wireless-AC 8265", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0AAA, name: "Intel Wireless-AC 9560", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0AA8, name: "Intel AX200 (Cyborg)",  quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0029, name: "Intel AX201 (Jefferson Peak)", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0032, name: "Intel AX210 (Garrison Peak)", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },
    BtDeviceEntry { vendor_id: 0x8087, product_id: 0x0033, name: "Intel AX211", quirks: BtQuirk::IntelDebug as u16 | BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },

    // =========================================================================
    // Realtek
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0x8761, name: "RTL8761BUV (BT 5.0)", quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0x8771, name: "RTL8771BUV (BT 5.1)", quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0x8822, name: "RTL8822CE",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0xC822, name: "RTL8822CU",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0xB023, name: "RTL8821CE",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0x4853, name: "RTL8822BU",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0x2B89, name: "RTL8723BE",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0xB720, name: "RTL8723BU",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0xD723, name: "RTL8723DE",            quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0BDA, product_id: 0x5B71, name: "RTL8188EU (BT combo)", quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x10B7, product_id: 0xB728, name: "RTL8723DS (Ampak)",   quirks: BtQuirk::RtlExtensions as u16 | BtQuirk::NeedFirmware as u16 },

    // =========================================================================
    // Qualcomm / Atheros
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x0CF3, product_id: 0xE300, name: "Qualcomm QCA6174A (WCN3680B)", quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0489, product_id: 0xE0A2, name: "QCA9377 (WCN3610)", quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0CF3, product_id: 0xE005, name: "AR3012 (Atheros)",  quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0CF3, product_id: 0x3004, name: "AR3011 (Atheros)",  quirks: 0 },
    BtDeviceEntry { vendor_id: 0x0489, product_id: 0xE036, name: "QCA61x4A",          quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0489, product_id: 0xE0B0, name: "QCA6390 (WCN685x)", quirks: BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },
    BtDeviceEntry { vendor_id: 0x0489, product_id: 0xE0C5, name: "QCA6696 (WCN6856)", quirks: BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },

    // =========================================================================
    // MediaTek
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x0E8D, product_id: 0x7921, name: "MT7921 (MT7961)", quirks: BtQuirk::NeedResetAfterFw as u16 | BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },
    BtDeviceEntry { vendor_id: 0x0E8D, product_id: 0x7922, name: "MT7922 (Filogic 330)", quirks: BtQuirk::NeedResetAfterFw as u16 | BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },
    BtDeviceEntry { vendor_id: 0x0E8D, product_id: 0x7663, name: "MT7663U", quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x0E8D, product_id: 0x7668, name: "MT7668U", quirks: BtQuirk::NeedFirmware as u16 | BtQuirk::HasIsoData as u16 },

    // =========================================================================
    // CSR (Cambridge Silicon Radio)
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x0A12, product_id: 0x0001, name: "CSR BlueCore4",    quirks: 0 },
    BtDeviceEntry { vendor_id: 0x0A12, product_id: 0x1000, name: "CSR BlueCore5",    quirks: 0 },
    BtDeviceEntry { vendor_id: 0x0A12, product_id: 0x0100, name: "CSR8510 A10",      quirks: 0 },
    BtDeviceEntry { vendor_id: 0x0A12, product_id: 0x0101, name: "CSR8510 A12",      quirks: 0 },

    // =========================================================================
    // Generic / Other
    // =========================================================================
    BtDeviceEntry { vendor_id: 0x1131, product_id: 0x1001, name: "Integrated System Solution (ISSC)", quirks: 0 },
    BtDeviceEntry { vendor_id: 0x13D3, product_id: 0x3402, name: "Azurewave (BCM-based)", quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x13D3, product_id: 0x3432, name: "Azurewave (Intel-based)", quirks: BtQuirk::NeedFirmware as u16 | BtQuirk::IntelDebug as u16 },
    BtDeviceEntry { vendor_id: 0x0930, product_id: 0x0225, name: "Toshiba BT Stack (generic)", quirks: 0 },
    BtDeviceEntry { vendor_id: 0x04CA, product_id: 0x2006, name: "Lite-On (Broadcom based)", quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x04CA, product_id: 0x3014, name: "Lite-On (MediaTek)", quirks: BtQuirk::NeedFirmware as u16 },
    BtDeviceEntry { vendor_id: 0x1CF1, product_id: 0x0030, name: "Dell DW5821e (QCA)", quirks: BtQuirk::NeedFirmware as u16 },
];

/// Look up a BT device entry by vendor/product ID.
pub fn lookup_bt_device(vendor_id: u16, product_id: u16) -> Option<&'static BtDeviceEntry> {
    BT_DEVICE_TABLE.iter().find(|e| e.vendor_id == vendor_id && e.product_id == product_id)
}

/// Check if a device has a specific quirk.
pub fn has_quirk(entry: &BtDeviceEntry, quirk: BtQuirk) -> bool {
    (entry.quirks & (quirk as u16)) != 0
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_intel_ax200() {
        let entry = lookup_bt_device(0x8087, 0x0AA8);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "Intel AX200 (Cyborg)");
    }

    #[test]
    fn test_lookup_csr() {
        let entry = lookup_bt_device(0x0A12, 0x0001);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "CSR BlueCore4");
    }

    #[test]
    fn test_lookup_unknown() {
        assert!(lookup_bt_device(0xDEAD, 0xBEEF).is_none());
    }

    #[test]
    fn test_quirks() {
        let entry = lookup_bt_device(0x8087, 0x0AA8).unwrap();
        assert!(has_quirk(entry, BtQuirk::IntelDebug));
        assert!(has_quirk(entry, BtQuirk::NeedFirmware));
        assert!(has_quirk(entry, BtQuirk::HasIsoData));
        assert!(!has_quirk(entry, BtQuirk::BrokenSco));
    }

    #[test]
    fn test_table_completeness() {
        // Every entry should have a non-empty name
        for entry in BT_DEVICE_TABLE {
            assert!(!entry.name.is_empty());
        }
    }
}
