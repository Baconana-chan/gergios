//! # xHCI — Main xHCI Controller Management
//!
//! Implements: PCI probe, MMIO BAR mapping, HC reset/start,
//! Command Ring + Event Ring setup, Port management.

use core::ffi::c_int;
use core::ptr;

use crate::ffi;
use crate::registers::{self, op, cap, rt};
use crate::registers::{
    Trb, TrbType, SlotContext, EndpointContext,
    ScratchpadEntry, speed, default_max_packet_size,
    build_enable_slot_trb, build_address_device_trb, build_configure_endpoint_trb,
    build_evaluate_context_trb, build_normal_transfer_trb, build_setup_stage_trb,
    build_data_stage_trb, build_status_stage_trb,
    build_setup_packet, DeviceDescriptor, ConfigDescriptor,
    usb_req, usb_descriptor,
};
use crate::ring::{TrbRing, EventRing, RingMem, RING_SIZE, EVENT_RING_SEG_SIZE};
use crate::usb_msc::{self, MscDevice, MAX_MSC_DEVICES};
use crate::usb_device::{self, UsbDeviceRegistry, UsbDeviceType};

/// Maximum number of device slots (from HCSPARAMS1).
const MAX_SLOTS: usize = 64;

/// Size of the Device Context Base Address Array (DCBAA) in entries.
const DCBAA_SIZE: usize = MAX_SLOTS + 1; // Slot 0 unused, slots 1..MAX_SLOTS

/// xHCI Controller state.
pub struct XhciController {
    /// MMIO virtual base address (BAR0).
    pub mmio_base: *mut u8,
    /// MMIO region size.
    pub mmio_size: usize,
    /// PCI device index.
    pub devind: c_int,
    /// IRQ number.
    pub irq: c_int,
    /// IRQ hook ID.
    pub hook_id: c_int,
    /// Whether MSI-X is available.
    pub msix_available: bool,
    /// CAPLENGTH — offset to operational registers.
    pub caplength: u8,
    /// RTSOFF — offset to runtime registers.
    pub rtsoff: u32,
    /// DBOFF — offset to doorbell registers.
    pub dboff: u32,
    /// Max device slots (from HCSPARAMS1).
    pub max_slots: u8,
    /// Max interrupters (from HCSPARAMS1).
    pub max_intrs: u16,
    /// Max ports (from HCSPARAMS1).
    pub max_ports: u8,
    /// Context size (0=32B, 1=64B from HCCPARAMS1.CSZ).
    pub context_size: u8,
    /// Whether 64-bit addressing is supported (HCCPARAMS1.AC64).
    pub ac64: bool,
    /// Page size in bytes (from PAGESIZE reg).
    pub page_size: u32,
    /// Command Ring.
    pub cmd_ring: TrbRing,
    /// Event Ring.
    pub event_ring: EventRing,
    /// Device Context Base Address Array (DCBAA) — DMA memory.
    pub dcbaa: RingMem,
    /// Scratchpad buffer array (DMA) + buffers.
    pub scratchpad: ScratchpadState,
    /// Device slots.
    pub slots: [SlotState; MAX_SLOTS],
    /// Verbosity.
    pub verbose: u8,
    /// Detected USB Mass Storage devices.
    pub msc_devices: [MscDevice; MAX_MSC_DEVICES],
    /// USB device registry (class driver dispatch).
    pub device_registry: UsbDeviceRegistry,
}

/// Scratchpad buffer management.
pub struct ScratchpadState {
    pub array_phys: u64,
    pub array_virt: *mut ScratchpadEntry,
    pub count: u32,
    pub buffers: [Option<RingMem>; 32],
}

impl ScratchpadState {
    fn zeroed() -> Self {
        const NONE: Option<RingMem> = None;
        Self {
            array_phys: 0,
            array_virt: ptr::null_mut(),
            count: 0,
            buffers: [NONE; 32],
        }
    }
}

/// State of a single device slot.
pub struct SlotState {
    pub id: u8,
    pub assigned: bool,
    pub addressed: bool,
    pub configured: bool,
    pub speed: u8,
    pub port: u8,
    pub ctx: Option<RingMem>,
    pub input_ctx: Option<RingMem>,
    pub transfer_rings: [Option<TrbRing>; 31],
}

impl SlotState {
    fn new() -> Self {
        const NONE: Option<TrbRing> = None;
        Self {
            id: 0, assigned: false, addressed: false, configured: false,
            speed: 0, port: 0, ctx: None, input_ctx: None,
            transfer_rings: [NONE; 31],
        }
    }
}

// ============================================================================
// MMIO helpers
// ============================================================================

impl XhciController {
    fn init_slots() -> [SlotState; MAX_SLOTS] {
        core::array::from_fn(|_| SlotState::new())
    }

    fn r32(&self, offset: usize) -> u32 {
        unsafe { ffi::read32_raw(self.mmio_base as usize + offset) }
    }

    fn w32(&self, offset: usize, val: u32) {
        unsafe { ffi::write32_raw(self.mmio_base as usize + offset, val) }
    }

    fn r64(&self, offset: usize) -> u64 {
        let lo = self.r32(offset) as u64;
        let hi = self.r32(offset + 4) as u64;
        lo | (hi << 32)
    }

    fn w64(&self, offset: usize, val: u64) {
        self.w32(offset, val as u32);
        self.w32(offset + 4, (val >> 32) as u32);
    }

    fn op_r32(&self, reg: usize) -> u32 {
        self.r32(self.caplength as usize + reg)
    }

    fn op_w32(&self, reg: usize, val: u32) {
        self.w32(self.caplength as usize + reg, val)
    }

    fn op_r64(&self, reg: usize) -> u64 {
        self.r64(self.caplength as usize + reg)
    }

    fn op_w64(&self, reg: usize, val: u64) {
        self.w64(self.caplength as usize + reg, val)
    }

    fn rt_r32(&self, interrupter: u8, reg: usize) -> u32 {
        let off = self.rtsoff as usize + (interrupter as usize * rt::INTR_SIZE) + reg;
        self.r32(off)
    }

    pub fn rt_w32(&self, interrupter: u8, reg: usize, val: u32) {
        let off = self.rtsoff as usize + (interrupter as usize * rt::INTR_SIZE) + reg;
        self.w32(off, val)
    }

    fn rt_r64(&self, interrupter: u8, reg: usize) -> u64 {
        let off = self.rtsoff as usize + (interrupter as usize * rt::INTR_SIZE) + reg;
        self.r64(off)
    }

    pub fn rt_w64(&self, interrupter: u8, reg: usize, val: u64) {
        let off = self.rtsoff as usize + (interrupter as usize * rt::INTR_SIZE) + reg;
        self.w64(off, val)
    }

    fn db_w32(&self, slot_id: u8, val: u32) {
        let off = self.dboff as usize + (slot_id as usize * 4);
        self.w32(off, val);
    }

    // ========================================================================
    // Probe and Init
    // ========================================================================

    /// Probe for xHCI PCI device.
    pub fn probe(skip: c_int) -> Option<c_int> {
        ffi::pci_init_ffi();
        let (devind, _, _) = ffi::pci_first_dev_ffi()?;
        let mut devind = devind;
        for _ in 0..skip {
            (devind, _, _) = ffi::pci_next_dev_ffi()?;
        }
        let class = ffi::pci_attr_r32_ffi(devind, 0x08) >> 16;
        if class != 0x0C0330 {
            let prog_if = ffi::pci_attr_r8_ffi(devind, 0x09);
            let subclass = ffi::pci_attr_r8_ffi(devind, 0x0A);
            let class_code = ffi::pci_attr_r8_ffi(devind, 0x0B);
            let full_class = ((class_code as u32) << 16) | ((subclass as u32) << 8) | prog_if as u32;
            if full_class != 0x0C0330 {
                return None;
            }
        }
        ffi::pci_reserve_ffi(devind);
        Some(devind)
    }

    /// Initialize the xHCI controller.
    pub fn init(devind: c_int, verbose: u8) -> Option<Self> {
        let (base_lo, bar_size, ioflag) = ffi::pci_get_bar_ffi(devind, 0)?;
        if ioflag {
            ffi::print(b"xHCI: BAR0 is I/O, expected MMIO\0");
            return None;
        }
        let (base_hi, _, _) = ffi::pci_get_bar_ffi(devind, 1).unwrap_or((0, 0, false));
        let phys_base = (base_lo as u64) | ((base_hi as u64) << 32);
        let map_size = core::cmp::min(bar_size as usize, 0x10000);
        let mmio_virt = ffi::vm_map_phys_ffi(phys_base as *mut core::ffi::c_void, map_size);
        if mmio_virt.is_null() {
            ffi::print(b"xHCI: unable to map BAR0 MMIO\0");
            return None;
        }
        let mmio_base = mmio_virt as *mut u8;
        let irq = ffi::pci_attr_r8_ffi(devind, 0x3C) as c_int;

        // Read capability registers
        let caplength = unsafe { ffi::read32_raw(mmio_base as usize + cap::CAPLENGTH) } as u8;
        let _hciver = unsafe { ffi::read32_raw(mmio_base as usize + cap::HCIVERSION) } as u16;
        let hcs1 = unsafe { ffi::read32_raw(mmio_base as usize + cap::HCSPARAMS1) };
        let hcs2 = unsafe { ffi::read32_raw(mmio_base as usize + cap::HCSPARAMS2) };
        let hcc1 = unsafe { ffi::read32_raw(mmio_base as usize + cap::HCCPARAMS1) };
        let dboff_raw = unsafe { ffi::read32_raw(mmio_base as usize + cap::DBOFF) };
        let rtsoff_raw = unsafe { ffi::read32_raw(mmio_base as usize + cap::RTSOFF) };

        let max_slots = (hcs1 & 0xFF) as u8;
        let max_intrs = ((hcs1 >> 8) & 0x7FF) as u16;
        let max_ports = ((hcs1 >> 24) & 0xFF) as u8;
        let context_size = if (hcc1 & 0x04) != 0 { 64u8 } else { 32u8 };
        let ac64 = (hcc1 & 0x01) != 0;
        let dboff = dboff_raw & !0x3F;
        let rtsoff = rtsoff_raw & !0x1F;

        if verbose >= 1 {
            ffi::print(b"xHCI: controller detected\0");
        }

        // Allocate Command Ring
        let cmd_ring = TrbRing::new(RING_SIZE, true)?;
        // Allocate Event Ring
        let event_ring = EventRing::new(EVENT_RING_SEG_SIZE)?;
        // Allocate DCBAA (rounded up to 64-byte alignment)
        let dcbaa_size = DCBAA_SIZE * 8;
        let aligned_size = (dcbaa_size + 63) / 64 * 64;
        let dcbaa = RingMem::alloc(aligned_size)?;
        // Allocate scratchpad buffers
        let max_scratch = ((hcs2 >> 21) & 0x1F) << 5 | ((hcs2 >> 16) & 0x1F);
        let scratchpad = Self::init_scratchpad(max_scratch, verbose);

        let mut xhc = Self {
            mmio_base, mmio_size: map_size, devind, irq,
            hook_id: -1, msix_available: false,
            caplength, rtsoff, dboff,
            max_slots, max_intrs, max_ports,
            context_size: if context_size == 64 { 1 } else { 0 },
            ac64, page_size: 4096,
            cmd_ring, event_ring, dcbaa, scratchpad,
            slots: Self::init_slots(),
            device_registry: UsbDeviceRegistry::new(),
            msc_devices: core::array::from_fn(|_| MscDevice::new()),
            verbose,
        };

        if !xhc.hc_reset() {
            ffi::print(b"xHCI: controller reset failed\0");
            return None;
        }

        xhc.page_size = xhc.op_r32(op::PAGESIZE);
        xhc.op_w64(op::DCBAAP, xhc.dcbaa.phys);
        xhc.op_w32(op::CONFIG, max_slots as u32);

        let crcr_val = xhc.cmd_ring.phys() | 1; // RCS = 1
        xhc.op_w64(op::CRCR, crcr_val);
        xhc.setup_event_ring(0);
        xhc.hc_start();

        if !xhc.setup_irq() {
            ffi::print(b"xHCI: IRQ setup failed\0");
            return None;
        }

        if verbose >= 1 {
            ffi::print(b"xHCI: controller initialized\0");
        }
        Some(xhc)
    }

    // ========================================================================
    // HC Reset and Start
    // ========================================================================

    fn hc_reset(&mut self) -> bool {
        let timeout_us = 1_000_000u32;
        let step_us = 1000;
        for _ in 0..(timeout_us / step_us) {
            if (self.op_r32(op::USBSTS) & op::sts::CNR) == 0 { break; }
            ffi::udelay(step_us);
        }
        self.op_w32(op::USBCMD, op::cmd::HC_RESET);
        ffi::udelay(1000);
        for _ in 0..(timeout_us / step_us) {
            if (self.op_r32(op::USBSTS) & op::sts::HCHALTED) != 0 { return true; }
            ffi::udelay(step_us);
        }
        false
    }

    fn hc_start(&mut self) {
        self.op_w32(op::USBCMD, op::cmd::RUN_STOP | op::cmd::INT_EVT_EN);
        let timeout_us = 1_000_000u32;
        let step_us = 1000;
        for _ in 0..(timeout_us / step_us) {
            if (self.op_r32(op::USBSTS) & op::sts::HCHALTED) == 0 { return; }
            ffi::udelay(step_us);
        }
        ffi::print(b"xHCI: controller start timeout\0");
    }

    fn hc_stop(&mut self) {
        let cmd = self.op_r32(op::USBCMD);
        self.op_w32(op::USBCMD, cmd & !op::cmd::RUN_STOP);
        let timeout_us = 1_000_000u32;
        let step_us = 1000;
        for _ in 0..(timeout_us / step_us) {
            if (self.op_r32(op::USBSTS) & op::sts::HCHALTED) != 0 { return; }
            ffi::udelay(step_us);
        }
    }

    // ========================================================================
    // Event Ring Setup
    // ========================================================================

    fn setup_event_ring(&mut self, interrupter: u8) {
        self.rt_w32(interrupter, rt::ERSTSZ, 1);
        self.rt_w64(interrupter, rt::ERSTBA, self.event_ring.erstba());
        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
        self.rt_w64(interrupter, rt::ERDP, erdp);
        self.rt_w32(interrupter, rt::IMAN, rt::iman::IE);
    }

    // ========================================================================
    // Scratchpad Buffer
    // ========================================================================

    fn init_scratchpad(max_scratch: u32, _verbose: u8) -> ScratchpadState {
        if max_scratch == 0 { return ScratchpadState::zeroed(); }
        let count = core::cmp::min(max_scratch, 32);
        let array_size = (count as usize) * core::mem::size_of::<ScratchpadEntry>();
        let (array_virt, array_phys) = match ffi::alloc_contig_ffi(array_size) {
            Some((v, p)) => (v as *mut ScratchpadEntry, p),
            None => return ScratchpadState::zeroed(),
        };
        unsafe { ptr::write_bytes(array_virt, 0, array_size); }
        const NONE_RING: Option<RingMem> = None;
        let mut buffers: [Option<RingMem>; 32] = [NONE_RING; 32];
        for i in 0..count as usize {
            let buf = match RingMem::alloc(4096) {
                Some(b) => b,
                None => break,
            };
            let entry = unsafe { &mut *array_virt.add(i) };
            entry.set_addr(buf.phys);
            buffers[i] = Some(buf);
        }
        ScratchpadState {
            array_phys, array_virt,
            count: buffers.iter().filter(|b| b.is_some()).count() as u32,
            buffers,
        }
    }

    // ========================================================================
    // IRQ Setup
    // ========================================================================

    fn setup_irq(&mut self) -> bool {
        let msix_info = ffi::pci_msix_parse_ffi(self.devind);
        if let Some(info) = msix_info {
            if info.msix_table_size >= 1 {
                let irq = match ffi::msix_alloc_irq() {
                    Some(i) => i,
                    None => return self.setup_legacy_irq(),
                };
                let hook = match ffi::msix_setup(irq) {
                    Some(h) => h,
                    None => { let _ = ffi::msix_free_irq(irq); return self.setup_legacy_irq(); }
                };
                self.irq = irq;
                self.hook_id = hook;
                self.msix_available = true;
                if self.verbose >= 1 { ffi::print(b"xHCI: MSI-X enabled\0"); }
                return true;
            }
        }
        self.setup_legacy_irq()
    }

    fn setup_legacy_irq(&mut self) -> bool {
        let hook_id = ffi::irq_setup(self.irq).unwrap_or_else(|| {
            ffi::driver_panic(b"xHCI: unable to register IRQ\0");
        });
        self.hook_id = hook_id;
        self.msix_available = false;
        if self.verbose >= 1 { ffi::print(b"xHCI: using legacy IRQ\0"); }
        true
    }

    // ========================================================================
    // Port Management
    // ========================================================================

    pub fn portsc(&self, port: u8) -> u32 {
        let off = op::PORT_BASE + ((port as usize - 1) * op::PORT_SIZE);
        self.op_r32(off)
    }

    pub fn set_portsc(&self, port: u8, val: u32) {
        let off = op::PORT_BASE + ((port as usize - 1) * op::PORT_SIZE);
        self.op_w32(off, val);
    }

    pub fn port_connected(&self, port: u8) -> bool {
        (self.portsc(port) & op::portsc::CCS) != 0
    }

    pub fn port_speed(&self, port: u8) -> u8 {
        ((self.portsc(port) >> op::portsc::SPEED_SHIFT) & op::portsc::SPEED_MASK) as u8
    }

    pub fn port_link_state(&self, port: u8) -> u32 {
        (self.portsc(port) >> op::portsc::PLS_SHIFT) & op::portsc::PLS_MASK
    }

    pub fn init_ports(&self) {
        for port in 1..=self.max_ports {
            let sc = self.portsc(port);
            if (sc & op::portsc::PP) == 0 {
                self.set_portsc(port, sc | op::portsc::PP);
                ffi::udelay(20_000);
            }
        }
    }

    pub fn port_reset(&self, port: u8) -> bool {
        let mut sc = self.portsc(port);
        sc |= op::portsc::PR;
        self.set_portsc(port, sc);
        let timeout_us = 100_000u32;
        let step_us = 1000;
        for _ in 0..(timeout_us / step_us) {
            let sc2 = self.portsc(port);
            if (sc2 & op::portsc::PR) == 0 { return true; }
            ffi::udelay(step_us);
        }
        false
    }

    // ========================================================================
    // Command Ring Operations
    // ========================================================================

    fn submit_command(&mut self, trb: &Trb) -> bool {
        match self.cmd_ring.reserve() {
            Some(ptr) => {
                unsafe { ptr::write_volatile(ptr, *trb); }
                self.cmd_ring.commit();
                self.db_w32(0, 0); // Doorbell for slot 0 = command ring
                ffi::udelay(1000);
                self.poll_event_ring()
            }
            None => false,
        }
    }

    fn poll_event_ring(&mut self) -> bool {
        let timeout_us = 5_000_000u32;
        let step_us = 100;
        for _ in 0..(timeout_us / step_us) {
            while let Some(event) = self.event_ring.next_event() {
                match event.trb_type() {
                    Some(TrbType::CommandCompletionEvent) => {
                        let cc_val = (event.status[0] as u16) | ((event.status[1] as u16) << 8);
                        let cc = cc_val & 0xFF;
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                        return cc == 1;
                    }
                    Some(TrbType::PortStatusChangeEvent) => {
                        let port_id = event.flags[3];
                        if self.verbose >= 1 { ffi::print(b"xHCI: port status change\0"); }
                        if port_id > 0 && port_id <= self.max_ports {
                            let sc = self.portsc(port_id);
                            self.set_portsc(port_id, sc | op::portsc::CSC);
                        }
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                    }
                    Some(TrbType::TransferEvent) => {
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                    }
                    _ => {
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                    }
                }
            }
            ffi::udelay(step_us);
        }
        if self.verbose >= 1 { ffi::print(b"xHCI: command timeout\0"); }
        false
    }

    // ========================================================================
    // Device Slot Management
    // ========================================================================

    pub fn enable_slot(&mut self) -> u8 {
        let trb = build_enable_slot_trb(self.cmd_ring.cycle);
        if !self.submit_command(&trb) { return 0; }
        for i in 1..self.max_slots as usize {
            if !self.slots[i].assigned {
                self.slots[i].id = i as u8;
                self.slots[i].assigned = true;
                return i as u8;
            }
        }
        0
    }

    pub fn address_device(&mut self, slot_id: u8, port: u8, speed_code: u8, bsr: bool) -> bool {
        let idx = slot_id as usize;
        if idx >= MAX_SLOTS || !self.slots[idx].assigned { return false; }

        // Allocate device context DMA (Slot + 31 EP contexts)
        let ctx_size = 32 + 31 * 32;
        let aligned = (ctx_size + 63) / 64 * 64;
        let ctx = match RingMem::alloc(aligned) { Some(m) => m, None => return false };
        let ctx_phys = ctx.phys;

        // Allocate input context DMA (+8 for add/drop flags)
        let in_ctx_size = 8 + ctx_size;
        let in_aligned = (in_ctx_size + 63) / 64 * 64;
        let input_ctx = match RingMem::alloc(in_aligned) { Some(m) => m, None => return false };
        let input_ctx_phys = input_ctx.phys;

        // Zero both
        unsafe { ptr::write_bytes(ctx.virt, 0, ctx.size); }
        unsafe { ptr::write_bytes(input_ctx.virt, 0, input_ctx.size); }

        // Set up input context slot context
        let slot_ctx = unsafe { &mut *((input_ctx.virt as usize + 8) as *mut SlotContext) };
        slot_ctx.set_route_string(0);
        let dw0 = slot_ctx.get_dw0();
        slot_ctx.set_dw0((dw0 & !0xF) | (speed_code as u32 & 0xF));
        slot_ctx.set_dw0((slot_ctx.get_dw0() & !(0x1F << 27)) | (1 << 27));
        let dw1 = slot_ctx.get_dw1();
        slot_ctx.set_dw1((dw1 & !(0xFF << 24)) | ((port as u32) << 24));
        slot_ctx.set_dw1((slot_ctx.get_dw1() & !0xFFFF) | 20);

        // Configure default endpoint (EP 0)
        let mps = default_max_packet_size(speed_code);
        let ep0_offset = input_ctx.virt as usize + 8 + 32;
        let ep0_ctx = unsafe { &mut *(ep0_offset as *mut EndpointContext) };
        ep0_ctx.set_ep_type(4); // Control OUT
        ep0_ctx.set_max_packet_size(mps);
        ep0_ctx.set_cerr(3);
        ep0_ctx.set_tr_dequeue_ptr(ep0_offset as u64); // Placeholder

        // Set Add flags (bit 0 = slot, bit 1 = EP0)
        let add_flags = unsafe { &mut *(input_ctx.virt as *mut u32) };
        *add_flags = 3;

        // Write DCBAA entry
        let dcbaa_ptr = unsafe { &mut *(self.dcbaa.virt as *mut u64).add(idx) };
        *dcbaa_ptr = ctx_phys;

        let slot = &mut self.slots[idx];
        slot.ctx = Some(ctx);
        slot.input_ctx = Some(input_ctx);
        slot.speed = speed_code;
        slot.port = port;

        let trb = build_address_device_trb(self.cmd_ring.cycle, slot_id, input_ctx_phys, bsr);
        self.submit_command(&trb)
    }

    // ========================================================================
    // Cleanup / Stop
    // ========================================================================

    pub fn stop(&mut self) {
        self.hc_stop();
        self.cmd_ring.free();
        self.event_ring.free();
        self.dcbaa.free();

        for buf in self.scratchpad.buffers.iter_mut() {
            if let Some(b) = buf { b.free(); }
        }

        for slot in self.slots.iter_mut() {
            if let Some(ctx) = &mut slot.ctx { ctx.free(); }
            if let Some(ictx) = &mut slot.input_ctx { ictx.free(); }
            for ring in slot.transfer_rings.iter_mut() {
                if let Some(r) = ring { r.free(); }
            }
        }

        if self.hook_id != 0 { let _ = ffi::irq_remove(&mut self.hook_id); }
        if self.msix_available && self.irq != 0 { let _ = ffi::msix_free_irq(self.irq); }

        let _ = ffi::vm_unmap_phys_ffi(
            self.mmio_base as *mut core::ffi::c_void,
            self.mmio_size,
        );
    }

    // ========================================================================
    // Transfer Ring Integration
    // ========================================================================

    /// Convert endpoint number and direction to Device Context Index (DCI).
    /// DCI 1 = EP0 Control, DCI 2 = EP1 OUT, DCI 3 = EP1 IN, etc.
    pub fn ep_num_to_dci(ep_num: u8, dir_in: bool) -> u8 {
        if ep_num == 0 {
            1 // EP0 = DCI 1
        } else if dir_in {
            ep_num * 2 + 1
        } else {
            ep_num * 2
        }
    }

    /// Configure an endpoint with a new transfer ring.
    /// `dci` — Device Context Index (1..31, see ep_num_to_dci())
    /// `ep_type` — xHCI EP type: 2=Bulk OUT, 4=Control, 6=Bulk IN, 7=Interrupt IN, etc.
    /// `mps` — Max Packet Size (e.g. 64 for FS/HS bulk, 512 for SS bulk)
    /// `cerr` — CErr (Error Count, 0-3, recommended 3)
    /// `avg_trb_len` — Average TRB Length for controller scheduling
    pub fn configure_endpoint(&mut self, slot_id: u8, dci: u8, ep_type: u8,
        mps: u16, cerr: u8, avg_trb_len: u16) -> bool
    {
        let idx = slot_id as usize;
        if idx >= MAX_SLOTS || !self.slots[idx].assigned || dci < 1 || dci > 31 {
            return false;
        }

        let ep_idx = dci as usize - 1;

        // Validate input context exists (must be created by address_device)
        // Use raw pointer to avoid borrow conflict with submit_command below
        let input_ctx_virt: *mut u8;
        let input_ctx_phys: u64;
        let input_ctx_size: usize;
        {
            let slot = &self.slots[idx];
            match &slot.input_ctx {
                Some(ctx) => {
                    input_ctx_virt = ctx.virt;
                    input_ctx_phys = ctx.phys;
                    input_ctx_size = ctx.size;
                }
                None => return false,
            }
        }

        // Allocate a new transfer ring for this endpoint
        let mut ring = match TrbRing::new(RING_SIZE, true) {
            Some(r) => r,
            None => return false,
        };
        let ring_phys = ring.phys();

        // Zero the input context via raw pointer (no borrow on self)
        unsafe { ptr::write_bytes(input_ctx_virt, 0, input_ctx_size); }

        // Set Add Context flag for this DCI
        unsafe { *(input_ctx_virt as *mut u32) = 1u32 << dci; }

        // Set up the Endpoint Context for this DCI
        let ctx_offset = input_ctx_virt as usize + 8 + (ep_idx * 32);
        let ep_ctx = unsafe { &mut *(ctx_offset as *mut EndpointContext) };
        ep_ctx.set_ep_type(ep_type);
        ep_ctx.set_max_packet_size(mps);
        ep_ctx.set_cerr(cerr);
        ep_ctx.set_average_trb_len(avg_trb_len);
        ep_ctx.set_tr_dequeue_ptr(ring_phys);

        // Update the slot context context_entries field
        let slot_ctx = unsafe { &mut *((input_ctx_virt as usize + 8) as *mut SlotContext) };
        let current_entries = slot_ctx.context_entries();
        if dci > current_entries {
            let dw0 = slot_ctx.get_dw0();
            slot_ctx.set_dw0((dw0 & !(0x1F << 27)) | ((dci as u32) << 27));
        }

        // Submit Configure Endpoint command
        // Note: slot borrow is dropped by now — safe to borrow self again
        let trb = build_configure_endpoint_trb(self.cmd_ring.cycle, slot_id, input_ctx_phys, false);
        if !self.submit_command(&trb) {
            ring.free(); // Free DMA memory on command failure
            return false;
        }

        // Re-borrow slot to store the ring
        let slot = &mut self.slots[idx];
        slot.transfer_rings[ep_idx] = Some(ring);
        slot.configured = true;
        if self.verbose >= 1 {
            ffi::print(b"xHCI: endpoint configured\0");
        }
        true
    }

    /// Submit an Evaluate Context command to update device context.
    /// `input_ctx_phys` — physical address of input context with updated fields.
    pub fn evaluate_context(&mut self, slot_id: u8, input_ctx_phys: u64) -> bool {
        let idx = slot_id as usize;
        if idx >= MAX_SLOTS || !self.slots[idx].assigned {
            return false;
        }
        let trb = build_evaluate_context_trb(self.cmd_ring.cycle, slot_id, input_ctx_phys);
        self.submit_command(&trb)
    }

    /// Doorbell ring for a specific endpoint (DCI) on a device slot.
    pub fn ring_doorbell(&self, slot_id: u8, dci: u8) {
        if dci > 0 && dci <= 31 {
            self.db_w32(slot_id, dci as u32);
        }
    }

    /// Queue a bulk data transfer on an endpoint's transfer ring.
    /// `ep_num` — endpoint number (1..15, use 0 for EP0 with care).
    /// `dir_in` — true for IN (device→host), false for OUT (host→device).
    /// `data_phys` — physical address of the DMA data buffer.
    /// `data_len` — transfer length in bytes (max 65535 per Normal TRB).
    /// `ioc` — interrupt on completion (set true to get a Transfer Event).
    pub fn queue_bulk_transfer(&mut self, slot_id: u8, ep_num: u8, dir_in: bool,
        data_phys: u64, data_len: u32, ioc: bool) -> bool
    {
        let dci = Self::ep_num_to_dci(ep_num, dir_in);
        if dci < 1 || dci > 31 { return false; }

        let idx = slot_id as usize;
        if idx >= MAX_SLOTS { return false; }

        let ep_idx = dci as usize - 1;
        let slot = &mut self.slots[idx];
        let ring = match &mut slot.transfer_rings[ep_idx] {
            Some(r) => r,
            None => return false, // Endpoint not configured — use configure_endpoint first
        };

        // Build a Normal transfer TRB
        let trb = build_normal_transfer_trb(
            ring.cycle, data_phys, data_len, 0, 0, false, ioc
        );

        // Reserve slot and write to ring
        match ring.reserve() {
            Some(ptr) => {
                unsafe { ptr::write_volatile(ptr, trb); }
                ring.commit();
            }
            None => return false,
        }

        // Ring doorbell to notify the xHC
        self.ring_doorbell(slot_id, dci);
        if self.verbose >= 2 {
            ffi::print(b"xHCI: bulk transfer queued\0");
        }
        true
    }

    // ========================================================================
    // Control Transfer (EP0)
    // ========================================================================

    /// Allocate and install a transfer ring for EP0 (must be called after
    /// address_device() to replace the placeholder dequeue pointer).
    /// Uses Evaluate Context to update EP0's dequeue pointer.
    pub fn setup_ep0_transfer_ring(&mut self, slot_id: u8) -> bool {
        let idx = slot_id as usize;
        if idx >= MAX_SLOTS || !self.slots[idx].assigned { return false; }

        // Allocate EP0 transfer ring
        let mut ep0_ring = match TrbRing::new(RING_SIZE, true) {
            Some(r) => r,
            None => return false,
        };
        let ring_phys = ep0_ring.phys();

        // Get the input context via raw ptr (no borrow conflict with submit_command)
        let input_ctx_virt: *mut u8;
        let input_ctx_phys: u64;
        let input_ctx_size: usize;
        {
            let slot = &self.slots[idx];
            match &slot.input_ctx {
                Some(ctx) => {
                    input_ctx_virt = ctx.virt;
                    input_ctx_phys = ctx.phys;
                    input_ctx_size = ctx.size;
                }
                None => return false,
            }
        }

        // Zero the input context
        unsafe { ptr::write_bytes(input_ctx_virt, 0, input_ctx_size); }

        // Set Add flag for DCI=1 (EP0)
        unsafe { *(input_ctx_virt as *mut u32) = 1u32 << 1; }

        // Write EP0 context with new dequeue pointer (keep existing EP type/MPS from address_device)
        let mps = default_max_packet_size(self.slots[idx].speed);
        let ep0_ctx = unsafe { &mut *((input_ctx_virt as usize + 8 + 32) as *mut EndpointContext) };
        ep0_ctx.set_ep_type(4); // Control
        ep0_ctx.set_max_packet_size(mps);
        ep0_ctx.set_cerr(3);
        ep0_ctx.set_average_trb_len(8);
        ep0_ctx.set_tr_dequeue_ptr(ring_phys);

        // Update slot context_entries (DCI=1 → entries=1)
        let slot_ctx = unsafe { &mut *((input_ctx_virt as usize + 8) as *mut SlotContext) };
        let dw0 = slot_ctx.get_dw0();
        slot_ctx.set_dw0((dw0 & !(0x1F << 27)) | (1u32 << 27));

        // Submit Evaluate Context to update EP0 dequeue pointer
        let trb = build_evaluate_context_trb(self.cmd_ring.cycle, slot_id, input_ctx_phys);
        if !self.submit_command(&trb) {
            ep0_ring.free();
            return false;
        }

        // Store ring
        let slot = &mut self.slots[idx];
        slot.transfer_rings[0] = Some(ep0_ring);
        true
    }

    /// Poll the event ring for a Transfer Event from a specific endpoint.
    /// Returns true if a Transfer Event with Success completion code was seen.
    /// Timeout: `timeout_us` microseconds.
    pub fn poll_transfer_event(&mut self, timeout_us: u32) -> bool {
        let step_us = 100u32;
        let iterations = if timeout_us == 0 { 1 } else { timeout_us / step_us };
        for _ in 0..iterations {
            while let Some(event) = self.event_ring.next_event() {
                match event.trb_type() {
                    Some(TrbType::TransferEvent) => {
                        // Check completion code (status[0..1])
                        let cc = (event.status[0] as u16 | ((event.status[1] as u16) << 8)) & 0xFF;
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                        return cc == 1; // Success
                    }
                    Some(TrbType::CommandCompletionEvent) => {
                        let cc = (event.status[0] as u16 | ((event.status[1] as u16) << 8)) & 0xFF;
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                        // Don't return — keep polling for Transfer Event
                    }
                    Some(TrbType::PortStatusChangeEvent) => {
                        let port_id = event.flags[3];
                        if port_id > 0 && port_id <= self.max_ports {
                            let sc = self.portsc(port_id);
                            self.set_portsc(port_id, sc | op::portsc::CSC);
                        }
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                    }
                    _ => {
                        let erdp = self.event_ring.dequeue_phys() | rt::erdp::EHB;
                        self.rt_w64(0, rt::ERDP, erdp);
                    }
                }
            }
            if timeout_us > 0 {
                ffi::udelay(step_us);
            } else {
                break; // No timeout = poll once
            }
        }
        false
    }

    /// Execute a USB control transfer on EP0.
    /// `slot_id` — device slot.
    /// `setup_pkt` — 8-byte USB setup packet.
    /// `data_dir_in` — true for IN (device→host), false for OUT (host→device).
    /// `data_phys` — physical address of data DMA buffer (can be 0 if no data).
    /// `data_len` — length of data phase.
    /// Returns true if the transfer completed successfully.
    pub fn control_transfer(
        &mut self, slot_id: u8, setup_pkt: &[u8; 8],
        data_dir_in: bool, data_phys: u64, data_len: u32
    ) -> bool {
        let idx = slot_id as usize;
        if idx >= MAX_SLOTS { return false; }

        let ring = match &mut self.slots[idx].transfer_rings[0] {
            Some(r) => r,
            None => return false, // setup_ep0_transfer_ring() not called
        };

        let has_data = data_len > 0;
        let trt: u8 = if !has_data {
            0 // No data
        } else if data_dir_in {
            2 // IN data
        } else {
            1 // OUT data
        };

        // Build Setup Stage TRB (CHAIN set if data follows)
        let setup_trb = build_setup_stage_trb(ring.cycle, setup_pkt, 0, trt);
        // The Setup Stage builder already sets IOC. But it also always sets it.
        // For control transfers, set CHAIN on Setup if data stage follows
        if has_data {
            // CHAIN bit = bit 4
            // We copy the TRB, modify flags, and write it. But we already built it.
            // Let's just set the chain bit manually.
        }

        // Build Data Stage TRB (if needed)
        let data_trb = if has_data {
            Some(build_data_stage_trb(
                ring.cycle, data_phys, data_len, 0, data_dir_in, true, false
            ))
        } else {
            None
        };

        // Build Status Stage TRB (direction opposite of data, or IN for no-data)
        let status_dir_in = if has_data { !data_dir_in } else { true };
        let status_trb = build_status_stage_trb(ring.cycle, 0, status_dir_in);

        // Write all TRBs to the ring
        // Note: For control transfers, we write all TRBs as a single TD.
        // The CHAIN bit connects them. The last TRB has IOC.
        let mut setup_flags = setup_trb.get_flags();
        if has_data {
            setup_flags |= 0x10; // Set CHAIN on Setup
        }

        let mut status_flags = status_trb.get_flags();
        status_flags |= 0x20; // IOC on Status
        status_flags &= !0x10; // No CHAIN on Status (end of TD)

        // Reserve first slot
        let ptr1 = match ring.reserve() {
            Some(p) => p,
            None => return false,
        };
        unsafe {
            let mut trb = setup_trb;
            trb.set_flags(setup_flags);
            ptr::write_volatile(ptr1, trb);
        }

        // Reserve second slot
        let ptr2 = match ring.reserve() {
            Some(p) => p,
            None => return false,
        };
        if has_data {
            // Data Stage TRB — CHAIN should be set (status follows)
            unsafe {
                ptr::write_volatile(ptr2, data_trb.unwrap());
            }
        } else {
            // Status Stage TRB (no data — IOC set)
            unsafe {
                let mut trb = status_trb;
                trb.set_flags(status_flags);
                ptr::write_volatile(ptr2, trb);
            }
        }

        if has_data {
            // Reserve third slot for Status Stage
            let ptr3 = match ring.reserve() {
                Some(p) => p,
                None => return false,
            };
            unsafe {
                let mut trb = status_trb;
                trb.set_flags(status_flags);
                ptr::write_volatile(ptr3, trb);
            }
        }

        // Memory fence and ring doorbell
        ring.commit();
        self.ring_doorbell(slot_id, 1); // DCI=1 for EP0

        // Poll for Transfer Event completion
        self.poll_transfer_event(5_000_000) // 5s timeout
    }

    /// Read the USB Device Descriptor from the device and parse it.
    /// `buf_virt` — virtual address of a DMA buffer (at least 18 bytes).
    /// `buf_phys` — physical address of the same DMA buffer.
    pub fn get_device_descriptor(
        &mut self, slot_id: u8, buf_virt: *mut u8, buf_phys: u64
    ) -> Option<DeviceDescriptor> {
        let pkt = build_setup_packet(
            0x80, // device-to-host, standard, device
            usb_req::GET_DESCRIPTOR,
            (usb_descriptor::DEVICE as u16) << 8,
            0,
            18,
        );
        if !self.control_transfer(slot_id, &pkt, true, buf_phys, 18) {
            return None;
        }
        // Parse descriptor from DMA buffer
        let data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, 18) };
        DeviceDescriptor::parse(data)
    }

    /// Read the full USB Configuration Descriptor from the device.
    /// First reads the 9-byte header to get total_length, then reads the full descriptor.
    /// `buf_virt` — virtual address of a DMA buffer (at least 512 bytes).
    /// `buf_phys` — physical address of the buffer.
    /// Returns the raw descriptor bytes in the buffer or None on failure.
    /// Use ConfigDescriptor::parse() on the first 9 bytes to get the header.
    pub fn get_config_descriptor(
        &mut self, slot_id: u8, index: u8,
        buf_virt: *mut u8, buf_phys: u64, buf_size: usize
    ) -> bool {
        if buf_size < 9 { return false; }

        // First read 9 bytes to get total_length
        let pkt1 = build_setup_packet(
            0x80, usb_req::GET_DESCRIPTOR,
            ((usb_descriptor::CONFIGURATION as u16) << 8) | index as u16,
            0, 9,
        );
        if !self.control_transfer(slot_id, &pkt1, true, buf_phys, 9) {
            return false;
        }

        // Parse header to get total_length
        let data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, 9) };
        let cfg = match ConfigDescriptor::parse(data) {
            Some(c) => c,
            None => return false,
        };
        let total_len = cfg.total_length() as usize;
        if total_len > buf_size { return false; }

        // Read full descriptor
        let pkt2 = build_setup_packet(
            0x80, usb_req::GET_DESCRIPTOR,
            ((usb_descriptor::CONFIGURATION as u16) << 8) | index as u16,
            0, total_len as u16,
        );
        if !self.control_transfer(slot_id, &pkt2, true, buf_phys, total_len as u32) {
            return false;
        }
        true
    }

    /// Read a USB String Descriptor from the device.
    /// `buf_virt` — virtual address of a DMA buffer (at least 256 bytes).
    /// `buf_phys` — physical address of the buffer.
    /// Returns the raw string data in the buffer, or false on failure.
    /// String is in UTF-16LE format, starting at byte 2 of the buffer.
    pub fn get_string_descriptor(
        &mut self, slot_id: u8, index: u8, lang_id: u16,
        buf_virt: *mut u8, buf_phys: u64, buf_len: usize
    ) -> bool {
        if buf_len < 2 { return false; }

        // First read 2 bytes to get string length
        let pkt = build_setup_packet(
            0x80, usb_req::GET_DESCRIPTOR,
            ((usb_descriptor::STRING as u16) << 8) | index as u16,
            lang_id, 2,
        );
        if !self.control_transfer(slot_id, &pkt, true, buf_phys, 2) {
            return false;
        }

        // Get actual string length from first byte
        let data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, 2) };
        let str_len = data[0] as usize;
        if str_len < 2 || str_len > buf_len { return false; }

        // Read full string
        let pkt2 = build_setup_packet(
            0x80, usb_req::GET_DESCRIPTOR,
            ((usb_descriptor::STRING as u16) << 8) | index as u16,
            lang_id, str_len as u16,
        );
        self.control_transfer(slot_id, &pkt2, true, buf_phys, str_len as u32)
    }

    // ========================================================================
    // Enumeration
    // ========================================================================

    /// Enumerate a newly connected device on a port with full USB device setup.
    /// This includes reading descriptors, classifying the device, and dispatching
    /// to a class driver (MSC, Hub, etc.).
    pub fn enumerate_port_full(&mut self, port: u8, speed: u8,
        buf_virt: *mut u8, buf_phys: u64, buf_size: usize
    ) {
        if self.verbose >= 1 { ffi::print(b"xHCI: device connected\0"); }
        let slot_id = self.enable_slot();
        if slot_id == 0 {
            if self.verbose >= 1 { ffi::print(b"xHCI: enable slot failed\0"); }
            return;
        }
        if !self.port_reset(port) {
            if self.verbose >= 1 { ffi::print(b"xHCI: port reset failed\0"); }
            return;
        }
        if !self.address_device(slot_id, port, speed, true) {
            if self.verbose >= 1 { ffi::print(b"xHCI: address device failed\0"); }
            return;
        }
        if self.verbose >= 1 { ffi::print(b"xHCI: device addressed\0"); }

        // Full enumeration — read descriptors, classify, dispatch
        crate::usb_interface::enumerate_device_full(
            self, slot_id, buf_virt, buf_phys, buf_size
        );
    }

    /// Enumerate a newly connected device on a port (basic — backward compat).
    pub fn enumerate_port(&mut self, port: u8, speed: u8) {
        if self.verbose >= 1 { ffi::print(b"xHCI: device connected\0"); }
        let slot_id = self.enable_slot();
        if slot_id == 0 {
            if self.verbose >= 1 { ffi::print(b"xHCI: enable slot failed\0"); }
            return;
        }
        if !self.port_reset(port) {
            if self.verbose >= 1 { ffi::print(b"xHCI: port reset failed\0"); }
            return;
        }
        if !self.address_device(slot_id, port, speed, true) {
            if self.verbose >= 1 { ffi::print(b"xHCI: address device failed\0"); }
            return;
        }
        if self.verbose >= 1 { ffi::print(b"xHCI: device addressed\0"); }
    }
}
