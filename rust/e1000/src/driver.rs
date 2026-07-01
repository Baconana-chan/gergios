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

pub struct E1000 {
    /// IRQ line number
    pub irq: c_int,
    /// IRQ hook (also stores hook ID)
    pub irq_hook: c_int,
    /// MMIO register base
    pub regs: *mut u8,
    /// Flash memory mapping (may be null)
    pub flash: *mut u8,
    /// Flash base address (sector-aligned)
    pub flash_base: u32,
    /// EEPROM configuration
    pub eeprom: EepromConfig,

    // Receive
    /// Receive descriptor ring
    pub rx_desc: *mut desc::RxDesc,
    pub rx_desc_count: usize,
    pub rx_buffer: *mut u8,
    pub rx_buffer_size: usize,

    // Transmit
    /// Transmit descriptor ring
    pub tx_desc: *mut desc::TxDesc,
    pub tx_desc_count: usize,
    pub tx_buffer: *mut u8,
    pub tx_buffer_size: usize,
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
            rx_desc: core::ptr::null_mut(),
            rx_desc_count: 0,
            rx_buffer: core::ptr::null_mut(),
            rx_buffer_size: 0,
            tx_desc: core::ptr::null_mut(),
            tx_desc_count: 0,
            tx_buffer: core::ptr::null_mut(),
            tx_buffer_size: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.regs.is_null()
    }

    /// Free all allocated resources.
    pub fn cleanup(&mut self) {
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
    /// Probe PCI bus for an e1000 controller.
    /// Returns true if found.
    pub fn probe(&mut self, skip: c_int) -> bool {
        ffi::pci_init_ffi();

        let (devind, vid, did) = match ffi::pci_first_dev_ffi() {
            Some(d) => d,
            None => return false,
        };

        let mut remaining = skip;
        let mut current_devind = devind;
        let mut current_vid = vid;
        let mut current_did = did;

        while remaining > 0 {
            remaining -= 1;
            match ffi::pci_next_dev_ffi() {
                Some(d) => { current_devind = d.0; current_vid = d.1; current_did = d.2; }
                None => return false,
            }
        }

        if !pci_ids::is_e1000(current_vid, current_did) {
            return false;
        }

        self.eeprom = pci_ids::eeprom_config(current_did);
        ffi::pci_reserve_ffi(current_devind);
        self.irq = ffi::pci_attr_r8_ffi(current_devind, 0x3F) as c_int;

        let (base, size, ioflag) = match ffi::pci_get_bar_ffi(current_devind, 0) {
            Some(b) => b,
            None => return false,
        };

        if ioflag {
            return false;
        }

        // Enable bus mastering
        let cr = ffi::pci_attr_r16_ffi(current_devind, 0x04);
        if (cr & 0x0004) == 0 {
            ffi::pci_attr_w16_ffi(current_devind, 0x04, cr | 0x0004);
        }

        self.regs = ffi::vm_map_phys_ffi(base, size);
        if self.regs.is_null() {
            return false;
        }

        self.map_flash(current_devind, current_did);
        true
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
    pub fn init_hw(&mut self) {
        self.irq_hook = self.irq;
        unsafe {
            let r = ffi::platform::sys_irqsetpolicy(self.irq, 0, &mut self.irq_hook);
            if r != 0 { panic!("sys_irqsetpolicy failed"); }
            let r = ffi::platform::sys_irqenable(&mut self.irq_hook);
            if r != 0 { panic!("sys_irqenable failed"); }
        }

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
    }

    pub fn init_buffers(&mut self) {
        self.rx_desc_count = reg::RXDESC_NR;
        self.tx_desc_count = reg::TXDESC_NR;

        // Allocate and set up receive descriptors
        let rx_desc_size = core::mem::size_of::<desc::RxDesc>() * self.rx_desc_count;
        let (rx_desc_ptr, rx_desc_phys) = ffi::alloc_contig_ffi(rx_desc_size)
            .expect("failed to alloc RX descriptors");
        self.rx_desc = rx_desc_ptr as *mut desc::RxDesc;
        unsafe { core::ptr::write_bytes(self.rx_desc, 0, self.rx_desc_count); }

        self.rx_buffer_size = reg::RXDESC_NR * reg::IOBUF_SIZE;
        let (rx_buf_ptr, rx_buf_phys) = ffi::alloc_contig_ffi(self.rx_buffer_size)
            .expect("failed to alloc RX buffers");
        self.rx_buffer = rx_buf_ptr as *mut u8;

        for i in 0..reg::RXDESC_NR {
            unsafe {
                let idx: usize = i;
                (*self.rx_desc.add(idx)).buffer = (rx_buf_phys + (idx * reg::IOBUF_SIZE) as u64) as u32;
            }
        }

        // Allocate and set up transmit descriptors
        let tx_desc_size = core::mem::size_of::<desc::TxDesc>() * self.tx_desc_count;
        let (tx_desc_ptr, tx_desc_phys) = ffi::alloc_contig_ffi(tx_desc_size)
            .expect("failed to alloc TX descriptors");
        self.tx_desc = tx_desc_ptr as *mut desc::TxDesc;
        unsafe { core::ptr::write_bytes(self.tx_desc, 0, self.tx_desc_count); }

        self.tx_buffer_size = reg::TXDESC_NR * reg::IOBUF_SIZE;
        let (tx_buf_ptr, tx_buf_phys) = ffi::alloc_contig_ffi(self.tx_buffer_size)
            .expect("failed to alloc TX buffers");
        self.tx_buffer = tx_buf_ptr as *mut u8;

        for i in 0..reg::TXDESC_NR {
            unsafe {
                let idx: usize = i;
                (*self.tx_desc.add(idx)).buffer = (tx_buf_phys + (idx * reg::IOBUF_SIZE) as u64) as u32;
            }
        }

        // Program RX ring registers
        write_reg(self.regs, reg::RDBAL, rx_desc_phys as u32);
        write_reg(self.regs, reg::RDBAH, 0);
        write_reg(self.regs, reg::RDLEN, (self.rx_desc_count * core::mem::size_of::<desc::RxDesc>()) as u32);
        write_reg(self.regs, reg::RDH, 0);
        write_reg(self.regs, reg::RDT, (self.rx_desc_count - 1) as u32);
        clear_reg(self.regs, reg::RCTL, reg::RCTL_BSIZE);
        set_reg(self.regs, reg::RCTL, reg::RCTL_EN);

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
    pub fn send(&self, data: *mut ffi::NetdriverData, size: usize) -> c_int {
        if size > reg::IOBUF_SIZE {
            return ffi::EINVAL;
        }

        // Check if TX queue has room
        let head = read_reg(self.regs, reg::TDH);
        let tail = read_reg(self.regs, reg::TDT);
        let next = (tail + 1) % self.tx_desc_count as u32;

        if next == head {
            return ffi::SUSPEND; // queue full
        }

        // Copy packet data into the TX buffer via netdriver_copyin
        let buf_ptr = unsafe { self.tx_buffer.add(tail as usize * reg::IOBUF_SIZE) };
        ffi::netdriver_copyin_ffi(data, 0, buf_ptr as *const core::ffi::c_void, size);

        // Mark descriptor ready
        unsafe {
            let desc = &mut *self.tx_desc.add(tail as usize);
            desc.status = 0;
            desc.length = size as u16;
            desc.cmd = desc::TX_CMD_EOP | desc::TX_CMD_FCS | desc::TX_CMD_RS;
        }

        // Advance tail to start transmission
        write_reg(self.regs, reg::TDT, next);

        ffi::OK
    }

    /// Receive a packet. Returns size or SUSPEND if none available.
    pub fn recv(&self, data: *mut ffi::NetdriverData, max: usize) -> isize {
        let head = read_reg(self.regs, reg::RDH);
        let tail = read_reg(self.regs, reg::RDT);

        if head == tail {
            return ffi::SUSPEND as isize; // queue empty
        }

        let cur = (tail + 1) % self.rx_desc_count as u32;
        let desc = unsafe { &*self.rx_desc.add(cur as usize) };

        if (desc.status & desc::RX_STATUS_DONE) == 0 {
            return ffi::SUSPEND as isize;
        }

        if (desc.status & desc::RX_STATUS_EOP) == 0 {
            panic!("e1000: received packet spans multiple descriptors");
        }

        let mut size = desc.length as usize;
        if size > max {
            size = max;
        }

        // Copy packet data to caller via netdriver_copyout
        let buf_ptr = unsafe { self.rx_buffer.add(cur as usize * reg::IOBUF_SIZE) };
        ffi::netdriver_copyout_ffi(data, 0, buf_ptr as *const core::ffi::c_void, size);

        // Reset descriptor
        unsafe {
            let desc_mut = &mut *self.rx_desc.add(cur as usize);
            desc_mut.status = 0;
        }

        // Advance tail
        write_reg(self.regs, reg::RDT, cur);

        size as isize
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
