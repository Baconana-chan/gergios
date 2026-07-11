//! # RFCOMM — Serial Port Emulation Protocol
//!
//! RFCOMM provides serial port emulation over Bluetooth L2CAP.
//! It supports up to 30 simultaneous virtual serial ports (DLCIs 2-30)
//! over a single L2CAP channel using a multiplexer protocol.
//!
//! ## Frame Format
//!
//! ```text
//! | Address (1) | Control (1) | Length (1-2) | Data (optional) | FCS (1) |
//! ```
//!
//! ## Address Field
//!
//! ```text
//! Bit 0: EA (Extended Address) — 1 = last byte of address
//! Bit 1: C/R (Command/Response)
//! Bits 2-7: DLCI (6 bits)
//! ```
//!
//! ## Control Field
//!
//! | Value | Type | Description |
//! |-------|------|-------------|
//! | 0x1F | SABM | Set Asynchronous Balanced Mode |
//! | 0x63 | UA | Unnumbered Acknowledgement |
//! | 0x0F | DM | Disconnected Mode |
//! | 0x43 | DISC | Disconnect |
//! | 0xEF | UIH | Unnumbered Information with Header check |

#![allow(dead_code)]

// ============================================================================
// Constants
// ============================================================================

/// RFCOMM default MTU.
pub const RFCOMM_DEFAULT_MTU: u16 = 127;
/// Minimum MTU per Bluetooth spec.
pub const RFCOMM_MIN_MTU: u16 = 23;
/// Maximum DLCI value (6-bit).
pub const RFCOMM_MAX_DLCI: u8 = 30;
/// DLCI for multiplexer control channel.
pub const RFCOMM_MUX_DLCI: u8 = 0;
/// Server channel number base (DLCI = channel * 2 + 1).
pub const RFCOMM_SERVER_CHANNEL_BASE: u8 = 1;
/// Maximum number of RFCOMM sessions per L2CAP channel.
pub const RFCOMM_MAX_SESSIONS: usize = 1;

// ============================================================================
// Address Field
// ============================================================================

/// Build an RFCOMM address byte.
///
/// # Format
/// - Bit 0: EA (1 = final byte)
/// - Bit 1: C/R (0 = command, 1 = response)
/// - Bits 2-7: DLCI (6 bits)
///
/// Address = (DLCI << 2) | (C/R << 1) | EA(1)
pub fn build_address(dlci: u8, command: bool) -> u8 {
    let cr = if command { 0 } else { 1 };
    (dlci << 2) | (cr << 1) | 0x01
}

/// Parse an RFCOMM address byte.
/// Returns (dlci, is_command) or None if invalid.
pub fn parse_address(addr: u8) -> Option<(u8, bool)> {
    let ea = addr & 0x01;
    if ea != 0x01 {
        return None; // Extended address — must be final byte
    }
    let cr = (addr >> 1) & 0x01;
    let dlci = addr >> 2;
    if dlci > RFCOMM_MAX_DLCI {
        return None;
    }
    Some((dlci, cr == 0)) // C/R = 0 → command
}

// ============================================================================
// Control Field
// ============================================================================

/// RFCOMM frame types (control field values).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RfcommControl {
    /// Set Asynchronous Balanced Mode (establish connection).
    Sabm = 0x1F,
    /// Unnumbered Acknowledgement (acknowledge SABM/DISC).
    Ua = 0x63,
    /// Disconnected Mode (reject SABM or confirm DISC).
    Dm = 0x0F,
    /// Disconnect (terminate connection).
    Disc = 0x43,
    /// Unnumbered Information with Header check (data frames).
    Uih = 0xEF,
}

impl RfcommControl {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x1F => Some(Self::Sabm),
            0x63 => Some(Self::Ua),
            0x0F => Some(Self::Dm),
            0x43 => Some(Self::Disc),
            0xEF => Some(Self::Uih),
            _ => None,
        }
    }
}

// ============================================================================
// Length Field
// ============================================================================

/// Encode RFCOMM length field (1 or 2 bytes).
/// Returns (length_bytes, total_length_field_size).
pub fn encode_length(len: usize) -> Vec<u8> {
    if len <= 0x7F {
        // 1-byte: EA=1, length in bits 1-7
        vec![((len as u8) << 1) | 0x01]
    } else if len <= 0x7FFF {
        // 2-byte: first byte EA=0, second byte EA=1
        let b0 = ((len as u8) << 1) & 0xFE; // bits 0-6 of length, EA=0
        let b1 = (((len >> 7) as u8) << 1) | 0x01; // bits 7-13 of length, EA=1
        vec![b0, b1]
    } else {
        vec![0x01] // Invalid, return 0-length
    }
}

/// Decode RFCOMM length field from bytes.
/// Returns (length, bytes_consumed) or None if incomplete.
pub fn decode_length(data: &[u8]) -> Option<(usize, usize)> {
    if data.is_empty() {
        return None;
    }
    let b0 = data[0];
    let ea = b0 & 0x01;
    let len0 = (b0 >> 1) as usize;

    if ea == 1 {
        // 1-byte length
        Some((len0, 1))
    } else if data.len() >= 2 {
        // 2-byte length
        let b1 = data[1];
        let len1 = ((b1 >> 1) as usize) << 7;
        Some((len0 | len1, 2))
    } else {
        None
    }
}

// ============================================================================
// Frame Check Sequence (FCS)
// ============================================================================

/// CRC-8 polynomial for RFCOMM FCS.
const CRC8_POLY: u8 = 0x07;

/// Compute CRC-8 for RFCOMM FCS.
/// The FCS covers the address, control, and length fields.
pub fn compute_fcs(data: &[u8]) -> u8 {
    let mut crc: u8 = 0xFF;
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ CRC8_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    crc ^ 0xFF
}

// ============================================================================
// RFCOMM Frame
// ============================================================================

/// A parsed/encoded RFCOMM frame.
#[derive(Clone, Debug)]
pub struct RfcommFrame {
    pub dlci: u8,
    pub command: bool,
    pub control: RfcommControl,
    pub data: Vec<u8>,
}

impl RfcommFrame {
    /// Build a raw RFCOMM frame from components.
    pub fn encode(&self) -> Vec<u8> {
        let address = build_address(self.dlci, self.command);
        let control = self.control as u8;
        let length = encode_length(self.data.len());
        let mut fcs_data = Vec::with_capacity(2 + length.len());
        fcs_data.push(address);
        fcs_data.push(control);
        fcs_data.extend_from_slice(&length);
        let fcs = compute_fcs(&fcs_data);

        let mut buf = Vec::with_capacity(fcs_data.len() + self.data.len() + 1);
        buf.extend_from_slice(&fcs_data);
        buf.extend_from_slice(&self.data);
        buf.push(fcs);
        buf
    }

    /// Parse an RFCOMM frame from raw bytes.
    /// Returns (frame, bytes_consumed) or None if incomplete/invalid.
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 4 {
            return None; // Minimum: addr + ctrl + len(1) + fcs
        }

        let (dlci, is_command) = parse_address(data[0])?;
        let control = RfcommControl::from_byte(data[1])?;
        let (data_len, len_size) = decode_length(&data[2..])?;

        let header_size = 2 + len_size;
        if data.len() < header_size + data_len + 1 {
            return None; // Incomplete frame
        }

        let payload = data[header_size..header_size + data_len].to_vec();
        let fcs = data[header_size + data_len];
        let total_size = header_size + data_len + 1;

        // Verify FCS
        let fcs_data = &data[..header_size];
        let expected_fcs = compute_fcs(fcs_data);
        if fcs != expected_fcs {
            return None; // FCS mismatch
        }

        Some((
            Self {
                dlci,
                command: is_command,
                control,
                data: payload,
            },
            total_size,
        ))
    }
}

// ============================================================================
// DLCI Management
// ============================================================================

/// RFCOMM DLCI direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlciDirection {
    /// Initiator (DTE) — DLCI = channel * 2
    Initiator(u8),
    /// Responder (DCE) — DLCI = channel * 2 + 1
    Responder(u8),
}

impl DlciDirection {
    /// Get the DLCI value.
    pub fn dlci(&self) -> u8 {
        match self {
            Self::Initiator(ch) => ch * 2,
            Self::Responder(ch) => ch * 2 + 1,
        }
    }

    /// Get the channel number.
    pub fn channel(&self) -> u8 {
        match self {
            Self::Initiator(ch) => *ch,
            Self::Responder(ch) => *ch,
        }
    }

    /// Create from DLCI value. Returns (channel, direction).
    pub fn from_dlci(dlci: u8) -> Option<(u8, DlciDirection)> {
        if dlci == 0 || dlci > RFCOMM_MAX_DLCI {
            return None;
        }
        let channel = dlci / 2;
        if dlci % 2 == 0 {
            Some((channel, Self::Initiator(channel)))
        } else {
            Some((channel, Self::Responder(channel)))
        }
    }
}

// ============================================================================
// Multiplexer Commands (sent on DLCI 0)
// ============================================================================

/// RFCOMM multiplexer command types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MuxCommandType {
    /// Modem Status Command
    Msc = 0xEF,
    /// Remote Port Negotiation Command
    Rpn = 0x10,
    /// Remote Line Status
    Rls = 0x14,
    /// DLC Parameter Negotiation
    Pn = 0x20,
    /// Non-Supported Command Response
    Nsc = 0x04,
    /// Test Command
    Test = 0x08,
    /// Flow Control On/Off
    Fcon = 0xA0,
    /// Flow Control Off
    Fcoff = 0x60,
}

impl MuxCommandType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0xEF => Some(Self::Msc),
            0x10 => Some(Self::Rpn),
            0x14 => Some(Self::Rls),
            0x20 => Some(Self::Pn),
            0x04 => Some(Self::Nsc),
            0x08 => Some(Self::Test),
            0xA0 => Some(Self::Fcon),
            0x60 => Some(Self::Fcoff),
            _ => None,
        }
    }
}

/// Multiplexer command TLV: type (1) + length (1) + value (N).
#[derive(Clone, Debug)]
pub struct MuxCommand {
    pub cmd_type: MuxCommandType,
    pub data: Vec<u8>,
}

impl MuxCommand {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + self.data.len());
        buf.push(self.cmd_type as u8);
        buf.push(self.data.len() as u8);
        buf.extend_from_slice(&self.data);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 {
            return None;
        }
        let cmd_type = MuxCommandType::from_byte(data[0])?;
        let len = data[1] as usize;
        if 2 + len > data.len() {
            return None;
        }
        Some((
            Self {
                cmd_type,
                data: data[2..2 + len].to_vec(),
            },
            2 + len,
        ))
    }
}

// ============================================================================
// Modem Status Command (MSC)
// ============================================================================

/// Modem signal bits for MSC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ModemSignals {
    /// Flow Control (FC)
    pub fc: bool,
    /// Ready To Communicate (RTC)
    pub rtc: bool,
    /// Ready To Receive (RTR)
    pub rtr: bool,
    /// Incoming Call (IC)
    pub ic: bool,
    /// Data Valid (DV)
    pub dv: bool,
}

impl ModemSignals {
    pub const fn new() -> Self {
        Self {
            fc: false,
            rtc: true,
            rtr: true,
            ic: false,
            dv: false,
        }
    }

    /// Encode modem signals into MSC signal byte.
    /// Format: EA(1) | FC | RTC | RTR | 0 | IC | DV | EA(0)
    pub fn encode_signal_byte(&self) -> u8 {
        let mut b = 0x00;
        if self.fc {
            b |= 0x02;
        }
        if self.rtc {
            b |= 0x04;
        }
        if self.rtr {
            b |= 0x08;
        }
        if self.ic {
            b |= 0x40;
        }
        if self.dv {
            b |= 0x80;
        }
        b
    }

    /// Decode modem signals from MSC signal byte.
    pub fn decode_signal_byte(b: u8) -> Self {
        Self {
            fc: (b & 0x02) != 0,
            rtc: (b & 0x04) != 0,
            rtr: (b & 0x08) != 0,
            ic: (b & 0x40) != 0,
            dv: (b & 0x80) != 0,
        }
    }
}

/// Build an MSC (Modem Status Command) frame.
/// DLCI identifies the virtual serial port, signals are the modem status.
pub fn build_msc(dlci: u8, signals: &ModemSignals, command: bool) -> Vec<u8> {
    // MSC data: dlci_addr(1) + signals(1)
    let dlci_addr = (dlci << 2) | 0x01; // EA=1, C/R from DLCI perspective
    let signal_byte = signals.encode_signal_byte();

    let msc_data = vec![dlci_addr, signal_byte];
    let msc = MuxCommand {
        cmd_type: MuxCommandType::Msc,
        data: msc_data,
    };
    let mux_payload = msc.encode();

    // Send as UIH frame on DLCI 0
    let frame = RfcommFrame {
        dlci: RFCOMM_MUX_DLCI,
        command,
        control: RfcommControl::Uih,
        data: mux_payload,
    };
    frame.encode()
}

/// Parse an MSC (Modem Status Command) from raw UIH data.
/// Returns (target_dlci, signals).
pub fn parse_msc(data: &[u8]) -> Option<(u8, ModemSignals)> {
    let (cmd, _) = MuxCommand::parse(data)?;
    if cmd.data.len() < 2 {
        return None;
    }
    let target_dlci = cmd.data[0] >> 2;
    if target_dlci > RFCOMM_MAX_DLCI {
        return None;
    }
    let signals = ModemSignals::decode_signal_byte(cmd.data[1]);
    Some((target_dlci, signals))
}

// ============================================================================
// DLC Parameter Negotiation (PN)
// ============================================================================

/// DLC Parameter Negotiation command.
#[derive(Clone, Debug)]
pub struct DlcParameterNegotiation {
    pub dlci: u8,
    /// I/F flow control (Credit-based or regular)
    pub credit_based_flow: bool,
    /// Priority
    pub priority: u8,
    /// Maximum frame size (MTU)
    pub max_frame_size: u16,
    /// Initial credits (for credit-based flow control)
    pub initial_credits: u8,
}

impl DlcParameterNegotiation {
    pub fn new(dlci: u8) -> Self {
        Self {
            dlci,
            credit_based_flow: true,
            priority: 0,
            max_frame_size: RFCOMM_DEFAULT_MTU,
            initial_credits: 7,
        }
    }

    /// Encode as PN command data.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(9);
        buf.push(self.dlci << 2); // DLCI in bits 2-7, EA=0 (will use 2 bytes)
        buf.push(0x01); // EA=1
        // bit 0-3: reserved, bit 4-5: priority, bit 6: credit-based flow, bit 7: I/F channel type
        let mut flags = self.priority << 4;
        if self.credit_based_flow {
            flags |= 0x40;
        }
        buf.push(flags);
        buf.push(0x00); // reserved
        buf.extend_from_slice(&self.max_frame_size.to_le_bytes()); // 2 bytes
        buf.push(0x00); // reserved
        buf.push(0x00); // reserved
        buf.push(self.initial_credits);
        buf
    }

    /// Parse from PN command data.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        let dlci = data[0] >> 2;
        let flags = data[2];
        let credit_based_flow = (flags & 0x40) != 0;
        let priority = (flags >> 4) & 0x03;
        let max_frame_size = u16::from_le_bytes([data[4], data[5]]);
        let initial_credits = data[8];
        Some(Self {
            dlci,
            credit_based_flow,
            priority,
            max_frame_size,
            initial_credits,
        })
    }
}

/// Build a PN (DLC Parameter Negotiation) RFCOMM command.
pub fn build_pn(pn: &DlcParameterNegotiation, command: bool) -> Vec<u8> {
    let pn_data = pn.encode();
    let msc = MuxCommand {
        cmd_type: MuxCommandType::Pn,
        data: pn_data,
    };
    let mux_payload = msc.encode();
    let frame = RfcommFrame {
        dlci: RFCOMM_MUX_DLCI,
        command,
        control: RfcommControl::Uih,
        data: mux_payload,
    };
    frame.encode()
}

// ============================================================================
// RFCOMM Session State
// ============================================================================

/// State of a DLCI connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DlciState {
    /// DLCI is closed.
    Closed,
    /// SABM sent, waiting for UA.
    Connecting,
    /// DLCI is open for data.
    Open,
    /// DISC sent, waiting for UA/DM.
    Disconnecting,
}

/// RFCOMM session — manages a single L2CAP connection with multiple DLCIs.
#[derive(Clone)]
pub struct RfcommSession {
    /// L2CAP channel's remote CID (for sending data).
    pub remote_cid: u16,
    /// Whether we are the initiator (DTE) or responder (DCE).
    pub initiator: bool,
    /// Maximum frame size for this session.
    pub max_frame_size: u16,
    /// Per-DLCI state.
    pub dlci_states: [DlciState; (RFCOMM_MAX_DLCI + 1) as usize],
    /// Per-DLCI modem signals.
    pub dlci_signals: [ModemSignals; (RFCOMM_MAX_DLCI + 1) as usize],
}

impl RfcommSession {
    pub fn new(remote_cid: u16, initiator: bool) -> Self {
        Self {
            remote_cid,
            initiator,
            max_frame_size: RFCOMM_DEFAULT_MTU,
            dlci_states: [DlciState::Closed; (RFCOMM_MAX_DLCI + 1) as usize],
            dlci_signals: [ModemSignals::new(); (RFCOMM_MAX_DLCI + 1) as usize],
        }
    }

    /// Get the state of a DLCI.
    pub fn dlci_state(&self, dlci: u8) -> DlciState {
        if dlci > RFCOMM_MAX_DLCI {
            return DlciState::Closed;
        }
        self.dlci_states[dlci as usize]
    }

    /// Set the state of a DLCI.
    pub fn set_dlci_state(&mut self, dlci: u8, state: DlciState) {
        if dlci <= RFCOMM_MAX_DLCI {
            self.dlci_states[dlci as usize] = state;
        }
    }

    /// Get the modem signals for a DLCI.
    pub fn dlci_signals(&self, dlci: u8) -> ModemSignals {
        if dlci > RFCOMM_MAX_DLCI {
            return ModemSignals::new();
        }
        self.dlci_signals[dlci as usize]
    }

    /// Set the modem signals for a DLCI.
    pub fn set_dlci_signals(&mut self, dlci: u8, signals: ModemSignals) {
        if dlci <= RFCOMM_MAX_DLCI {
            self.dlci_signals[dlci as usize] = signals;
        }
    }
}

// ============================================================================
// Convenience: Build SABM/DISC/UA/DM frames
// ============================================================================

/// Build a SABM frame to establish a DLCI connection.
pub fn build_sabm(dlci: u8, command: bool) -> Vec<u8> {
    let frame = RfcommFrame {
        dlci,
        command,
        control: RfcommControl::Sabm,
        data: Vec::new(),
    };
    frame.encode()
}

/// Build a UA frame to acknowledge SABM/DISC.
pub fn build_ua(dlci: u8, command: bool) -> Vec<u8> {
    let frame = RfcommFrame {
        dlci,
        command,
        control: RfcommControl::Ua,
        data: Vec::new(),
    };
    frame.encode()
}

/// Build a DM frame to reject a connection or confirm disconnection.
pub fn build_dm(dlci: u8, command: bool) -> Vec<u8> {
    let frame = RfcommFrame {
        dlci,
        command,
        control: RfcommControl::Dm,
        data: Vec::new(),
    };
    frame.encode()
}

/// Build a DISC frame to disconnect a DLCI.
pub fn build_disc(dlci: u8, command: bool) -> Vec<u8> {
    let frame = RfcommFrame {
        dlci,
        command,
        control: RfcommControl::Disc,
        data: Vec::new(),
    };
    frame.encode()
}

/// Build a UIH data frame for a DLCI.
pub fn build_uih(dlci: u8, data: &[u8], command: bool) -> Vec<u8> {
    let frame = RfcommFrame {
        dlci,
        command,
        control: RfcommControl::Uih,
        data: data.to_vec(),
    };
    frame.encode()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_roundtrip() {
        for dlci in [0, 1, 2, 5, 10, 15, 30] {
            let addr = build_address(dlci, true);
            let (parsed_dlci, is_cmd) = parse_address(addr).unwrap();
            assert_eq!(parsed_dlci, dlci);
            assert!(is_cmd);
        }
    }

    #[test]
    fn test_address_command_response() {
        let cmd_addr = build_address(5, true);
        let rsp_addr = build_address(5, false);
        assert_ne!(cmd_addr, rsp_addr);
        assert!(parse_address(cmd_addr).unwrap().1); // command
        assert!(!parse_address(rsp_addr).unwrap().1); // response
    }

    #[test]
    fn test_invalid_dlci() {
        assert!(parse_address(0xFF).is_none());
    }

    #[test]
    fn test_length_encode_decode() {
        // 1-byte lengths
        for len in [0, 1, 63, 127] {
            let encoded = encode_length(len);
            let (decoded, consumed) = decode_length(&encoded).unwrap();
            assert_eq!(decoded, len);
            assert_eq!(consumed, 1);
        }

        // 2-byte lengths
        let len = 200;
        let encoded = encode_length(len);
        assert_eq!(encoded.len(), 2);
        let (decoded, consumed) = decode_length(&encoded).unwrap();
        assert_eq!(decoded, len);
        assert_eq!(consumed, 2);
    }

    #[test]
    fn test_frame_roundtrip() {
        let original = RfcommFrame {
            dlci: 5,
            command: true,
            control: RfcommControl::Uih,
            data: vec![0x01, 0x02, 0x03],
        };
        let encoded = original.encode();
        let (parsed, consumed) = RfcommFrame::parse(&encoded).unwrap();
        assert_eq!(parsed.dlci, 5);
        assert_eq!(parsed.command, true);
        assert_eq!(parsed.control, RfcommControl::Uih);
        assert_eq!(parsed.data, vec![0x01, 0x02, 0x03]);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn test_sabm_frame() {
        let sabm = build_sabm(5, true);
        let (parsed, _) = RfcommFrame::parse(&sabm).unwrap();
        assert_eq!(parsed.dlci, 5);
        assert_eq!(parsed.control, RfcommControl::Sabm);
        assert!(parsed.data.is_empty());
    }

    #[test]
    fn test_ua_frame() {
        let ua = build_ua(5, false);
        let (parsed, _) = RfcommFrame::parse(&ua).unwrap();
        assert_eq!(parsed.dlci, 5);
        assert_eq!(parsed.control, RfcommControl::Ua);
    }

    #[test]
    fn test_dm_frame() {
        let dm = build_dm(5, false);
        let (parsed, _) = RfcommFrame::parse(&dm).unwrap();
        assert_eq!(parsed.control, RfcommControl::Dm);
    }

    #[test]
    fn test_disc_frame() {
        let disc = build_disc(5, true);
        let (parsed, _) = RfcommFrame::parse(&disc).unwrap();
        assert_eq!(parsed.control, RfcommControl::Disc);
    }

    #[test]
    fn test_modem_signals() {
        let signals = ModemSignals {
            fc: true,
            rtc: true,
            rtr: false,
            ic: true,
            dv: true,
        };
        let encoded = signals.encode_signal_byte();
        let decoded = ModemSignals::decode_signal_byte(encoded);
        assert_eq!(signals.fc, decoded.fc);
        assert_eq!(signals.rtc, decoded.rtc);
        assert_eq!(signals.rtr, decoded.rtr);
        assert_eq!(signals.ic, decoded.ic);
        assert_eq!(signals.dv, decoded.dv);
    }

    #[test]
    fn test_msc_roundtrip() {
        let signals = ModemSignals {
            fc: false,
            rtc: true,
            rtr: true,
            ic: false,
            dv: true,
        };
        let msc_frame = build_msc(5, &signals, true);
        let (parsed_frame, _) = RfcommFrame::parse(&msc_frame).unwrap();
        assert_eq!(parsed_frame.dlci, 0); // MSC sent on mux DLCI
        assert_eq!(parsed_frame.control, RfcommControl::Uih);

        let (target_dlci, parsed_signals) = parse_msc(&parsed_frame.data).unwrap();
        assert_eq!(target_dlci, 5);
        assert_eq!(parsed_signals.fc, signals.fc);
        assert_eq!(parsed_signals.dv, signals.dv);
    }

    #[test]
    fn test_pn_roundtrip() {
        let pn = DlcParameterNegotiation {
            dlci: 5,
            credit_based_flow: true,
            priority: 0,
            max_frame_size: 512,
            initial_credits: 7,
        };
        let encoded = pn.encode();
        let parsed = DlcParameterNegotiation::parse(&encoded).unwrap();
        assert_eq!(parsed.dlci, 5);
        assert_eq!(parsed.credit_based_flow, true);
        assert_eq!(parsed.max_frame_size, 512);
        assert_eq!(parsed.initial_credits, 7);
    }

    #[test]
    fn test_dlci_direction() {
        let init = DlciDirection::Initiator(5);
        assert_eq!(init.dlci(), 10);
        assert_eq!(init.channel(), 5);

        let resp = DlciDirection::Responder(5);
        assert_eq!(resp.dlci(), 11);
        assert_eq!(resp.channel(), 5);
    }

    #[test]
    fn test_session_management() {
        let mut session = RfcommSession::new(0x0041, true);
        assert_eq!(session.dlci_state(5), DlciState::Closed);

        session.set_dlci_state(5, DlciState::Open);
        assert_eq!(session.dlci_state(5), DlciState::Open);

        let signals = ModemSignals {
            fc: false,
            rtc: true,
            rtr: true,
            ic: false,
            dv: false,
        };
        session.set_dlci_signals(5, signals);
        assert_eq!(session.dlci_signals(5).rtc, true);
    }

    #[test]
    fn test_fcs_invalid_frame() {
        let mut frame = build_uih(3, &[0x01], true);
        // Corrupt the FCS
        if let Some(last) = frame.last_mut() {
            *last ^= 0xFF;
        }
        assert!(RfcommFrame::parse(&frame).is_none());
    }

    #[test]
    fn test_large_data_frame() {
        let data: Vec<u8> = (0..200).map(|i| i as u8).collect();
        let frame = build_uih(3, &data, true);
        let (parsed, _) = RfcommFrame::parse(&frame).unwrap();
        assert_eq!(parsed.data.len(), 200);
        assert_eq!(parsed.data[0], 0);
        assert_eq!(parsed.data[199], 199);
    }

    #[test]
    fn test_modem_signals_default() {
        let s = ModemSignals::new();
        assert!(!s.fc);
        assert!(s.rtc);
        assert!(s.rtr);
        assert!(!s.ic);
        assert!(!s.dv);
    }

    #[test]
    fn test_mux_command_parse() {
        let cmd = MuxCommand {
            cmd_type: MuxCommandType::Msc,
            data: vec![0x14, 0x8F],
        };
        let encoded = cmd.encode();
        let (parsed, _) = MuxCommand::parse(&encoded).unwrap();
        assert_eq!(parsed.cmd_type, MuxCommandType::Msc);
        assert_eq!(parsed.data, vec![0x14, 0x8F]);
    }

    #[test]
    fn test_pn_build_frame() {
        let pn = DlcParameterNegotiation::new(5);
        let frame = build_pn(&pn, true);
        let (parsed, _) = RfcommFrame::parse(&frame).unwrap();
        assert_eq!(parsed.dlci, 0); // Mux DLCI
        assert_eq!(parsed.control, RfcommControl::Uih);
    }
}
