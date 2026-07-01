//! # E1000 PCI Device IDs
//!
//! PCI vendor/device ID table for Intel PRO/1000 controllers.

/// Intel PCI Vendor ID
pub const VENDOR_INTEL: u16 = 0x8086;

// ============================================================================
// Device IDs
// ============================================================================

pub const DEV_82542: u16 = 0x1000;
pub const DEV_82543GC_FIBER: u16 = 0x1001;
pub const DEV_82543GC_COPPER: u16 = 0x1004;
pub const DEV_82544EI_COPPER: u16 = 0x1008;
pub const DEV_82544EI_FIBER: u16 = 0x1009;
pub const DEV_82544GC_COPPER: u16 = 0x100C;
pub const DEV_82544GC_LOM: u16 = 0x100D;
pub const DEV_82540EM: u16 = 0x100E;
pub const DEV_82545EM: u16 = 0x100F;
pub const DEV_82546EB_COPPER: u16 = 0x1010;
pub const DEV_82545EM_FIBER: u16 = 0x1011;
pub const DEV_82546EB_FIBER: u16 = 0x1012;
pub const DEV_82541EI: u16 = 0x1013;
pub const DEV_82541ER_LOM: u16 = 0x1014;
pub const DEV_82540EM_LOM: u16 = 0x1015;
pub const DEV_82540EP_LOM: u16 = 0x1016;
pub const DEV_82540EP: u16 = 0x1017;
pub const DEV_82541EI_MOBILE: u16 = 0x1018;
pub const DEV_82547EI: u16 = 0x1019;
pub const DEV_82547EI_MOBILE: u16 = 0x101A;
pub const DEV_82546EB_QUAD_COPPER: u16 = 0x101D;
pub const DEV_82540EP_LP: u16 = 0x101E;
pub const DEV_82545GM_COPPER: u16 = 0x1026;
pub const DEV_82545GM_FIBER: u16 = 0x1027;
pub const DEV_82545GM_SERDES: u16 = 0x1028;
pub const DEV_82541GI: u16 = 0x1076;
pub const DEV_82541GI_MOBILE: u16 = 0x1077;
pub const DEV_82541ER: u16 = 0x1078;
pub const DEV_82546GB_COPPER: u16 = 0x1079;
pub const DEV_82546GB_FIBER: u16 = 0x107A;
pub const DEV_82546GB_SERDES: u16 = 0x107B;
pub const DEV_82541GI_LF: u16 = 0x107C;
pub const DEV_82572EI_COPPER: u16 = 0x107D;
pub const DEV_82572EI_FIBER: u16 = 0x107E;
pub const DEV_82572EI_SERDES: u16 = 0x107F;
pub const DEV_82547GI: u16 = 0x1075;
pub const DEV_82546GB_PCIE: u16 = 0x108A;
pub const DEV_82573E: u16 = 0x108B;
pub const DEV_82573E_IAMT: u16 = 0x108C;
pub const DEV_82573L: u16 = 0x109A;
pub const DEV_82572EI: u16 = 0x10B9;
pub const DEV_82546GB_QUAD_COPPER: u16 = 0x1099;
pub const DEV_80003ES2LAN_COPPER_DPT: u16 = 0x1096;
pub const DEV_80003ES2LAN_SERDES_DPT: u16 = 0x1098;
pub const DEV_80003ES2LAN_COPPER_SPT: u16 = 0x10BA;
pub const DEV_80003ES2LAN_SERDES_SPT: u16 = 0x10BB;
pub const DEV_82546GB_QUAD_COPPER_KSP3: u16 = 0x10B5;
pub const DEV_82571EB_COPPER: u16 = 0x105E;
pub const DEV_82571EB_FIBER: u16 = 0x105F;
pub const DEV_82571EB_SERDES: u16 = 0x1060;
pub const DEV_82571EB_QUAD_COPPER: u16 = 0x10A4;
pub const DEV_82571EB_QUAD_FIBER: u16 = 0x10A5;
pub const DEV_82571EB_QUAD_COPPER_LP: u16 = 0x10BC;
pub const DEV_82571PT_QUAD_COPPER: u16 = 0x10D5;
pub const DEV_82571EB_SERDES_DUAL: u16 = 0x10D9;
pub const DEV_82571EB_SERDES_QUAD: u16 = 0x10DA;
pub const DEV_82575EB_COPPER: u16 = 0x10A7;
pub const DEV_82575EB_FIBER_SERDES: u16 = 0x10A9;
pub const DEV_82575GB_QUAD_COPPER: u16 = 0x10D6;
pub const DEV_82575GB_QUAD_COPPER_PM: u16 = 0x10E2;
pub const DEV_82576: u16 = 0x10C9;
pub const DEV_82576_FIBER: u16 = 0x10E6;
pub const DEV_82576_SERDES: u16 = 0x10E7;
pub const DEV_82576_QUAD_COPPER: u16 = 0x10E8;
pub const DEV_82576_NS: u16 = 0x150A;
pub const DEV_82576_SERDES_QUAD: u16 = 0x150D;
pub const DEV_82574L: u16 = 0x10D3;
pub const DEV_82574LA: u16 = 0x10F6;
pub const DEV_82583V: u16 = 0x150C;
pub const DEV_ICH8_IGP_M_AMT: u16 = 0x1049;
pub const DEV_ICH8_IGP_AMT: u16 = 0x104A;
pub const DEV_ICH8_IGP_C: u16 = 0x104B;
pub const DEV_ICH8_IFE: u16 = 0x104C;
pub const DEV_ICH8_IFE_GT: u16 = 0x10C4;
pub const DEV_ICH8_IFE_G: u16 = 0x10C5;
pub const DEV_ICH8_IGP_M: u16 = 0x104D;
pub const DEV_ICH9_IGP_M: u16 = 0x10BF;
pub const DEV_ICH9_IGP_M_AMT: u16 = 0x10F5;
pub const DEV_ICH9_IGP_M_V: u16 = 0x10CB;
pub const DEV_ICH9_IGP_AMT: u16 = 0x10BD;
pub const DEV_ICH9_BM: u16 = 0x10E5;
pub const DEV_ICH9_IGP_C: u16 = 0x294C;
pub const DEV_ICH9_IFE: u16 = 0x10C0;
pub const DEV_ICH9_IFE_GT: u16 = 0x10C3;
pub const DEV_ICH9_IFE_G: u16 = 0x10C2;
pub const DEV_ICH10_R_BM_LM: u16 = 0x10CC;
pub const DEV_ICH10_R_BM_LF: u16 = 0x10CD;
pub const DEV_ICH10_R_BM_V: u16 = 0x10CE;
pub const DEV_ICH10_D_BM_LM: u16 = 0x10DE;
pub const DEV_ICH10_D_BM_LF: u16 = 0x10DF;
pub const DEV_PCH_M_HV_LM: u16 = 0x10EA;
pub const DEV_PCH_M_HV_LC: u16 = 0x10EB;
pub const DEV_PCH_D_HV_DM: u16 = 0x10EF;
pub const DEV_PCH_D_HV_DC: u16 = 0x10F0;

// ============================================================================
// EEPROM variant table
// ============================================================================

/// EEPROM type for different device families
#[derive(Clone, Copy, PartialEq)]
pub enum EepromType {
    /// Standard EERD register (most 8254x/8257x)
    Eerd,
    /// ICH8 flash-based (82566/82567/ICH8-ICH10)
    Ich8,
}

/// Device-specific EEPROM configuration
pub struct EepromConfig {
    pub eeprom_type: EepromType,
    pub done_bit: u32,     // bit position for EERD.DONE
    pub addr_off: u32,     // bit offset for EERD address
}

/// Returns EEPROM configuration for a given device ID.
pub fn eeprom_config(did: u16) -> EepromConfig {
    match did {
        DEV_ICH10_D_BM_LM | DEV_ICH10_D_BM_LF => {
            EepromConfig {
                eeprom_type: EepromType::Ich8,
                done_bit: 0,
                addr_off: 0,
            }
        }
        DEV_82540EM | DEV_82545EM | DEV_82540EP_LP => {
            EepromConfig {
                eeprom_type: EepromType::Eerd,
                done_bit: 1 << 4,
                addr_off: 8,
            }
        }
        _ => {
            EepromConfig {
                eeprom_type: EepromType::Eerd,
                done_bit: 1 << 1,
                addr_off: 2,
            }
        }
    }
}

/// Check if a device/vendor pair matches an e1000 controller.
pub fn is_e1000(vid: u16, did: u16) -> bool {
    if vid != VENDOR_INTEL { return false; }
    // All device IDs defined above are e1000 controllers
    matches!(did,
        DEV_82542 | DEV_82543GC_FIBER | DEV_82543GC_COPPER |
        DEV_82544EI_COPPER | DEV_82544EI_FIBER | DEV_82544GC_COPPER |
        DEV_82544GC_LOM | DEV_82540EM | DEV_82545EM |
        DEV_82546EB_COPPER | DEV_82545EM_FIBER | DEV_82546EB_FIBER |
        DEV_82541EI | DEV_82541ER_LOM | DEV_82540EM_LOM |
        DEV_82540EP_LOM | DEV_82540EP | DEV_82541EI_MOBILE |
        DEV_82547EI | DEV_82547EI_MOBILE | DEV_82546EB_QUAD_COPPER |
        DEV_82540EP_LP | DEV_82545GM_COPPER | DEV_82545GM_FIBER |
        DEV_82545GM_SERDES | DEV_82541GI | DEV_82541GI_MOBILE |
        DEV_82541ER | DEV_82546GB_COPPER | DEV_82546GB_FIBER |
        DEV_82546GB_SERDES | DEV_82541GI_LF | DEV_82572EI_COPPER |
        DEV_82572EI_FIBER | DEV_82572EI_SERDES | DEV_82547GI |
        DEV_82546GB_PCIE | DEV_82573E | DEV_82573E_IAMT |
        DEV_82573L | DEV_82572EI | DEV_82546GB_QUAD_COPPER |
        DEV_80003ES2LAN_COPPER_DPT | DEV_80003ES2LAN_SERDES_DPT |
        DEV_80003ES2LAN_COPPER_SPT | DEV_80003ES2LAN_SERDES_SPT |
        DEV_82546GB_QUAD_COPPER_KSP3 | DEV_82571EB_COPPER |
        DEV_82571EB_FIBER | DEV_82571EB_SERDES |
        DEV_82571EB_QUAD_COPPER | DEV_82571EB_QUAD_FIBER |
        DEV_82571EB_QUAD_COPPER_LP | DEV_82571PT_QUAD_COPPER |
        DEV_82571EB_SERDES_DUAL | DEV_82571EB_SERDES_QUAD |
        DEV_82575EB_COPPER | DEV_82575EB_FIBER_SERDES |
        DEV_82575GB_QUAD_COPPER | DEV_82575GB_QUAD_COPPER_PM |
        DEV_82576 | DEV_82576_FIBER | DEV_82576_SERDES |
        DEV_82576_QUAD_COPPER | DEV_82576_NS | DEV_82576_SERDES_QUAD |
        DEV_82574L | DEV_82574LA | DEV_82583V |
        DEV_ICH8_IGP_M_AMT | DEV_ICH8_IGP_AMT | DEV_ICH8_IGP_C |
        DEV_ICH8_IFE | DEV_ICH8_IFE_GT | DEV_ICH8_IFE_G |
        DEV_ICH8_IGP_M | DEV_ICH9_IGP_M | DEV_ICH9_IGP_M_AMT |
        DEV_ICH9_IGP_M_V | DEV_ICH9_IGP_AMT | DEV_ICH9_BM |
        DEV_ICH9_IGP_C | DEV_ICH9_IFE | DEV_ICH9_IFE_GT |
        DEV_ICH9_IFE_G | DEV_ICH10_R_BM_LM | DEV_ICH10_R_BM_LF |
        DEV_ICH10_R_BM_V | DEV_ICH10_D_BM_LM | DEV_ICH10_D_BM_LF |
        DEV_PCH_M_HV_LM | DEV_PCH_M_HV_LC | DEV_PCH_D_HV_DM |
        DEV_PCH_D_HV_DC
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_device_recognized() {
        assert!(is_e1000(VENDOR_INTEL, DEV_82540EM));
        assert!(is_e1000(VENDOR_INTEL, DEV_82574L));
        assert!(is_e1000(VENDOR_INTEL, DEV_82576));
    }

    #[test]
    fn non_e1000_not_recognized() {
        assert!(!is_e1000(0x10EC, 0x8168)); // Realtek
        assert!(!is_e1000(0x8086, 0x0002)); // virtio-blk
    }

    #[test]
    fn eeprom_config_variants() {
        let cfg = eeprom_config(DEV_82540EM);
        assert!(matches!(cfg.eeprom_type, EepromType::Eerd));
        assert_eq!(cfg.done_bit, 1 << 4);
        assert_eq!(cfg.addr_off, 8);

        let cfg = eeprom_config(DEV_ICH10_D_BM_LM);
        assert!(matches!(cfg.eeprom_type, EepromType::Ich8));

        let cfg = eeprom_config(DEV_82574L);
        assert!(matches!(cfg.eeprom_type, EepromType::Eerd));
        assert_eq!(cfg.done_bit, 1 << 1);
        assert_eq!(cfg.addr_off, 2);
    }
}
