//! # Registers — Intel HDA Controller Register Definitions
//!
//! From Intel High Definition Audio Specification Rev. 1.0a.
//! All register offsets are relative to BAR0 (MMIO base).

#![allow(dead_code)]

use core::ffi::c_int;

// ============================================================================
// PCI Class Code
// ============================================================================

/// HDA PCI class: Multimedia controller (0x04), HD Audio (0x03).
pub const PCI_CLASS_MULTIMEDIA: u32 = 0x04;
pub const PCI_SUBCLASS_HDAUDIO: u32 = 0x03;

// ============================================================================
// Device ID table — Intel HDA controllers
// ============================================================================

/// Known Intel HDA controller vendor:device pairs.
/// These are the most common ICH/PCH HDA controllers.
pub const HDA_DEVICE_TABLE: &[(u16, u16)] = &[
    (0x8086, 0x2668), // ICH6
    (0x8086, 0x27D8), // ICH7
    (0x8086, 0x269A), // ICH7 Mobile
    (0x8086, 0x284B), // ICH8
    (0x8086, 0x293E), // ICH9
    (0x8086, 0x293F), // ICH9
    (0x8086, 0x3A3E), // ICH10
    (0x8086, 0x3A6E), // ICH10
    (0x8086, 0x3B56), // 5 Series
    (0x8086, 0x3B57), // 5 Series
    (0x8086, 0x1C20), // 6 Series (Cougar Point)
    (0x8086, 0x1D20), // 7 Series (Panther Point)
    (0x8086, 0x1E20), // 7 Series (Panther Point)
    (0x8086, 0x8C20), // 8 Series (Lynx Point)
    (0x8086, 0x8CA0), // 8 Series (Lynx Point)
    (0x8086, 0x9C20), // 8 Series (Lynx Point-LP)
    (0x8086, 0x9CA0), // 8 Series (Lynx Point-LP)
    (0x8086, 0x8D20), // C610/X99
    (0x8086, 0x8D21), // C610/X99
    (0x8086, 0xA170), // 100 Series (Sunrise Point)
    (0x8086, 0xA171), // 100 Series (Sunrise Point)
    (0x8086, 0xA2F0), // 200 Series (Union Point)
    (0x8086, 0x9D70), // 100 Series (Sunrise Point-LP)
    (0x8086, 0x9D71), // 100 Series (Sunrise Point-LP)
    (0x8086, 0xA348), // 300 Series (Cannon Point)
    (0x8086, 0xA348), // 300 Series
    (0x8086, 0x02C8), // Comet Lake
    (0x8086, 0x06C8), // Comet Lake
    (0x8086, 0x43C8), // Titan Ridge / Alder Lake
    (0x8086, 0x51C8), // Alder Lake-P
    (0x8086, 0x54C8), // Alder Lake-N
    (0x8086, 0x7AD0), // Raptor Lake
    (0x8086, 0xF0C8), // Jasper Lake
    (0x8086, 0x4B55), // Elkhart Lake
    (0x8086, 0x4DC8), // Tiger Lake
    (0x8086, 0xA0C8), // Ice Lake
    (0x8086, 0x1A84), // Cannon Point (100 Series)
    (0x1002, 0x4383), // AMD Hudson
    (0x1002, 0x9840), // AMD FCH
    (0x1022, 0x1457), // AMD Family 17h
    (0x1022, 0x15E3), // AMD Family 17h
    (0x10DE, 0x0BE3), // NVIDIA MCP HDMI
    (0x10DE, 0x0D96), // NVIDIA MCP HDMI
    (0x10DE, 0x0FBB), // NVIDIA GF110
    (0x10DE, 0x10EF), // NVIDIA MCP
    (0x10DE, 0x0BC8), // NVIDIA HDMI
];

// ============================================================================
// HDA Controller Register Offsets (BAR0, byte offsets)
// ============================================================================

pub mod regs {
    /// GCAP — Global Capabilities (16-bit)
    pub const GCAP: usize = 0x00;
    /// VMIN — Minor Version (8-bit)
    pub const VMIN: usize = 0x02;
    /// VMAJ — Major Version (8-bit)
    pub const VMAJ: usize = 0x03;
    /// OUTPAY — Output Payload Capability (16-bit)
    pub const OUTPAY: usize = 0x04;
    /// INPAY — Input Payload Capability (16-bit)
    pub const INPAY: usize = 0x06;
    /// GCAP2 — Extended Global Capabilities (32-bit, optional)
    pub const GCAP2: usize = 0x10;
    /// LLCH — Linked List Capabilities Header (32-bit)
    pub const LLCH: usize = 0x14;
    /// VSID — Vendor Specific ID (32-bit)
    pub const VSID: usize = 0x18;
    /// STATESTS — State Change Status (16-bit)
    pub const STATESTS: usize = 0x0E;
    /// GCTL — Global Control (32-bit)
    pub const GCTL: usize = 0x08;
    /// WAKEEN — Wake Enable (16-bit)
    pub const WAKEEN: usize = 0x0C;
    /// INTCTL — Interrupt Control (32-bit)
    pub const INTCTL: usize = 0x20;
    /// INTSTS — Interrupt Status (32-bit)
    pub const INTSTS: usize = 0x24;
    /// WALCLK — Wall Clock Counter (32-bit)
    pub const WALCLK: usize = 0x30;
    /// SSYNC — Stream Synchronization (32-bit)
    pub const SSYNC: usize = 0x38;

    // -----------------------------------------------------------------------
    // CORB — Command Output Ring Buffer (offset 0x40–0x5F)
    // -----------------------------------------------------------------------
    /// CORBLBASE — CORB Base Address Low (32-bit)
    pub const CORBLBASE: usize = 0x40;
    /// CORBUBASE — CORB Base Address High (32-bit)
    pub const CORBUBASE: usize = 0x44;
    /// CORBWP — CORB Write Pointer (16-bit)
    pub const CORBWP: usize = 0x48;
    /// CORBRP — CORB Read Pointer (16-bit)
    pub const CORBRP: usize = 0x4A;
    /// CORBCTL — CORB Control (8-bit)
    pub const CORBCTL: usize = 0x4C;
    /// CORBSTS — CORB Status (8-bit)
    pub const CORBSTS: usize = 0x4D;
    /// CORBSIZE — CORB Size (8-bit)
    pub const CORBSIZE: usize = 0x4E;

    // -----------------------------------------------------------------------
    // RIRB — Response Input Ring Buffer (offset 0x50–0x5F)
    // -----------------------------------------------------------------------
    /// RIRBLBASE — RIRB Base Address Low (32-bit)
    pub const RIRBLBASE: usize = 0x50;
    /// RIRBUBASE — RIRB Base Address High (32-bit)
    pub const RIRBUBASE: usize = 0x54;
    /// RIRBWP — RIRB Write Pointer (16-bit)
    pub const RIRBWP: usize = 0x58;
    /// RIRBCNT — RIRB Response Count (16-bit)
    pub const RIRBCNT: usize = 0x5A;
    /// RIRBCTL — RIRB Control (8-bit)
    pub const RIRBCTL: usize = 0x5C;
    /// RIRBSTS — RIRB Status (8-bit)
    pub const RIRBSTS: usize = 0x5D;
    /// RIRBSIZE — RIRB Size (8-bit)
    pub const RIRBSIZE: usize = 0x5E;

    // -----------------------------------------------------------------------
    // DMA Position Buffer (offset 0x70–0x7F)
    // -----------------------------------------------------------------------
    /// DPLBASE — DMA Position Buffer Base Address Low (32-bit)
    pub const DPLBASE: usize = 0x70;
    /// DPUBASE — DMA Position Buffer Base Address High (32-bit)
    pub const DPUBASE: usize = 0x74;

    // -----------------------------------------------------------------------
    // Immediate Command Output Interface (offset 0x78–0x7F, optional)
    // -----------------------------------------------------------------------
    /// IMMEDIATE_CMD_OUT — Immediate Command Output (32-bit)
    pub const IMMEDIATE_CMD_OUT: usize = 0x78;
    /// IMMEDIATE_CMD_IN — Immediate Command Input (32-bit)
    pub const IMMEDIATE_CMD_IN: usize = 0x7C;

    // -----------------------------------------------------------------------
    // Stream Descriptors (SDn) — base 0x80, each 0x20 bytes
    // n = 0..(GCAP.OSS + GCAP.ISS + GCAP.BSS - 1)
    // -----------------------------------------------------------------------
    /// SDnCTL — Stream Descriptor n Control/Status (8-bit)
    pub const SD_CTL: usize = 0x00;
    /// SDnSTS — Stream Descriptor n Status (8-bit)
    pub const SD_STS: usize = 0x03;
    /// SDnLPIB — Stream Descriptor n Link Position in Buffer (32-bit)
    pub const SD_LPIB: usize = 0x04;
    /// SDnCBL — Stream Descriptor n Cyclic Buffer Length (32-bit)
    pub const SD_CBL: usize = 0x08;
    /// SDnLVI — Stream Descriptor n Last Valid Index (16-bit)
    pub const SD_LVI: usize = 0x0C;
    /// SDnFIFOW — Stream Descriptor n FIFO Watermark (16-bit)
    pub const SD_FIFOW: usize = 0x0E;
    /// SDnFIFOS — Stream Descriptor n FIFO Size (16-bit)
    pub const SD_FIFOS: usize = 0x10;
    /// SDnFMT — Stream Descriptor n Format (16-bit)
    pub const SD_FMT: usize = 0x12;
    /// SDnBDPL — Stream Descriptor n Buffer Descriptor List Pointer Low (32-bit)
    pub const SD_BDPL: usize = 0x18;
    /// SDnBDPU — Stream Descriptor n Buffer Descriptor List Pointer High (32-bit)
    pub const SD_BDPU: usize = 0x1C;

    /// Compute the base offset of stream descriptor n.
    pub fn sd_base(n: u8) -> usize {
        0x80 + (n as usize) * 0x20
    }
}

// ============================================================================
// GCAP register bits
// ============================================================================

pub mod gcap {
    /// Number of Input Streams (bits 0-3).
    pub const ISS_MASK: u16 = 0x000F;
    /// Number of Output Streams (bits 4-7).
    pub const OSS_MASK: u16 = 0x00F0;
    pub const OSS_SHIFT: u16 = 4;
    /// Number of Bidirectional Streams (bits 8-11).
    pub const BSS_MASK: u16 = 0x0F00;
    pub const BSS_SHIFT: u16 = 8;
    /// Number of Serial Data Out Signals (bits 12-14).
    pub const NSDO_MASK: u16 = 0x7000;
    pub const NSDO_SHIFT: u16 = 12;
    /// 64-bit address support (bit 15, zero-indexed).
    pub const SIXTYFOUR: u16 = 1u16 << 15;

    pub fn iss(gcap: u16) -> u8 {
        (gcap & ISS_MASK) as u8
    }

    pub fn oss(gcap: u16) -> u8 {
        ((gcap & OSS_MASK) >> OSS_SHIFT) as u8
    }

    pub fn bss(gcap: u16) -> u8 {
        ((gcap & BSS_MASK) >> BSS_SHIFT) as u8
    }

    pub fn nsdo(gcap: u16) -> u8 {
        ((gcap & NSDO_MASK) >> NSDO_SHIFT) as u8
    }

    pub fn total_streams(gcap: u16) -> u8 {
        iss(gcap) + oss(gcap) + bss(gcap)
    }
}

// ============================================================================
// GCTL register bits
// ============================================================================

pub mod gctl {
    /// Controller Reset (CRST).
    pub const CRST: u32 = 1 << 0;
    /// Flush Control (FCNTRL).
    pub const FCNTRL: u32 = 1 << 1;
    /// Accept Unsolicited Responses (UNSOL).
    pub const UNSOL: u32 = 1 << 8;
}

// ============================================================================
// INTCTL / INTSTS register bits
// ============================================================================

pub mod intctl {
    /// Global Interrupt Enable (GIE).
    pub const GIE: u32 = 1u32 << 31;
    /// Controller Interrupt Enable (CIE).
    pub const CIE: u32 = 1u32 << 30;
    /// Stream Interrupt Enable bits (bits 0-29, one per stream).
    pub fn SIE(n: u8) -> u32 { 1u32 << (n as u32) }
}

pub mod intsts {
    /// Controller Interrupt Status (CIS).
    pub const CIS: u32 = 1u32 << 30;
    /// Stream Interrupt Status bits (bits 0-29).
    pub fn SIS(n: u8) -> u32 { 1u32 << (n as u32) }
}

// ============================================================================
// CORB/RIRB registers
// ============================================================================

pub mod corbctl {
    /// CORB DMA Enable.
    pub const CORB_DMA_ENABLE: u8 = 1 << 1;
}

pub mod corbsts {
    /// CORB Memory Error Indicator.
    pub const MEI: u8 = 1 << 0;
}

pub mod rirbctl {
    /// RIRB DMA Enable.
    pub const RIRB_DMA_ENABLE: u8 = 1 << 1;
    /// RIRB Interrupt Enable (response received).
    pub const RIRB_INT_ENABLE: u8 = 1 << 0;
    /// RIRB Overrun Interrupt Enable.
    pub const RIRB_OIC: u8 = 1 << 2;
}

pub mod rirbsts {
    /// RIRB Interrupt Status (response received).
    pub const RIRB_INT: u8 = 1 << 0;
    /// RIRB Overrun Status.
    pub const RIRB_OIS: u8 = 1 << 2;
}

/// CORB size encodings (write to CORBSIZE).
pub mod corbsize {
    pub const SIZE_2: u8 = 0x00;  // 2 entries
    pub const SIZE_16: u8 = 0x01; // 16 entries
    pub const SIZE_256: u8 = 0x02; // 256 entries
    /// Capabilities mask (read from CORBSIZE).
    pub const CAP_2: u8 = 0x10;
    pub const CAP_16: u8 = 0x20;
    pub const CAP_256: u8 = 0x40;
    /// Size mask.
    pub const SIZE_MASK: u8 = 0x03;
}

/// RIRB size encodings (same as CORBSIZE).
pub use corbsize as rirbsize;

// ============================================================================
// Stream Descriptor registers
// ============================================================================

pub mod sdctl {
    /// Stream Run (SRUN).
    pub const SRUN: u8 = 1 << 1;
    /// Stream Reset (SRST).
    pub const SRST: u8 = 1 << 0;
    /// DMA Direction (DIR) — 0=output (playback), 1=input (capture).
    pub const DIR: u8 = 1 << 3;
    /// FIFO Error Interrupt Enable (FIFOE).
    pub const FIFOE: u8 = 1 << 4;
    /// Descriptor Error Interrupt Enable (DEIE) — only for input.
    pub const DEIE: u8 = 1 << 5;
    /// Interrupt on Completion Enable (IOCE) — per-BDL entry.
    pub const IOCE: u8 = 1 << 6;
    /// Stream Specific Interrupt Enable (SSE) — buffer completion.
    pub const FEIE: u8 = 1 << 2;
    /// Traffic Priority (TP).
    pub const TP: u8 = 1 << 7;
    /// Stripe Control (bits 16:14).
    pub const STRIPE_SHIFT: u8 = 14;
    pub const STRIPE_MASK: u32 = 0x07 << 14;
    /// Payload Length (bits 25:20).
    pub const PAYLOAD_SHIFT: u8 = 20;
    pub const PAYLOAD_MASK: u32 = 0x3F << 20;
}

pub mod sdsts {
    /// FIFO Error (FIFOE).
    pub const FIFOE: u8 = 1 << 2;
    /// Descriptor Error (DESE).
    pub const DESE: u8 = 1 << 3;
    /// FIFO Ready (FIFORDY).
    pub const FIFORDY: u8 = 1 << 5;
    /// Buffer Completion Interrupt Status (BCIS).
    pub const BCIS: u8 = 1 << 6;
}

// ============================================================================
// Stream Format (SDnFMT)
// ============================================================================

pub mod fmt {
    /// Stream Type (bits 15:14) — 0=AC97, 1=HDA, 2=reserved, 3=reserved.
    pub const TYPE_SHIFT: u16 = 14;
    pub const TYPE_MASK: u16 = 0x3 << 14;
    pub const TYPE_AC97: u16 = 0x0 << 14;
    pub const TYPE_HDA: u16 = 0x1 << 14;

    /// Sample Base Rate (bits 13:11).
    pub const BASE_RATE_SHIFT: u16 = 11;
    pub const BASE_RATE_MASK: u16 = 0x7 << 11;
    pub const BASE_RATE_48KHZ: u16 = 0x0 << 11;
    pub const BASE_RATE_44P1KHZ: u16 = 0x1 << 11;

    /// Sample Rate Multiplier (bits 10:8).
    pub const MULT_SHIFT: u16 = 8;
    pub const MULT_MASK: u16 = 0x7 << 8;
    pub const MULT_1X: u16 = 0x0 << 8;
    pub const MULT_2X: u16 = 0x1 << 8;
    pub const MULT_3X: u16 = 0x2 << 8;
    pub const MULT_4X: u16 = 0x3 << 8;

    /// Sample Rate Divider (bits 7:6).
    pub const DIV_SHIFT: u16 = 6;
    pub const DIV_MASK: u16 = 0x3 << 6;
    pub const DIV_1: u16 = 0x0 << 6;
    pub const DIV_2: u16 = 0x1 << 6;
    pub const DIV_3: u16 = 0x2 << 6;
    pub const DIV_4: u16 = 0x3 << 6;

    /// Bits per Sample (bits 5:4).
    pub const BITS_SHIFT: u16 = 4;
    pub const BITS_MASK: u16 = 0x3 << 4;
    pub const BITS_8: u16 = 0x0 << 4;
    pub const BITS_16: u16 = 0x1 << 4;
    pub const BITS_20: u16 = 0x2 << 4;
    pub const BITS_24: u16 = 0x3 << 4;
    pub const BITS_32: u16 = 0x4 << 4;

    /// Number of Channels (bits 3:0).
    pub const CHAN_SHIFT: u16 = 0;
    pub const CHAN_MASK: u16 = 0xF;
    pub fn CHAN(count: u16) -> u16 {
        (count - 1) & 0xF
    }

    /// Build a HDA stream format word.
    /// `type_`: TYPE_HDA or TYPE_AC97
    /// `base_rate`: BASE_RATE_48KHZ or BASE_RATE_44P1KHZ
    /// `mult`: MULT_1X .. MULT_4X
    /// `div_: DIV_1 .. DIV_4
    /// `bits`: BITS_8 .. BITS_32 (encoding bits per sample)
    /// `channels`: number of channels (1-16)
    pub fn build(type_: u16, base_rate: u16, mult: u16,
                 div_: u16, bits: u16, channels: u16) -> u16
    {
        type_
            | base_rate
            | mult
            | div_
            | bits
            | ((channels.saturating_sub(1) & 0xF) << CHAN_SHIFT)
    }

    /// Get sample rate in Hz from the format word.
    pub fn sample_rate(fmt: u16) -> u32 {
        let base = if (fmt & BASE_RATE_MASK) == BASE_RATE_44P1KHZ {
            44100
        } else {
            48000
        };
        let mult_shift = ((fmt >> MULT_SHIFT) & 0x7) as u32;
        let div_shift = ((fmt >> DIV_SHIFT) & 0x3) as u32;
        let mult = match mult_shift {
            0 => 1, 1 => 2, 2 => 3, 3 => 4,
            _ => 1,
        };
        let div = match div_shift {
            0 => 1, 1 => 2, 2 => 3, 3 => 4,
            _ => 1,
        };
        (base * mult) / div
    }

    /// Get bits per sample from the format word.
    pub fn bits_per_sample(fmt: u16) -> u8 {
        let bits_code = (fmt >> BITS_SHIFT) & 0x7;
        match bits_code {
            0 => 8, 1 => 16, 2 => 20, 3 => 24, 4 => 32,
            _ => 16,
        }
    }

    /// Get number of channels from the format word.
    pub fn channels(fmt: u16) -> u8 {
        ((fmt & CHAN_MASK) + 1) as u8
    }

    /// Compute bytes per frame (one sample per channel).
    pub fn frame_size(fmt: u16) -> u8 {
        channels(fmt) * (bits_per_sample(fmt) / 8)
    }
}

// ============================================================================
// Buffer Descriptor List (BDL) entry
// ============================================================================

/// A single Buffer Descriptor List entry (16 bytes).
/// The BDL is an array of these entries in host memory.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct BdlEntry {
    /// Buffer address low (32-bit, must be 128-byte aligned).
    pub address_lo: u32,
    /// Buffer address high (32-bit).
    pub address_hi: u32,
    /// Buffer length in bytes (32-bit, max 0xFFFF).
    pub length: u32,
    /// Flags: bit 0 = IOC (Interrupt on Completion).
    pub flags: u32,
}

impl BdlEntry {
    /// Create a new BDL entry with the given physical address and length.
    /// `addr` must be 128-byte aligned.
    pub fn new(phys_addr: u64, length: u32, ioc: bool) -> Self {
        Self {
            address_lo: phys_addr as u32,
            address_hi: (phys_addr >> 32) as u32,
            length: core::cmp::min(length, 0xFFFF),
            flags: if ioc { 1 } else { 0 },
        }
    }

    pub const fn zeroed() -> Self {
        Self { address_lo: 0, address_hi: 0, length: 0, flags: 0 }
    }
}

// ============================================================================
// Codec communication — CORB/RIRB
// ============================================================================

/// HDA codec command verb (4 bytes).
/// Format:
///   - bits 31:28 — codec address (CAD)
///   - bits 27:20 — Node ID (NID)
///   - bits 19:8  — Verb ID
///   - bits 7:0   — Verb payload
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CodecCmd(u32);

impl CodecCmd {
    /// Create a new codec command verb.
    pub const fn new(cad: u8, nid: u8, verb: u16, payload: u16) -> Self {
        Self(
            ((cad as u32) << 28)
                | ((nid as u32) << 20)
                | ((verb as u32) << 8)
                | (payload as u32),
        )
    }

    pub fn raw(&self) -> u32 { self.0 }
}

/// HDA codec response (8 bytes for HDA protocol).
///   - RIRB entry: response (u32) + response_ex (u32)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CodecResp {
    /// Response payload (lower 32 bits — or full 32-bit verb response).
    pub response: u32,
    /// Extended response: bit 4 = unsolicited, bits 25:20 = SDI, bits 3:0 = CAD.
    pub response_ex: u32,
}

impl CodecResp {
    /// Codec address.
    pub fn codec_address(&self) -> u8 {
        (self.response_ex & 0xF) as u8
    }

    /// Whether this is an unsolicited response.
    pub fn is_unsolicited(&self) -> bool {
        (self.response_ex & (1 << 4)) != 0
    }

    /// Stream tag index (for unsolicited responses).
    pub fn sdi(&self) -> u8 {
        ((self.response_ex >> 20) & 0x3F) as u8
    }
}

// ============================================================================
// Codec Verbs (standard HDA register set)
// ============================================================================

pub mod verb {
    // -----------------------------------------------------------------------
    // 8-bit verbs (GET / SET)
    // -----------------------------------------------------------------------
    /// Get Parameter (GET_PARAM).
    pub const GET_PARAM: u16 = 0xF00;
    /// Set Power State.
    pub const SET_POWER_STATE: u16 = 0x705;
    /// Get Power State.
    pub const GET_POWER_STATE: u16 = 0xF05;
    /// Set Converter Format.
    pub const SET_CONVERTER_FORMAT: u16 = 0x200;
    /// Get Converter Format.
    pub const GET_CONVERTER_FORMAT: u16 = 0xA00;
    /// Get Converter Stream/Channel.
    pub const GET_CONVERTER_STREAM_CHAN: u16 = 0x600;
    /// Set Converter Stream/Channel.
    pub const SET_CONVERTER_STREAM_CHAN: u16 = 0x200;
    /// Set Amplifier Gain/Mute.
    pub const SET_AMP_GAIN_MUTE: u16 = 0x300;
    /// Get Amplifier Gain/Mute.
    pub const GET_AMP_GAIN_MUTE: u16 = 0xB00;
    /// Set Connection Select Control.
    pub const SET_CONNECTION_SELECT: u16 = 0x701;
    /// Get Connection Select Control.
    pub const GET_CONNECTION_SELECT: u16 = 0xF01;
    /// Set Connection List Entry.
    pub const SET_CONNECTION_LIST_ENTRY: u16 = 0x702;
    /// Get Connection List Entry.
    pub const GET_CONNECTION_LIST_ENTRY: u16 = 0xF02;
    /// Set Processing Coefficient.
    pub const SET_PROC_COEFF: u16 = 0x400;
    /// Get Processing Coefficient.
    pub const GET_PROC_COEFF: u16 = 0xC00;
    /// Set GPIO Data.
    pub const SET_GPIO_DATA: u16 = 0x7E0;
    /// Get GPIO Data.
    pub const GET_GPIO_DATA: u16 = 0xFE0;
    /// Set GPIO Enable.
    pub const SET_GPIO_ENABLE: u16 = 0x7E1;
    /// Get GPIO Enable.
    pub const GET_GPIO_ENABLE: u16 = 0xFE1;
    /// Set GPIO Direction.
    pub const SET_GPIO_DIRECTION: u16 = 0x7E2;
    /// Get GPIO Direction.
    pub const GET_GPIO_DIRECTION: u16 = 0xFE2;
    /// Set GPIO Wake Enable.
    pub const SET_GPIO_WAKE: u16 = 0x7E3;
    /// Get GPIO Wake Enable.
    pub const GET_GPIO_WAKE: u16 = 0xFE3;
    /// Set GPIO Sticky Mask.
    pub const SET_GPIO_STICKY: u16 = 0x7E4;
    /// Get GPIO Sticky Mask.
    pub const GET_GPIO_STICKY: u16 = 0xFE4;
    /// Set Unsolicited Response Enable.
    pub const SET_UNSOLICITED: u16 = 0x70A;
    /// Get Unsolicited Response Enable.
    pub const GET_UNSOLICITED: u16 = 0xF0A;
    /// Set Pin Widget Control.
    pub const SET_PIN_WIDGET_CTRL: u16 = 0x70C;
    /// Get Pin Widget Control.
    pub const GET_PIN_WIDGET_CTRL: u16 = 0xF0C;
    /// Set Configuration Default.
    pub const SET_CONFIG_DEFAULT: u16 = 0x71C;
    /// Get Configuration Default.
    pub const GET_CONFIG_DEFAULT_BYTE0: u16 = 0xF1C;
    /// Set EAPD/BTL Enable.
    pub const SET_EAPD_BTL: u16 = 0x70F;
    /// Get EAPD/BTL Enable.
    pub const GET_EAPD_BTL: u16 = 0xF0F;
    /// Set Volume Knob.
    pub const SET_VOLUME_KNOB: u16 = 0x70F;
    /// Get Volume Knob.
    pub const GET_VOLUME_KNOB: u16 = 0xF0F;
    /// Set Subsystem ID.
    pub const SET_SUBSYSTEM_ID: u16 = 0x720;
    /// Get Subsystem ID.
    pub const GET_SUBSYSTEM_ID: u16 = 0xF20;
    /// Set SDI Select.
    pub const SET_SDI_SELECT: u16 = 0x704;
    /// Get SDI Select.
    pub const GET_SDI_SELECT: u16 = 0xF04;
    /// Set Digital Converter Control.
    pub const SET_DIGITAL_CONV: u16 = 0x70D;
    /// Get Digital Converter Control.
    pub const GET_DIGITAL_CONV: u16 = 0xF0D;
    /// Set Audio Descriptor.
    pub const SET_AUDIO_DESC: u16 = 0x710;
    /// Get Audio Descriptor.
    pub const GET_AUDIO_DESC: u16 = 0xF10;
}

// ============================================================================
// Codec Parameter IDs (for GET_PARAM verb)
// ============================================================================

pub mod param {
    /// Vendor ID (32-bit: vendor ID in upper 16 bits, device ID in lower 16).
    pub const VENDOR_ID: u8 = 0x00;
    /// Revision ID (32-bit: major.minor in upper/lower 16 bits).
    pub const REVISION_ID: u8 = 0x02;
    /// Subordinate Node Count (8:3 = starting NID, 2:0 = count).
    pub const SUBORDINATE_NODE_COUNT: u8 = 0x04;
    /// Function Group Type (8:0 = type, 1=misc, otherwise audio).
    pub const FUNCTION_GROUP_TYPE: u8 = 0x05;
    /// Audio Function Group Capabilities.
    pub const AFG_CAPS: u8 = 0x08;
    /// Audio Widget Capabilities.
    pub const AW_CAPS: u8 = 0x09;
    /// Supported PCM Sizes/Rates.
    pub const SUPP_PCM: u8 = 0x0A;
    /// Supported PCM Stream Formats (bits per sample).
    pub const SUPP_FORMATS: u8 = 0x0B;
    /// Input Amplifier Capabilities.
    pub const IN_AMP_CAPS: u8 = 0x0D;
    /// Output Amplifier Capabilities.
    pub const OUT_AMP_CAPS: u8 = 0x12;
    /// Connection List Length.
    pub const CONNECTION_LIST_LEN: u8 = 0x0E;
    /// Supported Power States.
    pub const SUPP_POWER_STATE: u8 = 0x0F;
    /// Processing Capabilities.
    pub const PROCESSING_CAPS: u8 = 0x10;
    /// GPIO Counts.
    pub const GPIO_COUNT: u8 = 0x11;
    /// Volume Knob Capabilities.
    pub const VOLUME_KNOB_CAPS: u8 = 0x13;
    /// DAC-ADC Information.
    pub const DAC_ADC_CAPS: u8 = 0x14;
    /// Pin Capabilities.
    pub const PIN_CAPS: u8 = 0x0C;
    /// Configuration Default (4 bytes at NID + i, where i=0..3).
    pub const CONFIG_DEFAULT_BASE: u8 = 0x1C;
}

// ============================================================================
// Widget types (from Audio Widget Capabilities)
// ============================================================================

pub mod widget_type {
    pub const AUDIO_OUTPUT: u8 = 0x00;
    pub const AUDIO_INPUT: u8 = 0x01;
    pub const AUDIO_MIXER: u8 = 0x02;
    pub const AUDIO_SELECTOR: u8 = 0x03;
    pub const PIN: u8 = 0x04;
    pub const POWER: u8 = 0x05;
    pub const VOLUME_KNOB: u8 = 0x06;
    pub const BEEP_GEN: u8 = 0x07;
    pub const VENDOR: u8 = 0x0F;
}

// ============================================================================
// Pin Widget capabilities
// ============================================================================

pub mod pin_caps {
    /// Output capable.
    pub const OUTPUT: u32 = 1 << 0;
    /// Input capable.
    pub const INPUT: u32 = 1 << 1;
    /// Headphone drive capable (per-pin HP).
    pub const HP_DRV: u32 = 1 << 3;
    /// Output impedance sensing.
    pub const OUT_IMP_SENSE: u32 = 1 << 4;
    /// Trigger required for jack detection.
    pub const TRIGGER_REQ: u32 = 1 << 5;
    /// Presence detect capable.
    pub const PRES_DETECT: u32 = 1 << 7;
    /// Headphone jack sense override.
    pub const HP_SENSE: u32 = 1 << 8;
    /// Display port / HDMI pin.
    pub const DP: u32 = 1 << 24;
    /// High Definition Multimedia Interface.
    pub const HDMI: u32 = 1 << 25;
}

// ============================================================================
// Configuration Default (Pin Configuration) register (32-bit)
// ============================================================================

pub mod cfg_default {
    /// Port connectivity (bits 1:0).
    pub const PORT_MASK: u32 = 0x3;
    pub const PORT_JACK: u32 = 0x0;
    pub const PORT_NC: u32 = 0x1;
    pub const PORT_FIXED: u32 = 0x2;
    pub const PORT_BOTH: u32 = 0x3;

    /// Location (bits 7:4).
    pub const LOC_MASK: u32 = 0xF << 4;
    pub const LOC_SHIFT: u32 = 4;

    /// Default device (bits 11:8).
    pub const DEVICE_MASK: u32 = 0xF << 8;
    pub const DEVICE_SHIFT: u32 = 8;
    pub const DEVICE_LINEOUT: u32 = 0x0;
    pub const DEVICE_SPEAKER: u32 = 0x1;
    pub const DEVICE_HP_OUT: u32 = 0x2;
    pub const DEVICE_CD: u32 = 0x4;
    pub const DEVICE_SPDIF_OUT: u32 = 0x5;
    pub const DEVICE_DIGITAL_OTHER_OUT: u32 = 0x6;
    pub const DEVICE_MODEM: u32 = 0x7;
    pub const DEVICE_MIC: u32 = 0x8;
    pub const DEVICE_LINEIN: u32 = 0x9;
    pub const DEVICE_SPDIF_IN: u32 = 0xA;
    pub const DEVICE_DIGITAL_OTHER_IN: u32 = 0xB;
    pub const DEVICE_AUX: u32 = 0xF;

    /// Connection type (bits 15:12).
    pub const CONN_MASK: u32 = 0xF << 12;

    /// Color (bits 23:20).
    pub const COLOR_MASK: u32 = 0xF << 20;

    /// Misc (bits 27:24).
    pub const MISC_MASK: u32 = 0xF << 24;

    /// Default association (bits 31:28).
    pub const DEF_ASSOC_SHIFT: u32 = 28;
    pub const DEF_ASSOC_MASK: u32 = 0xF << 28;

    /// Sequence (bits 27:24) — actually part of NO_PRES_DETECT.
    pub const SEQUENCE_SHIFT: u32 = 20;
    pub const SEQUENCE_MASK: u32 = 0xF << 20;

    /// Parse a configuration default value.
    pub struct CfgDefault(pub u32);

    impl CfgDefault {
        pub fn port(&self) -> u32 { self.0 & PORT_MASK }
        pub fn location(&self) -> u32 { (self.0 >> 4) & 0xF }
        pub fn device(&self) -> u32 { (self.0 >> 8) & 0xF }
        pub fn conn_type(&self) -> u32 { (self.0 >> 12) & 0xF }
        pub fn color(&self) -> u32 { (self.0 >> 20) & 0xF }
        pub fn def_assoc(&self) -> u32 { self.0 >> 28 }
        pub fn sequence(&self) -> u32 { (self.0 >> 20) & 0xF }
        pub fn is_jack(&self) -> bool { self.port() == PORT_JACK }
        pub fn is_fixed(&self) -> bool { self.port() == PORT_FIXED }
        pub fn is_nc(&self) -> bool { self.port() == PORT_NC }
        pub fn is_loudspeaker(&self) -> bool {
            (self.0 >> 8) & 0xF == DEVICE_SPEAKER
        }
        pub fn is_hp(&self) -> bool {
            (self.0 >> 8) & 0xF == DEVICE_HP_OUT
        }
        pub fn is_mic(&self) -> bool {
            (self.0 >> 8) & 0xF == DEVICE_MIC
        }
        pub fn is_line_in(&self) -> bool {
            (self.0 >> 8) & 0xF == DEVICE_LINEIN
        }
        pub fn is_line_out(&self) -> bool {
            (self.0 >> 8) & 0xF == DEVICE_LINEOUT
        }
        pub fn is_digital(&self) -> bool {
            let dev = (self.0 >> 8) & 0xF;
            dev == DEVICE_SPDIF_OUT || dev == DEVICE_SPDIF_IN
                || dev == DEVICE_DIGITAL_OTHER_OUT
                || dev == DEVICE_DIGITAL_OTHER_IN
        }
        /// Whether this pin has presence detect (jack detection).
        pub fn has_presence_detect(&self) -> bool {
            (self.0 >> 24) & 0x8 == 0 // NO_PRES_DETECT bit
        }
    }
}

// ============================================================================
// Power States
// ============================================================================

pub mod power {
    pub const D0: u32 = 0x00;  // Fully on (active)
    pub const D1: u32 = 0x01;  // Low-power (some analog circuits off)
    pub const D2: u32 = 0x02;  // Lower-power (digital off)
    pub const D3: u32 = 0x03;  // Power off

    /// Build a SET_POWER_STATE verb payload:
    ///   bit 7 = reset, bit 1:0 = power state
    pub fn set_state(state: u8, reset: bool) -> u8 {
        if reset { state | 0x80 } else { state & 0x03 }
    }
}

// ============================================================================
// Audio Function Group Capabilities
// ============================================================================

pub mod afg_caps {
    /// Number of GPIOs (bits 7:4).
    pub const GPIO_COUNT_SHIFT: u32 = 4;
    pub const GPIO_COUNT_MASK: u32 = 0xF0;
    pub const GPIO_UNC_MASK: u32 = 1 << 16;
}

// ============================================================================
// Stream number assignment
// ============================================================================

/// Number of stream descriptors.
pub const MAX_STREAMS: u8 = 30;

/// Default audio DMA buffer size per stream (in bytes).
pub const AUDIO_DMA_BUF_SIZE: usize = 65536; // 64KB

/// Default BDL entries per stream.
pub const BDL_ENTRIES: usize = 2; // Double-buffer

/// CORB buffer size (number of command entries).
pub const CORB_ENTRIES: usize = 256;

/// RIRB buffer size (number of response entries).
pub const RIRB_ENTRIES: usize = 256;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::fmt;

    #[test]
    fn register_offsets() {
        assert_eq!(regs::GCAP, 0x00);
        assert_eq!(regs::VMIN, 0x02);
        assert_eq!(regs::VMAJ, 0x03);
        assert_eq!(regs::STATESTS, 0x0E);
        assert_eq!(regs::GCTL, 0x08);
        assert_eq!(regs::INTCTL, 0x20);
        assert_eq!(regs::INTSTS, 0x24);
        assert_eq!(regs::CORBLBASE, 0x40);
        assert_eq!(regs::RIRBLBASE, 0x50);
        assert_eq!(regs::DPLBASE, 0x70);
        assert_eq!(regs::sd_base(0), 0x80);
        assert_eq!(regs::sd_base(1), 0xA0);
        assert_eq!(regs::sd_base(15), 0x80 + 15 * 0x20);
    }

    #[test]
    fn gcap_parsing() {
        // 1 ISS, 4 OSS, 2 BSS, 1 SDO
        let gcap_val: u16 = (1) | (4 << 4) | (2 << 8) | (1 << 12);
        assert_eq!(gcap::iss(gcap_val), 1);
        assert_eq!(gcap::oss(gcap_val), 4);
        assert_eq!(gcap::bss(gcap_val), 2);
        assert_eq!(gcap::nsdo(gcap_val), 1);
        assert_eq!(gcap::total_streams(gcap_val), 7);
    }

    #[test]
    fn format_build() {
        let fmt_val = fmt::build(fmt::TYPE_HDA, fmt::BASE_RATE_48KHZ,
            fmt::MULT_1X, fmt::DIV_1, fmt::BITS_16, 2);
        assert_eq!(fmt::sample_rate(fmt_val), 48000);
        assert_eq!(fmt::bits_per_sample(fmt_val), 16);
        assert_eq!(fmt::channels(fmt_val), 2);
        assert_eq!(fmt::frame_size(fmt_val), 4);
    }

    #[test]
    fn format_44khz() {
        let fmt_val = fmt::build(fmt::TYPE_HDA, fmt::BASE_RATE_44P1KHZ,
            fmt::MULT_2X, fmt::DIV_1, fmt::BITS_24, 1);
        assert_eq!(fmt::sample_rate(fmt_val), 88200);
        assert_eq!(fmt::bits_per_sample(fmt_val), 24);
        assert_eq!(fmt::channels(fmt_val), 1);
    }

    #[test]
    fn bdl_entry_layout() {
        assert_eq!(core::mem::size_of::<BdlEntry>(), 16);
        let mut e = BdlEntry::zeroed();
        e = BdlEntry::new(0x1000, 4096, true);
        assert_eq!(e.address_lo, 0x1000);
        assert_eq!(e.length, 4096);
        assert_eq!(e.flags, 1);
    }

    #[test]
    fn codec_cmd_build() {
        let cmd = CodecCmd::new(0, 0x01, verb::GET_PARAM, 0x00); // VENDOR_ID
        // CAD=0 <<28=0, NID=1<<20=0x100000, verb=F00<<8=0xF0000
        assert_eq!(cmd.raw(), 0x001F_0000u32);
    }

    #[test]
    fn codec_resp_parsing() {
        let resp = CodecResp { response: 0x8086_01, response_ex: 0 };
        assert_eq!(resp.codec_address(), 0);
        let unsol = CodecResp { response: 0, response_ex: (1 << 4) | 0x3 };
        assert!(unsol.is_unsolicited());
        assert_eq!(unsol.codec_address(), 0x3);
    }

    #[test]
    fn cfg_default_pin_detect() {
        // Default speaker config: port=fixed(2), device=speaker(1)
        let cfg = cfg_default::CfgDefault(
            (0x2) | (0x1 << 8) | (0x1 << 28)  // jack, speaker, assoc=1
        );
        assert!(cfg.is_fixed());
        assert!(cfg.is_loudspeaker());
        assert!(!cfg.is_jack());

        // Front panel mic: jack, mic
        let cfg2 = cfg_default::CfgDefault(
            (0x0) | (0x8 << 8) | (0x2 << 28) | (0x0 << 20) // jack, mic, assoc=2
        );
        assert!(cfg2.is_jack());
        assert!(cfg2.is_mic());
    }

    #[test]
    fn pci_class_code() {
        assert_eq!(PCI_CLASS_MULTIMEDIA, 0x04);
        assert_eq!(PCI_SUBCLASS_HDAUDIO, 0x03);
    }

    #[test]
    fn power_state_set() {
        assert_eq!(power::set_state(power::D0 as u8, false), 0x00);
        assert_eq!(power::set_state(power::D3 as u8, false), 0x03);
        assert_eq!(power::set_state(power::D0 as u8, true), 0x80);
    }
}
