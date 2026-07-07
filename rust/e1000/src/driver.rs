//! # E1000 Driver — State Management & Core Operations
//!
//! Implements the core driver logic: hardware init, buffer setup,
//! packet send/receive, interrupt handling, link management.

use core::ffi::c_int;

use crate::desc;
use crate::eeprom::{self, read_reg, write_reg, set_reg, clear_reg};
use crate::ffi;
use crate::pci_ids::{self, EepromConfig, EepromType};
use crate::reg;

// ============================================================================
// Driver State
// ============================================================================

/// MSI-X vector indices for e1000
pub const MSIX_VEC_RX: usize = 0;    // Receive vector
pub const MSIX_VEC_TX: usize = 1;    // Transmit vector
pub const MSIX_VEC_OTHER: usize = 2; // Link/error vector
pub const MSIX_VEC_COUNT: usize = 3; // Total MSI-X vectors

pub const MSIX_ENTRY_SIZE: usize = 16;
pub const MSIX_MESSAGE_ADDR: u32 = 0xFEE00000;
pub const MSIX_VECTOR_BASE: u32 = 0x50;

pub struct E1000 {
    /// IRQ line number
    pub irq: c_int,
    /// Legacy IRQ hook ID
    pub irq_hook: c_int,
    /// MMIO register base
    pub regs: *mut u8,
    /// Flash memory mapping (may be null)
    pub flash: *mut u8,
    /// Flash base address (sector-aligned)
    pub flash_base: u32,
    /// EEPROM configuration
    pub eeprom: EepromConfig,
    /// Per-descriptor buffer size (configurable via e1000_bufsize env var)
    pub buf_size: usize,

    // Receive
    pub rx_desc: *mut desc::RxDesc,
    pub rx_desc_count: usize,
    pub rx_buffer: *mut u8,
    pub rx_buffer_size: usize,

    // Transmit
    pub tx_desc: *mut desc::TxDesc,
    pub tx_desc_count: usize,
    pub tx_buffer: *mut u8,
    pub tx_buffer_size: usize,

    // MSI-X fields
    pub msix_available: bool,
    pub msix_bar_base: *mut u8,    // Original mapped BAR start
    pub msix_table_ptr: *mut u8,   // Pointer to MSI-X table within BAR
    pub msix_bar_size: usize,       // Size of mapped BAR region
    pub msix_irqs: [c_int; MSIX_VEC_COUNT],      // Per-vector IRQs
    pub msix_hook_ids: [c_int; MSIX_VEC_COUNT],  // Per-vector hook IDs
}

// ============================================================================
// Construction
// ============================================================================

impl E1000 {
    pub fn new() -> Self {
        E1000 {
            irq: 0,
            irq_hook: 0,
            regs: core::ptr::null_mut(),
            flash: core::ptr::null_mut(),
            flash_base: 0,
            eeprom: EepromConfig {
                eeprom_type: EepromType::Eerd,
                done_bit: 1 << 1,
                addr_off: 2,
            },
            buf_size: reg::IOBUF_SIZE,
            rx_desc: core::ptr::null_mut(),
            rx_desc_count: 0,
            rx_buffer: core::ptr::null_mut(),
            rx_buffer_size: 0,
            tx_desc: core::ptr::null_mut(),
            tx_desc_count: 0,
            tx_buffer: core::ptr::null_mut(),
            tx_buffer_size: 0,
            msix_available: false,
            msix_bar_base: core::ptr::null_mut(),
            msix_table_ptr: core::ptr::null_mut(),
            msix_bar_size: 0,
            msix_irqs: [0; MSIX_VEC_COUNT],
            msix_hook_ids: [0; MSIX_VEC_COUNT],
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.regs.is_null()
    }

    /// Free all allocated resources.
    fn cleanup_msix(&mut self) {
        if !self.msix_available { return; }
        for vec_idx in 0..MSIX_VEC_COUNT {
            if self.msix_hook_ids[vec_idx] != 0 {
                let _ = ffi::irq_remove_ffi(&mut self.msix_hook_ids[vec_idx]);
                self.msix_hook_ids[vec_idx] = 0;
            }
            if self.msix_irqs[vec_idx] != 0 {
                let _ = ffi::msix_free_irq(self.msix_irqs[vec_idx]);
                self.msix_irqs[vec_idx] = 0;
            }
        }
        if !self.msix_bar_base.is_null() {
            let _ = ffi::vm_unmap_phys_ffi(
                self.msix_bar_base as *mut core::ffi::c_void,
                self.msix_bar_size,
            );
        }
        self.msix_available = false;
        self.msix_bar_base = core::ptr::null_mut();
        self.msix_table_ptr = core::ptr::null_mut();
    }

    fn setup_msix(&mut self, devind: c_int) -> bool {
        let msix_info = match ffi::pci_msix_parse_ffi(devind) {
            Some(info) => info,
            None => return false,
        };

        let table_offset = (msix_info.msix_table_offset as usize) * 8;
        let table_size = msix_info.msix_table_size as usize;
        let table_bytes = table_size * MSIX_ENTRY_SIZE;

        // Map MSI-X table BAR
        let bar_idx = msix_info.msix_table_bir;
        let (bar_base, bar_size, ioflag) = match ffi::pci_get_bar_ffi(devind, bar_idx) {
            Some(v) => v,
            None => return false,
        };
        if ioflag { return false; }

        let map_size = core::cmp::min(bar_size as usize, table_offset + table_bytes);
        let bar_ptr = ffi::vm_map_phys_ffi(bar_base, map_size as u32);
        if bar_ptr.is_null() { return false; }

        let table_base = unsafe { (bar_ptr as *mut u8).add(table_offset) };
        self.msix_bar_base = bar_ptr as *mut u8;
        self.msix_table_ptr = table_base;
        self.msix_bar_size = map_size;

        // Allocate 3 MSI-X vectors: RX, TX, OTHER
        let max_vec = core::cmp::min(MSIX_VEC_COUNT, table_size);
        let mut all_ok = true;

        for vec_idx in 0..max_vec {
            let irq = match ffi::msix_alloc_irq() {
                Some(irq) => irq,
                None => { all_ok = false; break; }
            };
            let hook_id = match ffi::msix_setup(irq) {
                Some(hook) => hook,
                None => {
                    let _ = ffi::msix_free_irq(irq);
                    all_ok = false;
                    break;
                }
            };
            self.msix_irqs[vec_idx] = irq;
            self.msix_hook_ids[vec_idx] = hook_id;

            // Program MSI-X table entry
            unsafe {
                let entry = table_base.add(vec_idx * MSIX_ENTRY_SIZE) as *mut u32;
                core::ptr::write_volatile(entry, MSIX_MESSAGE_ADDR);           // msg addr low
                core::ptr::write_volatile(entry.add(1), 0);                     // msg addr high
                core::ptr::write_volatile(entry.add(2), MSIX_VECTOR_BASE + irq as u32); // msg data
                core::ptr::write_volatile(entry.add(3), 0);                     // vector control
            }
        }

        if !all_ok {
            for vec_idx in 0..max_vec {
                if self.msix_hook_ids[vec_idx] != 0 {
                    let _ = ffi::irq_remove_ffi(&mut self.msix_hook_ids[vec_idx]);
                    self.msix_hook_ids[vec_idx] = 0;
                }
                if self.msix_irqs[vec_idx] != 0 {
                    let _ = ffi::msix_free_irq(self.msix_irqs[vec_idx]);
                    self.msix_irqs[vec_idx] = 0;
                }
            }
            if !bar_ptr.is_null() {
                let _ = ffi::vm_unmap_phys_ffi(bar_ptr as *mut core::ffi::c_void, map_size);
            }
            self.msix_bar_base = core::ptr::null_mut();
            self.msix_table_ptr = core::ptr::null_mut();
            return false;
        }

        self.msix_available = true;
        true
    }

    pub fn cleanup(&mut self) {
        self.cleanup_msix();
        if !self.rx_desc.is_null() {
            let size = core::mem::size_of::<desc::RxDesc>() * self.rx_desc_count;
            ffi::free_contig_ffi(self.rx_desc as *mut core::ffi::c_void, size);
            self.rx_desc = core::ptr::null_mut();
        }
        if !self.rx_buffer.is_null() {
            ffi::free_contig_ffi(self.rx_buffer as *mut core::ffi::c_void, self.rx_buffer_size);
            self.rx_buffer = core::ptr::null_mut();
        }
        if !self.tx_desc.is_null() {
            let size = core::mem::size_of::<desc::TxDesc>() * self.tx_desc_count;
            ffi::free_contig_ffi(self.tx_desc as *mut core::ffi::c_void, size);
            self.tx_desc = core::ptr::null_mut();
        }
        if !self.tx_buffer.is_null() {
            ffi::free_contig_ffi(self.tx_buffer as *mut core::ffi::c_void, self.tx_buffer_size);
            self.tx_buffer = core::ptr::null_mut();
        }
    }
}

// ============================================================================
// Hardware reset
// ============================================================================

impl E1000 {
    /// Reset the hardware.
    pub fn reset_hw(&mut self) {
        set_reg(self.regs, reg::CTRL, reg::CTRL_RST);
        ffi::udelay(16_000);
    }
}

// ============================================================================
// PCI probe
// ============================================================================

impl E1000 {
    /// Probe PCI bus for an e1000 controller (legacy, returns bool).
    /// Returns true if found.
    pub fn probe(&mut self, skip: c_int) -> bool {
        self.probe_v2(skip) >= 0
    }

    /// Probe PCI bus for an e1000 controller. Returns devind on success, -1 on failure.
    pub fn probe_v2(&mut self, skip: c_int) -> c_int {
        ffi::pci_init_ffi();

        let (devind, vid, did) = match ffi::pci_first_dev_ffi() {
            Some(d) => d,
            None => return -1,
        };

        let mut remaining = skip;
        let mut current_devind = devind;
        let mut current_vid = vid;
        let mut current_did = did;

        while remaining > 0 {
            remaining -= 1;
            match ffi::pci_next_dev_ffi() {
                Some(d) => { current_devind = d.0; current_vid = d.1; current_did = d.2; }
                None => return -1,
            }
        }

        if !pci_ids::is_e1000(current_vid, current_did) {
            return -1;
        }

        self.eeprom = pci_ids::eeprom_config(current_did);
        ffi::pci_reserve_ffi(current_devind);
        self.irq = ffi::pci_attr_r8_ffi(current_devind, 0x3F) as c_int;

        let (base, size, ioflag) = match ffi::pci_get_bar_ffi(current_devind, 0) {
            Some(b) => b,
            None => return -1,
        };

        if ioflag {
            return -1;
        }

        // Enable bus mastering
        let cr = ffi::pci_attr_r16_ffi(current_devind, 0x04);
        if (cr & 0x0004) == 0 {
            ffi::pci_attr_w16_ffi(current_devind, 0x04, cr | 0x0004);
        }

        self.regs = ffi::vm_map_phys_ffi(base, size);
        if self.regs.is_null() {
            return -1;
        }

        self.map_flash(current_devind, current_did);
        current_devind
    }

    fn map_flash(&mut self, devind: c_int, did: u16) {
        let flash_addr = ffi::pci_attr_r32_ffi(devind, 0x18);
        if flash_addr == 0 { return; }

        let mut flash_size: u32 = 0x10000;
        if did == pci_ids::DEV_ICH10_D_BM_LM || did == pci_ids::DEV_ICH10_R_BM_LF {
            flash_size = 0x1000;
        }

        match did {
            pci_ids::DEV_82540EM | pci_ids::DEV_82545EM |
            pci_ids::DEV_82540EP | pci_ids::DEV_82540EP_LP => return,
            _ => {}
        }

        self.flash = ffi::vm_map_phys_ffi(flash_addr, flash_size);
        let gfpreg = read_reg(self.flash, reg::ICH_FLASH_GFPREG);
        let sector_base = gfpreg & reg::FLASH_GFPREG_BASE_MASK;
        self.flash_base = sector_base << reg::FLASH_SECTOR_ADDR_SHIFT;
    }
}

// ============================================================================
// Hardware initialization
// ============================================================================

impl E1000 {
    pub fn init_hw(&mut self, devind: c_int) {
        // Hardware reset FIRST — clears all registers to defaults
        self.reset_hw();

        set_reg(self.regs, reg::CTRL, reg::CTRL_ASDE | reg::CTRL_SLU);
        clear_reg(self.regs, reg::CTRL, reg::CTRL_LRST);
        clear_reg(self.regs, reg::CTRL, reg::CTRL_PHY_RST);
        clear_reg(self.regs, reg::CTRL, reg::CTRL_ILOS);

        write_reg(self.regs, reg::FCAL, 0);
        write_reg(self.regs, reg::FCAH, 0);
        write_reg(self.regs, reg::FCT, 0);
        write_reg(self.regs, reg::FCTTV, 0);
        clear_reg(self.regs, reg::CTRL, reg::CTRL_VME);

        for i in 0..128 {
            write_reg(self.regs, reg::MTA + i * 4, 0);
        }
        for i in 0..64 {
            write_reg(self.regs, reg::CRCERRS + i * 4, 0);
        }

        // Try MSI-X setup AFTER hardware init (but before legacy IRQ)
        let msix_ok = self.setup_msix(devind);

        if msix_ok {
            // Configure IVAR: each entry is 16-bit
            // Byte 0: RX queue 0 vector + valid bit 15
            // Byte 2: TX queue 0 vector + valid bit 15
            // At IVAR+8 (separate 32-bit reg): OTHER causes
            let ivar = (MSIX_VEC_RX as u32 | reg::IVAR_VALID)
                | ((MSIX_VEC_TX as u32 | reg::IVAR_VALID) << 16);
            write_reg(self.regs, reg::IVAR, ivar);
            write_reg(self.regs, reg::IVAR + 8, MSIX_VEC_OTHER as u32 | reg::IVAR_VALID);
            // Enable MSI-X auto-clear and mask set
            write_reg(self.regs, reg::EIMS, reg::EICR_RX0 | reg::EICR_TX0 | reg::EICR_OTHER);
            write_reg(self.regs, reg::EIAC, reg::EICR_RX0 | reg::EICR_TX0 | reg::EICR_OTHER);
        } else {
            // Legacy IRQ setup
            self.irq_hook = self.irq;
            unsafe {
                let r = ffi::platform::sys_irqsetpolicy(self.irq, 0, &mut self.irq_hook);
                if r != 0 { panic!("sys_irqsetpolicy failed"); }
                let r = ffi::platform::sys_irqenable(&mut self.irq_hook);
                if r != 0 { panic!("sys_irqenable failed"); }
            }
        }
    }

    /// Map a buffer size (2048/4096/8192/16384) to RCTL BSIZE bits and BSEX flag.
    fn bufsize_to_rctl(bufsize: usize) -> (bool, u32) {
        match bufsize {
            2048  => (false, 0),              // BSEX=0, BSIZE=00
            4096  => (false, 2 << 16),         // BSEX=0, BSIZE=10
            8192  => (false, 3 << 16),         // BSEX=0, BSIZE=11
            _     => (true,  0),               // BSEX=1, BSIZE=00 (= 16384, default)
        }
    }

    pub fn init_buffers(&mut self) {
        self.rx_desc_count = reg::RXDESC_NR;
        self.tx_desc_count = reg::TXDESC_NR;

        // Read optional buffer size override from environment.
        // Usage: boot monitor:  "e1000_bufsize=2048"  saves 6 MB contiguous.
        let env_size = ffi::env_parse_long(
            b"e1000_bufsize\0",
            reg::IOBUF_SIZE as isize,
            2048,
            16384,
        ) as usize;

        self.buf_size = match env_size {
            2048 => 2048,
            4096 => 4096,
            8192 => 8192,
            _ => 16384, // default — also the fallback for unknown values
        };

        // Allocate and set up receive descriptors
        let rx_desc_size = core::mem::size_of::<desc::RxDesc>() * self.rx_desc_count;
        let (rx_desc_ptr, rx_desc_phys) = ffi::alloc_contig_ffi(rx_desc_size)
            .expect("failed to alloc RX descriptors");
        self.rx_desc = rx_desc_ptr as *mut desc::RxDesc;
        unsafe { core::ptr::write_bytes(self.rx_desc, 0, self.rx_desc_count); }

        self.rx_buffer_size = reg::RXDESC_NR * self.buf_size;
        let (rx_buf_ptr, rx_buf_phys) = ffi::alloc_contig_ffi(self.rx_buffer_size)
            .expect("failed to alloc RX buffers");
        self.rx_buffer = rx_buf_ptr as *mut u8;

        for i in 0..reg::RXDESC_NR {
            unsafe {
                let idx: usize = i;
                (*self.rx_desc.add(idx)).buffer = (rx_buf_phys + (idx * self.buf_size) as u64) as u32;
            }
        }

        // Allocate and set up transmit descriptors
        let tx_desc_size = core::mem::size_of::<desc::TxDesc>() * self.tx_desc_count;
        let (tx_desc_ptr, tx_desc_phys) = ffi::alloc_contig_ffi(tx_desc_size)
            .expect("failed to alloc TX descriptors");
        self.tx_desc = tx_desc_ptr as *mut desc::TxDesc;
        unsafe { core::ptr::write_bytes(self.tx_desc, 0, self.tx_desc_count); }

        self.tx_buffer_size = reg::TXDESC_NR * self.buf_size;
        let (tx_buf_ptr, tx_buf_phys) = ffi::alloc_contig_ffi(self.tx_buffer_size)
            .expect("failed to alloc TX buffers");
        self.tx_buffer = tx_buf_ptr as *mut u8;

        for i in 0..reg::TXDESC_NR {
            unsafe {
                let idx: usize = i;
                (*self.tx_desc.add(idx)).buffer = (tx_buf_phys + (idx * self.buf_size) as u64) as u32;
            }
        }

        // Program RX ring registers
        write_reg(self.regs, reg::RDBAL, rx_desc_phys as u32);
        write_reg(self.regs, reg::RDBAH, 0);
        write_reg(self.regs, reg::RDLEN, (self.rx_desc_count * core::mem::size_of::<desc::RxDesc>()) as u32);
        write_reg(self.regs, reg::RDH, 0);
        write_reg(self.regs, reg::RDT, (self.rx_desc_count - 1) as u32);

        // Configure RCTL buffer size to match self.buf_size.
        // Clear BSIZE/BSEX, then set the correct combination + EN in one write.
        let (bsex, bsize_bits) = Self::bufsize_to_rctl(self.buf_size);
        clear_reg(self.regs, reg::RCTL, reg::RCTL_BSIZE | reg::RCTL_BSEX);
        if bsex {
            set_reg(self.regs, reg::RCTL, reg::RCTL_BSEX | bsize_bits | reg::RCTL_EN);
        } else {
            set_reg(self.regs, reg::RCTL, bsize_bits | reg::RCTL_EN);
        }

        // Program TX ring registers
        write_reg(self.regs, reg::TDBAL, tx_desc_phys as u32);
        write_reg(self.regs, reg::TDBAH, 0);
        write_reg(self.regs, reg::TDLEN, (self.tx_desc_count * core::mem::size_of::<desc::TxDesc>()) as u32);
        write_reg(self.regs, reg::TDH, 0);
        write_reg(self.regs, reg::TDT, 0);
        set_reg(self.regs, reg::TCTL, reg::TCTL_EN | reg::TCTL_PSP);
    }

    pub fn enable_intr(&mut self) {
        set_reg(self.regs, reg::IMS,
            reg::ICR_LSC | reg::ICR_RXO | reg::ICR_RXT |
            reg::ICR_TXQE | reg::ICR_TXDW);
    }
}

// ============================================================================
// MAC address
// ============================================================================

impl E1000 {
    pub fn read_mac(&self, addr: &mut ffi::NetdriverAddr) {
        for i in 0..3usize {
            let word = eeprom::eeprom_read(
                self.eeprom.eeprom_type,
                self.regs,
                self.flash,
                self.flash_base,
                i as u32,
                self.eeprom.done_bit,
                self.eeprom.addr_off,
            );
            addr[i * 2] = (word & 0xff) as u8;
            addr[i * 2 + 1] = ((word >> 8) & 0xff) as u8;
        }
    }

    pub fn set_hwaddr(&self, addr: &ffi::NetdriverAddr) {
        let low = u32::from_le_bytes([addr[0], addr[1], addr[2], addr[3]]);
        let high = u16::from_le_bytes([addr[4], addr[5]]);
        write_reg(self.regs, reg::RAL, low);
        write_reg(self.regs, reg::RAH, high as u32 | reg::RAH_AV);
    }
}

// ============================================================================
// Packet send/receive
// ============================================================================

impl E1000 {
    /// Send a packet. Returns OK or SUSPEND if queue is full.
    ///
    /// Supports two modes:
    /// - **Normal path**: single legacy descriptor with optional IPv4 checksum offload.
    /// - **TSO path** (Legacy TSE): single legacy descriptor with TSE bit set for
    ///   hardware TCP segmentation.  The 82540EM/82545EM hardware automatically
    ///   segments the super-segment into MSS-sized chunks, updating IP headers,
    ///   TCP sequence numbers, and checksums — all without software intervention.
    ///
    /// TSO is triggered when:
    ///   - Packet size >= E1000_TSO_MIN_SIZE (ETH_HDR + 20 IP + 20 TCP + 1460 MSS)
    ///   - Ethertype == 0x0800 (IPv4)
    ///   - IP protocol == 0x06 (TCP)
    pub fn send(&mut self, data: *mut ffi::NetdriverData, size: usize) -> c_int {
        if size > self.buf_size {
            return ffi::EINVAL;
        }

        // Check if TX queue has room
        let head = read_reg(self.regs, reg::TDH);
        let tail = read_reg(self.regs, reg::TDT);
        let next = (tail + 1) % self.tx_desc_count as u32;

        if next == head {
            return ffi::SUSPEND; // queue full
        }

        // Copy packet data into the TX buffer FIRST so we can inspect headers
        let buf_ptr = unsafe { self.tx_buffer.add(tail as usize * self.buf_size) };
        ffi::netdriver_copyin_ffi(data, 0, buf_ptr as *const core::ffi::c_void, size);

        #[allow(clippy::identity_op)]
        const ETH_HDR_LEN: usize = 14;

        // Determine if this is a TSO candidate: TCP over IPv4, large enough.
        // Check: size >= ETH(14) + IP(20) + TCP(20) + MSS(1460) = 1514
        let is_tcp_ipv4 = size >= (ETH_HDR_LEN + 20 + 20 + 1460)
            && unsafe { *buf_ptr.add(12) } == 0x08
            && unsafe { *buf_ptr.add(13) } == 0x00
            && unsafe { *buf_ptr.add(ETH_HDR_LEN + 9) } == 0x06;

        unsafe {
            let desc = &mut *self.tx_desc.add(tail as usize);
            desc.status = 0;
            desc.length = size as u16;
            desc.special = 0;

            if is_tcp_ipv4 {
                // ============================================================
                // TSO path: single legacy descriptor with TSE bit
                // ============================================================
                // Parse IP header length (IHL, lower nibble of byte at ETH+14)
                // ip_hlen_raw is u8, cast to usize for arithmetic
                let ip_hlen_raw = (unsafe { *buf_ptr.add(ETH_HDR_LEN) } & 0x0F) * 4;
                let ip_hlen = if ip_hlen_raw < 20 { 20usize } else { ip_hlen_raw as usize };

                desc.cmd = desc::TX_CMD_EOP | desc::TX_CMD_FCS | desc::TX_CMD_RS
                    | desc::TX_CMD_IC | desc::TX_CMD_TSE;
                desc.css = (ETH_HDR_LEN + ip_hlen) as u8;           // CSS: TCP header start
                desc.cso = (ETH_HDR_LEN + ip_hlen + 16) as u8;      // CSO: TCP checksum field
                desc.special = 1460;                                  // MSS for segmentation
            } else {
                // ============================================================
                // Normal path: single legacy descriptor with optional IPv4
                // checksum offload
                // ============================================================
                desc.cmd = desc::TX_CMD_EOP | desc::TX_CMD_FCS | desc::TX_CMD_RS;

                // Enable IPv4 header checksum offload for IPv4 packets
                if size >= (ETH_HDR_LEN + 2)
                    && unsafe { *buf_ptr.add(12) } == 0x08
                    && unsafe { *buf_ptr.add(13) } == 0x00
                {
                    desc.cmd |= desc::TX_CMD_IC;
                    desc.css = ETH_HDR_LEN as u8;       // CSS: start of IP header
                    desc.cso = (ETH_HDR_LEN + 10) as u8; // CSO: IP checksum field
                }
            }
        }

        // Advance tail to start transmission
        write_reg(self.regs, reg::TDT, next);

        ffi::OK
    }

    /// Receive a packet. Returns size or SUSPEND if none available.
    ///
    /// Handles multi-buffer packets using a two-phase approach:
    ///   1. **Peek** — read-only scan to verify all fragments down to EOP
    ///      are DONE and error-free.
    ///   2. **Consume** — copy data and reset descriptors only after
    ///      confirming the full packet is ready.
    ///
    /// This avoids the hazard of returning some descriptors to the hardware
    /// pool while later fragments are still being DMA'd (which could cause
    /// stale/garbled data to be received on the next call).
    pub fn recv(&mut self, data: *mut ffi::NetdriverData, max: usize) -> isize {
        let head = read_reg(self.regs, reg::RDH);
        let tail = read_reg(self.regs, reg::RDT);

        if head == tail {
            return ffi::SUSPEND as isize; // queue empty
        }

        // ====================================================================
        // Phase 1 — Peek: verify the entire packet is ready.
        // ====================================================================

        // Pre-count fragments and total size; detect errors.
        let mut peek = (tail + 1) % self.rx_desc_count as u32;
        let mut frags = 0u32;
        let mut total_len = 0usize;

        loop {
            let status = unsafe { (*self.rx_desc.add(peek as usize)).status };
            let length  = unsafe { (*self.rx_desc.add(peek as usize)).length } as usize;
            let errors  = unsafe { (*self.rx_desc.add(peek as usize)).errors };

            if (status & desc::RX_STATUS_DONE) == 0 || errors != 0 {
                if errors != 0 {
                    ffi::netdriver_stat_ierror_ffi(errors as u32);
                }
                return ffi::SUSPEND as isize; // not ready or corrupt — try later
            }

            total_len += length;
            frags += 1;

            if (status & desc::RX_STATUS_EOP) != 0 {
                break; // full packet found
            }

            peek = (peek + 1) % self.rx_desc_count as u32;
            if peek == head {
                return ffi::SUSPEND as isize; // ring wrapped without EOP — should not happen
            }
        }

        // ====================================================================
        // Phase 2 — Consume: copy data and return descriptors to HW.
        // ====================================================================

        let total = if total_len > max { max } else { total_len };
        let mut offset = 0usize;
        let mut cur = (tail + 1) % self.rx_desc_count as u32;

        for _ in 0..frags {
            let length = unsafe { (*self.rx_desc.add(cur as usize)).length } as usize;

            // How much to copy from this fragment (respecting caller's max).
            let copy_size = if offset + length > total {
                total.saturating_sub(offset)
            } else {
                length
            };

            if copy_size > 0 {
                let buf_ptr = unsafe { self.rx_buffer.add(cur as usize * self.buf_size) };
                ffi::netdriver_copyout_ffi(data, offset,
                    buf_ptr as *const core::ffi::c_void, copy_size);
                offset += copy_size;
            }

            // Return this descriptor to the hardware.
            unsafe { (*self.rx_desc.add(cur as usize)).status = 0; }

            cur = (cur + 1) % self.rx_desc_count as u32;
        }

        // Tell the hardware how many descriptors we consumed.
        // `last_consumed = (tail + frags) % count` — the final descriptor
        // we processed in the chain.
        let last_consumed = ((tail as u32 + frags) % self.rx_desc_count as u32);
        write_reg(self.regs, reg::RDT, last_consumed);

        if offset == 0 {
            ffi::SUSPEND as isize
        } else {
            offset as isize
        }
    }
}

// ============================================================================
// Interrupt handling
// ============================================================================

impl E1000 {
    /// Handle an interrupt. Returns event flags:
    /// bit 0 = link change, bit 1 = recv, bit 2 = send
    pub fn handle_intr(&self) -> u32 {
        let cause = read_reg(self.regs, reg::ICR);
        if cause == 0 { return 0; }

        let mut events = 0u32;
        if (cause & reg::ICR_LSC) != 0 { events |= 1; }
        if (cause & (reg::ICR_RXO | reg::ICR_RXT)) != 0 { events |= 2; }
        if (cause & (reg::ICR_TXQE | reg::ICR_TXDW)) != 0 { events |= 4; }
        events
    }
}

// ============================================================================
// Link status
// ============================================================================

impl E1000 {
    pub fn get_link(&self) -> (u32, u32) {
        let status = read_reg(self.regs, reg::STATUS);
        if (status & reg::STATUS_LU) == 0 {
            return (ffi::NDEV_LINK_DOWN, 0);
        }

        let mut media = ffi::IFM_ETHER;
        if (status & reg::STATUS_FD) != 0 {
            media |= ffi::IFM_FDX;
        } else {
            media |= ffi::IFM_HDX;
        }

        match status & reg::STATUS_SPEED {
            reg::STATUS_SPEED_10 => media |= ffi::IFM_10_T,
            reg::STATUS_SPEED_100 => media |= ffi::IFM_100_TX,
            _ => media |= ffi::IFM_1000_T,
        }

        (ffi::NDEV_LINK_UP, media)
    }
}

// ============================================================================
// Statistics
// ============================================================================

impl E1000 {
    pub fn update_stats(&self) {
        let rxerr = read_reg(self.regs, reg::RXERRC);
        let crcerr = read_reg(self.regs, reg::CRCERRS);
        let mpc = read_reg(self.regs, reg::MPC);
        let colc = read_reg(self.regs, reg::COLC);

        ffi::netdriver_stat_ierror_ffi(rxerr.wrapping_add(crcerr).wrapping_add(mpc));
        ffi::netdriver_stat_coll_ffi(colc);
    }
}
