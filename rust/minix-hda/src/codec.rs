//! # Codec — HDA Codec Enumeration and Management
//!
//! Handles: codec discovery (Vendor ID, Revision), AFG/SFG parsing,
//! widget enumeration, pin configuration, volume/mute controls.

#![allow(dead_code)]

use core::ffi::c_int;

use crate::ffi;
use crate::registers::{self, verb, param, widget_type, power, pin_caps, cfg_default};
use crate::controller::HdaController;

// ============================================================================
// Codec identity
// ============================================================================

/// HDA codec identification data.
#[derive(Clone)]
pub struct CodecId {
    /// Codec Address (CAD).
    pub cad: u8,
    /// Vendor ID (upper 16 bits).
    pub vendor_id: u16,
    /// Device ID (lower 16 bits).
    pub device_id: u16,
    /// Major revision.
    pub major_rev: u8,
    /// Minor revision.
    pub minor_rev: u8,
    /// Function Group Type (0x01 = audio, 0x02 = modem).
    pub function_group_type: u8,
    /// Starting Node ID of the function group.
    pub start_nid: u8,
    /// Number of nodes in the function group.
    pub node_count: u8,
    /// Whether this is an audio function group.
    pub is_audio: bool,
}

impl CodecId {
    /// Get a human-readable vendor name.
    pub fn vendor_name(&self) -> &'static [u8] {
        match self.vendor_id {
            0x10EC => b"Realtek\0",
            0x8086 => b"Intel\0",
            0x1002 => b"AMD/ATI\0",
            0x1022 => b"AMD\0",
            0x10DE => b"NVIDIA\0",
            0x11D4 => b"Analog Devices\0",
            0x8384 => b"Sigmatel/IDT\0",
            0x14F1 => b"Conexant\0",
            0x1AEC => b"Wolfson\0",
            0x1B0A => b"Creative\0",
            0x1106 => b"VIA\0",
            0x13F6 => b"C-Media\0",
            _ => b"Unknown\0",
        }
    }
}

// ============================================================================
// Audio Widget
// ============================================================================

/// An audio widget (node) in the codec.
#[derive(Clone)]
pub struct AudioWidget {
    /// Node ID.
    pub nid: u8,
    /// Widget type (see widget_type).
    pub wtype: u8,
    /// Audio Widget capabilities (from AW_CAPS param).
    pub aw_caps: u32,
    /// Supported PCM rates/sizes.
    pub pcm: u32,
    /// Supported formats.
    pub formats: u32,
    /// Pin capabilities (only for PIN widgets).
    pub pin_caps: u32,
    /// Configuration default (only for PIN widgets).
    pub cfg_default: u32,
    /// Connection list length.
    pub conn_list_len: u8,
    /// Amp capabilities: out amp present, in amp present.
    pub has_out_amp: bool,
    pub has_in_amp: bool,
    /// Output amplifier capabilities.
    pub out_amp_caps: u32,
    /// Input amplifier capabilities.
    pub in_amp_caps: u32,
}

impl AudioWidget {
    pub fn new(nid: u8) -> Self {
        Self {
            nid, wtype: 0, aw_caps: 0, pcm: 0, formats: 0,
            pin_caps: 0, cfg_default: 0, conn_list_len: 0,
            has_out_amp: false, has_in_amp: false,
            out_amp_caps: 0, in_amp_caps: 0,
        }
    }

    /// Is this an output widget (DAC)?
    pub fn is_output(&self) -> bool { self.wtype == widget_type::AUDIO_OUTPUT }
    /// Is this an input widget (ADC)?
    pub fn is_input(&self) -> bool { self.wtype == widget_type::AUDIO_INPUT }
    /// Is this a pin widget?
    pub fn is_pin(&self) -> bool { self.wtype == widget_type::PIN }
    /// Is this a mixer widget?
    pub fn is_mixer(&self) -> bool { self.wtype == widget_type::AUDIO_MIXER }
    /// Is this a selector widget?
    pub fn is_selector(&self) -> bool { self.wtype == widget_type::AUDIO_SELECTOR }
    /// Is this a volume knob widget?
    pub fn is_volume_knob(&self) -> bool { self.wtype == widget_type::VOLUME_KNOB }
    /// Is this a beep generator?
    pub fn is_beep(&self) -> bool { self.wtype == widget_type::BEEP_GEN }
}

// ============================================================================
// HDA codec state
// ============================================================================

/// Complete state of an HDA codec.
pub struct HdaCodec {
    /// Codec identity.
    pub id: CodecId,
    /// Audio Function Group Node ID (typically 0x01).
    pub afg_nid: u8,
    /// AFG capabilities.
    pub afg_caps: u32,
    /// List of audio widgets.
    pub widgets: [AudioWidget; 64],  // Max 64 widgets per codec
    /// Number of widgets.
    pub num_widgets: u8,
    /// Default PCM parameters.
    pub default_sample_rate: u32,
    pub default_bits: u8,
    pub default_channels: u8,
    /// The DAC widget used for playback (output).
    pub dac_nid: Option<u8>,
    /// The ADC widget used for capture (input).
    pub adc_nid: Option<u8>,
    /// Pin widgets for output.
    pub output_pins: [u8; 16],
    pub num_output_pins: u8,
    /// Pin widgets for input.
    pub input_pins: [u8; 16],
    pub num_input_pins: u8,
    /// Verbose.
    pub verbose: u8,
}

impl HdaCodec {
    /// Create a new HDA codec state.
    pub fn new(cad: u8, verbose: u8) -> Self {
        Self {
            id: CodecId {
                cad, vendor_id: 0, device_id: 0,
                major_rev: 0, minor_rev: 0,
                function_group_type: 0, start_nid: 0, node_count: 0,
                is_audio: false,
            },
            afg_nid: 0, afg_caps: 0,
            widgets: core::array::from_fn(|i| AudioWidget::new(i as u8)),
            num_widgets: 0,
            default_sample_rate: 48000, default_bits: 16, default_channels: 2,
            dac_nid: None, adc_nid: None,
            output_pins: [0; 16], num_output_pins: 0,
            input_pins: [0; 16], num_input_pins: 0,
            verbose,
        }
    }

    /// Enumerate a codec completely.
    /// `ctrl` is the HDA controller to communicate with.
    pub fn enumerate(ctrl: &mut HdaController, cad: u8) -> Option<Self> {
        let mut codec = Self::new(cad, ctrl.verbose);

        // Read Vendor ID (NID 0)
        let vendor_raw = ctrl.read_param(cad, 0, param::VENDOR_ID)?;
        codec.id.vendor_id = (vendor_raw >> 16) as u16;
        codec.id.device_id = vendor_raw as u16;

        // Read Revision ID
        let rev_raw = ctrl.read_param(cad, 0, param::REVISION_ID)?;
        codec.id.major_rev = (rev_raw >> 20) as u8 & 0xF;
        codec.id.minor_rev = (rev_raw >> 16) as u8 & 0xF;

        if codec.verbose >= 1 {
            ffi::print(b"HDA: codec detected\0");
            if codec.verbose >= 2 {
                ffi::print(codec.id.vendor_name());
            }
        }

        // Read Subordinate Node Count
        let snc_raw = ctrl.read_param(cad, 0, param::SUBORDINATE_NODE_COUNT)?;
        let start_nid = (snc_raw >> 8) as u8;
        let total_nodes = (snc_raw & 0xFF) as u8;

        codec.id.start_nid = start_nid;
        codec.id.node_count = total_nodes;

        // Find AFG (Audio Function Group)
        let mut afg_nid = start_nid;
        for nid in start_nid..(start_nid + total_nodes) {
            if nid == 0 { break; }
            let fgt_raw = ctrl.read_param(cad, nid, param::FUNCTION_GROUP_TYPE)?;
            let fgt = (fgt_raw & 0xFF) as u8;
            if fgt == 0x01 {
                // Audio Function Group found
                afg_nid = nid;
                codec.id.function_group_type = fgt;
                codec.id.is_audio = true;
                codec.afg_nid = nid;
                break;
            }
        }

        if !codec.id.is_audio {
            if codec.verbose >= 1 {
                ffi::print(b"HDA: codec is not audio, skipping\0");
            }
            return None;
        }

        // Read AFG capabilities
        let afg_caps_raw = ctrl.read_param(cad, afg_nid, param::AFG_CAPS)?;
        codec.afg_caps = afg_caps_raw;

        // Enumerate widgets under AFG
        let snc_afg = ctrl.read_param(cad, afg_nid, param::SUBORDINATE_NODE_COUNT)?;
        let widget_start = (snc_afg >> 8) as u8;
        let widget_count = (snc_afg & 0xFF) as u8;

        for nid in widget_start..(widget_start + widget_count) {
            if nid == 0 || codec.num_widgets as usize >= codec.widgets.len() {
                break;
            }
            codec.enumerate_widget(ctrl, cad, afg_nid, nid);
        }

        if codec.verbose >= 1 {
            if codec.num_output_pins > 0 {
                ffi::print(b"HDA: output pins found\0");
            }
            if codec.num_input_pins > 0 {
                ffi::print(b"HDA: input pins found\0");
            }
        }

        // Set power state to D0 for AFG
        ctrl.send_verb(cad, afg_nid, verb::SET_POWER_STATE,
            power::set_state(power::D0 as u8, false) as u16);

        // Also set power state D0 for all output and input widgets
        for widget in codec.widgets.iter() {
            if widget.nid >= widget_start && widget.nid < widget_start + widget_count && widget.nid != 0 {
                ctrl.send_verb(cad, widget.nid, verb::SET_POWER_STATE,
                    power::set_state(power::D0 as u8, false) as u16);
            }
        }

        Some(codec)
    }

    /// Enumerate a single widget node.
    fn enumerate_widget(
        &mut self,
        ctrl: &mut HdaController,
        cad: u8,
        afg_nid: u8,
        nid: u8,
    ) {
        let idx = self.num_widgets as usize;
        let mut widget = AudioWidget::new(nid);

        // Read Audio Widget Capabilities
        let aw_caps = match ctrl.read_param(cad, nid, param::AW_CAPS) {
            Some(v) => v,
            None => return,
        };
        widget.aw_caps = aw_caps;
        widget.wtype = (aw_caps & 0xF) as u8;

        // Read supported PCM rates/sizes
        widget.pcm = ctrl.read_param(cad, nid, param::SUPP_PCM).unwrap_or(0);

        // Read supported formats
        widget.formats = ctrl.read_param(cad, nid, param::SUPP_FORMATS).unwrap_or(0);

        // Check for input/output amplifier
        widget.has_out_amp = (aw_caps & (1 << 11)) != 0;
        widget.has_in_amp = (aw_caps & (1 << 10)) != 0;

        if widget.has_out_amp {
            widget.out_amp_caps = ctrl.read_param(cad, nid, param::OUT_AMP_CAPS).unwrap_or(0);
        }
        if widget.has_in_amp {
            widget.in_amp_caps = ctrl.read_param(cad, nid, param::IN_AMP_CAPS).unwrap_or(0);
        }

        // Read connection list length
        widget.conn_list_len = match ctrl.read_param(cad, nid, param::CONNECTION_LIST_LEN) {
            Some(v) => ((v >> 8) & 0xFF) as u8,
            None => 0,
        };

        // Read pin capabilities and config default (for pin widgets only)
        if widget.wtype == widget_type::PIN {
            widget.pin_caps = ctrl.read_param(cad, nid, param::PIN_CAPS).unwrap_or(0);

            // Read configuration default (4 bytes at NID + 0x1C)
            let mut cfg: u32 = 0;
            for i in 0..4 {
                let byte = ctrl.send_verb(cad, nid,
                    verb::GET_CONFIG_DEFAULT_BYTE0 + i as u16, 0);
                if let Some(b) = byte {
                    cfg |= (b & 0xFF) << (i * 8);
                }
            }
            widget.cfg_default = cfg;

            // Categorize pin
            let cp = cfg_default::CfgDefault(cfg);
            if cp.is_nc() {
                // No connect — skip
            } else if (widget.pin_caps & pin_caps::OUTPUT) != 0 {
                // Output capable pin
                if (self.num_output_pins as usize) < self.output_pins.len() {
                    self.output_pins[self.num_output_pins as usize] = nid;
                    self.num_output_pins += 1;
                }
            }
            if (widget.pin_caps & pin_caps::INPUT) != 0 {
                // Input capable pin
                if (self.num_input_pins as usize) < self.input_pins.len() {
                    self.input_pins[self.num_input_pins as usize] = nid;
                    self.num_input_pins += 1;
                }
            }
        }

        // Save first DAC/ADC as default
        if widget.wtype == widget_type::AUDIO_OUTPUT && self.dac_nid.is_none() {
            self.dac_nid = Some(nid);
        }
        if widget.wtype == widget_type::AUDIO_INPUT && self.adc_nid.is_none() {
            self.adc_nid = Some(nid);
        }

        self.widgets[idx] = widget;
        self.num_widgets += 1;
    }

    /// Set output volume for a DAC widget (via SET_AMP_GAIN_MUTE).
    pub fn set_output_volume(&self, ctrl: &mut HdaController, volume: u8, muted: bool) {
        if let Some(dac_nid) = self.dac_nid {
            // Set output amplifier gain/mute
            // Verb payload format:
            //   bit 15 = mute (1=mute)
            //   bits 14:13 = pad
            //   bits 12:8 = right gain (0-64, 0.5dB steps)
            //   bit 7 = right channel
            //   bits 6:5 = pad
            //   bits 4:0 = left gain (0-64, 0.5dB steps)
            let gain = (volume as u16) >> 2; // Map 0-255 to 0-63
            let payload = if muted {
                (1 << 15) | (gain << 8) | (1 << 7) | gain
            } else {
                (gain << 8) | gain
            };
            ctrl.send_verb(self.id.cad, dac_nid, verb::SET_AMP_GAIN_MUTE, payload);
        }
    }

    /// Set mute on/off for a DAC widget.
    pub fn set_mute(&self, ctrl: &mut HdaController, muted: bool) {
        if let Some(dac_nid) = self.dac_nid {
            let payload = if muted {
                (1 << 15) | (0 << 8) | (1 << 7) | 0 // mute both channels, gain=0
            } else {
                0 // unmute, gain=0
            };
            ctrl.send_verb(self.id.cad, dac_nid, verb::SET_AMP_GAIN_MUTE, payload);
        }
    }

    /// Set the converter stream and channel for a widget.
    /// stream_tag: 1-15, channel: 0-15
    pub fn set_converter(&self, ctrl: &mut HdaController, widget_nid: u8,
        stream_tag: u8, channel: u8)
    {
        let payload = ((stream_tag & 0x0F) as u16) | (((channel & 0x0F) as u16) << 4);
        ctrl.send_verb(self.id.cad, widget_nid, verb::SET_CONVERTER_STREAM_CHAN, payload);
    }

    /// Set the converter format for a widget.
    pub fn set_converter_format(&self, ctrl: &mut HdaController,
        widget_nid: u8, fmt: u16)
    {
        ctrl.send_verb(self.id.cad, widget_nid, verb::SET_CONVERTER_FORMAT, fmt);
    }

    /// Set pin widget control (enable/disable output, input, etc.).
    pub fn set_pin_control(&self, ctrl: &mut HdaController,
        pin_nid: u8, enable_out: bool, enable_in: bool, hp_drv: bool)
    {
        let mut payload = 0u16;
        if enable_out { payload |= 0x40; }   // bits 6 = output
        if enable_in  { payload |= 0x20; }   // bits 5 = input
        if hp_drv     { payload |= 0x80; }   // bits 7 = headphone drive
        ctrl.send_verb(self.id.cad, pin_nid, verb::SET_PIN_WIDGET_CTRL, payload);
    }

    /// Get the current pin sense (jack detection).
    pub fn get_pin_sense(&self, ctrl: &mut HdaController,
        pin_nid: u8) -> Option<bool>
    {
        let sense = ctrl.send_verb(self.id.cad, pin_nid, verb::GET_PIN_WIDGET_CTRL, 0)?;
        Some((sense & 0x8000_0000) != 0) // bit 31 = presence detect
    }

    /// Find an output pin widget suitable for a given device type (speaker, HP, line-out).
    pub fn find_pin_by_device(&self, device_type: u32) -> Option<u8> {
        for i in 0..self.num_widgets as usize {
            let w = &self.widgets[i];
            if !w.is_pin() { continue; }
            let cfg = cfg_default::CfgDefault(w.cfg_default);
            match device_type {
                1 if cfg.is_loudspeaker() => return Some(w.nid), // speaker
                2 if cfg.is_hp() => return Some(w.nid),          // headphone
                0 if cfg.is_line_out() => return Some(w.nid),    // line-out
                _ => {}
            }
        }
        None
    }

    /// Print codec information (verbose).
    pub fn dump_info(&self) {
        if self.verbose < 1 { return; }

        ffi::print(b"HDA: codec info\0");
        if self.verbose >= 2 {
            ffi::print(b"  vendor/device: ID\0");
            ffi::print(b"  revision: major.minor\0");
            ffi::print(b"  AFG NID: nid\0");
            ffi::print(b"  widgets: count\0");
        }

        if self.verbose >= 3 {
            for i in 0..self.num_widgets as usize {
                let w = &self.widgets[i];
                let _type_name: &[u8] = match w.wtype {
                    0 => b"Audio Output\0",
                    1 => b"Audio Input\0",
                    2 => b"Mixer\0",
                    3 => b"Selector\0",
                    4 => b"Pin\0",
                    5 => b"Power\0",
                    6 => b"Volume Knob\0",
                    7 => b"Beep Generator\0",
                    0x0F => b"Vendor\0",
                    _ => b"Unknown\0",
                };
                // type_name unused for now
                ffi::print(b"  NID: widget\0");
            }
        }
    }
}
