//! # Controller — Intel HDA Controller Initialization
//!
//! Implements: PCI probe, BAR0 mapping, controller reset, CORB/RIRB setup,
//! SDI stream allocation, interrupt management.

#![allow(dead_code)]

use core::ffi::c_int;
use core::ptr;

use crate::ffi;
use crate::registers::{self, regs, gcap, gctl, intctl, intsts, sdctl, sdsts,
    corbctl, corbsts, rirbctl, rirbsts, corbsize, rirbsize};
use crate::registers::{CodecCmd, CodecResp, BdlEntry, MAX_STREAMS,
    CORB_ENTRIES, RIRB_ENTRIES, BDL_ENTRIES, AUDIO_DMA_BUF_SIZE};
use crate::registers::fmt as audio_fmt;

use minix_driver::mmio::MmioRegion;

/// DMA memory descriptor.
#[derive(Clone)]
pub struct DmaMem {
    pub virt: *mut u8,
    pub phys: u64,
    pub size: usize,
}

impl DmaMem {
    pub fn zeroed() -> Self {
        Self { virt: ptr::null_mut(), phys: 0, size: 0 }
    }

    pub fn is_valid(&self) -> bool { !self.virt.is_null() }
}

/// Audio stream descriptor state.
#[derive(Clone)]
pub struct HdaStream {
    /// Stream ID (SDn index).
    pub sd_index: u8,
    /// Whether this stream is active (running).
    pub active: bool,
    /// DMA direction: false = output (playback), true = input (capture).
    pub input: bool,
    /// Stream tag (SDI / stream number for codec communication).
    pub stream_tag: u8,
    /// Stream format.
    pub format: u16,
    /// BDL physical address.
    pub bdl_phys: u64,
    /// BDL virtual address.
    pub bdl_virt: *mut u8,
    /// BDL entries count.
    pub bdl_entries: u16,
    /// DMA buffer (audio data).
    pub dma_buf: DmaMem,
    /// Position in buffer (bytes).
    pub position: u32,
    /// Number of bytes per period (interrupt interval).
    pub period_bytes: u32,
}

impl HdaStream {
    fn new(sd_index: u8) -> Self {
        Self {
            sd_index,
            active: false,
            input: false,
            stream_tag: 0,
            format: 0,
            bdl_phys: 0,
            bdl_virt: ptr::null_mut(),
            bdl_entries: 0,
            dma_buf: DmaMem::zeroed(),
            position: 0,
            period_bytes: 0,
        }
    }
}

/// HDA controller state.
pub struct HdaController {
    /// MMIO region (BAR0).
    pub mmio: MmioRegion,
    /// MMIO region size.
    pub mmio_size: usize,
    /// PCI device index.
    pub devind: c_int,
    /// IRQ number.
    pub irq: c_int,
    /// IRQ hook ID.
    pub hook_id: c_int,
    /// Whether MSI is being used.
    pub msi_available: bool,
    // -----------------------------------------------------------------------
    // Capabilities
    // -----------------------------------------------------------------------
    /// Number of Input Streams.
    pub num_input_streams: u8,
    /// Number of Output Streams.
    pub num_output_streams: u8,
    /// Number of Bidirectional Streams.
    pub num_bidir_streams: u8,
    /// HDA version (major.minor).
    pub version_major: u8,
    pub version_minor: u8,
    // -----------------------------------------------------------------------
    // CORB / RIRB
    // -----------------------------------------------------------------------
    /// CORB DMA buffer.
    pub corb_mem: DmaMem,
    /// CORB write pointer.
    pub corb_wp: u16,
    /// RIRB DMA buffer.
    pub rirb_mem: DmaMem,
    /// RIRB read pointer.
    pub rirb_rp: u16,
    /// RIRB interrupt count.
    pub rirb_count: u16,
    // -----------------------------------------------------------------------
    // Streams
    // -----------------------------------------------------------------------
    /// Allocated stream descriptors.
    pub streams: [HdaStream; MAX_STREAMS as usize],
    /// Number of streams in use.
    pub num_streams: u8,
    /// Next stream tag to assign.
    pub next_stream_tag: u8,
    // -----------------------------------------------------------------------
    // Codecs
    // -----------------------------------------------------------------------
    /// Bitmask of detected codecs (bit N = codec N present).
    pub codec_mask: u16,
    // -----------------------------------------------------------------------
    // Audio format
    // -----------------------------------------------------------------------
    /// Current PCM sample rate.
    pub sample_rate: u32,
    /// Bits per sample.
    pub bits_per_sample: u8,
    /// Number of channels.
    pub channels: u8,
    /// Current stream format word.
    pub stream_format: u16,
    /// Volume (0-255).
    pub volume: u8,
    /// Mute state.
    pub muted: bool,
    /// Verbose level.
    pub verbose: u8,
}

impl HdaController {
    /// Read a 32-bit controller register.
    #[inline]
    fn r32(&self, offset: usize) -> u32 {
        self.mmio.read32(offset).unwrap_or(0)
    }

    /// Write a 32-bit controller register.
    #[inline]
    fn w32(&self, offset: usize, val: u32) {
        let _ = self.mmio.write32(offset, val);
    }

    /// Read a 16-bit controller register (halfword access).
    #[inline]
    fn r16(&self, offset: usize) -> u16 {
        self.mmio.read16(offset).unwrap_or(0)
    }

    /// Write a 16-bit controller register.
    #[inline]
    fn w16(&self, offset: usize, val: u16) {
        let _ = self.mmio.write16(offset, val);
    }

    /// Read an 8-bit controller register.
    #[inline]
    fn r8(&self, offset: usize) -> u8 {
        self.mmio.read8(offset).unwrap_or(0)
    }

    /// Write an 8-bit controller register.
    #[inline]
    fn w8(&self, offset: usize, val: u8) {
        let _ = self.mmio.write8(offset, val);
    }

    /// Allocate a DMA buffer.
    fn alloc_dma(size: usize) -> Option<DmaMem> {
        let (virt, phys) = ffi::alloc_contig_ffi(size)?;
        unsafe { ptr::write_bytes(virt, 0, size); }
        Some(DmaMem { virt: virt as *mut u8, phys, size })
    }

    /// Free a DMA buffer.
    fn free_dma(mem: &DmaMem) {
        if !mem.virt.is_null() {
            ffi::free_contig_ffi(mem.virt as *mut core::ffi::c_void, mem.size);
        }
    }

    /// Probe for HDA PCI device (class 0x04, subclass 0x03).
    pub fn probe(skip: c_int) -> Option<c_int> {
        ffi::pci_init_ffi();
        let (devind, vid, did) = ffi::pci_first_dev_ffi()?;
        let mut current = devind;
        let mut current_vid = vid;
        let mut current_did = did;

        for _ in 0..skip {
            (current, current_vid, current_did) = ffi::pci_next_dev_ffi()?;
        }

        // Check PCI class: multimedia (0x04), HD audio (0x03)
        let class = ffi::pci_attr_r32_ffi(current, 0x08) >> 16;
        let base_class = (class >> 8) as u8;
        let sub_class = (class & 0xFF) as u8;

        if base_class == 0x04 && sub_class == 0x03 {
            ffi::pci_reserve_ffi(current);
            if current_vid == 0x8086 || current_vid == 0x1002
                || current_vid == 0x1022 || current_vid == 0x10DE
            {
                if ffi::pci_attr_r16_ffi(current, 0x00) != 0xFFFF {
                    return Some(current);
                }
            }
            return Some(current);
        }

        // Fallback: check vendor ID table
        if registers::HDA_DEVICE_TABLE.contains(&(current_vid, current_did)) {
            ffi::pci_reserve_ffi(current);
            return Some(current);
        }

        None
    }

    /// Read a codec parameter.
    pub fn read_param(&mut self, cad: u8, nid: u8, param_id: u8) -> Option<u32> {
        let cmd = CodecCmd::new(cad, nid, registers::verb::GET_PARAM, param_id as u16);
        let resp = self.send_corb_command(cmd)?;
        Some(resp.response)
    }

    /// Send a verb to a codec via CORB and wait for response on RIRB.
    pub fn send_verb(&mut self, cad: u8, nid: u8, verb: u16, payload: u16) -> Option<u32> {
        let cmd = CodecCmd::new(cad, nid, verb, payload);
        let resp = self.send_corb_command(cmd)?;
        if resp.is_unsolicited() {
            // Try one more time
            let cmd2 = CodecCmd::new(cad, nid, verb, payload);
            let resp2 = self.send_corb_command(cmd2)?;
            Some(resp2.response)
        } else {
            Some(resp.response)
        }
    }

    /// Send a CORB command and wait for RIRB response.
    fn send_corb_command(&mut self, cmd: CodecCmd) -> Option<CodecResp> {
        // Wait for CORB space
        let timeout_us = 50_000; // 50ms
        let step_us = 10;
        for _ in 0..(timeout_us / step_us) {
            let rp = self.r16(regs::CORBRP) & 0xFF;
            let next_wp = ((self.corb_wp + 1) as u16) & 0xFF;
            if next_wp != rp {
                break;
            }
            ffi::udelay(step_us);
        }

        // Write command to CORB buffer
        let entry_size = 4; // 4 bytes per CORB entry
        let offset = (self.corb_wp as usize) * entry_size;
        let corb_base = self.corb_mem.virt as *mut u32;
        unsafe { ptr::write_volatile(corb_base.add(self.corb_wp as usize), cmd.raw()); }
        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);

        // Update CORB write pointer
        self.corb_wp = (self.corb_wp + 1) & 0xFF;
        self.w16(regs::CORBWP, self.corb_wp);

        // Wait for response on RIRB
        for _ in 0..(timeout_us / step_us) {
            let wp = self.r16(regs::RIRBWP) & 0xFF;
            if wp != self.rirb_rp {
                let entry_size_rirb = 8; // 8 bytes per RIRB entry (resp + resp_ex)
                let resp_offset = (self.rirb_rp as usize) * entry_size_rirb;
                let rirb_base = self.rirb_mem.virt as *mut u32;
                let response = unsafe { ptr::read_volatile(rirb_base.add(self.rirb_rp as usize * 2)) };
                let response_ex = unsafe { ptr::read_volatile(rirb_base.add(self.rirb_rp as usize * 2 + 1)) };

                // Update RIRB read pointer
                self.rirb_rp = (self.rirb_rp + 1) & 0xFF;
                self.w16(regs::RIRBWP, /* RIRBWP is write-only clear */ self.rirb_rp);

                // Clear RIRB interrupt status
                self.w8(regs::RIRBSTS, rirbsts::RIRB_INT);

                return Some(CodecResp { response, response_ex });
            }
            ffi::udelay(step_us);

            // Check for RIRB overrun
            let rirb_sts = self.r8(regs::RIRBSTS);
            if (rirb_sts & rirbsts::RIRB_OIS) != 0 {
                self.w8(regs::RIRBSTS, rirbsts::RIRB_OIS);
                self.rirb_rp = self.r16(regs::RIRBWP) as u8 as u16;
                break;
            }
        }

        None
    }

    /// Initialize the HDA controller.
    pub fn init(devind: c_int, verbose: u8) -> Option<Self> {
        // Map BAR0 (MMIO)
        let (base_lo, bar_size, ioflag) = ffi::pci_get_bar_ffi(devind, 0)?;
        if ioflag {
            ffi::print(b"HDA: BAR0 is I/O, expected MMIO\0");
            return None;
        }

        let (base_hi, _, _) = ffi::pci_get_bar_ffi(devind, 1).unwrap_or((0, 0, false));
        let phys_base = (base_lo as u64) | ((base_hi as u64) << 32);

        let map_size = if bar_size > 0 { bar_size as usize } else { 0x4000 };
        let mmio_virt = ffi::vm_map_phys_ffi(phys_base as *mut core::ffi::c_void, map_size);
        if mmio_virt.is_null() {
            ffi::print(b"HDA: unable to map BAR0 MMIO\0");
            return None;
        }

        let mmio = MmioRegion::new_unaligned(mmio_virt as *mut u8, map_size).ok()?;
        let irq = ffi::pci_attr_r8_ffi(devind, 0x3C) as c_int;

        // Read capabilities
        let gcap_val = mmio.read16(regs::GCAP).unwrap_or(0);
        let num_input = gcap::iss(gcap_val);
        let num_output = gcap::oss(gcap_val);
        let num_bidir = gcap::bss(gcap_val);
        let version_major = mmio.read8(regs::VMAJ).unwrap_or(1);
        let version_minor = mmio.read8(regs::VMIN).unwrap_or(0);

        if verbose >= 1 {
            ffi::print(b"HDA: controller found\0");
            if verbose >= 2 {
                let total = num_input + num_output + num_bidir;
                let vers = |v: u8| v;
                ffi::print(b"  version: major.minor\0");
            }
        }

        // Reset controller
        let _ = mmio.write32(regs::GCTL, gctl::CRST);
        ffi::udelay(25); // > 521µs per spec
        let timeout_us = 1_000_000;
        for _ in 0..(timeout_us / 10) {
            if (mmio.read32(regs::GCTL).unwrap_or(0) & gctl::CRST) != 0 {
                break;
            }
            ffi::udelay(10);
        }

        // Take controller out of reset
        let _ = mmio.write32(regs::GCTL, 0);
        ffi::udelay(25);
        for _ in 0..(timeout_us / 10) {
            if (mmio.read32(regs::GCTL).unwrap_or(0) & gctl::CRST) == 0 {
                break;
            }
            ffi::udelay(10);
        }

        if verbose >= 1 {
            ffi::print(b"HDA: controller reset complete\0");
        }

        // Allocate CORB DMA buffer
        let corb_size = CORB_ENTRIES * 4; // 4 bytes per entry
        let corb_mem = Self::alloc_dma(corb_size)?;

        // Allocate RIRB DMA buffer
        let rirb_size = RIRB_ENTRIES * 8; // 8 bytes per entry (response + ext)
        let rirb_mem = Self::alloc_dma(rirb_size)?;

        let mut ctrl = Self {
            mmio, mmio_size: map_size, devind, irq, hook_id: -1,
            msi_available: false,
            num_input_streams: num_input,
            num_output_streams: num_output,
            num_bidir_streams: num_bidir,
            version_major, version_minor,
            corb_mem, corb_wp: 0,
            rirb_mem, rirb_rp: 0, rirb_count: 0,
            streams: core::array::from_fn(|i| HdaStream::new(i as u8)),
            num_streams: 0, next_stream_tag: 1,
            codec_mask: 0,
            sample_rate: 48000, bits_per_sample: 16, channels: 2,
            stream_format: 0,
            volume: 200, muted: false,
            verbose,
        };

        // Set up CORB
        ctrl.setup_corb()?;

        // Set up RIRB
        ctrl.setup_rirb()?;

        // Read codec mask from STATESTS
        let states = ctrl.r16(regs::STATESTS);
        ctrl.codec_mask = states;
        // Clear STATESTS by writing the read value back (write-1-to-clear)
        ctrl.w16(regs::STATESTS, states);

        if verbose >= 1 && ctrl.codec_mask != 0 {
            for cad in 0..15 {
                if (ctrl.codec_mask & (1 << cad)) != 0 {
                    ffi::print(b"HDA: codec detected on CAD\0");
                }
            }
        }

        // Set up interrupt
        ctrl.setup_interrupt()?;

        // Accept unsolicited responses
        let gctl_val = ctrl.r32(regs::GCTL) | gctl::UNSOL;
        ctrl.w32(regs::GCTL, gctl_val);

        Some(ctrl)
    }

    /// Set up the CORB (Command Output Ring Buffer).
    fn setup_corb(&mut self) -> Option<()> {
        // Stop CORB DMA
        self.w8(regs::CORBCTL, 0);

        // Set CORB size to 256 entries
        self.w8(regs::CORBSIZE, corbsize::SIZE_256);

        // Set CORB base address
        self.w32(regs::CORBLBASE, self.corb_mem.phys as u32);
        self.w32(regs::CORBUBASE, (self.corb_mem.phys >> 32) as u32);

        // Set read pointer to 0 (also clears it)
        self.w16(regs::CORBRP, 0);

        // Initialize write pointer
        self.corb_wp = 0;
        self.w16(regs::CORBWP, 0);

        // Start CORB DMA
        self.w8(regs::CORBCTL, corbctl::CORB_DMA_ENABLE);

        if self.verbose >= 2 {
            ffi::print(b"HDA: CORB initialized\0");
        }

        Some(())
    }

    /// Set up the RIRB (Response Input Ring Buffer).
    fn setup_rirb(&mut self) -> Option<()> {
        // Stop RIRB DMA
        self.w8(regs::RIRBCTL, 0);

        // Set RIRB size to 256 entries
        self.w8(regs::RIRBSIZE, rirbsize::SIZE_256);

        // Set RIRB base address
        self.w32(regs::RIRBLBASE, self.rirb_mem.phys as u32);
        self.w32(regs::RIRBUBASE, (self.rirb_mem.phys >> 32) as u32);

        // Set RIRB response interrupt count (interrupt every response)
        self.w16(regs::RIRBCNT, 1);

        // Initialize read pointer
        self.rirb_rp = 0;

        // Start RIRB DMA with interrupt enabled
        self.w8(regs::RIRBCTL, rirbctl::RIRB_DMA_ENABLE | rirbctl::RIRB_INT_ENABLE);

        if self.verbose >= 2 {
            ffi::print(b"HDA: RIRB initialized\0");
        }

        Some(())
    }

    /// Set up interrupt handling (MSI or legacy).
    fn setup_interrupt(&mut self) -> Option<()> {
        // Try MSI first
        let msix_info = ffi::pci_msix_parse_ffi(self.devind);
        if let Some(info) = msix_info {
            if info.msix_table_size >= 1 {
                let irq = ffi::msix_alloc_irq()?;
                let hook_id = ffi::msix_setup(irq)?;
                self.irq = irq;
                self.hook_id = hook_id;
                self.msi_available = true;
                if self.verbose >= 1 {
                    ffi::print(b"HDA: MSI enabled\0");
                }
            }
        }

        if !self.msi_available {
            let hook_id = ffi::irq_setup(self.irq)?;
            self.hook_id = hook_id;
            if self.verbose >= 1 {
                ffi::print(b"HDA: using legacy IRQ\0");
            }
        }

        // Enable interrupts: Global Interrupt Enable + Controller Interrupt Enable
        self.w32(regs::INTCTL, intctl::GIE | intctl::CIE);

        Some(())
    }

    /// Allocate a stream descriptor for audio DMA.
    pub fn alloc_stream(&mut self, input: bool) -> Option<u8> {
        let total = self.num_input_streams + self.num_output_streams + self.num_bidir_streams;

        // Find free stream
        for sd_index in 0..total {
            let idx = sd_index as usize;
            if !self.streams[idx].active {
                // Assign stream tag
                let stream_tag = self.next_stream_tag;
                self.next_stream_tag = (self.next_stream_tag % 15) + 1;

                // Allocate DMA buffer
                let dma_buf = Self::alloc_dma(AUDIO_DMA_BUF_SIZE)?;

                // Allocate BDL
                let bdl_size = BDL_ENTRIES * core::mem::size_of::<BdlEntry>();
                let bdl_mem = Self::alloc_dma(bdl_size)?;

                self.streams[idx] = HdaStream {
                    sd_index,
                    active: true,
                    input,
                    stream_tag,
                    format: 0,
                    bdl_phys: bdl_mem.phys,
                    bdl_virt: bdl_mem.virt,
                    bdl_entries: 0,
                    dma_buf,
                    position: 0,
                    period_bytes: 0,
                };
                self.num_streams += 1;

                if self.verbose >= 1 {
                    ffi::print(b"HDA: stream allocated\0");
                }
                return Some(stream_tag);
            }
        }

        None
    }

    /// Free a stream descriptor.
    pub fn free_stream(&mut self, stream_tag: u8) {
        // Find stream by index to avoid borrow conflicts
        let sd_idx = match self.streams.iter().position(|s| s.active && s.stream_tag == stream_tag) {
            Some(i) => i,
            None => return,
        };

        // Stop DMA
        let sd_base = regs::sd_base(sd_idx as u8);
        self.w8(sd_base + regs::SD_CTL, 0);

        // Now free the stream resources
        let sd = &mut self.streams[sd_idx];
        Self::free_dma(&sd.dma_buf);
        if !sd.bdl_virt.is_null() {
            let bdl_size = BDL_ENTRIES * core::mem::size_of::<BdlEntry>();
            ffi::free_contig_ffi(sd.bdl_virt as *mut core::ffi::c_void, bdl_size);
        }
        sd.active = false;
        self.num_streams -= 1;
    }

    /// Start DMA on a stream.
    pub fn start_stream(
        &mut self,
        stream_tag: u8,
        sample_rate: u32,
        bits: u8,
        channels: u8,
    ) -> bool {
        // Find stream by tag using index to avoid borrow conflicts
        let sd_info = {
            let idx = match self.streams.iter().position(|s| s.active && s.stream_tag == stream_tag) {
                Some(i) => i,
                None => return false,
            };
            let sd_base = regs::sd_base(idx as u8);
            let sd = &mut self.streams[idx];

            // Determine base rate and multiplier
            let (base_rate, mult) = match sample_rate {
                44100 => (audio_fmt::BASE_RATE_44P1KHZ, audio_fmt::MULT_1X),
                48000 => (audio_fmt::BASE_RATE_48KHZ, audio_fmt::MULT_1X),
                88200 => (audio_fmt::BASE_RATE_44P1KHZ, audio_fmt::MULT_2X),
                96000 => (audio_fmt::BASE_RATE_48KHZ, audio_fmt::MULT_2X),
                176400 => (audio_fmt::BASE_RATE_44P1KHZ, audio_fmt::MULT_4X),
                192000 => (audio_fmt::BASE_RATE_48KHZ, audio_fmt::MULT_4X),
                22050 => (audio_fmt::BASE_RATE_44P1KHZ, audio_fmt::MULT_2X),
                24000 => (audio_fmt::BASE_RATE_48KHZ, audio_fmt::MULT_2X),
                _ => (audio_fmt::BASE_RATE_48KHZ, audio_fmt::MULT_1X),
            };

            let bits_enc = match bits {
                8 => audio_fmt::BITS_8,
                16 => audio_fmt::BITS_16,
                20 => audio_fmt::BITS_20,
                24 => audio_fmt::BITS_24,
                32 => audio_fmt::BITS_32,
                _ => audio_fmt::BITS_16,
            };

            let fmt = audio_fmt::build(
                audio_fmt::TYPE_HDA, base_rate, mult,
                if sample_rate <= 24000 { audio_fmt::DIV_2 } else { audio_fmt::DIV_1 },
                bits_enc, channels as u16,
            );

            sd.format = fmt;

            // Build BDL — 2 entries (double buffer)
            let page_size = AUDIO_DMA_BUF_SIZE as u32 / BDL_ENTRIES as u32;
            sd.bdl_entries = BDL_ENTRIES as u16;
            sd.period_bytes = page_size;

            let bdl_ptr = sd.bdl_virt as *mut BdlEntry;
            for i in 0..BDL_ENTRIES {
                let entry = BdlEntry::new(
                    sd.dma_buf.phys + (i as u64) * page_size as u64,
                    page_size,
                    true, // IOC = interrupt on completion
                );
                unsafe { ptr::write_volatile(bdl_ptr.add(i), entry); }
            }
            core::sync::atomic::fence(core::sync::atomic::Ordering::Release);

            let input = sd.input;
            let sd_index = sd.sd_index;
            let bdl_phys = sd.bdl_phys;

            (sd_base, fmt, page_size, input, sd_index, bdl_phys)
        };

        // Now all self access is after the mutable borrow of streams is released
        let (sd_base, fmt, _page_size, input, sd_index, bdl_phys) = sd_info;

        // Set stream format
        self.w16(sd_base + regs::SD_FMT, fmt);

        // Set cyclic buffer length
        let buf_size = AUDIO_DMA_BUF_SIZE as u32;
        self.w32(sd_base + regs::SD_CBL, buf_size);

        // Set last valid index
        let lvi = (BDL_ENTRIES - 1) as u16;
        self.w16(sd_base + regs::SD_LVI, lvi);

        // Set BDL base address
        self.w32(sd_base + regs::SD_BDPL, bdl_phys as u32);
        self.w32(sd_base + regs::SD_BDPU, (bdl_phys >> 32) as u32);

        // Reset stream
        self.w8(sd_base + regs::SD_CTL, sdctl::SRST);
        ffi::udelay(25);
        self.w8(sd_base + regs::SD_CTL, 0);
        ffi::udelay(25);

        // Set direction
        let dir_flag = if input { sdctl::DIR } else { 0 };

        // Start stream
        self.w8(sd_base + regs::SD_CTL,
            sdctl::SRUN | sdctl::IOCE | sdctl::FEIE | dir_flag);

        self.w32(regs::INTCTL,
            self.r32(regs::INTCTL) | intctl::SIE(sd_index));

        if self.verbose >= 1 {
            ffi::print(b"HDA: stream started\0");
        }

        true
    }

    /// Stop DMA on a stream.
    pub fn stop_stream(&mut self, stream_tag: u8) {
        // Copy index before borrowing self to avoid borrow conflict
        let sd_idx = match self.streams.iter().position(|s| s.active && s.stream_tag == stream_tag) {
            Some(idx) => idx as u8,
            None => return,
        };
        let sd_base = regs::sd_base(sd_idx);
        self.w8(sd_base + regs::SD_CTL, 0);
        self.w32(regs::INTCTL,
            self.r32(regs::INTCTL) & !intctl::SIE(sd_idx));
    }

    /// Handle interrupt — called from interrupt handler.
    /// Returns bitmap of stream indices that have buffer completion events.
    pub fn handle_interrupt(&mut self) -> u32 {
        let intsts = self.r32(regs::INTSTS);

        // Check controller interrupt
        if (intsts & intsts::CIS) != 0 {
            // RIRB interrupt — responses available
            let rirb_sts = self.r8(regs::RIRBSTS);
            if (rirb_sts & rirbsts::RIRB_INT) != 0 {
                self.w8(regs::RIRBSTS, rirbsts::RIRB_INT);
                self.rirb_count += 1;
            }
        }

        // Check stream interrupts
        let mut completed = 0u32;
        let total = self.num_input_streams + self.num_output_streams + self.num_bidir_streams;
        for sd_idx in 0..total {
            if (intsts & intsts::SIS(sd_idx)) != 0 {
                // Check BCIS (Buffer Completion Interrupt Status)
                let sd_base = regs::sd_base(sd_idx);
                let sts = self.r8(sd_base + regs::SD_STS);
                if (sts & sdsts::BCIS) != 0 {
                    self.w8(sd_base + regs::SD_STS, sdsts::BCIS);
                    completed |= 1u32 << (sd_idx as u32);

                    // Update position: read LPIB directly without borrowing self.streams
                    let pos = self.r32(sd_base + regs::SD_LPIB);
                    let idx = sd_idx as usize;
                    if idx < self.streams.len() && self.streams[idx].active {
                        self.streams[idx].position = pos;
                    }
                }
            }
        }

        completed
    }

    /// Clean up resources.
    pub fn stop(&mut self) {
        // Disable interrupts
        self.w32(regs::INTCTL, 0);

        // Stop all streams — collect tags first to avoid borrow conflict
        let tags: [u8; MAX_STREAMS as usize] = core::array::from_fn(|i| {
            if self.streams[i].active { self.streams[i].stream_tag } else { 0 }
        });
        for tag in tags.iter() {
            if *tag != 0 {
                self.stop_stream(*tag);
                self.free_stream(*tag);
            }
        }

        // Free CORB/RIRB
        self.w8(regs::CORBCTL, 0);
        self.w8(regs::RIRBCTL, 0);
        Self::free_dma(&self.corb_mem);
        Self::free_dma(&self.rirb_mem);

        // Remove IRQ handler
        if self.hook_id != 0 {
            let _ = ffi::irq_remove(&mut self.hook_id);
        }
        if self.msi_available && self.irq != 0 {
            let _ = ffi::msix_free_irq(self.irq);
        }

        let _ = ffi::vm_unmap_phys_ffi(
            self.mmio.base() as *mut core::ffi::c_void,
            self.mmio_size,
        );

        if self.verbose >= 1 {
            ffi::print(b"HDA: controller stopped\0");
        }
    }
}
