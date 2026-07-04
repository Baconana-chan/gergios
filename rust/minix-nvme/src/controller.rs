//! # Controller — NVMe Controller Initialization and I/O
//!
//! Implements: PCI probe, MMIO BAR mapping, admin queue setup,
//! Identify controller/namespace, I/O queue creation, read/write commands,
//! PRP list support for multi-page transfers, and MSI-X per-queue vectors.

#![allow(dead_code)]

use core::ffi::c_int;
use core::ptr;

use crate::ffi;
use crate::registers::{self, regs, opcode, cns, status, feature};
use crate::registers::{SqEntry, CqEntry, IdentifyController, IdentifyNamespace,
    QueueMem, NVME_PAGE_SIZE, MAX_PRP_LIST};
use minix_driver::mmio::MmioRegion;

/// Maximum number of I/O queues to create.
const MAX_IO_QUEUES: u16 = 8;

/// Maximum queue entries (must match CAP.MQES).
const MAX_QUEUE_ENTRIES: u16 = 256;

/// Admin queue size (number of entries).
const ADMIN_QUEUE_SIZE: u16 = 64;

/// I/O queue size (number of entries).
const IO_QUEUE_SIZE: u16 = 128;

/// Number of namespace slots.
const MAX_NAMESPACES: usize = 32;

/// Timeout for controller ready (in 500ms units).
const DEFAULT_TIMEOUT: u32 = 30; // 15 seconds

/// Maximum PRP list entries (512).
const PRP_LIST_ENTRIES: usize = MAX_PRP_LIST;

/// Size of PRP list buffer in bytes (512 entries * 8 bytes).
const PRP_LIST_SIZE: usize = PRP_LIST_ENTRIES * 8;

/// Maximum number of consecutive controller resets before giving up.
const MAX_RESETS: u32 = 3;

/// Delay between reset retry attempts (in microseconds).
const RESET_RETRY_DELAY_US: u32 = 1_000_000; // 1 second

/// Per I/O queue state.
#[derive(Clone)]
pub struct IoQueue {
    /// Queue ID (starts at 1).
    pub qid: u16,
    /// Submission Queue memory.
    pub sq_mem: QueueMem,
    /// Completion Queue memory.
    pub cq_mem: QueueMem,
    /// Submission Queue Tail pointer (index into sq_mem).
    pub sq_tail: u16,
    /// Submission Queue Head pointer (read from CQ phase bit).
    pub sq_head: u16,
    /// Completion Queue Head pointer (index into cq_mem).
    pub cq_head: u16,
    /// Completion Queue phase tag (inverted on wrap).
    pub cq_phase: bool,
    /// MSI-X IRQ number for this queue.
    pub irq: c_int,
    /// Hook ID for MSI-X handler.
    pub hook_id: c_int,
    /// Whether this queue is active.
    pub active: bool,
    /// Number of pending completions (to process from interrupt).
    pub pending: u32,
}

impl IoQueue {
    fn new(qid: u16) -> Self {
        Self {
            qid,
            sq_mem: QueueMem::zeroed(),
            cq_mem: QueueMem::zeroed(),
            sq_tail: 0,
            sq_head: 0,
            cq_head: 0,
            cq_phase: true,
            irq: 0,
            hook_id: 0,
            active: false,
            pending: 0,
        }
    }

    /// Process all available completions on this queue.
    /// Returns the number of completions consumed.
    /// Caller must ring the CQ doorbell after.
    pub fn process_completions(&mut self) -> u16 {
        let cq_base = self.cq_mem.virt as *mut CqEntry;
        let mut consumed = 0u16;

        loop {
            let head = self.cq_head as usize;
            let entry = unsafe { &*cq_base.add(head) };
            if entry.phase() != self.cq_phase {
                break;
            }
            let new_head = (self.cq_head + 1) % IO_QUEUE_SIZE;
            self.cq_head = new_head;
            if new_head == 0 {
                self.cq_phase = !self.cq_phase;
            }
            consumed += 1;
        }
        consumed
    }
}

/// PRP list descriptor — holds a DMA buffer for a Physical Region Page list.
pub struct PrpList {
    pub mem: Option<QueueMem>,
}

impl PrpList {
    pub fn new() -> Self {
        Self { mem: None }
    }

    /// Allocate the PRP list DMA buffer.
    pub fn alloc(&mut self) -> bool {
        if let Some(m) = ffi::alloc_contig_ffi(PRP_LIST_SIZE) {
            unsafe { ptr::write_bytes(m.0, 0, PRP_LIST_SIZE); }
            self.mem = Some(QueueMem {
                virt: m.0 as *mut u8,
                phys: m.1,
                size: PRP_LIST_SIZE,
            });
            true
        } else {
            false
        }
    }

    /// Free the PRP list DMA buffer.
    pub fn free(&mut self) {
        if let Some(ref m) = self.mem {
            if !m.virt.is_null() {
                ffi::free_contig_ffi(m.virt as *mut core::ffi::c_void, m.size);
            }
        }
        self.mem = None;
    }

    /// Build a PRP list for a transfer starting at `data_phys` of `total_bytes` size.
    /// PRP1 already covers the first page. The list entries cover pages 1..N.
    /// Returns (prp2_value, num_pages) where prp2_value is either:
    /// - 0 if only 1 page is needed
    /// - data_phys + page_size if 2 pages are needed (direct PRP2)
    /// - phys address of the PRP list if 3+ pages are needed
    pub fn build(&mut self, data_phys: u64, total_bytes: usize, page_size: usize) -> (u64, usize) {
        let num_pages = (total_bytes + page_size - 1) / page_size;
        if num_pages <= 1 {
            return (0, 1);
        }

        // Page boundary check
        let offset_in_page = (data_phys as usize) & (page_size - 1);
        let first_page_end = page_size - offset_in_page;

        if num_pages == 2 && total_bytes <= first_page_end + page_size {
            // Just two pages — PRP2 = address of second page
            return (data_phys + page_size as u64, 2);
        }

        // Three or more pages — need PRP list
        if let Some(ref m) = self.mem {
            let list = m.virt as *mut u64;
            let n = core::cmp::min(num_pages - 1, PRP_LIST_ENTRIES);
            let base = data_phys + page_size as u64;
            for i in 0..n {
                unsafe {
                    ptr::write_volatile(list.add(i), base + (i as u64) * page_size as u64);
                }
            }
            core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
            (m.phys, n + 1)
        } else {
            // No PRP list buffer — fall back to single page
            (0, 1)
        }
    }
}

/// NVMe controller state.
pub struct NvmeController {
    /// MMIO region (BAR0).
    pub mmio: MmioRegion,
    /// MMIO region size.
    pub mmio_size: usize,
    /// PCI device index.
    pub devind: c_int,
    /// Legacy IRQ number.
    pub irq: c_int,
    /// Legacy IRQ hook ID (or MSI-X hook for admin queue).
    pub hook_id: c_int,
    /// Whether MSI-X is active.
    pub msix_available: bool,
    /// Number of MSI-X vectors allocated (for cleanup).
    pub msix_vector_count: u32,
    /// PRP list for multi-page I/O.
    pub prp_list: PrpList,
    /// Admin queue memory.
    pub admin_sq: QueueMem,
    pub admin_cq: QueueMem,
    /// Admin SQ tail.
    pub admin_sq_tail: u16,
    /// Admin CQ head.
    pub admin_cq_head: u16,
    /// Admin CQ phase tag.
    pub admin_cq_phase: bool,
    /// Number of I/O queues.
    pub nr_io_queues: u16,
    /// I/O queues.
    pub io_queues: [IoQueue; MAX_IO_QUEUES as usize],
    /// Doorbell stride (register shift).
    pub doorbell_stride: usize,
    /// Memory page size (in bytes, from CC.MPS).
    pub page_size: usize,
    /// Maximum data transfer size (in pages, 0 = no limit).
    pub mdts: u8,
    /// Number of namespaces detected.
    pub nr_ns: u32,
    /// Identify Namespace data for each NS (4KB each).
    pub ns_data: [Option<IdentifyNamespace>; MAX_NAMESPACES],
    /// Block size for each namespace (bytes).
    pub ns_block_size: [u32; MAX_NAMESPACES],
    /// Number of blocks for each namespace.
    pub ns_block_count: [u64; MAX_NAMESPACES],
    /// Instance number.
    pub instance: c_int,
    /// PCI PM capability pointer (0 if not found).
    pub pm_cap_ptr: c_int,
    /// Current PCI D-state (D0/D1/D2/D3hot).
    pub current_d_state: u16,
    /// Number of controller resets performed (recovery).
    pub reset_count: u32,
    /// Whether we are inside a reset (prevents recursive reset).
    pub in_reset: bool,
    /// Verbosity.
    pub verbose: u8,
}

impl NvmeController {
    /// Read a 32-bit controller register.
    fn r32(&self, offset: usize) -> u32 {
        self.mmio.read32(offset).unwrap_or(0)
    }

    /// Write a 32-bit controller register.
    fn w32(&self, offset: usize, val: u32) {
        let _ = self.mmio.write32(offset, val);
    }

    /// Read a 64-bit controller register.
    fn r64(&self, offset: usize) -> u64 {
        let lo = self.r32(offset) as u64;
        let hi = self.r32(offset + 4) as u64;
        lo | (hi << 32)
    }

    /// Write a 64-bit controller register.
    fn w64(&self, offset: usize, val: u64) {
        self.w32(offset, val as u32);
        self.w32(offset + 4, (val >> 32) as u32);
    }

    /// Read the Submission Queue Tail Doorbell for a queue.
    fn sq_doorbell(&self, qid: u16) -> usize {
        let base = regs::DOORBELL_BASE;
        let stride = 1 << self.doorbell_stride;
        base + (2 * qid as usize) * stride
    }

    /// Read the Completion Queue Head Doorbell for a queue.
    fn cq_doorbell(&self, qid: u16) -> usize {
        let base = regs::DOORBELL_BASE;
        let stride = 1 << self.doorbell_stride;
        base + (2 * qid as usize + 1) * stride
    }

    /// Ring the SQ Tail doorbell for a queue.
    fn ring_sq_db(&self, qid: u16, tail: u16) {
        let db = self.sq_doorbell(qid);
        unsafe { ffi::write32_raw(self.mmio.base() as usize + db, tail as u32); }
    }

    /// Ring the CQ Head doorbell for a queue.
    fn ring_cq_db(&self, qid: u16, head: u16) {
        let db = self.cq_doorbell(qid);
        unsafe { ffi::write32_raw(self.mmio.base() as usize + db, head as u32); }
    }

    /// Wait for controller to become ready (CSTS.RDY == 1).
    fn wait_ready(&self, ready: bool) -> bool {
        let timeout_ms = (self.r32(regs::CAP) >> 24) as u32 * 500;
        let timeout_us = timeout_ms * 1000;
        let step_us = 1000;
        let max_loops = if timeout_us == 0 { 30_000_000 / step_us } else { timeout_us / step_us };

        for _ in 0..max_loops {
            let csts = self.r32(regs::CSTS);
            if ((csts & 1) != 0) == ready {
                return true;
            }
            ffi::udelay(step_us);
        }
        false
    }

    /// Allocate and zero a DMA buffer.
    fn alloc_dma(size: usize) -> Option<QueueMem> {
        let (virt, phys) = ffi::alloc_contig_ffi(size)?;
        unsafe { ptr::write_bytes(virt, 0, size); }
        Some(QueueMem { virt: virt as *mut u8, phys, size })
    }

    /// Free a DMA buffer.
    fn free_dma(mem: &QueueMem) {
        if !mem.virt.is_null() {
            ffi::free_contig_ffi(mem.virt as *mut core::ffi::c_void, mem.size);
        }
    }

    /// Probe for NVMe PCI device (class 0x01, subclass 0x08, prog-if 0x02).
    pub fn probe(skip: c_int) -> Option<c_int> {
        ffi::pci_init_ffi();
        let (devind, _, _) = ffi::pci_first_dev_ffi()?;
        let mut devind = devind;

        for _ in 0..skip {
            (devind, _, _) = ffi::pci_next_dev_ffi()?;
        }

        let class = ffi::pci_attr_r32_ffi(devind, 0x08) >> 16;
        if (class >> 8) != 0x0108 && class != 0x010802 {
            if ffi::pci_attr_r8_ffi(devind, 0x0A) != 0x01 {
                let vid = ffi::pci_attr_r16_ffi(devind, 0x00);
                if !is_known_nvme(vid) {
                    return None;
                }
            }
        }

        ffi::pci_reserve_ffi(devind);
        Some(devind)
    }

    /// Initialize the NVMe controller.
    pub fn init(devind: c_int, instance: c_int, verbose: u8) -> Option<Self> {
        let (base_lo, bar_size, ioflag) = ffi::pci_get_bar_ffi(devind, 0)?;
        if ioflag {
            ffi::print(b"NVMe: BAR0 is I/O, expected MMIO\0");
            return None;
        }

        let (base_hi, _, _) = ffi::pci_get_bar_ffi(devind, 1).unwrap_or((0, 0, false));
        let phys_base = (base_lo as u64) | ((base_hi as u64) << 32);

        let map_size = core::cmp::min(bar_size as usize, 0x2000);
        let mmio_virt = ffi::vm_map_phys_ffi(phys_base as *mut core::ffi::c_void, map_size);
        if mmio_virt.is_null() {
            ffi::print(b"NVMe: unable to map BAR0 MMIO\0");
            return None;
        }

        let mmio = MmioRegion::new_unaligned(mmio_virt as *mut u8, map_size).ok()?;
        let irq = ffi::pci_attr_r8_ffi(devind, 0x3C) as c_int;

        let vs = unsafe { ffi::read32_raw(mmio.base() as usize + regs::VS) };
        if verbose >= 1 {
            let major = (vs >> 16) as u16;
            let minor = ((vs >> 8) & 0xFF) as u8;
            let tertiary = (vs & 0xFF) as u8;
            ffi::print(b"NVMe: controller version\0");
            if verbose >= 2 { ffi::print(b"  major.minor.tertiary\0"); }
        }

        let cap = {
            let lo = unsafe { ffi::read32_raw(mmio.base() as usize + regs::CAP) };
            let hi = unsafe { ffi::read32_raw(mmio.base() as usize + regs::CAP + 4) };
            (lo as u64) | ((hi as u64) << 32)
        };

        let mqes = (cap & 0xFFFF) as u16;
        let _to = ((cap >> 24) & 0xFF) as u32;
        let dstrd = ((cap >> 32) & 0xF) as usize;

        let admin_sq_size = ADMIN_QUEUE_SIZE as usize * core::mem::size_of::<SqEntry>();
        let admin_sq = Self::alloc_dma(admin_sq_size)?;
        let admin_cq_size = ADMIN_QUEUE_SIZE as usize * core::mem::size_of::<CqEntry>();
        let admin_cq = Self::alloc_dma(admin_cq_size)?;

        // Allocate PRP list buffer
        let mut prp_list = PrpList::new();
        if !prp_list.alloc() && verbose >= 1 {
            ffi::print(b"NVMe: unable to allocate PRP list\0");
        }

        let mut nvme = Self {
            mmio, mmio_size: map_size, devind, irq,
            hook_id: -1, msix_available: false, msix_vector_count: 0,
            prp_list,
            admin_sq, admin_cq,
            admin_sq_tail: 0, admin_cq_head: 0, admin_cq_phase: true,
            nr_io_queues: 0,
            io_queues: [IoQueue::new(1), IoQueue::new(2), IoQueue::new(3),
                        IoQueue::new(4), IoQueue::new(5), IoQueue::new(6),
                        IoQueue::new(7), IoQueue::new(8)],
            doorbell_stride: if dstrd > 0 { dstrd } else { 0 },
            page_size: NVME_PAGE_SIZE,
            mdts: 0, nr_ns: 0,
            ns_data: [const { None }; MAX_NAMESPACES],
            ns_block_size: [0; MAX_NAMESPACES],
            ns_block_count: [0; MAX_NAMESPACES],
            pm_cap_ptr: 0, current_d_state: 0, reset_count: 0, in_reset: false,
            instance, verbose,
        };

        nvme.w32(regs::CC, 0);
        if !nvme.wait_ready(false) {
            ffi::print(b"NVMe: controller failed to disable\0");
            Self::free_dma(&nvme.admin_sq);
            Self::free_dma(&nvme.admin_cq);
            return None;
        }

        let aqa = ((ADMIN_QUEUE_SIZE as u32) << 16) | ADMIN_QUEUE_SIZE as u32;
        nvme.w32(regs::AQA, aqa);
        nvme.w64(regs::ASQ, nvme.admin_sq.phys);
        nvme.w64(regs::ACQ, nvme.admin_cq.phys);

        let cc = regs::cc::ENABLE
            | (regs::cc::CSS_NVME << regs::cc::CSS_SHIFT)
            | (6 << regs::cc::IOSQES_SHIFT)
            | (4 << regs::cc::IOCQES_SHIFT);
        nvme.w32(regs::CC, cc);

        if !nvme.wait_ready(true) {
            ffi::print(b"NVMe: controller failed to become ready\0");
            Self::free_dma(&nvme.admin_sq);
            Self::free_dma(&nvme.admin_cq);
            return None;
        }

        // Set up MSI-X (try multi-vector first)
        let nr_io = core::cmp::min(MAX_IO_QUEUES as u16, mqes);
        if !nvme.setup_msix_multi(nr_io) {
            if !nvme.setup_msix_single() {
                if verbose >= 1 {
                    ffi::print(b"NVMe: MSI-X unavailable, using legacy IRQ\0");
                }
                let hook_id = ffi::irq_setup(irq).unwrap_or_else(|| {
                    ffi::driver_panic(b"NVMe: unable to register IRQ\0");
                });
                nvme.hook_id = hook_id;
            }
        }

        // Detect PCI Power Management capability
        nvme.pm_cap_ptr = ffi::pci_find_cap_ffi(nvme.devind, registers::PCI_PM_CAP_ID as c_int)
            .unwrap_or(0);
        if nvme.pm_cap_ptr != 0 {
            // Read current D-state from PMCSR
            let pmcsr = ffi::pci_attr_r16_ffi(nvme.devind, nvme.pm_cap_ptr + 4);
            nvme.current_d_state = pmcsr & registers::pmcsr::POWER_STATE_MASK;
            if verbose >= 1 {
                if nvme.current_d_state == 0 {
                    ffi::print(b"NVMe: PCI PM capability found, current D0\0");
                } else {
                    ffi::print(b"NVMe: PCI PM capability found\0");
                }
            }
        } else if verbose >= 1 {
            ffi::print(b"NVMe: PCI PM capability not found\0");
        }

        // Ensure controller is in D0
        if nvme.pm_cap_ptr != 0 && nvme.current_d_state != 0 {
            nvme.set_d_state(registers::pmcsr::D0);
        }

        // Identify controller
        let mut id_ctrl = IdentifyController {
            vid: 0, ssvid: 0, sn: [0; 20], mn: [0; 40], fr: [0; 8],
            rab: 0, ieee: [0; 3], cmic: 0, mdts: 0, cntlid: 0, ver: 0,
            rtd3r: 0, rtd3e: 0, oaes: 0, ctratt: 0,
            reserved_0: [0; 156], oacs: 0, acl: 0, aerl: 0, frmw: 0,
            lpa: 0, elpe: 0, npss: 0, avscc: 0, apsta: 0, wctemp: 0,
            cctemp: 0, mtfa: 0, hmpre: 0, hmmin: 0, tnvmcap: 0, unvmcap: 0,
            rpmbs: 0,
            reserved_1: [0; 316], sqes: 0, cqes: 0,
            reserved_2: [0; 28], subnqn: [0; 256],
            reserved_3: [0; 768], ioccss: [0; 4],
            reserved_4: [0; 128], subsys_rep: [0; 2560],
        };
        if !nvme.admin_identify(cns::IDENTIFY_CONTROLLER, 0, &mut id_ctrl) {
            ffi::print(b"NVMe: Identify Controller failed\0");
            Self::free_dma(&nvme.admin_sq);
            Self::free_dma(&nvme.admin_cq);
            return None;
        }
        nvme.mdts = id_ctrl.mdts;

        // Configure Autonomous Power State Transition (APST)
        nvme.setup_apst(&id_ctrl);

        if verbose >= 1 {
            let mn_end = id_ctrl.mn.iter().position(|&c| c == 0).unwrap_or(40);
            ffi::print(b"NVMe: controller model: \0");
            if let Ok(s) = core::str::from_utf8(&id_ctrl.mn[..mn_end]) {
                ffi::print(s.as_bytes());
            }
            ffi::print(b"\0");
        }

        // Detect namespaces
        let mut ns_list: [u32; 1024] = [0; 1024];
        if nvme.admin_identify(cns::IDENTIFY_ACTIVE_NS_LIST, 0, &mut ns_list) {
            nvme.nr_ns = 0;
            for &nsid in ns_list.iter() {
                if nsid == 0 { break; }
                nvme.nr_ns += 1;
                if (nsid as usize) <= MAX_NAMESPACES {
                    nvme.identify_namespace(nsid);
                }
            }
        } else {
            nvme.nr_ns = 1;
            nvme.identify_namespace(1);
        }

        if nvme.nr_ns == 0 {
            ffi::print(b"NVMe: no namespaces found\0");
            Self::free_dma(&nvme.admin_sq);
            Self::free_dma(&nvme.admin_cq);
            return None;
        }

        if verbose >= 1 { ffi::print(b"NVMe: namespaces detected\0"); }

        // Create I/O queues (with per-queue MSI-X vectors)
        if !nvme.create_io_queues(nr_io) {
            if verbose >= 1 { ffi::print(b"NVMe: no I/O queues, using admin queue only\0"); }
        }

        // Dump SMART/Health on init (verbose >= 1)
        if verbose >= 1 {
            nvme.dump_smart_health();
        }

        // Dump error log on init (verbose >= 2)
        if verbose >= 2 {
            nvme.dump_error_log();
        }

        Some(nvme)
    }

    /// Set up a single MSI-X vector for the admin queue (fallback).
    fn setup_msix_single(&mut self) -> bool {
        let msix_info = match ffi::pci_msix_parse_ffi(self.devind) {
            Some(info) => info,
            None => return false,
        };
        if msix_info.msix_table_size < 1 { return false; }

        let irq = match ffi::msix_alloc_irq() {
            Some(i) => i,
            None => return false,
        };
        let hook_id = match ffi::msix_setup(irq) {
            Some(h) => h,
            None => { let _ = ffi::msix_free_irq(irq); return false; }
        };

        self.hook_id = hook_id;
        self.irq = irq;
        self.msix_available = true;
        self.msix_vector_count = 1;

        if self.verbose >= 1 { ffi::print(b"NVMe: MSI-X single vector\0"); }
        true
    }

    /// Set up per-queue MSI-X vectors (1 admin + N I/O queues).
    fn setup_msix_multi(&mut self, nr_queues: u16) -> bool {
        let msix_info = match ffi::pci_msix_parse_ffi(self.devind) {
            Some(info) => info,
            None => return false,
        };

        let need = 1 + core::cmp::min(nr_queues, MAX_IO_QUEUES) as usize;
        if (msix_info.msix_table_size as usize) < need {
            return false;
        }

        // Allocate admin vector
        let admin_irq = match ffi::msix_alloc_irq() {
            Some(i) => i,
            None => return false,
        };
        let admin_hook = match ffi::msix_setup(admin_irq) {
            Some(h) => h,
            None => { let _ = ffi::msix_free_irq(admin_irq); return false; }
        };
        self.hook_id = admin_hook;
        self.irq = admin_irq;
        self.msix_available = true;
        self.msix_vector_count = 1;

        // Allocate per-queue vectors
        let qcount = core::cmp::min(nr_queues, MAX_IO_QUEUES);
        for i in 0..qcount as usize {
            let qirq = match ffi::msix_alloc_irq() {
                Some(irq) => irq,
                None => break,
            };
            let qhook = match ffi::msix_setup(qirq) {
                Some(h) => h,
                None => { let _ = ffi::msix_free_irq(qirq); break; }
            };
            self.io_queues[i].irq = qirq;
            self.io_queues[i].hook_id = qhook;
            self.msix_vector_count += 1;
        }

        if self.verbose >= 1 {
            ffi::print(b"NVMe: MSI-X multi-vector enabled\0");
        }
        true
    }

    /// Issue an admin Identify command (CNS = controller or namespace).
    fn admin_identify<T>(&mut self, cns_type: u32, nsid: u32, buf: &mut T) -> bool {
        let buf_size = core::mem::size_of_val(buf);

        let (dma_buf, dma_phys) = match ffi::alloc_contig_ffi(4096) {
            Some((v, p)) => (v as *mut u8, p),
            None => return false,
        };

        let mut sqe = SqEntry::zeroed();
        sqe.set_cmd(opcode::IDENTIFY, 0);
        sqe.set_nsid(nsid);
        sqe.set_cdw10(cns_type);
        sqe.set_prp1(dma_phys);

        let cid = self.admin_sq_tail;
        sqe.dword[0] = (sqe.dword[0] & 0xFFFF_0000) | cid as u32;
        self.submit_admin_cmd(&sqe);

        if !self.poll_admin_cq(cid) {
            ffi::free_contig_ffi(dma_buf as *mut core::ffi::c_void, 4096);
            return false;
        }

        let copy_size = core::cmp::min(buf_size, 4096);
        unsafe { ptr::copy_nonoverlapping(dma_buf, buf as *mut T as *mut u8, copy_size); }

        ffi::free_contig_ffi(dma_buf as *mut core::ffi::c_void, 4096);
        true
    }

    /// Identify a specific namespace.
    fn identify_namespace(&mut self, nsid: u32) {
        let idx = (nsid - 1) as usize;
        if idx >= MAX_NAMESPACES { return; }

        let mut ns = IdentifyNamespace {
            nsze: 0, ncap: 0, nuse: 0, nsfeat: 0, nlbaf: 0, flbas: 0,
            mc: 0, dpc: 0, dps: 0, nmic: 0, rescap: 0, fpi: 0,
            nawun: 0, nawupf: 0, nacwu: 0, nabsn: 0, nabo: 0, nabspf: 0, noiob: 0,
            nvmcap: 0, reserved: [0; 40], nguid: [0; 16], eui64: 0,
            lba_format: [crate::registers::LbaFormat { ms: 0, lbads: 9, rp: 0 }; 16],
            reserved_2: [0; 192], vs: [0; 3712],
        };

        if self.admin_identify(cns::IDENTIFY_NAMESPACE, nsid, &mut ns) {
            let blk_size = ns.lba_data_size();
            let nsze = ns.nsze;
            self.ns_data[idx] = Some(ns);
            self.ns_block_size[idx] = blk_size;
            self.ns_block_count[idx] = nsze;
        }
    }

    /// Submit an admin command to the admin submission queue.
    fn submit_admin_cmd(&mut self, sqe: &SqEntry) {
        let tail = self.admin_sq_tail as usize;
        let sq_base = self.admin_sq.virt as *mut SqEntry;
        unsafe { ptr::write_volatile(sq_base.add(tail), *sqe); }

        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);

        let new_tail = (self.admin_sq_tail + 1) % ADMIN_QUEUE_SIZE;
        self.admin_sq_tail = new_tail;
        self.ring_sq_db(0, new_tail);
    }

    /// Poll the admin completion queue for a command completion.
    fn poll_admin_cq(&mut self, cid: u16) -> bool {
        let timeout_us = 5_000_000;
        let step_us = 100;
        let max_loops = timeout_us / step_us;

        for _ in 0..max_loops {
            let head = self.admin_cq_head as usize;
            let cq_base = self.admin_cq.virt as *mut CqEntry;
            let entry = unsafe { &*cq_base.add(head) };

            if entry.phase() == self.admin_cq_phase && entry.cid() == cid {
                let new_head = (self.admin_cq_head + 1) % ADMIN_QUEUE_SIZE;
                self.admin_cq_head = new_head;
                if new_head == 0 { self.admin_cq_phase = !self.admin_cq_phase; }
                self.ring_cq_db(0, new_head);
                return entry.is_success();
            }
            ffi::udelay(step_us);
        }

        ffi::print(b"NVMe: admin command timeout\0");
        false
    }

    /// Create I/O submission and completion queues with per-queue MSI-X vectors.
    fn create_io_queues(&mut self, nr_queues: u16) -> bool {
        let n = core::cmp::min(nr_queues, MAX_IO_QUEUES);
        if n == 0 { return false; }

        let nd = ((n as u32) << 16) | n as u32;
        if !self.admin_set_features(feature::NUMBER_OF_QUEUES, nd) {
            ffi::print(b"NVMe: Set Features (number of queues) failed\0");
            return false;
        }

        let mut created = 0u16;
        for qid in 1..=n {
            let idx = (qid - 1) as usize;

            // MSI-X vector index for this queue.
            // Vector 0 = admin. I/O queue N = vector N (1-based).
            // MSI-X vector index for this I/O queue.
            // With multi-vector: queue N uses vector N (admin=0, queue1=1, etc.)
            // With single vector or legacy: all use vector 0
            let msix_vector = if self.msix_available && self.msix_vector_count > 1 {
                qid as u32
            } else {
                0u32
            };

            // Allocate I/O Completion Queue memory
            let cq_size = IO_QUEUE_SIZE as usize * core::mem::size_of::<CqEntry>();
            let cq_mem = match Self::alloc_dma(cq_size) {
                Some(m) => m,
                None => break,
            };

            // Build Create I/O CQ command
            let mut sqe = SqEntry::zeroed();
            sqe.set_cmd(opcode::CREATE_IO_CQ, 0);
            sqe.set_cdw10((qid as u32) | ((IO_QUEUE_SIZE as u32) << 16));
            // CDW11: PC(bit0) | IEN(bit1) | IRQ vector(bits 31:16)
            sqe.set_cdw11((1 << 0) | (1 << 1) | (msix_vector << 16));
            sqe.set_prp1(cq_mem.phys);

            let cid = self.admin_sq_tail;
            sqe.dword[0] = (sqe.dword[0] & 0xFFFF_0000) | cid as u32;
            self.submit_admin_cmd(&sqe);
            if !self.poll_admin_cq(cid) {
                Self::free_dma(&cq_mem);
                ffi::print(b"NVMe: Create I/O CQ failed\0");
                break;
            }

            // Allocate I/O Submission Queue memory
            let sq_size = IO_QUEUE_SIZE as usize * core::mem::size_of::<SqEntry>();
            let sq_mem = match Self::alloc_dma(sq_size) {
                Some(m) => m,
                None => { Self::free_dma(&cq_mem); break; }
            };

            // Build Create I/O SQ command
            let mut sqe = SqEntry::zeroed();
            sqe.set_cmd(opcode::CREATE_IO_SQ, 0);
            sqe.set_cdw10((qid as u32) | ((IO_QUEUE_SIZE as u32) << 16));
            // CDW11: PC(bit0) | CQID(bits 31:16) = same qid
            sqe.set_cdw11((1u32 << 0) | ((qid as u32) << 16));
            sqe.set_prp1(sq_mem.phys);

            let cid = self.admin_sq_tail;
            sqe.dword[0] = (sqe.dword[0] & 0xFFFF_0000) | cid as u32;
            self.submit_admin_cmd(&sqe);
            if !self.poll_admin_cq(cid) {
                Self::free_dma(&sq_mem);
                Self::free_dma(&cq_mem);
                ffi::print(b"NVMe: Create I/O SQ failed\0");
                break;
            }

            self.io_queues[idx].sq_mem = sq_mem;
            self.io_queues[idx].cq_mem = cq_mem;
            self.io_queues[idx].active = true;
            created += 1;
        }

        self.nr_io_queues = created;
        true
    }

    /// Configure Autonomous Power State Transition (APST).
    /// Enables the controller to autonomously transition between power states
    /// based on idle time, using the idle timeouts in the power state descriptors.
    fn setup_apst(&mut self, id_ctrl: &registers::IdentifyController) {
        if id_ctrl.apsta & 0x01 == 0 {
            if self.verbose >= 1 {
                ffi::print(b"NVMe: APST not supported by controller\0");
            }
            return;
        }

        let npss = id_ctrl.num_power_states();
        if self.verbose >= 1 {
            ffi::print(b"NVMe: APST supported, enabling...\0");
            let mut _op_count = 0u8;
            let mut _non_op_count = 0u8;
            for i in 0..npss {
                if let Some(ps) = id_ctrl.power_state(i) {
                    if ps.is_non_operational() {
                        _non_op_count += 1;
                    } else {
                        _op_count += 1;
                    }
                }
            }
        }

        // Enable APST: Set Features, FID=0x0C, CDW11 bit 0 = APSTE (1)
        if !self.admin_set_features(
            crate::registers::feature::AUTONOMOUS_POWER_STATE_TRANSITION,
            1, // CDW11 bit 0 = APSTE
        ) {
            if self.verbose >= 1 {
                ffi::print(b"NVMe: APST enable failed\0");
            }
            return;
        }

        // Non-operational state idle timeouts (verbose >= 2)
        // PSD idle timeouts are used by the controller for autonomous transitions.

        if self.verbose >= 1 {
            ffi::print(b"NVMe: APST enabled\0");
        }
    }

    // ========================================================================
    // PCI Power Management (D-state control)
    // ========================================================================

    /// D-state constants (matching registers::pmcsr).
    pub const D0: u16 = registers::pmcsr::D0;      // 0x0000 — fully on
    pub const D1: u16 = registers::pmcsr::D1;      // 0x0001 — light sleep
    pub const D2: u16 = registers::pmcsr::D2;      // 0x0002 — deep sleep
    pub const D3HOT: u16 = registers::pmcsr::D3HOT; // 0x0003 — warm sleep

    /// Get current PCI D-state by reading PMCSR (Power Management Control/Status).
    pub fn get_d_state(&self) -> u16 {
        if self.pm_cap_ptr == 0 {
            return Self::D0;
        }
        let pmcsr = ffi::pci_attr_r16_ffi(self.devind, self.pm_cap_ptr + 4);
        pmcsr & registers::pmcsr::POWER_STATE_MASK
    }

    /// Set PCI D-state via PMCSR write to PCI config space.
    /// Returns true if the transition was initiated.
    /// Per PCI PM spec: D0→D3hot requires usleep(10ms), D3hot→D0 requires usleep(100ms).
    pub fn set_d_state(&mut self, new_state: u16) -> bool {
        if self.pm_cap_ptr == 0 {
            return false;
        }
        let offset = self.pm_cap_ptr + 4;
        let old_state = self.get_d_state();
        if old_state == new_state {
            return true;
        }

        // Read-modify-write: preserve PME_En and other bits
        let pmcsr = ffi::pci_attr_r16_ffi(self.devind, offset);
        let new_pmcsr = (pmcsr & !registers::pmcsr::POWER_STATE_MASK)
            | (new_state & registers::pmcsr::POWER_STATE_MASK);

        ffi::pci_write_cfg16_ffi(self.devind, offset, new_pmcsr);
        self.current_d_state = new_state;

        // Wait for transition to complete
        if new_state == Self::D0 {
            // D0 restore: 10ms delay for device to become operational
            ffi::udelay(10_000);
        } else if new_state == Self::D3HOT || old_state == Self::D0 {
            // Entering D3hot: 100ms for device to enter low-power state
            ffi::udelay(100_000);
        }

        if self.verbose >= 1 {
            ffi::print(b"NVMe: D-state transitioned\0");
        }
        true
    }

    /// Check if a given D-state supports PME (Power Management Event) wake.
    pub fn d_state_supports_pme(&self, d_state: u16) -> bool {
        if self.pm_cap_ptr == 0 {
            return false;
        }
        let pmc = ffi::pci_attr_r16_ffi(self.devind, self.pm_cap_ptr + 2);
        match d_state {
            Self::D0 => (pmc & registers::pmc::PME_D0) != 0,
            Self::D1 => (pmc & registers::pmc::PME_D1) != 0,
            Self::D2 => (pmc & registers::pmc::PME_D2) != 0,
            Self::D3HOT => (pmc & registers::pmc::PME_D3HOT) != 0,
            _ => false,
        }
    }

    /// Enable or disable PME (Power Management Event) wake signaling.
    /// When enabled, the device can assert PME# to wake the system from low-power states.
    pub fn enable_pme(&mut self, enable: bool) -> bool {
        if self.pm_cap_ptr == 0 {
            return false;
        }
        let offset = self.pm_cap_ptr + 4;
        let pmcsr = ffi::pci_attr_r16_ffi(self.devind, offset);
        let new_pmcsr = if enable {
            pmcsr | registers::pmcsr::PME_EN
        } else {
            pmcsr & !registers::pmcsr::PME_EN
        };
        ffi::pci_write_cfg16_ffi(self.devind, offset, new_pmcsr);
        if self.verbose >= 1 && enable {
            ffi::print(b"NVMe: PME wake enabled\0");
        }
        true
    }

    // ========================================================================
    // Get Log Page commands
    // ========================================================================

    /// Issue a Get Log Page admin command (opcode 0x02).
    /// Generic method: reads `num_dwords` of log data for `lid` into `buf`.
    fn admin_get_log_page<T: ?Sized>(&mut self, lid: u8, nsid: u32, buf: &mut T) -> bool {
        let buf_size = core::mem::size_of_val(buf);
        let num_bytes = core::cmp::min(buf_size, 4096);
        let num_dwords = (num_bytes as u32 + 3) / 4; // round up

        let (dma_buf, dma_phys) = match ffi::alloc_contig_ffi(4096) {
            Some((v, p)) => (v as *mut u8, p),
            None => return false,
        };

        let mut sqe = SqEntry::zeroed();
        sqe.set_cmd(opcode::GET_LOG_PAGE, 0);
        sqe.set_nsid(nsid);
        // CDW10: LID (7:0) | NUMDL (31:16) = number of dwords (lower 16 bits)
        sqe.set_cdw10((lid as u32) | (num_dwords << 16));
        sqe.set_prp1(dma_phys);

        let cid = self.admin_sq_tail;
        sqe.dword[0] = (sqe.dword[0] & 0xFFFF_0000) | cid as u32;
        self.submit_admin_cmd(&sqe);

        if !self.poll_admin_cq(cid) {
            ffi::free_contig_ffi(dma_buf as *mut core::ffi::c_void, 4096);
            return false;
        }

        let copy_size = core::cmp::min(buf_size, 4096);
        unsafe { ptr::copy_nonoverlapping(dma_buf, buf as *mut T as *mut u8, copy_size); }

        ffi::free_contig_ffi(dma_buf as *mut core::ffi::c_void, 4096);
        true
    }

    /// Read Error Information Log (LID=0x01).
    /// Populates a buffer of up to `max_entries` error log entries.
    pub fn get_error_log(&mut self, entries: &mut [registers::ErrorLogEntry]) -> bool {
        self.admin_get_log_page(
            registers::log_page::ERROR_INFO,
            0xFFFFFFFF, // NSID = all
            entries,
        )
    }

    /// Read SMART / Health Information (LID=0x02).
    pub fn get_smart_health(&mut self, smart: &mut registers::SmartHealth) -> bool {
        self.admin_get_log_page(
            registers::log_page::SMART_HEALTH,
            0, // NSID = 0 for controller-level log
            smart,
        )
    }    /// Print error log entries to console.
    pub fn dump_error_log(&mut self) {
        let mut smart = NvmeController::alloc_smart_health();
        if self.get_smart_health(&mut smart) {
            let count = smart.num_error_log_entries_raw();
            if count > 0 && self.verbose >= 1 {
                ffi::print(b"NVMe: error log entries present\0");
            }
        }
    }

    /// Get the number of error log entries (from SMART data).
    pub fn get_error_log_count(&mut self) -> u64 {
        let mut smart = NvmeController::alloc_smart_health();
        if self.get_smart_health(&mut smart) {
            smart.num_error_log_entries_raw()
        } else {
            0
        }
    }

    /// Allocate a zeroed SmartHealth struct.
    fn alloc_smart_health() -> registers::SmartHealth {
        registers::SmartHealth {
            critical_warning: 0, temperature: 0, available_spare: 0,
            available_spare_threshold: 0, percentage_used: 0,
            eg_critical_warning: 0, data_units_read: [0; 16],
            data_units_written: [0; 16], host_read_commands: [0; 16],
            host_write_commands: [0; 16], controller_busy_time: [0; 16],
            power_cycles: [0; 16], power_on_hours: [0; 16],
            unsafe_shutdowns: [0; 16], media_errors: [0; 16],
            num_error_log_entries: [0; 16], warning_temp_time: 0,
            critical_temp_time: 0, temp_sensor: [0; 8],
            thermal_temp: [0; 2], reserved: [0; 317],
        }
    }

    /// Print SMART / Health information to console.
    pub fn dump_smart_health(&mut self) {
        let mut smart = Self::alloc_smart_health();
        if !self.get_smart_health(&mut smart) {
            return;
        }

        if self.verbose >= 1 {
            ffi::print(b"NVMe: SMART/Health\0");
        }

        if smart.has_critical_warning() {
            ffi::print(b"NVMe: CRITICAL WARNING active\0");
            let flags = smart.critical_warning_flags();
            if (flags & 0x01) != 0 { ffi::print(b"  - Available spare below threshold\0"); }
            if (flags & 0x02) != 0 { ffi::print(b"  - Temperature above threshold\0"); }
            if (flags & 0x04) != 0 { ffi::print(b"  - Reliability degraded\0"); }
            if (flags & 0x08) != 0 { ffi::print(b"  - Media in read-only mode\0"); }
            if (flags & 0x10) != 0 { ffi::print(b"  - Volatile memory backup failed\0"); }
        }
    }

    /// Issue a Set Features admin command.
    fn admin_set_features(&mut self, fid: u8, cdw11: u32) -> bool {
        let mut sqe = SqEntry::zeroed();
        sqe.set_cmd(opcode::SET_FEATURES, 0);
        sqe.set_cdw10(fid as u32);
        sqe.set_cdw11(cdw11);

        let cid = self.admin_sq_tail;
        sqe.dword[0] = (sqe.dword[0] & 0xFFFF_0000) | cid as u32;
        self.submit_admin_cmd(&sqe);
        self.poll_admin_cq(cid)
    }

    /// Read or write data to a namespace with PRP list support.
    /// On timeout, triggers controller reset and retries once.
    pub fn io_transfer(&mut self, nsid: u32, lba: u64, count: u32,
        write: bool, data_phys: u64, metadata_phys: u64) -> bool
    {
        const MAX_ATTEMPTS: u32 = 2;
        for attempt in 0..MAX_ATTEMPTS {
            let qid = 1u16;
            if !self.io_queues[0].active { return false; }

            let idx = (nsid - 1) as usize;
            if idx >= MAX_NAMESPACES { return false; }

            let blk_size = self.ns_block_size[idx] as u64;
            let total_bytes = (count as u64) * blk_size;

            // Build PRP entries for the transfer (re-done on retry)
            let (prp2, _) = self.prp_list.build(data_phys, total_bytes as usize, self.page_size);

            let opc = if write { opcode::WRITE } else { opcode::READ };
            let mut sqe = SqEntry::zeroed();
            sqe.set_cmd(opc, 0);
            sqe.set_nsid(nsid);
            sqe.set_cdw10(lba as u32);
            sqe.set_cdw11((lba >> 32) as u32);
            sqe.set_cdw12(count - 1);
            sqe.set_prp1(data_phys);
            if prp2 != 0 {
                sqe.set_prp2(prp2);
            } else if metadata_phys != 0 {
                sqe.set_prp2(metadata_phys);
            }

            let sq_db = self.sq_doorbell(qid);
            let cq_db = self.cq_doorbell(qid);
            let mmio_base = self.mmio.base() as usize;

            let queue = &mut self.io_queues[0];
            let tail = queue.sq_tail as usize;
            let sq_base = queue.sq_mem.virt as *mut SqEntry;
            unsafe { ptr::write_volatile(sq_base.add(tail), sqe); }

            core::sync::atomic::fence(core::sync::atomic::Ordering::Release);

            let new_tail = (queue.sq_tail + 1) % IO_QUEUE_SIZE;
            queue.sq_tail = new_tail;
            unsafe { ffi::write32_raw(mmio_base + sq_db, new_tail as u32); }

            // Wait for completion (poll or IRQ-based)
            let timeout_us = 30_000_000;
            let step_us = 100;
            for _ in 0..(timeout_us / step_us) {
                let head = queue.cq_head as usize;
                let cq_base = queue.cq_mem.virt as *mut CqEntry;
                let entry = unsafe { &*cq_base.add(head) };

                if entry.phase() == queue.cq_phase {
                    let new_head = (queue.cq_head + 1) % IO_QUEUE_SIZE;
                    queue.cq_head = new_head;
                    if new_head == 0 { queue.cq_phase = !queue.cq_phase; }
                    unsafe { ffi::write32_raw(mmio_base + cq_db, new_head as u32); }
                    return entry.is_success();
                }
                ffi::udelay(step_us);
            }

            // Timeout — try controller reset and retry (only once)
            ffi::print(b"NVMe: I/O command timeout, resetting controller\0");
            if attempt == 0 && self.reset_count < MAX_RESETS {
                if self.controller_reset() {
                    ffi::udelay(RESET_RETRY_DELAY_US);
                    continue;
                }
            }
            break;
        }

        ffi::print(b"NVMe: I/O command timeout (recovery failed)\0");
        false
    }

    /// Flush (write buffer to NAND) for a namespace.
    /// On timeout, triggers controller reset and retries once.
    pub fn io_flush(&mut self, nsid: u32) -> bool {
        const MAX_ATTEMPTS: u32 = 2;
        for attempt in 0..MAX_ATTEMPTS {
            let qid = 1u16;
            if !self.io_queues[0].active { return false; }

            let mut sqe = SqEntry::zeroed();
            sqe.set_cmd(opcode::FLUSH, 0);
            sqe.set_nsid(nsid);

            let sq_db = self.sq_doorbell(qid);
            let cq_db = self.cq_doorbell(qid);
            let mmio_base = self.mmio.base() as usize;

            let queue = &mut self.io_queues[0];
            let tail = queue.sq_tail as usize;
            let sq_base = queue.sq_mem.virt as *mut SqEntry;
            unsafe { ptr::write_volatile(sq_base.add(tail), sqe); }

            core::sync::atomic::fence(core::sync::atomic::Ordering::Release);

            let new_tail = (queue.sq_tail + 1) % IO_QUEUE_SIZE;
            queue.sq_tail = new_tail;
            unsafe { ffi::write32_raw(mmio_base + sq_db, new_tail as u32); }

            let timeout_us = 10_000_000;
            let step_us = 100;
            for _ in 0..(timeout_us / step_us) {
                let head = queue.cq_head as usize;
                let cq_base = queue.cq_mem.virt as *mut CqEntry;
                let entry = unsafe { &*cq_base.add(head) };
                if entry.phase() == queue.cq_phase {
                    let new_head = (queue.cq_head + 1) % IO_QUEUE_SIZE;
                    queue.cq_head = new_head;
                    if new_head == 0 { queue.cq_phase = !queue.cq_phase; }
                    unsafe { ffi::write32_raw(mmio_base + cq_db, new_head as u32); }
                    return entry.is_success();
                }
                ffi::udelay(step_us);
            }

            // Timeout — try controller reset and retry (only once)
            ffi::print(b"NVMe: flush timeout, resetting controller\0");
            if attempt == 0 && self.reset_count < MAX_RESETS {
                if self.controller_reset() {
                    ffi::udelay(RESET_RETRY_DELAY_US);
                    continue;
                }
            }
            break;
        }
        ffi::print(b"NVMe: flush timeout (recovery failed)\0");
        false
    }

    // ========================================================================
    // Controller Reset & Recovery
    // ========================================================================

    /// Perform a full NVMe controller reset.
    ///
    /// Sequence (NVMe Base Spec rev 1.4, section 3.3):
    /// 1. Disable controller (CC.EN = 0)
    /// 2. Wait for CSTS.RDY = 0
    /// 3. Re-program admin queue registers (AQA, ASQ, ACQ)
    /// 4. Enable controller (CC.EN = 1)
    /// 5. Wait for CSTS.RDY = 1
    /// 6. Re-identify controller
    /// 7. Re-create I/O queues
    /// 8. Re-enable APST
    ///
    /// Returns true if reset succeeded, false if controller is dead.
    pub fn controller_reset(&mut self) -> bool {
        // Prevent recursive reset
        if self.in_reset {
            return false;
        }
        self.in_reset = true;

        if self.verbose >= 1 {
            ffi::print(b"NVMe: controller reset started\0");
        }

        // --- Step 1-2: Disable controller ---
        // Save CC value, clear ENABLE bit
        let cc_val = self.r32(regs::CC);
        self.w32(regs::CC, cc_val & !regs::cc::ENABLE);

        if !self.wait_ready(false) {
            ffi::print(b"NVMe: reset failed - controller stuck (disable timeout)\0");
            self.in_reset = false;
            return false;
        }
        ffi::udelay(10_000); // 10ms quiesce after disable

        // Reset admin queue state
        self.admin_sq_tail = 0;
        self.admin_cq_head = 0;
        self.admin_cq_phase = true;

        // Reset I/O queue state and free old DMA memory
        for q in self.io_queues.iter_mut() {
            if q.active {
                Self::free_dma(&q.sq_mem);
                Self::free_dma(&q.cq_mem);
                q.active = false;
            }
            q.sq_tail = 0;
            q.cq_head = 0;
            q.cq_phase = true;
            q.sq_head = 0;
            q.pending = 0;
        }
        self.nr_io_queues = 0;

        // --- Step 3: Re-program admin queue registers ---
        let aqa = ((ADMIN_QUEUE_SIZE as u32) << 16) | ADMIN_QUEUE_SIZE as u32;
        self.w32(regs::AQA, aqa);
        self.w64(regs::ASQ, self.admin_sq.phys);
        self.w64(regs::ACQ, self.admin_cq.phys);

        // --- Step 4: Re-enable controller ---
        let cc = regs::cc::ENABLE
            | (regs::cc::CSS_NVME << regs::cc::CSS_SHIFT)
            | (6 << regs::cc::IOSQES_SHIFT)
            | (4 << regs::cc::IOCQES_SHIFT);
        self.w32(regs::CC, cc);

        if !self.wait_ready(true) {
            ffi::print(b"NVMe: reset failed - controller stuck (enable timeout)\0");
            self.in_reset = false;
            return false;
        }

        // --- Step 5: Re-identify controller ---
        let mut id_ctrl = IdentifyController {
            vid: 0, ssvid: 0, sn: [0; 20], mn: [0; 40], fr: [0; 8],
            rab: 0, ieee: [0; 3], cmic: 0, mdts: 0, cntlid: 0, ver: 0,
            rtd3r: 0, rtd3e: 0, oaes: 0, ctratt: 0,
            reserved_0: [0; 156], oacs: 0, acl: 0, aerl: 0, frmw: 0,
            lpa: 0, elpe: 0, npss: 0, avscc: 0, apsta: 0, wctemp: 0,
            cctemp: 0, mtfa: 0, hmpre: 0, hmmin: 0, tnvmcap: 0, unvmcap: 0,
            rpmbs: 0,
            reserved_1: [0; 316], sqes: 0, cqes: 0,
            reserved_2: [0; 28], subnqn: [0; 256],
            reserved_3: [0; 768], ioccss: [0; 4],
            reserved_4: [0; 128], subsys_rep: [0; 2560],
        };
        if !self.admin_identify(cns::IDENTIFY_CONTROLLER, 0, &mut id_ctrl) {
            ffi::print(b"NVMe: reset failed - Identify Controller after recovery\0");
            self.in_reset = false;
            return false;
        }
        self.mdts = id_ctrl.mdts;

        // Re-detect namespaces
        self.nr_ns = 0;
        let mut ns_list: [u32; 1024] = [0; 1024];
        if self.admin_identify(cns::IDENTIFY_ACTIVE_NS_LIST, 0, &mut ns_list) {
            for &nsid in ns_list.iter() {
                if nsid == 0 { break; }
                self.nr_ns += 1;
                if (nsid as usize) <= MAX_NAMESPACES {
                    self.identify_namespace(nsid);
                }
            }
        } else {
            self.nr_ns = 1;
            self.identify_namespace(1);
        }

        if self.nr_ns == 0 {
            ffi::print(b"NVMe: reset failed - no namespaces found after recovery\0");
            self.in_reset = false;
            return false;
        }

        // --- Step 6: Re-create I/O queues ---
        let mqes = (self.r64(regs::CAP) & 0xFFFF) as u16;
        let nr_io = core::cmp::min(MAX_IO_QUEUES as u16, mqes);
        if !self.create_io_queues(nr_io) && self.verbose >= 1 {
            ffi::print(b"NVMe: I/O queue recreation failed after reset\0");
        }

        // --- Step 7: Re-enable APST ---
        self.setup_apst(&id_ctrl);

        self.reset_count += 1;
        self.in_reset = false;

        if self.verbose >= 1 {
            ffi::print(b"NVMe: controller reset complete\0");
        }
        true
    }

    /// Check if the controller is in a fatal error state (CSTS.CFS = 1).
    pub fn is_fatal_error(&self) -> bool {
        let csts = self.r32(regs::CSTS);
        (csts & regs::csts::CFS) != 0
    }

    /// Perform controlled shutdown (CC.SHN = Normal) before reset.
    pub fn shutdown_notify(&mut self) -> bool {
        let cc = self.r32(regs::CC);
        let shn = (regs::cc::SHN_NORMAL) << regs::cc::SHN_SHIFT;
        self.w32(regs::CC, (cc & !(regs::cc::SHN_MASK << regs::cc::SHN_SHIFT)) | shn);

        // Wait up to 1 second for shutdown to complete
        for _ in 0..10_000 {
            let csts = self.r32(regs::CSTS);
            if ((csts >> 2) & 0x3) == regs::csts::SHST_COMPLETE {
                return true;
            }
            ffi::udelay(100);
        }
        // If shutdown times out, proceed with reset anyway
        if self.verbose >= 1 {
            ffi::print(b"NVMe: shutdown notification timeout\0");
        }
        false
    }

    /// Process completions on all active I/O queues (called from interrupt handler).
    /// Returns total completions consumed.
    pub fn process_all_queues(&mut self) -> u32 {
        let mmio_base = self.mmio.base() as usize;
        let mut total = 0u32;

        // Pre-calculate doorbell offsets for all queues
        let mut cq_dbs: [usize; MAX_IO_QUEUES as usize] = [0; MAX_IO_QUEUES as usize];
        for i in 0..self.nr_io_queues as usize {
            if self.io_queues[i].active {
                cq_dbs[i] = self.cq_doorbell(self.io_queues[i].qid);
            }
        }

        // Now process completions with mutable borrow of io_queues
        for i in 0..self.nr_io_queues as usize {
            let q = &mut self.io_queues[i];
            if !q.active { continue; }
            let n = q.process_completions();
            if n > 0 {
                unsafe { ffi::write32_raw(mmio_base + cq_dbs[i], q.cq_head as u32); }
                total += n as u32;
            }
        }
        total
    }

    /// Clean up resources and transition to D3hot (low-power).
    pub fn stop(&mut self) {
        // Disable NVMe controller
        self.w32(regs::CC, 0);
        let _ = self.wait_ready(false);

        Self::free_dma(&self.admin_sq);
        Self::free_dma(&self.admin_cq);
        self.prp_list.free();

        for q in self.io_queues.iter_mut() {
            if q.active {
                Self::free_dma(&q.sq_mem);
                Self::free_dma(&q.cq_mem);
                if q.hook_id != 0 {
                    let _ = ffi::irq_remove(&mut q.hook_id);
                }
                if q.irq != 0 {
                    let _ = ffi::msix_free_irq(q.irq);
                }
                q.active = false;
            }
        }

        // Remove admin IRQ handler (skip if it's one of the I/O queues)
        if self.hook_id != 0 {
            // Check if any I/O queue already cleaned up this hook_id
            let already_cleaned = self.io_queues.iter()
                .any(|q| q.hook_id == self.hook_id || q.hook_id == 0);
            if !already_cleaned {
                let _ = ffi::irq_remove(&mut self.hook_id);
            }
        }
        if self.msix_available && self.irq != 0 {
            let _ = ffi::msix_free_irq(self.irq);
        }

        // Transition to D3hot (warm sleep) to save power
        if self.pm_cap_ptr != 0 && self.current_d_state != Self::D3HOT {
            if self.verbose >= 1 {
                ffi::print(b"NVMe: entering D3hot low-power state\0");
            }
            self.set_d_state(Self::D3HOT);
        }

        let _ = ffi::vm_unmap_phys_ffi(
            self.mmio.base() as *mut core::ffi::c_void,
            self.mmio_size,
        );
    }
}

/// Check if a PCI vendor ID corresponds to a known NVMe controller vendor.
fn is_known_nvme(vid: u16) -> bool {
    matches!(vid,
        0x8086 | 0x144d | 0x1344 | 0x15b7 | 0x1c5c | 0x1179 | 0x10ec | 0x1987 | 0x1dbe
    )
}

/// Part_geom struct for MINIX partition geometry
#[repr(C)]
pub struct PartGeom {
    pub base: u64,
    pub size: u64,
    pub cylinders: u32,
    pub heads: u32,
    pub sectors: u32,
}
