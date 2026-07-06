//! # Registers — xHCI (USB 3.0) Controller Register Definitions
//!
//! From eXtensible Host Controller Interface for Universal Serial Bus
//! (xHCI) Specification, Rev 1.2.
//!
//! Register spaces:
//!   - CAPREG (0x00..): Capability registers (CAPLENGTH, HCIVERSION, HCSPARAMS, etc.)
//!   - OPREG (CAPLENGTH..): Operational registers (USBCMD, USBSTS, CRCR, DCBAAP, CONFIG, PORTRSC)
//!   - RTSOFF: Runtime registers (IMAN, IMOD, ERSTSZ, ERSTBA, ERDP per interrupter)
//!   - DBOFF: Doorbell registers (per device slot)

#![allow(dead_code)]

// ============================================================================
// PCI Class Code
// ============================================================================

/// USB 3.0 xHCI controller PCI class.
pub const PCI_CLASS_SERIAL: u32 = 0x0C;
pub const PCI_SUBCLASS_USB: u32 = 0x03;
pub const PCI_PROGIF_XHCI: u32 = 0x30;

// ============================================================================
// Capability Registers (CAPREG at BAR0 offset 0)
// ============================================================================

pub mod cap {
    /// CAPLENGTH — offset to Operational registers (0x00, 8-bit)
    pub const CAPLENGTH: usize = 0x00;
    /// HCIVERSION — xHCI spec version (0x02, 16-bit)
    pub const HCIVERSION: usize = 0x02;

    /// HCSPARAMS1 — Structural Parameters 1 (0x04, 32-bit)
    pub const HCSPARAMS1: usize = 0x04;
    pub mod hcs1 {
        pub const MAX_SLOTS_SHIFT: u32 = 0;
        pub const MAX_SLOTS_MASK: u32 = 0xFF;
        pub const MAX_INTRS_SHIFT: u32 = 8;
        pub const MAX_INTRS_MASK: u32 = 0x7FF;
        pub const MAX_PORTS_SHIFT: u32 = 24;
        pub const MAX_PORTS_MASK: u32 = 0xFF;
    }

    /// HCSPARAMS2 — Structural Parameters 2 (0x08, 32-bit)
    pub const HCSPARAMS2: usize = 0x08;
    pub mod hcs2 {
        pub const IST_SHIFT: u32 = 0;
        pub const IST_MASK: u32 = 0xF;
        pub const ERST_MAX_SHIFT: u32 = 4;
        pub const ERST_MAX_MASK: u32 = 0xF;
        pub const MAX_SCRATCHPAD_BUF_HI_SHIFT: u32 = 21;
        pub const MAX_SCRATCHPAD_BUF_HI_MASK: u32 = 0x1F;
        pub const SCRATCHPAD_RESTORE: u32 = 1 << 27;
        pub const MAX_SCRATCHPAD_BUF_LO_SHIFT: u32 = 16;
        pub const MAX_SCRATCHPAD_BUF_LO_MASK: u32 = 0x1F;
    }

    /// HCSPARAMS3 — Structural Parameters 3 (0x0C, 32-bit)
    pub const HCSPARAMS3: usize = 0x0C;
    pub mod hcs3 {
        pub const U1_DEVICE_EXIT_LATENCY_MASK: u32 = 0xFF;
        pub const U2_DEVICE_EXIT_LATENCY_SHIFT: u32 = 8;
        pub const U2_DEVICE_EXIT_LATENCY_MASK: u32 = 0xFFFF;
    }

    /// HCCPARAMS1 — Capability Parameters 1 (0x10, 32-bit)
    pub const HCCPARAMS1: usize = 0x10;
    pub mod hcc1 {
        pub const AC64: u32 = 1 << 0;       // 64-bit addressing
        pub const BNC: u32 = 1 << 1;        // BW negotiation
        pub const CSZ: u32 = 1 << 2;        // Context Size (0=32B, 1=64B)
        pub const PPC: u32 = 1 << 3;        // Port Power Control
        pub const PIND: u32 = 1 << 4;       // Port Indicators
        pub const LHRC: u32 = 1 << 5;       // Light HC Reset
        pub const LTC: u32 = 1 << 6;        // Latency Tolerance
        pub const NSC: u32 = 1 << 7;        // No Secondary SID
        pub const RSVD_P: u32 = 1 << 8;
        pub const MAX_PSA_SIZE_SHIFT: u32 = 12;
        pub const MAX_PSA_SIZE_MASK: u32 = 0xF;
        pub const X_ECN_SHIFT: u32 = 16;
        pub const X_ECN_MASK: u32 = 0xFFFF;  // Extended Capabilities Next pointer
    }

    /// HCCPARAMS2 — Capability Parameters 2 (0x12, 16-bit)
    pub const HCCPARAMS2: usize = 0x12;
    pub mod hcc2 {
        pub const U3C: u32 = 1 << 0;        // U3 entry capability
        pub const CMC: u32 = 1 << 1;        // Configure endpoint max
        pub const FSC: u32 = 1 << 2;        // Force save context
        pub const CTC: u32 = 1 << 3;        // Compliance transition
        pub const LEC: u32 = 1 << 4;        // Large ESIT payload
        pub const CIC: u32 = 1 << 5;        // Config info context
        pub const ETC: u32 = 1 << 6;        // Extended TBC/TBC multiplier
    }

    /// DBOFF — Doorbell Array offset (0x14, 32-bit)
    pub const DBOFF: usize = 0x14;
    /// RTSOFF — Runtime Register Space offset (0x18, 32-bit)
    pub const RTSOFF: usize = 0x18;
}

// ============================================================================
// Operational Registers (OPREG at BAR0 + CAPLENGTH)
// ============================================================================

pub mod op {
    /// USBCMD — USB Command (0x00, 32-bit)
    pub const USBCMD: usize = 0x00;
    pub mod cmd {
        pub const RUN_STOP: u32 = 1 << 0;      // RS
        pub const HC_RESET: u32 = 1 << 1;       // HCRST
        pub const INT_EVT_EN: u32 = 1 << 2;     // INTE
        pub const HSEE: u32 = 1 << 3;           // Host System Error Enable
        pub const LIGHT_HC_RESET: u32 = 1 << 7; // LHCRST
        pub const CSS: u32 = 1 << 8;            // Cmd Ring Stopped
        pub const CRS: u32 = 1 << 9;            // Cmd Ring Start
        pub const EWE: u32 = 1 << 10;           // Event Ring Wrap
        pub const EU3S: u32 = 1 << 11;          // Enable U3 entry
    }

    /// USBSTS — USB Status (0x04, 32-bit)
    pub const USBSTS: usize = 0x04;
    pub mod sts {
        pub const HCHALTED: u32 = 1 << 0;
        pub const HSE: u32 = 1 << 2;        // Host System Error
        pub const EINT: u32 = 1 << 3;       // Event Interrupt
        pub const PCD: u32 = 1 << 4;        // Port Change Detect
        pub const SRE: u32 = 1 << 10;       // Save Restore Error
        pub const CNR: u32 = 1 << 11;       // Controller Not Ready
        pub const HCE: u32 = 1 << 12;       // Host Controller Error
    }

    /// PAGESIZE — Page Size (0x08, 32-bit)
    pub const PAGESIZE: usize = 0x08;

    /// DNCTRL — Device Notification Control (0x14, 32-bit)
    pub const DNCTRL: usize = 0x14;

    /// CRCR — Command Ring Control (0x18, 64-bit)
    pub const CRCR: usize = 0x18;
    pub mod crcr {
        pub const RCS: u64 = 1 << 0;         // Ring Cycle State
        pub const CS: u64 = 1 << 1;          // Command Stop
        pub const CA: u64 = 1 << 2;          // Command Abort
        pub const CRR: u64 = 1 << 3;         // Command Ring Running
        pub const CRCR_LO_MASK: u64 = !0x3F; // Pointer bits [63:6]
    }

    /// DCBAAP — Device Context Base Address Array Pointer (0x30, 64-bit)
    pub const DCBAAP: usize = 0x30;

    /// CONFIG — Configure (0x38, 32-bit)
    pub const CONFIG: usize = 0x38;
    pub mod config {
        pub const MAX_DEV_SLOT_EN_SHIFT: u32 = 0;
        pub const MAX_DEV_SLOT_EN_MASK: u32 = 0xFF;
        pub const U3E: u32 = 1 << 8;  // U3 Entry Enable
    }

    /// Port Register Set starts at 0x400 (offset within OPREG)
    pub const PORT_BASE: usize = 0x400;
    /// Each Port Register Set is 16 bytes (0x10)
    pub const PORT_SIZE: usize = 0x10;

    /// PORTSC — Port Status and Control (first port, 32-bit)
    pub const PORTSC: usize = 0x00;
    pub mod portsc {
        pub const CCS: u32 = 1 << 0;       // Current Connect Status
        pub const PED: u32 = 1 << 1;       // Port Enabled/Disabled
        pub const OCC: u32 = 1 << 3;       // Over-current Change
        pub const PR: u32 = 1 << 4;        // Port Reset
        pub const PLS_SHIFT: u32 = 5;      // Port Link State
        pub const PLS_MASK: u32 = 0xF;
        pub const   PLS_U0: u32 = 0;
        pub const   PLS_U1: u32 = 1;
        pub const   PLS_U2: u32 = 2;
        pub const   PLS_U3: u32 = 3;
        pub const   PLS_DISABLED: u32 = 4;
        pub const   PLS_RX_DETECT: u32 = 5;
        pub const   PLS_INACTIVE: u32 = 6;
        pub const   PLS_POLLING: u32 = 7;
        pub const   PLS_RECOVERY: u32 = 8;
        pub const   PLS_HOT_RESET: u32 = 9;
        pub const   PLS_COMPLIANCE: u32 = 10;
        pub const   PLS_LOOPBACK: u32 = 11;
        pub const PP: u32 = 1 << 9;        // Port Power
        pub const PIC_SHIFT: u32 = 14;     // Port Indicator Control
        pub const PIC_MASK: u32 = 0x3;
        pub const LWS: u32 = 1 << 16;      // Link Write Strobe
        pub const CSC: u32 = 1 << 17;      // Connect Status Change
        pub const WRC: u32 = 1 << 19;      // Warm Reset Change
        pub const WDE: u32 = 1 << 21;      // Warm Reset
        pub const DR: u32 = 1 << 22;       // Device Removable
        pub const WPR: u32 = 1 << 23;      // Warm Port Reset
        pub const SPEED_SHIFT: u32 = 26;   // Port Speed
        pub const SPEED_MASK: u32 = 0xF;
        pub const   SPEED_FULL: u32 = 4;
        pub const   SPEED_LOW: u32 = 5;
        pub const   SPEED_HIGH: u32 = 6;
        pub const   SPEED_SUPER: u32 = 7;
        pub const PTC_SHIFT: u32 = 20;     // Port Test Control
        pub const PTC_MASK: u32 = 0xF;
    }
}

// ============================================================================
// Runtime Registers (at BAR0 + RTSOFF)
// ============================================================================

pub mod rt {
    /// Offset of the first interrupter's register set.
    /// Each set is 32 bytes (0x20).
    pub const INTR_BASE: usize = 0x00;
    pub const INTR_SIZE: usize = 0x20;

    /// IMAN — Interrupt Management (0x00, 32-bit)
    pub const IMAN: usize = 0x00;
    pub mod iman {
        pub const IP: u32 = 1 << 0;     // Interrupt Pending
        pub const IE: u32 = 1 << 1;     // Interrupt Enable
    }

    /// IMOD — Interrupt Moderation (0x04, 32-bit)
    pub const IMOD: usize = 0x04;
    pub mod imod {
        pub const IMODI_SHIFT: u32 = 0;    // Interval (max 10ms)
        pub const IMODI_MASK: u32 = 0xFFFF;
        pub const IMODC_SHIFT: u32 = 16;   // Counter
        pub const IMODC_MASK: u32 = 0xFFFF;
    }

    /// ERSTSZ — Event Ring Segment Table Size (0x08, 32-bit)
    pub const ERSTSZ: usize = 0x08;

    /// ERSTBA — Event Ring Segment Table Base Address (0x10, 64-bit)
    pub const ERSTBA: usize = 0x10;

    /// ERDP — Event Ring Dequeue Pointer (0x18, 64-bit)
    pub const ERDP: usize = 0x18;
    pub mod erdp {
        pub const EHB: u64 = 1 << 3;   // Event Handler Busy
        pub const MASK: u64 = !0xF;    // 16-byte aligned
    }
}

// ============================================================================
// Doorbell Registers (at BAR0 + DBOFF)
// ============================================================================

pub mod db {
    /// Doorbell for device slot N (0x00 + N*4)
    pub const DOORBELL: usize = 0x00;
    pub mod doorbell {
        pub const DB_TARGET_SHIFT: u32 = 0;
        pub const DB_TARGET_MASK: u32 = 0xFF;
        pub const DB_STREAM_ID_SHIFT: u32 = 16;
        pub const DB_STREAM_ID_MASK: u32 = 0xFFFF;
    }
}

// ============================================================================
// Extended Capabilities
// ============================================================================

pub mod xcap {
    /// Extended capability ID: USB Legacy Support
    pub const ID_USB_LEGACY: u8 = 1;
    /// Extended capability ID: Supported Protocol
    pub const ID_SUPPORTED_PROTOCOL: u8 = 2;
    /// Extended capability ID: Extended Power Management
    pub const ID_EXT_PM: u8 = 3;
    /// Extended capability ID: I/O Virtualization
    pub const ID_IOV: u8 = 4;
    /// Extended capability ID: Message Interrupt
    pub const ID_MSG_INTR: u8 = 5;
    /// Extended capability ID: Local Memory
    pub const ID_LOCAL_MEM: u8 = 6;
    /// Extended capability ID: USB Hardware LPM
    pub const ID_USB_HW_LPM: u8 = 7;
    /// Extended capability ID: Port U1/U2 Halt
    pub const ID_PORT_U_HALT: u8 = 9;

    /// Supported Protocol capability: port offset/mask
    pub const PROTOCOL_REVISION_MASK: u32 = 0xFF;
    pub const PROTOCOL_SPEED_ID_COUNT_SHIFT: u8 = 8;
    pub const PROTOCOL_SPEED_ID_COUNT_MASK: u32 = 0xFF;
    pub const PROTOCOL_PORT_OFFSET_SHIFT: u32 = 24;
    pub const PROTOCOL_PORT_OFFSET_MASK: u32 = 0xFF;
    pub const PROTOCOL_PORT_COUNT_SHIFT: u32 = 16;
    pub const PROTOCOL_PORT_COUNT_MASK: u32 = 0xFF;
}

// ============================================================================
// TRB (Transfer Request Block) Structures
// ============================================================================

/// TRB type values (in TRB flags dword bits 9:6 for Transfers, bits 31:24 for Events)
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrbType {
    /// Normal Transfer TRB
    Normal = 1,
    /// Setup Stage TRB (control transfer)
    SetupStage = 2,
    /// Data Stage TRB (control transfer)
    DataStage = 3,
    /// Status Stage TRB (control transfer)
    StatusStage = 4,
    /// Isoch TRB
    Isoch = 5,
    /// Link TRB (ring linking)
    Link = 6,
    /// Event Data TRB
    EventData = 7,
    /// No-op TRB
    NoOp = 8,
    /// Enable Slot Command
    EnableSlotCmd = 9,
    /// Disable Slot Command
    DisableSlotCmd = 10,
    /// Address Device Command
    AddressDeviceCmd = 11,
    /// Configure Endpoint Command
    ConfigureEndpointCmd = 12,
    /// Evaluate Context Command
    EvaluateContextCmd = 13,
    /// Reset Endpoint Command
    ResetEndpointCmd = 14,
    /// Stop Endpoint Command
    StopEndpointCmd = 15,
    /// Set TR Dequeue Pointer Command
    SetTrDequeuePtrCmd = 16,
    /// Reset Device Command
    ResetDeviceCmd = 17,
    /// Force Event Command (xHCI 1.1+)
    ForceEventCmd = 18,
    /// Negotiate Bandwidth Command
    NegotiateBandwidthCmd = 19,
    /// Set Latency Tolerance Value Command
    SetLatencyToleranceCmd = 20,
    /// Get Port Bandwidth Command
    GetPortBandwidthCmd = 21,
    /// Force Header Command
    ForceHeaderCmd = 22,
    /// No Op Command
    NoOpCmd = 23,
    /// Transfer Event TRB
    TransferEvent = 32,
    /// Command Completion Event TRB
    CommandCompletionEvent = 33,
    /// Port Status Change Event TRB
    PortStatusChangeEvent = 34,
    /// Bandwidth Request Event TRB
    BandwidthRequestEvent = 35,
    /// Doorbell Event TRB (xHCI 1.1+)
    DoorbellEvent = 36,
    /// Host Controller Event TRB
    HostControllerEvent = 37,
    /// Device Notification TRB
    DeviceNotification = 38,
    /// MFINDEX Wrap Event TRB
    MfindexWrapEvent = 39,
}

impl TrbType {
    /// Create from u8 value (for event parsing).
    pub fn from_u8(v: u8) -> Option<TrbType> {
        use TrbType::*;
        match v {
            1 => Some(Normal),
            2 => Some(SetupStage),
            3 => Some(DataStage),
            4 => Some(StatusStage),
            5 => Some(Isoch),
            6 => Some(Link),
            7 => Some(EventData),
            8 => Some(NoOp),
            9 => Some(EnableSlotCmd),
            10 => Some(DisableSlotCmd),
            11 => Some(AddressDeviceCmd),
            12 => Some(ConfigureEndpointCmd),
            13 => Some(EvaluateContextCmd),
            14 => Some(ResetEndpointCmd),
            15 => Some(StopEndpointCmd),
            16 => Some(SetTrDequeuePtrCmd),
            17 => Some(ResetDeviceCmd),
            18 => Some(ForceEventCmd),
            19 => Some(NegotiateBandwidthCmd),
            20 => Some(SetLatencyToleranceCmd),
            21 => Some(GetPortBandwidthCmd),
            22 => Some(ForceHeaderCmd),
            23 => Some(NoOpCmd),
            32 => Some(TransferEvent),
            33 => Some(CommandCompletionEvent),
            34 => Some(PortStatusChangeEvent),
            35 => Some(BandwidthRequestEvent),
            36 => Some(DoorbellEvent),
            37 => Some(HostControllerEvent),
            38 => Some(DeviceNotification),
            39 => Some(MfindexWrapEvent),
            _ => None,
        }
    }
}

/// Generic TRB structure (16 bytes = 4 dwords).
/// All TRBs share this basic layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Trb {
    /// Parameter (lower 64 bits of TRB — varies by type)
    pub parameter: [u8; 8],
    /// Status / Transfer Length / other (bits vary by TRB type)
    pub status: [u8; 4],
    /// Flags: TRB type (bits 9:6), Cycle bit (0), Chain (4), etc.
    pub flags: [u8; 4],
}

impl Trb {
    /// Create a zeroed TRB.
    pub fn zeroed() -> Self {
        Self {
            parameter: [0; 8],
            status: [0; 4],
            flags: [0; 4],
        }
    }

    /// Get the 64-bit parameter field.
    pub fn get_parameter(&self) -> u64 {
        u64::from_le_bytes(self.parameter)
    }

    /// Set the 64-bit parameter field.
    pub fn set_parameter(&mut self, val: u64) {
        self.parameter = val.to_le_bytes();
    }

    /// Get the status/length field as u32.
    pub fn get_status(&self) -> u32 {
        u32::from_le_bytes(self.status)
    }

    /// Set the status/length field.
    pub fn set_status(&mut self, val: u32) {
        self.status = val.to_le_bytes();
    }

    /// Get the flags dword as u32.
    pub fn get_flags(&self) -> u32 {
        u32::from_le_bytes(self.flags)
    }

    /// Set the flags dword.
    pub fn set_flags(&mut self, val: u32) {
        self.flags = val.to_le_bytes();
    }

    /// Get the TRB type from flags.
    pub fn trb_type(&self) -> Option<TrbType> {
        let flags = self.get_flags();
        let type_val = ((flags >> 10) & 0x3F) as u8;
        TrbType::from_u8(type_val)
    }

    /// Get the cycle bit.
    pub fn cycle(&self) -> bool {
        (self.get_flags() & 0x01) != 0
    }

    /// Set the cycle bit.
    pub fn set_cycle(&mut self, c: bool) {
        if c { self.flags[0] |= 0x01; }
        else { self.flags[0] &= !0x01; }
    }

    /// Get the chain bit.
    pub fn chain(&self) -> bool {
        (self.get_flags() & (1 << 4)) != 0
    }

    /// Set the TRB type.
    pub fn set_trb_type(&mut self, t: TrbType) {
        let flags = self.get_flags();
        self.set_flags((flags & !(0x3F << 10)) | ((t as u32) << 10));
    }

    /// Set the cycle bit and TRB type (common operation).
    pub fn set_cycle_type(&mut self, cycle: bool, t: TrbType) {
        let c_bit = if cycle { 1u32 } else { 0u32 };
        self.set_flags(c_bit | ((t as u32) << 10));
    }

    /// Set the interrupt target (for transfer TRBs).
    pub fn set_interrupter_target(&mut self, target: u16) {
        let flags = self.get_flags();
        self.set_flags((flags & !(0x3FF << 22)) | ((target as u32) << 22));
    }

    /// Set transfer length (lower 17 bits of status).
    pub fn set_transfer_length(&mut self, len: u32) {
        self.set_status(len & 0x1FFFF);
    }

    /// Set TD Size (bits 21:17 of status).
    pub fn set_td_size(&mut self, size: u8) {
        let s = self.get_status();
        self.set_status((s & !(0x1F << 17)) | ((size as u32) << 17));
    }
}

// ============================================================================
// Command TRB builders
// ============================================================================

/// Build an Enable Slot command TRB.
pub fn build_enable_slot_trb(cycle: bool) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::EnableSlotCmd);
    trb
}

/// Build a Disable Slot command TRB.
pub fn build_disable_slot_trb(cycle: bool, slot_id: u8) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::DisableSlotCmd);
    trb.status[0] = slot_id;
    trb
}

/// Build an Address Device command TRB.
/// `input_ctx_phys` — physical address of input device context (aligned to 64 bytes).
/// `bsr` — Block Set Address Request (0 = full, 1 = BSR).
pub fn build_address_device_trb(cycle: bool, slot_id: u8, input_ctx_phys: u64, bsr: bool) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::AddressDeviceCmd);
    trb.status[0] = slot_id;
    if bsr { trb.status[1] = 0x08; } // BSR bit 3
    trb.set_parameter(input_ctx_phys);
    trb
}

/// Build a Configure Endpoint command TRB.
/// `input_ctx_phys` — physical address of input device context.
/// `deconfigure` — set to true to deconfigure (disable all endpoints).
pub fn build_configure_endpoint_trb(cycle: bool, slot_id: u8, input_ctx_phys: u64, deconfigure: bool) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::ConfigureEndpointCmd);
    trb.status[0] = slot_id;
    if deconfigure { trb.status[1] = 0x08; } // DC bit
    trb.set_parameter(input_ctx_phys);
    trb
}

/// Build a Normal Transfer TRB.
pub fn build_normal_transfer_trb(cycle: bool, data_phys: u64, transfer_len: u32,
    td_size: u8, intr_target: u16, chain: bool, ioc: bool) -> Trb
{
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::Normal);
    trb.set_parameter(data_phys);
    trb.set_transfer_length(transfer_len);
    trb.set_td_size(td_size);
    trb.set_interrupter_target(intr_target);
    if chain { trb.flags[0] |= 0x10; }
    if ioc { trb.flags[0] |= 0x20; }   // IOC = bit 5
    trb
}

/// Build a Setup Stage TRB (for control transfers).
pub fn build_setup_stage_trb(cycle: bool, setup_pkt: &[u8; 8], intr_target: u16, trt: u8) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::SetupStage);
    // Setup packet bytes 0-7 → parameter
    trb.parameter.copy_from_slice(setup_pkt);
    trb.set_interrupter_target(intr_target);
    // TRT (Transfer Type): bits 17:16
    // 0 = no data, 1 = OUT data, 2 = IN data, 3 = no data
    let flags = trb.get_flags();
    trb.set_flags(flags | ((trt as u32) << 16));
    // Set IOC for completion notification
    trb.flags[0] |= 0x20;
    trb
}

/// Build a Data Stage TRB (for control transfers).
pub fn build_data_stage_trb(cycle: bool, data_phys: u64, transfer_len: u32,
    intr_target: u16, direction_in: bool, chain: bool, ioc: bool) -> Trb
{
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::DataStage);
    trb.set_parameter(data_phys);
    trb.set_transfer_length(transfer_len);
    trb.set_interrupter_target(intr_target);
    trb.set_td_size(0);
    // DIR (Direction) = bit 16
    if direction_in { trb.flags[2] |= 0x01; }
    if chain { trb.flags[0] |= 0x10; }
    if ioc { trb.flags[0] |= 0x20; }
    trb
}

/// Build a Status Stage TRB (for control transfers).
pub fn build_status_stage_trb(cycle: bool, intr_target: u16, direction_in: bool) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::StatusStage);
    trb.set_interrupter_target(intr_target);
    // DIR (Direction) = bit 16 for status IN
    if direction_in { trb.flags[2] |= 0x01; }
    // Set IOC for completion notification
    trb.flags[0] |= 0x20;
    trb
}

/// Build a Link TRB for ring linking.
pub fn build_link_trb(cycle: bool, ring_phys: u64, toggle_cycle: bool, chain: bool) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::Link);
    trb.set_parameter(ring_phys);
    trb.set_status(0);
    // Toggle Cycle (TC) = bit 1, Chain = bit 4
    if toggle_cycle { trb.flags[0] |= 0x02; }
    if chain { trb.flags[0] |= 0x10; }
    trb
}

/// Build an Evaluate Context command TRB.
/// Updates the device context without deconfiguring endpoints.
/// `input_ctx_phys` — physical address of input device context.
pub fn build_evaluate_context_trb(cycle: bool, slot_id: u8, input_ctx_phys: u64) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::EvaluateContextCmd);
    trb.status[0] = slot_id;
    trb.set_parameter(input_ctx_phys);
    trb
}

/// Build a No-Op TRB.
pub fn build_noop_trb(cycle: bool, intr_target: u16, chain: bool) -> Trb {
    let mut trb = Trb::zeroed();
    trb.set_cycle_type(cycle, TrbType::NoOp);
    trb.set_interrupter_target(intr_target);
    if chain { trb.flags[0] |= 0x10; }
    trb
}

// ============================================================================
// Event TRB Parsing
// ============================================================================

/// Completion code values for event TRBs.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionCode {
    Success = 1,
    Unknown = 0,
    Invalid = 2,
    // ... other codes truncated for brevity
}

impl CompletionCode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => CompletionCode::Success,
            2 => CompletionCode::Invalid,
            _ => CompletionCode::Unknown,
        }
    }
}

/// Parse a Command Completion Event TRB.
/// Returns (slot_id, completion_code, command_cycle, trb_phys).
/// Per xHCI spec Rev 1.2 §6.4.2.2:
/// - status[0..1] = Completion Code (bits 15:0)
/// - status[2] = Slot ID
/// - parameter = command TRB physical address
pub fn parse_command_completion_event(trb: &Trb) -> (u8, CompletionCode, bool, u64) {
    let cc_val_low = ((trb.status[0] as u16) | ((trb.status[1] as u16) << 8)) & 0xFF;
    let cc = CompletionCode::from_u8(cc_val_low as u8);
    let slot_id = trb.status[2];
    let cmd_cycle = (trb.get_flags() & 1) != 0;
    let cmd_trb_addr = trb.get_parameter();
    (slot_id, cc, cmd_cycle, cmd_trb_addr)
}

/// Parse a Port Status Change Event TRB.
/// Returns (port_id, port_change_bits).
pub fn parse_port_status_change_event(trb: &Trb) -> (u8, u32) {
    let flags = trb.get_flags();
    let port_id = (flags >> 24) as u8;
    let port_change = trb.get_status();
    (port_id, port_change)
}

// ============================================================================
// Device Context Structures
// ============================================================================

/// Size of device context entry (32 or 64 bytes depending on CSZ).
pub const DEVICE_CONTEXT_ENTRY_SIZE: usize = 32;

/// Slot Context (first entry in Device Context).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SlotContext {
    /// Dword 0: Route String, Speed, etc.
    pub dw0: [u8; 4],
    /// Dword 1: Max Exit Latency, Root Hub Port Number
    pub dw1: [u8; 4],
    /// Dword 2: Number of Ports, Parent Hub Slot ID, Parent Port Number
    pub dw2: [u8; 4],
    /// Dword 3: TT info, etc.
    pub dw3: [u8; 4],
    /// Dword 4-7: reserved / device address
    pub dw4: [u8; 4],
    pub dw5: [u8; 4],
    pub dw6: [u8; 4],
    pub dw7: [u8; 4],
}

impl SlotContext {
    pub fn zeroed() -> Self {
        Self { dw0: [0; 4], dw1: [0; 4], dw2: [0; 4], dw3: [0; 4],
               dw4: [0; 4], dw5: [0; 4], dw6: [0; 4], dw7: [0; 4] }
    }

    pub fn get_dw0(&self) -> u32 { u32::from_le_bytes(self.dw0) }
    pub fn set_dw0(&mut self, v: u32) { self.dw0 = v.to_le_bytes(); }
    pub fn get_dw1(&self) -> u32 { u32::from_le_bytes(self.dw1) }
    pub fn set_dw1(&mut self, v: u32) { self.dw1 = v.to_le_bytes(); }
    pub fn get_dw7(&self) -> u32 { u32::from_le_bytes(self.dw7) }
    pub fn set_dw7(&mut self, v: u32) { self.dw7 = v.to_le_bytes(); }

    /// Get Route String (bits 31:0 of dw0).
    pub fn route_string(&self) -> u32 { self.get_dw0() }
    pub fn set_route_string(&mut self, rs: u32) { self.set_dw0(rs); }

    /// Get speed (bits 7:0 of dw0).
    pub fn speed(&self) -> u8 { (self.get_dw0() & 0xF) as u8 }

    /// Get the context entries field (bits 31:27 of dw0).
    pub fn context_entries(&self) -> u8 { ((self.get_dw0() >> 27) & 0x1F) as u8 }

    /// Get root hub port number (bits 31:24 of dw1).
    pub fn root_hub_port_num(&self) -> u8 { ((self.get_dw1() >> 24) & 0xFF) as u8 }

    /// Get device address (bits 7:0 of dw7).
    pub fn device_address(&self) -> u8 { (self.get_dw7() & 0xFF) as u8 }

    /// Set device address.
    pub fn set_device_address(&mut self, addr: u8) {
        let v = self.get_dw7();
        self.set_dw7((v & !0xFF) | addr as u32);
    }
}

/// Endpoint Context (entry 2+ in Device Context).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EndpointContext {
    pub dw0: [u8; 4],  // EP state, interval, etc.
    pub dw1: [u8; 4],  // Max ESIT payload, Max Burst, etc.
    pub dw2: [u8; 4],  // TR Dequeue Pointer (lower)
    pub dw3: [u8; 4],  // TR Dequeue Pointer (upper)
    pub dw4: [u8; 4],  // Average TRB Length, Max ESIT, etc.
    pub dw5: [u8; 4],
    pub dw6: [u8; 4],
    pub dw7: [u8; 4],
}

impl EndpointContext {
    pub fn zeroed() -> Self {
        Self { dw0: [0; 4], dw1: [0; 4], dw2: [0; 4], dw3: [0; 4],
               dw4: [0; 4], dw5: [0; 4], dw6: [0; 4], dw7: [0; 4] }
    }

    pub fn get_dw0(&self) -> u32 { u32::from_le_bytes(self.dw0) }
    pub fn set_dw0(&mut self, v: u32) { self.dw0 = v.to_le_bytes(); }
    pub fn get_dw2(&self) -> u32 { u32::from_le_bytes(self.dw2) }
    pub fn get_dw3(&self) -> u32 { u32::from_le_bytes(self.dw3) }
    pub fn get_dw4(&self) -> u32 { u32::from_le_bytes(self.dw4) }
    pub fn set_dw4(&mut self, v: u32) { self.dw4 = v.to_le_bytes(); }

    /// Get EP Type (bits 5:3 of dw0).
    pub fn ep_type(&self) -> u8 { ((self.get_dw0() >> 3) & 0x7) as u8 }
    pub fn set_ep_type(&mut self, t: u8) {
        let v = self.get_dw0();
        self.set_dw0((v & !(0x7 << 3)) | ((t as u32) << 3));
    }

    /// Get Max Packet Size (bits 31:16 of dw0).
    pub fn max_packet_size(&self) -> u16 { (self.get_dw0() >> 16) as u16 }
    pub fn set_max_packet_size(&mut self, mps: u16) {
        let v = self.get_dw0();
        self.set_dw0((v & 0xFFFF) | ((mps as u32) << 16));
    }

    /// Get CErr (bits 5:4 of dw4 — short for ...).
    pub fn cerr(&self) -> u8 { ((self.get_dw4() >> 4) & 0x3) as u8 }
    pub fn set_cerr(&mut self, c: u8) {
        let v = self.get_dw4();
        self.set_dw4((v & !(0x3 << 4)) | ((c as u32) << 4));
    }

    /// Get TR Dequeue Pointer (64-bit physical address).
    pub fn tr_dequeue_ptr(&self) -> u64 {
        (self.get_dw2() as u64) | ((self.get_dw3() as u64) << 32)
    }

    /// Set TR Dequeue Pointer.
    pub fn set_tr_dequeue_ptr(&mut self, addr: u64) {
        self.dw2 = (addr as u32).to_le_bytes();
        self.dw3 = ((addr >> 32) as u32).to_le_bytes();
    }

    /// Get Average TRB Length (bits 15:0 of dw4).
    pub fn average_trb_len(&self) -> u16 { (self.get_dw4() & 0xFFFF) as u16 }
    pub fn set_average_trb_len(&mut self, len: u16) {
        let v = self.get_dw4();
        self.set_dw4((v & !0xFFFF) | len as u32);
    }

    /// Get Max ESIT Payload (bits 31:16 of dw4).
    pub fn max_esit_payload(&self) -> u16 { (self.get_dw4() >> 16) as u16 }
    pub fn set_max_esit_payload(&mut self, p: u16) {
        let v = self.get_dw4();
        self.set_dw4((v & 0xFFFF) | ((p as u32) << 16));
    }
}

/// Input Context — used for Address Device and Configure Endpoint commands.
/// Contains: Input Control Context (8 bytes) + Slot Context + Endpoint Contexts.
#[repr(C, packed)]
pub struct InputContext {
    /// Dword 0: Add Context flags (which entries to add).
    pub add_flags: [u8; 4],
    /// Dword 1: Drop Context flags.
    pub drop_flags: [u8; 4],
    /// Slot Context.
    pub slot: SlotContext,
    /// Endpoint Context 0-31 (only used entries need be valid).
    pub ep: [EndpointContext; 31],
}

impl InputContext {
    pub fn zeroed() -> Self {
        Self {
            add_flags: [0; 4],
            drop_flags: [0; 4],
            slot: SlotContext::zeroed(),
            ep: [EndpointContext::zeroed(); 31],
        }
    }

    pub fn set_add_context_flag(&mut self, idx: u8) {
        let v = u32::from_le_bytes(self.add_flags) | (1u32 << idx);
        self.add_flags = v.to_le_bytes();
    }

    pub fn set_drop_context_flag(&mut self, idx: u8) {
        let v = u32::from_le_bytes(self.drop_flags) | (1u32 << idx);
        self.drop_flags = v.to_le_bytes();
    }
}

/// Device Context (output from controller) — same layout as Input Context
/// but without the add/drop flags.
#[repr(C, packed)]
pub struct DeviceContext {
    pub slot: SlotContext,
    pub ep: [EndpointContext; 31],
}

impl DeviceContext {
    pub fn zeroed() -> Self {
        Self {
            slot: SlotContext::zeroed(),
            ep: [EndpointContext::zeroed(); 31],
        }
    }
}

/// A single segment in the Event Ring Segment Table (ERST).
/// Each entry: 16 bytes (8B base addr, 8B size/rsvd).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ErstEntry {
    pub seg_base_lo: u32,
    pub seg_base_hi: u32,
    pub seg_size: u16,
    pub rsvd: [u8; 6],
}

impl ErstEntry {
    pub fn zeroed() -> Self {
        Self { seg_base_lo: 0, seg_base_hi: 0, seg_size: 0, rsvd: [0; 6] }
    }

    pub fn base_addr(&self) -> u64 {
        (self.seg_base_lo as u64) | ((self.seg_base_hi as u64) << 32)
    }

    pub fn set_base_addr(&mut self, addr: u64) {
        self.seg_base_lo = addr as u32;
        self.seg_base_hi = (addr >> 32) as u32;
    }
}

// ============================================================================
// Scratchpad Buffer Array
// ============================================================================

/// Scratchpad buffer array entry (8 bytes).
#[repr(C, packed)]
pub struct ScratchpadEntry {
    pub lo: u32,
    pub hi: u32,
}

impl ScratchpadEntry {
    pub fn zeroed() -> Self { Self { lo: 0, hi: 0 } }

    pub fn addr(&self) -> u64 { (self.lo as u64) | ((self.hi as u64) << 32) }
    pub fn set_addr(&mut self, addr: u64) {
        self.lo = addr as u32;
        self.hi = (addr >> 32) as u32;
    }
}

// ============================================================================
// USB Device Speed mapping
// ============================================================================

pub mod speed {
    pub const FULL: u8 = 4;   // USB 1.1 Full Speed (12 Mbps)
    pub const LOW: u8 = 5;    // USB 1.0 Low Speed (1.5 Mbps)
    pub const HIGH: u8 = 6;   // USB 2.0 High Speed (480 Mbps)
    pub const SUPER: u8 = 7;  // USB 3.0 Super Speed (5 Gbps)
}

// ============================================================================
// USB Standard Requests (Chapter 9 of USB 2.0 Spec)
// ============================================================================

/// USB request type: direction (host→device).
pub const USB_DIR_OUT: u8 = 0x00;
/// USB request type: direction (device→host).
pub const USB_DIR_IN: u8 = 0x80;

/// USB request type: type is Standard.
pub const USB_TYPE_STANDARD: u8 = 0x00;
/// USB request type: type is Class.
pub const USB_TYPE_CLASS: u8 = 0x20;
/// USB request type: type is Vendor.
pub const USB_TYPE_VENDOR: u8 = 0x40;

/// USB request type: recipient is Device.
pub const USB_RECIP_DEVICE: u8 = 0x00;
/// USB request type: recipient is Interface.
pub const USB_RECIP_INTERFACE: u8 = 0x01;
/// USB request type: recipient is Endpoint.
pub const USB_RECIP_ENDPOINT: u8 = 0x02;
/// USB request type: recipient is Other.
pub const USB_RECIP_OTHER: u8 = 0x03;

/// Build an 8-byte USB setup packet for control transfers.
pub fn build_setup_packet(
    bm_request_type: u8, b_request: u8,
    w_value: u16, w_index: u16, w_length: u16
) -> [u8; 8] {
    let mut pkt = [0u8; 8];
    pkt[0] = bm_request_type;
    pkt[1] = b_request;
    pkt[2] = (w_value & 0xFF) as u8;
    pkt[3] = ((w_value >> 8) & 0xFF) as u8;
    pkt[4] = (w_index & 0xFF) as u8;
    pkt[5] = ((w_index >> 8) & 0xFF) as u8;
    pkt[6] = (w_length & 0xFF) as u8;
    pkt[7] = ((w_length >> 8) & 0xFF) as u8;
    pkt
}

/// USB standard request codes.
pub mod usb_req {
    /// GET_STATUS
    pub const GET_STATUS: u8 = 0;
    /// CLEAR_FEATURE
    pub const CLEAR_FEATURE: u8 = 1;
    /// SET_FEATURE
    pub const SET_FEATURE: u8 = 3;
    /// SET_ADDRESS
    pub const SET_ADDRESS: u8 = 5;
    /// GET_DESCRIPTOR
    pub const GET_DESCRIPTOR: u8 = 6;
    /// SET_DESCRIPTOR
    pub const SET_DESCRIPTOR: u8 = 7;
    /// GET_CONFIGURATION
    pub const GET_CONFIGURATION: u8 = 8;
    /// SET_CONFIGURATION
    pub const SET_CONFIGURATION: u8 = 9;
    /// GET_INTERFACE
    pub const GET_INTERFACE: u8 = 10;
    /// SET_INTERFACE
    pub const SET_INTERFACE: u8 = 11;
    /// SYNCH_FRAME
    pub const SYNCH_FRAME: u8 = 12;
}

/// USB descriptor type codes (wValue high byte for GET_DESCRIPTOR).
pub mod usb_descriptor {
    /// DEVICE descriptor type code
    pub const DEVICE: u8 = 1;
    /// CONFIGURATION descriptor type code
    pub const CONFIGURATION: u8 = 2;
    /// STRING descriptor type code
    pub const STRING: u8 = 3;
    /// INTERFACE descriptor type code
    pub const INTERFACE: u8 = 4;
    /// ENDPOINT descriptor type code
    pub const ENDPOINT: u8 = 5;
    /// DEVICE_QUALIFIER descriptor type code
    pub const DEVICE_QUALIFIER: u8 = 6;
    /// OTHER_SPEED_CONFIGURATION descriptor type code
    pub const OTHER_SPEED_CONFIG: u8 = 7;
    /// INTERFACE_POWER descriptor type code
    pub const INTERFACE_POWER: u8 = 8;
    /// BOS descriptor (USB 3.0)
    pub const BOS: u8 = 15;
    /// SuperSpeed USB Endpoint Companion descriptor
    pub const SS_ENDPOINT_COMPANION: u8 = 48;
}

// ============================================================================
// USB Descriptor Structures
// ============================================================================

/// USB Device Descriptor (18 bytes).
/// Standard USB 2.0 §9.6.1.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct DeviceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bcdUSB: [u8; 2],
    pub bDeviceClass: u8,
    pub bDeviceSubClass: u8,
    pub bDeviceProtocol: u8,
    pub bMaxPacketSize0: u8,
    pub idVendor: [u8; 2],
    pub idProduct: [u8; 2],
    pub bcdDevice: [u8; 2],
    pub iManufacturer: u8,
    pub iProduct: u8,
    pub iSerialNumber: u8,
    pub bNumConfigurations: u8,
}

impl DeviceDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 18 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    pub fn usb_version(&self) -> u16 { u16::from_le_bytes(self.bcdUSB) }
    pub fn vendor_id(&self) -> u16 { u16::from_le_bytes(self.idVendor) }
    pub fn product_id(&self) -> u16 { u16::from_le_bytes(self.idProduct) }
    pub fn device_version(&self) -> u16 { u16::from_le_bytes(self.bcdDevice) }
}

/// USB Configuration Descriptor Header (9 bytes).
/// Multiple interface + endpoint descriptors follow.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ConfigDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub wTotalLength: [u8; 2],
    pub bNumInterfaces: u8,
    pub bConfigurationValue: u8,
    pub iConfiguration: u8,
    pub bmAttributes: u8,
    pub bMaxPower: u8,
}

impl ConfigDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    pub fn total_length(&self) -> u16 { u16::from_le_bytes(self.wTotalLength) }
    pub fn is_self_powered(&self) -> bool { (self.bmAttributes & 0x40) != 0 }
    pub fn supports_remote_wakeup(&self) -> bool { (self.bmAttributes & 0x20) != 0 }
}

/// USB Interface Descriptor (9 bytes).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct InterfaceDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bInterfaceNumber: u8,
    pub bAlternateSetting: u8,
    pub bNumEndpoints: u8,
    pub bInterfaceClass: u8,
    pub bInterfaceSubClass: u8,
    pub bInterfaceProtocol: u8,
    pub iInterface: u8,
}

impl InterfaceDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }
}

/// USB Endpoint Descriptor (7 bytes standard).
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EndpointDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub bEndpointAddress: u8,
    pub bmAttributes: u8,
    pub wMaxPacketSize: [u8; 2],
    pub bInterval: u8,
}

impl EndpointDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    /// Endpoint number (bits 3:0).
    pub fn endpoint_number(&self) -> u8 { self.bEndpointAddress & 0x0F }
    /// Direction: true = IN (device→host).
    pub fn is_in(&self) -> bool { (self.bEndpointAddress & 0x80) != 0 }
    /// Max packet size.
    pub fn max_packet_size(&self) -> u16 { u16::from_le_bytes(self.wMaxPacketSize) & 0x7FF }
    /// Transfer type: 0=Control, 1=Isoch, 2=Bulk, 3=Interrupt.
    pub fn transfer_type(&self) -> u8 { self.bmAttributes & 0x03 }
}

// ============================================================================
// USB Mass Storage Bulk-Only Transport (BOT) — USB MS CBI 1.0
// ============================================================================

/// CBW (Command Block Wrapper) signature: "USBC" = 0x43425355.
pub const CBW_SIGNATURE: u32 = 0x43425355;
/// CSW (Command Status Wrapper) signature: "USBS" = 0x53425355.
pub const CSW_SIGNATURE: u32 = 0x53425355;

/// CBW flags: direction is device-to-host (IN).
pub const CBW_FLAGS_IN: u8 = 0x80;
/// CBW flags: direction is host-to-device (OUT).
pub const CBW_FLAGS_OUT: u8 = 0x00;

/// CSW status: command passed.
pub const CSW_STATUS_PASS: u8 = 0x00;
/// CSW status: command failed.
pub const CSW_STATUS_FAIL: u8 = 0x01;
/// CSW status: phase error.
pub const CSW_STATUS_PHASE_ERROR: u8 = 0x02;

/// Command Block Wrapper (31 bytes) — sent from host to device.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Cbw {
    pub dCBWSignature: [u8; 4],      // 0x43425355 "USBC"
    pub dCBWTag: [u8; 4],            // Command tag
    pub dCBWDataTransferLength: [u8; 4], // Data transfer length
    pub bmCBWFlags: u8,              // Direction (0x80=IN, 0x00=OUT)
    pub bCBWLUN: u8,                 // LUN
    pub bCBWCBLength: u8,            // Length of CDB (command descriptor block)
    pub CBWCB: [u8; 16],             // CDB (SCSI command)
}

impl Cbw {
    pub fn new(tag: u32, lun: u8, dir_in: bool, data_len: u32, cdb: &[u8]) -> Self {
        let mut cbw = Cbw {
            dCBWSignature: CBW_SIGNATURE.to_le_bytes(),
            dCBWTag: tag.to_le_bytes(),
            dCBWDataTransferLength: data_len.to_le_bytes(),
            bmCBWFlags: if dir_in { CBW_FLAGS_IN } else { CBW_FLAGS_OUT },
            bCBWLUN: lun,
            bCBWCBLength: core::cmp::min(cdb.len() as u8, 16),
            CBWCB: [0u8; 16],
        };
        let copy_len = core::cmp::min(cdb.len(), 16);
        cbw.CBWCB[..copy_len].copy_from_slice(&cdb[..copy_len]);
        cbw
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, 31) }
    }
}

/// Command Status Wrapper (13 bytes) — sent from device to host.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Csw {
    pub dCSWSignature: [u8; 4],      // Must be 0x53425355 "USBS"
    pub dCSWTag: [u8; 4],            // Must match CBW tag
    pub dCSWDataResidue: [u8; 4],    // Residual data (unused bytes)
    pub bCSWStatus: u8,              // 0=pass, 1=fail, 2=phase error
}

impl Csw {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 13 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    pub fn signature(&self) -> u32 { u32::from_le_bytes(self.dCSWSignature) }
    pub fn tag(&self) -> u32 { u32::from_le_bytes(self.dCSWTag) }
    pub fn residue(&self) -> u32 { u32::from_le_bytes(self.dCSWDataResidue) }
    pub fn status(&self) -> u8 { self.bCSWStatus }
    pub fn is_ok(&self) -> bool { self.signature() == CSW_SIGNATURE && self.bCSWStatus == CSW_STATUS_PASS }
}

// ============================================================================
// SCSI Command Descriptor Blocks (CDB)
// ============================================================================

pub mod scsi_cmd {
    pub const TEST_UNIT_READY: u8 = 0x00;
    pub const REQUEST_SENSE: u8 = 0x03;
    pub const INQUIRY: u8 = 0x12;
    pub const MODE_SENSE6: u8 = 0x1A;
    pub const READ_CAPACITY10: u8 = 0x25;
    pub const READ10: u8 = 0x28;
    pub const WRITE10: u8 = 0x2A;
    pub const READ_CAPACITY16: u8 = 0x9E;
    pub const READ16: u8 = 0x88;
    pub const WRITE16: u8 = 0x8A;
    pub const SYNCHRONIZE_CACHE10: u8 = 0x35;
}

/// Build a 10-byte SCSI READ10 CDB.
/// `lba` — logical block address.
/// `num_blocks` — number of blocks to read (1..65535).
pub fn build_read10_cdb(lba: u32, num_blocks: u16) -> [u8; 10] {
    let mut cdb = [0u8; 10];
    cdb[0] = scsi_cmd::READ10;
    cdb[2] = ((lba >> 24) & 0xFF) as u8;
    cdb[3] = ((lba >> 16) & 0xFF) as u8;
    cdb[4] = ((lba >> 8) & 0xFF) as u8;
    cdb[5] = (lba & 0xFF) as u8;
    cdb[7] = ((num_blocks >> 8) & 0xFF) as u8;
    cdb[8] = (num_blocks & 0xFF) as u8;
    cdb
}

/// Build a 10-byte SCSI WRITE10 CDB.
pub fn build_write10_cdb(lba: u32, num_blocks: u16) -> [u8; 10] {
    let mut cdb = [0u8; 10];
    cdb[0] = scsi_cmd::WRITE10;
    cdb[2] = ((lba >> 24) & 0xFF) as u8;
    cdb[3] = ((lba >> 16) & 0xFF) as u8;
    cdb[4] = ((lba >> 8) & 0xFF) as u8;
    cdb[5] = (lba & 0xFF) as u8;
    cdb[7] = ((num_blocks >> 8) & 0xFF) as u8;
    cdb[8] = (num_blocks & 0xFF) as u8;
    cdb
}

/// Build a 10-byte SCSI READ CAPACITY(10) CDB.
pub fn build_read_capacity10_cdb() -> [u8; 10] {
    let mut cdb = [0u8; 10];
    cdb[0] = scsi_cmd::READ_CAPACITY10;
    cdb
}

/// Build a 6-byte SCSI INQUIRY CDB.
/// `page_code` — 0 for standard inquiry.
pub fn build_inquiry_cdb(page_code: u8, alloc_len: u16) -> [u8; 6] {
    let mut cdb = [0u8; 6];
    cdb[0] = scsi_cmd::INQUIRY;
    cdb[2] = page_code;
    cdb[3] = ((alloc_len >> 8) & 0xFF) as u8;
    cdb[4] = (alloc_len & 0xFF) as u8;
    cdb
}

/// Build a 6-byte SCSI TEST UNIT READY CDB.
pub fn build_test_unit_ready_cdb() -> [u8; 6] {
    let mut cdb = [0u8; 6];
    cdb[0] = scsi_cmd::TEST_UNIT_READY;
    cdb
}

/// Build a 6-byte SCSI REQUEST SENSE CDB.
/// `alloc_len` — allocation length (max 18 for fixed format).
pub fn build_request_sense_cdb(alloc_len: u8) -> [u8; 6] {
    let mut cdb = [0u8; 6];
    cdb[0] = scsi_cmd::REQUEST_SENSE;
    cdb[4] = alloc_len;
    cdb
}

// ============================================================================
// SCSI Sense Data — Fixed Format (18 bytes)
// ============================================================================

/// SCSI Fixed Format Sense Data (18 bytes).
/// Per SPC-4 §4.5, Table 119.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ScsiSenseData {
    /// Byte 0: Response code & Valid bit.
    pub response_code: u8,
    /// Byte 1: Obsolete / Segment Number.
    pub obsolete: u8,
    /// Byte 2: Sense Key (bits 3:0).
    pub sense_key: u8,
    /// Byte 3: Information field (MSB).
    pub info_msb: u8,
    /// Bytes 4-6: Information field (cont).
    pub info_3: [u8; 3],
    /// Byte 7: Additional sense length (n-7).
    pub additional_sense_length: u8,
    /// Byte 8: Command-specific information (MSB).
    pub cmd_spec_info_msb: u8,
    /// Bytes 9-11: Command-specific information (cont).
    pub cmd_spec_info: [u8; 3],
    /// Byte 12: Additional Sense Code (ASC).
    pub asc: u8,
    /// Byte 13: Additional Sense Code Qualifier (ASCQ).
    pub ascq: u8,
    /// Byte 14: Field Replaceable Unit Code.
    pub fru_code: u8,
    /// Byte 15: Sense Key Specific (MSB).
    pub sks_msb: u8,
    /// Bytes 16-17: Sense Key Specific (cont).
    pub sks: [u8; 2],
}

impl ScsiSenseData {
    pub fn zeroed() -> Self {
        Self {
            response_code: 0, obsolete: 0, sense_key: 0,
            info_msb: 0, info_3: [0; 3], additional_sense_length: 0,
            cmd_spec_info_msb: 0, cmd_spec_info: [0; 3],
            asc: 0, ascq: 0, fru_code: 0,
            sks_msb: 0, sks: [0; 2],
        }
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 18 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    /// Response code (bits 6:0).
    pub fn response_code_val(&self) -> u8 { self.response_code & 0x7F }

    /// Valid bit (bit 7).
    pub fn valid(&self) -> bool { (self.response_code & 0x80) != 0 }

    /// Sense Key (bits 3:0 of byte 2).
    pub fn sense_key_val(&self) -> u8 { self.sense_key & 0x0F }

    /// Filemark / EOM / ILI (bits 7:5 of byte 2).
    pub fn filemark(&self) -> bool { (self.sense_key & 0x80) != 0 }
    pub fn eom(&self) -> bool { (self.sense_key & 0x40) != 0 }
    pub fn ili(&self) -> bool { (self.sense_key & 0x20) != 0 }

    /// Human-readable name for the sense key.
    pub fn sense_key_name(&self) -> &'static str {
        match self.sense_key_val() {
            0 => "NO SENSE",
            1 => "RECOVERED ERROR",
            2 => "NOT READY",
            3 => "MEDIUM ERROR",
            4 => "HARDWARE ERROR",
            5 => "ILLEGAL REQUEST",
            6 => "UNIT ATTENTION",
            7 => "DATA PROTECT",
            8 => "BLANK CHECK",
            9 => "VENDOR SPECIFIC",
            10 => "COPY ABORTED",
            11 => "ABORTED COMMAND",
            12 => "OBSOLETE (VOLUME OVERFLOW)",
            13 => "MISCOMPARE",
            14 => "COMPLETED",
            15 => "OBSOLETE (reserved)",
            _ => "UNKNOWN",
        }
    }

    /// Check if the sense key indicates a retryable condition.
    pub fn is_retryable(&self) -> bool {
        match self.sense_key_val() {
            2 | 6 | 11 => true,  // NOT READY, UNIT ATTENTION, ABORTED COMMAND
            _ => false,
        }
    }

    /// Check if sense indicates a fatal error (non-retryable).
    pub fn is_fatal(&self) -> bool {
        match self.sense_key_val() {
            3 | 4 | 7 | 10 | 13 => true,  // MEDIUM/HARDWARE/DATA PROTECT/COPY ABORTED/MISCOMPARE
            _ => false,
        }
    }

    /// Format sense info as a byte slice for debug logging.
    pub fn as_bytes(&self) -> &[u8; 18] {
        unsafe { &*(self as *const Self as *const [u8; 18]) }
    }
}

/// SCSI Sense Key constants.
pub mod sense_key {
    pub const NO_SENSE: u8 = 0x00;
    pub const RECOVERED_ERROR: u8 = 0x01;
    pub const NOT_READY: u8 = 0x02;
    pub const MEDIUM_ERROR: u8 = 0x03;
    pub const HARDWARE_ERROR: u8 = 0x04;
    pub const ILLEGAL_REQUEST: u8 = 0x05;
    pub const UNIT_ATTENTION: u8 = 0x06;
    pub const DATA_PROTECT: u8 = 0x07;
    pub const BLANK_CHECK: u8 = 0x08;
    pub const VENDOR_SPECIFIC: u8 = 0x09;
    pub const COPY_ABORTED: u8 = 0x0A;
    pub const ABORTED_COMMAND: u8 = 0x0B;
    pub const VOLUME_OVERFLOW: u8 = 0x0D;
    pub const MISCOMPARE: u8 = 0x0E;
    pub const COMPLETED: u8 = 0x0F;
}

/// Common ASC/ASCQ (Additional Sense Code / Qualifier) values.
pub mod asc {
    pub const NO_ADDITIONAL_SENSE_INFO: u8 = 0x00;
    pub const LUN_NOT_READY_CAUSE_NOT_REPORTABLE: u8 = 0x04;
    pub const LOGICAL_UNIT_NOT_READY_INIT_REQ: u8 = 0x04;
    pub const LOGICAL_UNIT_NOT_READY_FORMAT: u8 = 0x04;
    pub const LOGICAL_UNIT_NOT_READY_IN_PROGRESS: u8 = 0x04;
    pub const LOGICAL_UNIT_COMMUNICATION_FAILURE: u8 = 0x08;
    pub const UNRECOVERED_READ_ERROR: u8 = 0x11;
    pub const MISCOMPARE_IN_DATA: u8 = 0x1D;
    pub const INVALID_COMMAND_OPCODE: u8 = 0x20;
    pub const MEDIA_CHANGE: u8 = 0x28;
    pub const MEDIUM_NOT_PRESENT: u8 = 0x3A;
    pub const POWER_ON_RESET: u8 = 0x29;
    pub const MEDIUM_REMOVAL_PREVENTED: u8 = 0x53;
}

pub mod ascq {
    pub const NO_ADDITIONAL_SENSE: u8 = 0x00;
    pub const LUN_NOT_READY_INIT_REQ: u8 = 0x02;
    pub const LUN_NOT_READY_FORMAT: u8 = 0x04;
    pub const LUN_NOT_READY_IN_PROGRESS: u8 = 0x01;
    pub const MEDIUM_NOT_PRESENT_TRAY_CLOSED: u8 = 0x02;
    pub const MEDIUM_NOT_PRESENT_TRAY_OPEN: u8 = 0x01;
}

// ============================================================================
// SCSI Read Capacity 10 Response (8 bytes)
// ============================================================================

/// Parsed READ CAPACITY(10) response.
pub struct ReadCapacity10 {
    pub last_lba: u32,
    pub block_size: u32,
}

impl ReadCapacity10 {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 { return None; }
        let last_lba = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let block_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        Some(Self { last_lba, block_size })
    }

    pub fn total_blocks(&self) -> u64 { (self.last_lba as u64) + 1 }
}

/// USB transfer type constants (from EndpointDescriptor.bmAttributes).
pub mod usb_xfer_type {
    pub const CONTROL: u8 = 0;
    pub const ISOCHRONOUS: u8 = 1;
    pub const BULK: u8 = 2;
    pub const INTERRUPT: u8 = 3;
}

// ============================================================================
// USB Class Codes
// ============================================================================

pub mod usb_class {
    pub const PER_INTERFACE: u8 = 0x00;
    pub const AUDIO: u8 = 0x01;
    pub const COMMUNICATIONS: u8 = 0x02;
    pub const HID: u8 = 0x03;
    pub const PHYSICAL: u8 = 0x05;
    pub const IMAGE: u8 = 0x06;
    pub const PRINTER: u8 = 0x07;
    pub const MASS_STORAGE: u8 = 0x08;
    pub const HUB: u8 = 0x09;
    pub const CDC_DATA: u8 = 0x0A;
    pub const SMART_CARD: u8 = 0x0B;
    pub const VIDEO: u8 = 0x0E;
    pub const WIRELESS: u8 = 0xE0;
}

// ============================================================================
// USB Hub Descriptor + Port Features (USB 2.0 §11)
// ============================================================================

/// USB Hub descriptor type code.
pub const USB_DT_HUB: u8 = 0x29;
/// USB 3.0 Hub descriptor type code.
pub const USB_DT_SS_HUB: u8 = 0x2A;

/// Hub descriptor (USB 2.0) — variable length.
/// Standard USB 2.0 §11.23.2.1.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct HubDescriptor {
    pub bDescLength: u8,
    pub bDescriptorType: u8,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: [u8; 2],
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    // Followed by: DeviceRemovable (variable), PortPwrCtrlMask (variable)
}

impl HubDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    pub fn num_ports(&self) -> u8 { self.bNbrPorts }
    pub fn hub_characteristics(&self) -> u16 { u16::from_le_bytes(self.wHubCharacteristics) }
    pub fn is_tt_required(&self) -> bool { (self.hub_characteristics() & 0x03) != 0 } // Non-individual port power switching
    pub fn is_individual_power(&self) -> bool { (self.hub_characteristics() & 0x04) != 0 }
    pub fn is_compound_device(&self) -> bool { (self.hub_characteristics() & 0x08) != 0 }
    pub fn power_on_to_good(&self) -> u8 { self.bPwrOn2PwrGood }
    pub fn tt_think_time(&self) -> u8 { ((self.hub_characteristics() >> 5) & 0x03) as u8 }
}

/// USB 3.0 Hub descriptor.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct SsHubDescriptor {
    pub bDescLength: u8,
    pub bDescriptorType: u8,
    pub bNbrPorts: u8,
    pub wHubCharacteristics: [u8; 2],
    pub bPwrOn2PwrGood: u8,
    pub bHubContrCurrent: u8,
    pub bHubHdrDecLat: u8,
    pub wHubDelay: [u8; 2],
    pub bDeviceRemovable: u16,
}

impl SsHubDescriptor {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 12 { return None; }
        Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Self) })
    }

    pub fn num_ports(&self) -> u8 { self.bNbrPorts }
    pub fn hub_characteristics(&self) -> u16 { u16::from_le_bytes(self.wHubCharacteristics) }
}

/// Hub class request codes.
pub mod hub_req {
    /// Get port status
    pub const GET_PORT_STATUS: u8 = 0;
    /// Clear port feature
    pub const CLEAR_PORT_FEATURE: u8 = 1;
    /// Set port feature
    pub const SET_PORT_FEATURE: u8 = 3;
    /// Get hub descriptor
    pub const GET_HUB_DESCRIPTOR: u8 = 6;
}

/// Hub port feature selectors (USB 2.0 §11.24.2.2).
pub mod hub_port_feature {
    pub const PORT_CONNECTION: u16 = 0;
    pub const PORT_ENABLE: u16 = 1;
    pub const PORT_SUSPEND: u16 = 2;
    pub const PORT_OVER_CURRENT: u16 = 3;
    pub const PORT_RESET: u16 = 4;
    pub const PORT_POWER: u16 = 8;
    pub const PORT_LOW_SPEED: u16 = 9;
    pub const PORT_HIGH_SPEED: u16 = 10;
    pub const PORT_TEST: u16 = 21;
    pub const PORT_INDICATOR: u16 = 22;
}

/// Hub port status bits (USB 2.0 §11.24.2.1).
pub mod hub_port_status {
    pub const PORT_CONNECTION: u16 = 1 << 0;
    pub const PORT_ENABLE: u16 = 1 << 1;
    pub const PORT_SUSPEND: u16 = 1 << 2;
    pub const PORT_OVER_CURRENT: u16 = 1 << 3;
    pub const PORT_RESET: u16 = 1 << 4;
    pub const PORT_POWER: u16 = 1 << 8;
    pub const PORT_LOW_SPEED: u16 = 1 << 9;
    pub const PORT_HIGH_SPEED: u16 = 1 << 10;
}

/// Hub port change bits (u16 — change field from USB hub port status request).
/// These correspond to bits 16-20 of the 32-bit port status register.
pub mod hub_port_change {
    pub const PORT_CONNECTION: u16 = 1 << 0;   // was bit 16 in 32-bit
    pub const PORT_ENABLE: u16 = 1 << 1;       // was bit 17
    pub const PORT_SUSPEND: u16 = 1 << 2;      // was bit 18
    pub const PORT_OVER_CURRENT: u16 = 1 << 3;  // was bit 19
    pub const PORT_RESET: u16 = 1 << 4;        // was bit 20
}

// ============================================================================
// USB HID Class Constants
// ============================================================================

/// HID descriptor type (class-specific).
pub const USB_HID_DT_HID: u8 = 0x21;
/// HID report descriptor type.
pub const USB_HID_DT_REPORT: u8 = 0x22;
/// HID physical descriptor type.
pub const USB_HID_DT_PHYSICAL: u8 = 0x23;

/// HID class-specific request codes.
pub mod hid_req {
    /// Get/set idle rate (8-bit report interval in 4ms units).
    pub const SET_IDLE: u8 = 0x0A;
    pub const GET_IDLE: u8 = 0x02;
    /// Get/set protocol (0=Boot, 1=Report).
    pub const SET_PROTOCOL: u8 = 0x0B;
    pub const GET_PROTOCOL: u8 = 0x03;
    /// Get/set report (control transfer for feature/output reports).
    pub const SET_REPORT: u8 = 0x09;
    pub const GET_REPORT: u8 = 0x01;
}

// ============================================================================
// HID Report Descriptor Item Types
// ============================================================================

/// HID report descriptor short item prefix.
pub mod hid_item {
    /// Item tag bit shift within prefix byte.
    pub const TAG_SHIFT: u8 = 4;
    pub const TAG_MASK: u8 = 0xF0;
    /// Item type bits.
    pub const TYPE_SHIFT: u8 = 2;
    pub const TYPE_MASK: u8 = 0x0C;
    /// Data size (0=0 bytes, 1=1, 2=2, 3=4).
    pub const SIZE_SHIFT: u8 = 0;
    pub const SIZE_MASK: u8 = 0x03;

    /// Item types.
    pub const TYPE_MAIN: u8 = 0x00;
    pub const TYPE_GLOBAL: u8 = 0x01;
    pub const TYPE_LOCAL: u8 = 0x02;

    // Main item tags (type=0).
    pub const TAG_INPUT: u8 = 0x08;
    pub const TAG_OUTPUT: u8 = 0x09;
    pub const TAG_FEATURE: u8 = 0x0B;
    pub const TAG_COLLECTION: u8 = 0x0A;
    pub const TAG_END_COLLECTION: u8 = 0x0C;

    // Global item tags (type=1).
    pub const TAG_USAGE_PAGE: u8 = 0x00;
    pub const TAG_LOGICAL_MIN: u8 = 0x01;
    pub const TAG_LOGICAL_MAX: u8 = 0x02;
    pub const TAG_PHYSICAL_MIN: u8 = 0x03;
    pub const TAG_PHYSICAL_MAX: u8 = 0x04;
    pub const TAG_UNIT_EXPONENT: u8 = 0x05;
    pub const TAG_UNIT: u8 = 0x06;
    pub const TAG_REPORT_SIZE: u8 = 0x07;
    pub const TAG_REPORT_ID: u8 = 0x08;
    pub const TAG_REPORT_COUNT: u8 = 0x09;
    pub const TAG_PUSH: u8 = 0x0A;
    pub const TAG_POP: u8 = 0x0B;

    // Local item tags (type=2).
    pub const TAG_USAGE: u8 = 0x00;
    pub const TAG_USAGE_MIN: u8 = 0x01;
    pub const TAG_USAGE_MAX: u8 = 0x02;

    // Collection types.
    pub const COL_PHYSICAL: u8 = 0x00;
    pub const COL_APPLICATION: u8 = 0x01;
    pub const COL_LOGICAL: u8 = 0x02;
    pub const COL_REPORT: u8 = 0x03;

    /// Input item flags.
    pub mod input {
        pub const DATA_CONST: u16 = 1 << 0;    // 0=Data, 1=Constant
        pub const VAR_ARRAY: u16 = 1 << 1;     // 0=Array, 1=Variable
        pub const ABS_REL: u16 = 1 << 2;       // 0=Absolute, 1=Relative
        pub const NO_WRAP: u16 = 1 << 3;       // 0=No wrap, 1=Wrap
        pub const LINEAR: u16 = 1 << 4;        // 0=Linear, 1=Non-linear
        pub const PREFERRED: u16 = 1 << 5;     // 0=Preferred, 1=No preferred
        pub const NULL_STATE: u16 = 1 << 6;    // 0=No null, 1=Null state
        pub const VOLATILE: u16 = 1 << 7;      // 0=Non-volatile, 1=Volatile
        pub const BUFFERED: u16 = 1 << 8;      // 0=Bit field, 1=Buffered bytes
    }
}

// ============================================================================
// HID Usage Tables (from USB HID Usage Tables v1.12)
// ============================================================================

/// HID Usage Pages.
pub mod hid_usage_page {
    pub const GENERIC_DESKTOP: u16 = 0x01;
    pub const SIMULATION: u16 = 0x02;
    pub const VR: u16 = 0x03;
    pub const SPORT: u16 = 0x04;
    pub const GAME: u16 = 0x05;
    pub const GENERIC_DEVICE: u16 = 0x06;
    pub const KEYBOARD_KEYPAD: u16 = 0x07;
    pub const LED: u16 = 0x08;
    pub const BUTTON: u16 = 0x09;
    pub const ORDINAL: u16 = 0x0A;
    pub const TELEPHONY: u16 = 0x0B;
    pub const CONSUMER: u16 = 0x0C;
    pub const DIGITIZER: u16 = 0x0D;
    pub const PID: u16 = 0x0F;
    pub const UNICODE: u16 = 0x10;
    pub const ALPHANUMERIC: u16 = 0x14;
    pub const MEDICAL: u16 = 0x40;
    pub const MONITOR: u16 = 0x80;
    pub const POWER: u16 = 0x84;
    pub const BARCODE: u16 = 0x8C;
    pub const SCALE: u16 = 0x8D;
    pub const MSR: u16 = 0x8E;
    pub const CAMERA: u16 = 0x90;
    pub const ARCADE: u16 = 0x91;
    pub const VENDOR: u16 = 0xFF00;
}

/// Generic Desktop Page usages (usage page 0x01).
pub mod hid_generic_desktop {
    pub const POINTER: u16 = 0x01;
    pub const MOUSE: u16 = 0x02;
    pub const JOYSTICK: u16 = 0x04;
    pub const GAMEPAD: u16 = 0x05;
    pub const KEYBOARD: u16 = 0x06;
    pub const KEYPAD: u16 = 0x07;
    pub const MULTI_AXIS: u16 = 0x08;
    pub const X: u16 = 0x30;
    pub const Y: u16 = 0x31;
    pub const Z: u16 = 0x32;
    pub const RX: u16 = 0x33;
    pub const RY: u16 = 0x34;
    pub const RZ: u16 = 0x35;
    pub const SLIDER: u16 = 0x36;
    pub const DIAL: u16 = 0x37;
    pub const WHEEL: u16 = 0x38;
    pub const HAT_SWITCH: u16 = 0x39;
}

/// Keyboard/Keypad Page usages (usage page 0x07) — modifier keys.
pub mod hid_keyboard {
    pub const ERROR_ROLLOVER: u8 = 0x01;
    pub const POST_FAIL: u8 = 0x02;
    pub const ERROR_UNDEFINED: u8 = 0x03;
    pub const A: u8 = 0x04;
    pub const B: u8 = 0x05;
    pub const C: u8 = 0x06;
    pub const D: u8 = 0x07;
    pub const E: u8 = 0x08;
    pub const F: u8 = 0x09;
    pub const G: u8 = 0x0A;
    pub const H: u8 = 0x0B;
    pub const I: u8 = 0x0C;
    pub const J: u8 = 0x0D;
    pub const K: u8 = 0x0E;
    pub const L: u8 = 0x0F;
    pub const M: u8 = 0x10;
    pub const N: u8 = 0x11;
    pub const O: u8 = 0x12;
    pub const P: u8 = 0x13;
    pub const Q: u8 = 0x14;
    pub const R: u8 = 0x15;
    pub const S: u8 = 0x16;
    pub const T: u8 = 0x17;
    pub const U: u8 = 0x18;
    pub const V: u8 = 0x19;
    pub const W: u8 = 0x1A;
    pub const X: u8 = 0x1B;
    pub const Y: u8 = 0x1C;
    pub const Z: u8 = 0x1D;
    pub const _1: u8 = 0x1E;
    pub const _2: u8 = 0x1F;
    pub const _3: u8 = 0x20;
    pub const _4: u8 = 0x21;
    pub const _5: u8 = 0x22;
    pub const _6: u8 = 0x23;
    pub const _7: u8 = 0x24;
    pub const _8: u8 = 0x25;
    pub const _9: u8 = 0x26;
    pub const _0: u8 = 0x27;
    pub const ENTER: u8 = 0x28;
    pub const ESCAPE: u8 = 0x29;
    pub const BACKSPACE: u8 = 0x2A;
    pub const TAB: u8 = 0x2B;
    pub const SPACEBAR: u8 = 0x2C;
    pub const MINUS: u8 = 0x2D;
    pub const EQUAL: u8 = 0x2E;
    pub const LEFT_BRACKET: u8 = 0x2F;
    pub const RIGHT_BRACKET: u8 = 0x30;
    pub const BACKSLASH: u8 = 0x31;
    pub const HASH: u8 = 0x32;
    pub const SEMICOLON: u8 = 0x33;
    pub const QUOTE: u8 = 0x34;
    pub const GRAVE: u8 = 0x35;
    pub const COMMA: u8 = 0x36;
    pub const DOT: u8 = 0x37;
    pub const SLASH: u8 = 0x38;
    pub const CAPS_LOCK: u8 = 0x39;
    pub const F1: u8 = 0x3A;
    pub const F2: u8 = 0x3B;
    pub const F3: u8 = 0x3C;
    pub const F4: u8 = 0x3D;
    pub const F5: u8 = 0x3E;
    pub const F6: u8 = 0x3F;
    pub const F7: u8 = 0x40;
    pub const F8: u8 = 0x41;
    pub const F9: u8 = 0x42;
    pub const F10: u8 = 0x43;
    pub const F11: u8 = 0x44;
    pub const F12: u8 = 0x45;
    pub const PRINT_SCREEN: u8 = 0x46;
    pub const SCROLL_LOCK: u8 = 0x47;
    pub const PAUSE: u8 = 0x48;
    pub const INSERT: u8 = 0x49;
    pub const HOME: u8 = 0x4A;
    pub const PAGE_UP: u8 = 0x4B;
    pub const DELETE: u8 = 0x4C;
    pub const END: u8 = 0x4D;
    pub const PAGE_DOWN: u8 = 0x4E;
    pub const RIGHT_ARROW: u8 = 0x4F;
    pub const LEFT_ARROW: u8 = 0x50;
    pub const DOWN_ARROW: u8 = 0x51;
    pub const UP_ARROW: u8 = 0x52;
    pub const NUM_LOCK: u8 = 0x53;
    pub const KEYPAD_DIVIDE: u8 = 0x54;
    pub const KEYPAD_MULTIPLY: u8 = 0x55;
    pub const KEYPAD_SUBTRACT: u8 = 0x56;
    pub const KEYPAD_ADD: u8 = 0x57;
    pub const KEYPAD_ENTER: u8 = 0x58;
    pub const KEYPAD_1: u8 = 0x59;
    pub const KEYPAD_2: u8 = 0x5A;
    pub const KEYPAD_3: u8 = 0x5B;
    pub const KEYPAD_4: u8 = 0x5C;
    pub const KEYPAD_5: u8 = 0x5D;
    pub const KEYPAD_6: u8 = 0x5E;
    pub const KEYPAD_7: u8 = 0x5F;
    pub const KEYPAD_8: u8 = 0x60;
    pub const KEYPAD_9: u8 = 0x61;
    pub const KEYPAD_0: u8 = 0x62;
    pub const KEYPAD_DOT: u8 = 0x63;
    pub const KEYPAD_SLASH: u8 = 0x64;  // 102nd key (non-US backslash)
    pub const APPLICATION: u8 = 0x65;
    pub const POWER: u8 = 0x66;
    pub const KEYPAD_EQUAL: u8 = 0x67;
    pub const F13: u8 = 0x68;
    pub const F14: u8 = 0x69;
    pub const F15: u8 = 0x6A;
    pub const F16: u8 = 0x6B;
    pub const F17: u8 = 0x6C;
    pub const F18: u8 = 0x6D;
    pub const F19: u8 = 0x6E;
    pub const F20: u8 = 0x6F;
    pub const F21: u8 = 0x70;
    pub const F22: u8 = 0x71;
    pub const F23: u8 = 0x72;
    pub const F24: u8 = 0x73;

    // Modifier key usages (usage 0xE0-0xE7 on Keyboard/Keypad page 0x07).
    pub const LEFT_CTRL: u8 = 0xE0;
    pub const LEFT_SHIFT: u8 = 0xE1;
    pub const LEFT_ALT: u8 = 0xE2;
    pub const LEFT_GUI: u8 = 0xE3;
    pub const RIGHT_CTRL: u8 = 0xE4;
    pub const RIGHT_SHIFT: u8 = 0xE5;
    pub const RIGHT_ALT: u8 = 0xE6;
    pub const RIGHT_GUI: u8 = 0xE7;
}

/// Button Page usages (usage page 0x09).
pub mod hid_button {
    // Button 1 = usage 0x01, Button 2 = 0x02, etc.
    pub const PRIMARY: u8 = 0x01;   // Left button
    pub const SECONDARY: u8 = 0x02; // Right button
    pub const TERTIARY: u8 = 0x03;  // Middle button
}

/// Default control EP 0 max packet size per speed.
pub fn default_max_packet_size(speed: u8) -> u16 {
    match speed {
        speed::LOW => 8,
        speed::FULL => 64,
        speed::HIGH => 64,
        speed::SUPER => 512,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trb_size() {
        assert_eq!(core::mem::size_of::<Trb>(), 16);
    }

    #[test]
    fn event_ring_segment_size() {
        assert_eq!(core::mem::size_of::<ErstEntry>(), 16);
    }

    #[test]
    fn device_context_size() {
        assert_eq!(core::mem::size_of::<SlotContext>(), 32);
        assert_eq!(core::mem::size_of::<EndpointContext>(), 32);
        assert_eq!(core::mem::size_of::<InputContext>(), 8 + 32 + 31 * 32);
    }

    #[test]
    fn trb_cycle_type() {
        let mut trb = Trb::zeroed();
        trb.set_cycle_type(true, TrbType::EnableSlotCmd);
        assert!(trb.cycle());
        assert_eq!(trb.trb_type(), Some(TrbType::EnableSlotCmd));
    }

    #[test]
    fn enable_slot_cmd() {
        let trb = build_enable_slot_trb(true);
        assert!(trb.cycle());
        assert_eq!(trb.trb_type(), Some(TrbType::EnableSlotCmd));
    }

    #[test]
    fn address_device_cmd() {
        let trb = build_address_device_trb(true, 1, 0x1000, false);
        assert_eq!(trb.trb_type(), Some(TrbType::AddressDeviceCmd));
        assert_eq!(trb.get_parameter(), 0x1000);
        assert_eq!(trb.status[0], 1);
    }

    #[test]
    fn normal_transfer() {
        let trb = build_normal_transfer_trb(true, 0x200000, 4096, 0, 0, false, true);
        assert_eq!(trb.trb_type(), Some(TrbType::Normal));
        assert_eq!(trb.get_parameter(), 0x200000);
        // IOC = bit 5
        assert_ne!(trb.get_flags() & 0x20, 0);
    }

    #[test]
    fn link_trb() {
        let trb = build_link_trb(true, 0x100000, false, false);
        assert_eq!(trb.trb_type(), Some(TrbType::Link));
        assert_eq!(trb.get_parameter(), 0x100000);
    }

    #[test]
    fn setup_stage_trb() {
        let setup = [0x80u8, 0x06, 0x00, 0x01, 0x00, 0x00, 0x40, 0x00]; // GET_DESCRIPTOR
        let trb = build_setup_stage_trb(true, &setup, 0, 2);
        assert_eq!(trb.trb_type(), Some(TrbType::SetupStage));
        assert_eq!(trb.parameter, setup);
    }

    #[test]
    fn input_context() {
        let ctx = InputContext::zeroed();
        assert_eq!(ctx.add_flags, [0; 4]);
        // Not all EP contexts need to be initialized for size check
        assert!(core::mem::size_of::<InputContext>() > 8 + 32);
    }

    #[test]
    fn speed_defaults() {
        assert_eq!(default_max_packet_size(speed::LOW), 8);
        assert_eq!(default_max_packet_size(speed::FULL), 64);
        assert_eq!(default_max_packet_size(speed::HIGH), 64);
        assert_eq!(default_max_packet_size(speed::SUPER), 512);
    }
}
