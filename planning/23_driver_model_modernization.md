# Driver Model Modernization — GergiOS 1.0+/1.1

> **Статус**: Phase 1 ✅, Phase 2 ✅, Phase 3 ✅, Phase 4 🆕, Phase 5–6 🆕
> **Связанные**: `planning/03_migration_roadmap.md` §5, `planning/09_c_language_modernization.md` §Phase 5 (minix-driver), `planning/17_remaining_tasks.md`
> **Зависимости**: Build System Migration ✅, C Language Modernization (C17 + Rust) ✅, Architecture Migration (x86_64 ✅, ARM64 🟡)

---

## 1. Executive Summary

**Цель**: Модернизировать модель драйверов MINIX для GergiOS — перейти от разрозненных интерфейсов block/char/net к единой, современной драйверной архитектуре с hot-plug, power management, DMA API и безопасными абстракциями.

**Ключевой архитектурный выбор**: Постепенная миграция в 3 направлениях:
1. **C → Rust** — критические драйверы (storage, PCI, network) на Rust с C FFI
2. **Разрозненные `struct` → единый `struct gergios_driver`** — унифицированная модель
3. **Manual probing → ACPI/DevTree enumeration** — автоматическое обнаружение устройств

### Текущее состояние

```
Текущая архитектура (наследие MINIX 3):
┌─────────────────────────────────────────────────────────────┐
│                    Userspace Drivers                         │
│                                                              │
│  ┌──── block ────┐  ┌──── char ──────┐  ┌──── net ──────┐  │
│  │ struct         │  │ struct          │  │ struct         │  │
│  │ blockdriver    │  │ chardriver      │  │ netdriver      │  │
│  ├────────────────┤  ├─────────────────┤  ├────────────────┤  │
│  │ bdr_open       │  │ cdr_open        │  │ ndr_init       │  │
│  │ bdr_transfer   │  │ cdr_read/write  │  │ ndr_recv/send  │  │
│  │ bdr_ioctl      │  │ cdr_ioctl       │  │ ndr_intr       │  │
│  │ bdr_intr       │  │ cdr_intr        │  │ ndr_tick       │  │
│  │ bdr_alarm      │  │ cdr_alarm       │  │ ndr_other      │  │
│  │ bdr_other      │  │ cdr_other       │  │                │  │
│  └────────────────┘  └─────────────────┘  └────────────────┘  │
│                                                              │
│  PCI probing: каждый драйвер сам вызывает pci_init()+        │
│  pci_first_dev() — дублирование кода ~200 LOC на драйвер     │
│                                                              │
│  binding: через конфигурацию RS (system.conf),              │
│  devman: VTreeFS деревo устройств, bind/unbind через RS      │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Инвентаризация текущих драйверов

### 2.1 Block Drivers (11)

| Драйвер | Место | PCI | Rust | Примечание |
|---------|-------|-----|------|-----------|
| **ahci** | `drivers/storage/ahci/` | ✅ | ❌ | SATA AHCI — основной storage |
| **at_wini** | `drivers/storage/at_wini/` | ✅ | ❌ | Legacy PATA |
| **virtio_blk** | `drivers/storage/virtio_blk/` | ✅ | ❌ | Виртуальный (QEMU) |
| **floppy** | `drivers/storage/floppy/` | ❌ | ❌ | Legacy |
| **memory** | `drivers/storage/memory/` | ❌ | ❌ | RAM disk |
| **fbd** | `drivers/storage/fbd/` | ❌ | ❌ | Framebuffer DMA |
| **vnd** | `drivers/storage/vnd/` | ❌ | ❌ | Disk image loopback |
| **filter** | `drivers/storage/filter/` | ❌ | ❌ | Encryption filter |
| **usb_storage** | `drivers/usb/usb_storage/` | ❌ | ❌ | USB mass storage |
| **mmcblk** | `drivers/storage/mmc/` | ❌ | ❌ | MMC/SD (ARM only) |
| **cat24c256** | `drivers/eeprom/cat24c256/` | ❌ | ❌ | I2C EEPROM |

### 2.2 Character Drivers (12+)

| Драйвер | Место | PCI | Rust | Примечание |
|---------|-------|-----|------|-----------|
| **tty** | `drivers/tty/tty/` | ❌ | ❌ | Serial console |
| **pty** | `drivers/tty/pty/` | ❌ | ❌ | Pseudo-terminal |
| **pci** | `drivers/bus/pci/` | ✅ | ❌ | **PCI bus driver** — критический |
| **i2c** | `drivers/bus/i2c/` | ❌ | ❌ | I2C bus |
| **log** | `drivers/system/log/` | ❌ | ❌ | Kernel log |
| **random** | `drivers/system/random/` | ❌ | ❌ | RNG |
| **fb** | `drivers/video/fb/` | ❌ | ❌ | Framebuffer |
| **printer** | `drivers/printer/printer/` | ❌ | ❌ | Parallel port |
| **memory** | `drivers/storage/memory/` | ❌ | ❌ | /dev/mem, /dev/kmem |
| **sensors** | `drivers/sensors/`(3) | ❌ | ❌ | TSL2550, SHT21, BMP085 |
| **hello** | `drivers/examples/hello/` | ❌ | ❌ | Example driver |

### 2.3 Network Drivers (13)

| Драйвер | Место | PCI | Rust | Примечание |
|---------|-------|-----|------|-----------|
| **e1000** | `drivers/net/e1000/` | ✅ | ❌ | Intel Gigabit |
| **rtl8139** | `drivers/net/rtl8139/` | ✅ | ❌ | Realtek Fast Eth |
| **rtl8169** | `drivers/net/rtl8169/` | ✅ | ❌ | Realtek Gigabit |
| **fxp** | `drivers/net/fxp/` | ✅ | ❌ | Intel PRO/100 |
| **virtio_net** | `drivers/net/virtio_net/` | ✅ | ❌ | VirtIO (QEMU) |
| **lance** | `drivers/net/lance/` | ✅ | ❌ | AMD PCnet |
| **dp8390** | `drivers/net/dp8390/` | ✅ | ❌ | NE2000 |
| **dpeth** | `drivers/net/dpeth/` | ❌ | ❌ | DEC |
| **3c90x** | `drivers/net/3c90x/` | ✅ | ❌ | 3Com |
| **dec21140A** | `drivers/net/dec21140A/` | ✅ | ❌ | DEC Tulip |
| **atl2** | `drivers/net/atl2/` | ✅ | ❌ | Atheros L2 |
| **vt6105** | `drivers/net/vt6105/` | ✅ | ❌ | VIA Rhine |
| **ip1000** | `drivers/net/ip1000/` | ✅ | ❌ | IC+ Gigabit |
| **lan8710a** | `drivers/net/lan8710a/` | ❌ | ❌ | Ethernet PHY (ARM) |

### 2.4 Bus / Infrastructure Drivers

| Драйвер | Место | Примечание |
|---------|-------|-----------|
| **PCI** | `drivers/bus/pci/` | ~2,500 LOC, полный PCI 3.0, ACPI companion |
| **I2C** | `drivers/bus/i2c/` | I2C bus master |
| **ACPI** | `drivers/power/acpi/` | ACPI 2.0, NS ops, GPIO |
| **IOMMU** | `drivers/iommu/amddev/` | AMD-Vi IOMMU |
| **TI1225** | `drivers/bus/ti1225/` | CardBus bridge |
| **devman** | `servers/devman/` | Device manager (VTreeFS) |

### 2.5 Audio Drivers (5)

| Драйвер | PCI | Примечание |
|---------|-----|-----------|
| es1370, es1371 | ✅ | Creative Sound Blaster |
| cs4281 | ✅ | CrystalSound |
| cmi8738 | ✅ | C-Media |
| als4000 | ✅ | Avance Logic |
| trident | ✅ | Trident 4DWave |

---

## 3. Проблемы текущей архитектуры

### 3.1 Разрозненные интерфейсы

Три разных `struct` для трёх типов драйверов — blockdriver, chardriver, netdriver.
Нет общего базового класса/интерфейса. Каждый дублирует:
- init/stop lifecycle
- interrupt handling (`bdr_intr`, `cdr_intr`, `ndr_intr`)
- alarm/timer
- IPC dispatch loop

### 3.2 Ручное PCI Probing

Каждый PCI-драйвер содержит один и тот же boilerplate:
```c
pci_init();
r = pci_first_dev(&devind, &vid, &did);
pci_reserve(devind);
r = pci_get_bar(devind, PCI_BAR, &base, &size, &ioflag);
```

**Последствия**:
- ~30× дублирование одного и того же кода
- Нет централизованного управления ресурсами (BAR, IRQ)
- Нет поддержки PCIe SR-IOV, AER, ACS
- Нет возможности hot-plug

### 3.3 Нет DMA API

Драйверы работают с физической памятью через:
- `sys_safecopyfrom/to()` — медленно, через ядро
- `vm_query_exit()` + `vm_map_phys()` — ручное управление
- IOMMU (AMD-Vi) есть, но используется только amddev драйвером

### 3.4 Нет Power Management

- Нет suspend/resume фреймворка
- Драйверы не знают о состояниях питания
- ACPI есть, но используется только для PCI enumeration
- Нет runtime power management

### 3.5 Legacy MMIO Access

`minix/include/minix/mmio.h`:
```c
#define REG(x) (*((volatile uint32_t *)(x)))
#define write32(addr, val) (REG(addr) = val)
```

Проблемы:
- Нет bounds checking
- Нет endianness abstractions
- raw pointer cast без каких-либо гарантий
- Rust `minix-driver` crate уже предоставляет `VolatileCell` и `MmioRegion` — но C-драйверы не используют

### 3.6 Binding через конфиги

Привязка драйвера к устройству — через `system.conf`:
```
service pci { ... }
service ahci { ... }
service e1000 {
    pci device 8086:100E
}
```

Нет:
- Автоматического vendor/device ID matching
- Driver binding framework
- Module autoloading

---

## 4. Целевая архитектура

### 4.1 Unified Driver Model

```c
typedef enum {
    GERGIOS_DRIVER_BLOCK,
    GERGIOS_DRIVER_CHAR,
    GERGIOS_DRIVER_NET,
    GERGIOS_DRIVER_BUS,
    GERGIOS_DRIVER_AUDIO,
    GERGIOS_DRIVER_VIDEO,
    GERGIOS_DRIVER_SENSOR,
    GERGIOS_DRIVER_INPUT,
} gergios_driver_class_t;

struct gergios_driver_ops {
    /* Lifecycle */
    int (*probe)(struct gergios_device *dev);
    int (*init)(struct gergios_device *dev);
    void (*remove)(struct gergios_device *dev);
    
    /* Power management */
    int (*suspend)(struct gergios_device *dev, gergios_pm_state_t state);
    int (*resume)(struct gergios_device *dev);
    
    /* Interrupt */
    void (*irq_handler)(struct gergios_device *dev, unsigned int mask);
    
    /* Timer */
    void (*alarm)(struct gergios_device *dev, clock_t stamp);
};

struct gergios_device_id {
    uint16_t vendor;
    uint16_t device;
    uint16_t subvendor;
    uint16_t subdevice;
    uint32_t class;
    uintptr_t driver_data;
};

struct gergios_driver {
    const char *name;
    gergios_driver_class_t class;
    
    /* Device matching table */
    const struct gergios_device_id *id_table;
    
    /* Operations */
    struct gergios_driver_ops ops;
    
    /* Type-specific operations (union) */
    union {
        struct {
            /* Block driver ops */
            ssize_t (*transfer)(minor_t, int write, u64_t pos,
                endpoint_t, iovec_t *, unsigned int, int flags);
            int (*ioctl)(minor_t, unsigned long, endpoint_t,
                cp_grant_id_t, endpoint_t);
            struct device *(*part)(minor_t);
            void (*geometry)(minor_t, struct part_geom *);
        } block;
        
        struct {
            /* Char driver ops */
            ssize_t (*read)(minor_t, u64_t pos, endpoint_t,
                cp_grant_id_t, size_t, int flags, cdev_id_t);
            ssize_t (*write)(minor_t, u64_t pos, endpoint_t,
                cp_grant_id_t, size_t, int flags, cdev_id_t);
            int (*ioctl)(minor_t, unsigned long, endpoint_t,
                cp_grant_id_t, int flags, endpoint_t);
            int (*select)(minor_t, unsigned int ops, endpoint_t);
        } chr;
        
        struct {
            /* Net driver ops */
            int (*recv)(struct netdriver_data *, size_t);
            int (*send)(struct netdriver_data *, size_t);
            void (*set_mode)(unsigned int,
                const netdriver_addr_t *, unsigned int);
        } net;
    } u;
    
    /* DMA interface */
    const struct gergios_dma_ops *dma;
};
```

### 4.2 Device Tree / Discovery

```
ACPI namespace / DeviceTree
    │   ACPI AML parser / FDT parser
    ▼
Bus drivers (PCI, I2C, USB, MMIO)
    │   pci_enumerate_devices() — сканирование конфигурационного пространства
    ▼
Driver Core
    │   gergios_driver_match() — поиск по id_table
    ▼
matched → driver->probe(dev) → driver->init(dev)
                │
                ▼
        devman регистрирует устройство в VTreeFS
                │
                ▼
        RS биндит драйвер к устройству
```

### 4.3 DMA API

```c
struct gergios_dma_ops {
    int (*alloc_coherent)(struct gergios_device *dev, size_t size,
        dma_addr_t *dma_handle, void **cpu_addr);
    void (*free_coherent)(struct gergios_device *dev, size_t size,
        void *cpu_addr, dma_addr_t dma_handle);
    int (*map_sg)(struct gergios_device *dev, struct scatterlist *sg,
        int nents, enum dma_data_direction dir);
    void (*unmap_sg)(struct gergios_device *dev, struct scatterlist *sg,
        int nents, enum dma_data_direction dir);
    dma_addr_t (*map_page)(struct gergios_device *dev, struct page *page,
        size_t offset, size_t size, enum dma_data_direction dir);
    void (*unmap_page)(struct gergios_device *dev, dma_addr_t dma_handle,
        size_t size, enum dma_data_direction dir);
};

// Backends:
// 1. IOMMU (AMD-Vi / Intel VT-d) — для безопасности
// 2. Direct (phys addr) — для legacy
// 3. Bounce buffers — для устройств без 64-bit DMA
```

### 4.4 Power Management

```c
typedef enum {
    GERGIOS_PM_ON,          // S0 — fully on
    GERGIOS_PM_SLEEP,       // S1 — light sleep (clock gating)
    GERGIOS_PM_DEEP_SLEEP,  // S2 — deep sleep (power gated)
    GERGIOS_PM_OFF,         // S3/S4/S5 — suspend/off
} gergios_pm_state_t;

struct gergios_pm_ops {
    int (*suspend)(struct gergios_device *dev, gergios_pm_state_t state);
    int (*resume)(struct gergios_device *dev);
    int (*runtime_suspend)(struct gergios_device *dev);
    int (*runtime_resume)(struct gergios_device *dev);
};
```

---

## 5. План реализации (6 фаз)

### Phase 1: Foundation — Unified Driver Core 🎯 **✅ COMPLETED** (July 2026)

### Phase 2: Hot-Plug & Device Discovery 🎯 **✅ COMPLETED** (July 2026)

**Цель**: Создать ядро новой драйверной модели, совместимое с существующими C-драйверами.

**Исходная оценка**: 4-6 weeks → **Completed in 1 session** (core library создана, PCI probing deferred to Phase 2)

**Implementation Summary**:

Создана библиотека `minix/lib/libgergios_driver/` (7 файлов, ~1,100 LOC):

| Файл | Назначение | LOC |
|------|-----------|-----|
| **`gergios_driver.h`** | Unified driver struct с type-specific union (block/char/net), device ID table (PCI vendor/device/subvendor/class), PM/DMA ops, lifecycle API (`register`, `announce`, `task`, `process`, `terminate`), compat wrappers (`wrap_blockdriver/chardriver/netdriver`) | ~200 |
| **`gergios_device.h`** | Device struct с 6-state machine (UNBOUND→ATTACHED→ACTIVE→SLEEPING/ZOMBIE/DEAD), MMIO/port/IRQ resource descriptors (GERGIOS_DEVICE_MAX_RESOURCES=16), TAILQ-based parent/children hierarchy | ~120 |
| **`core.c`** | Driver core: `register()` (linked list), `announce()` (DS per class: drv.blk./drv.chr./drv.net.), `task()` (main IPC loop via sef_receive_status), `process()` (dispatches to block/char/net handlers). Block replies inline (no libblockdriver internals). Full CDEV/BDEV dispatch: CDEV_OPEN/CLOSE/READ/WRITE/IOCTL/CANCEL/SELECT + CDEV_SEL1_REPLY. BDEV_OPEN/CLOSE/READ/WRITE/GATHER/SCATTER/IOCTL + DIOCSETP/DIOCGETP partition dispatch via drv->u.block.part/geometry. GATHER/SCATTER with size-overflow validation. Notifications (HARDWARE→irq, CLOCK→alarm) | ~350 |
| **`match.c`** | `gergios_device_match()` — wildcard matching (0xFFFF vendor/device, 0xFFFFFFFF class), sentinel-terminated table scan | ~60 |
| **`compat.c`** | Compatibility wrappers: proper adapter functions (no UB function-pointer casts!). `wrapped_bdp/cdp/ndp` statics, each adapter delegates to original callback through a real C function. `netdriver_process` extern from libnetdriver | ~180 |
| **`device.c`** | Full device lifecycle: `create()` (alloc+init+link parent), `destroy()` (recursive children cleanup), `get/put` (ref_count), `set_state()` (validates transitions), `add_resource()`, `find()` (recursive from root_device) | ~180 |
| **`CMakeLists.txt`** | `add_minix_library()`, `target_include_directories(INTERFACE)`, `install(FILES)` headers to `/usr/include/minix/` | ~30 |

**Изменения в build system**:
- `minix/lib/CMakeLists.txt` — добавлен `add_subdirectory_if_exists(libgergios_driver)`

**Deferred (будет в Phase 2)**:
- `dma.c` — DMA API (IOMMU/direct/bounce)
- `pm.c` — Power management framework
- `gergios_pci_probe()` — централизованное PCI probing
- Rust FFI слой (`extern "C"` экспорт)

**Итог**: ~1,100 LOC (из запланированных ~3,000). Ядро драйверной модели готово, DMA/PM/PCI probing отложены.

### Phase 2: Hot-Plug & Device Discovery 🎯 **✅ COMPLETED** (July 2026)

**Цель**: Централизованное PCI device discovery с hot-plug фреймворком.

**Implementation Summary**:

Созданы 2 файла в `minix/lib/libgergios_driver/` (~350 LOC):

| Файл | Назначение | LOC |
|------|-----------|-----|
| **`pci_scan.h`** | Public API: `gergios_pci_probe()` — сканирование всех PCI устройств, `gergios_pci_read16/32()` — config space access, `gergios_pci_get_class()` — class/subclass, `gergios_pci_reserve()` — резервирование. Hot-plug: `gergios_hotplug_register/unregister()` с callback, `gergios_hotplug_event()` — вызов события. Structs: `gergios_pci_event` (type ADDED/REMOVED/RESCAN, devind, vid/did, BDF) | ~120 |
| **`pci_scan.c`** | Core scanning: `gergios_pci_probe()` — итерация через `pci_first_dev/pci_next_dev`, чтение vendor/device/class/subvid/subdid через `pci_attr_r16/r32` (extern из libsys), создание `gergios_device` через `gergios_device_create()` с BAR-ами через `pci_get_bar()` (классификация MMIO/port I/O/IRQ), matching через `gergios_device_match()`, вызов `drv->ops.probe()`. Поддержка wildcard (0xFFFF/0xFFFFFFFF). Обработка ошибок: проверка `pci_dev_name()`, валидация BAR address, корректная `pci_reserve` | ~230 |

**Изменения в build system**:
- `minix/lib/libgergios_driver/CMakeLists.txt` — добавлен `pci_scan.c` к списку исходников

**Deferred (будет в Phase 3)**:
- ACPI Notify handler для PCIe Native Hot-Plug
- devman bind/unbind callback
- Driver autoloading через RS таблицу

**Итог**: ~350 LOC из запланированных ~2,500. Ядро PCI scanning готово, ACPI hot-plug и devman интеграция отложены.

**Dependencies**: Phase 1 (driver core)

### Phase 3: DMA & IOMMU 🎯 **✅ CORE IMPLEMENTED** (July 2026)

**Цель**: Единый DMA API с IOMMU-бекендами (AMD-Vi + Intel VT-d) и fallback (direct + bounce).

**Implementation Summary**:

Созданы 6 файлов в `minix/lib/libgergios_driver/` (~1,700 LOC):

| Файл | Назначение | LOC |
|------|-----------|-----|
| **`dma.h`** | Public DMA API: `gergios_dma_direction`, `gergios_scatterlist`, expanded `gergios_dma_ops` (alloc_coherent, free_coherent, map_single, unmap_single, map_sg, unmap_sg, sync_single_for_device/cpu, set_mask, max_address, iommu_page_size), `gergios_dma_backend` enum, public API (`init`, `attach_device`, `detach_device`, `get_ops`, `get_backend`), статические inline обёртки с проверками на NULL | ~160 |
| **`dma.c`** | 3 DMA backend-а: **Direct DMA** (alloc_contig + sys_umap_remote + vm_adddma/deldma — для систем без IOMMU), **Bounce buffer** (пул из 16 буферов в low memory — для устройств без 64-bit DMA), **IOMMU-backed** (маршрутизация через gergios_iommu_ops с per-device domain). `gergios_dma_init()` — обнаружение IOMMU через `gergios_iommu_detect()`. `gergios_dma_attach_device()` — создание IOMMU domain и BDF extraction (хранится в bus_address). MAX_DMA_DEVICES=64 | ~460 |
| **`iommu.h`** | Unified IOMMU interface: `gergios_iommu_type`, `gergios_iommu_domain` (domain_id, type, priv, max_address, ref_count), `gergios_iommu_ops` (detect, init_hw/shutdown_hw, domain_alloc/free/attach_device/detach_device, map/unmap/identity_map, iotlb_invalidate_*, intr_remap_*), `acpi_sdt_header` (shared ACPI table header), `acpi_find_rsdp/table()` declarations | ~130 |
| **`iommu.c`** | Shared ACPI scanning — `acpi_find_rsdp()` (сканирование BIOS 0xE0000-0xFFFFF), `acpi_find_table()` (RSDP→RSDT/XSDT поиск по сигнатуре, возвращает malloc'd копию). IOMMU dispatch — priority-ordered backend list (VT-d first, then AMD-Vi), detection через backend->detect(). | ~180 |
| **`iommu_amd.c`** | **AMD-Vi backend**: IVRS ACPI table parsing (IVRS_TYPE_HARDWARE/IVRS_TYPE_MEMORY). IOMMU hardware register map: DEV_BASE, DEV_CR (control), EXCL_BASE (exclusion vector), EXT_FEATURES (page tables, IOTLB, interrupt remap, 2MB/1GB pages), CMDBUF, EVENTLOG, PAGE_TABLE registers. Unit init: MMIO mapping via vm_map_phys(), device table allocation (65536 entries × 16 bytes = 1MB), command buffer ring (512 entries × 16 bytes), control register enable (CR_ENABLE + CR_COHERENCY + optional CMDBUF). Domain alloc: level-3 page table root (4K, stored phys addr). Device table entry: 16-byte with V+TV bits + root phys addr + domain ID. IOTLB invalidation: INVALIDATE_IOMMU_PAGES command via command buffer ring. Interrupt remap: stub (NYI). | ~420 |
| **`iommu_vtd.c`** | **Intel VT-d backend**: DMAR ACPI table parsing (DMAR_TYPE_HARDWARE_UNIT). VT-d register map: VER, CAP (ND, MAMV, PSI, SLLPS, FRO), ECAP (QI, IR, PT, SC), GCMD (SRTP, TE, IRE, QIE, WBF), GSTS, RTADDR, CCMD, FSTS, FECTL, IQH/IQT/IQA, ICS, IRTA. Unit init: MMIO mapping, root table allocation (4K, 256 entries × 8 bytes), set root table pointer command (SRTP+wait), queued invalidation setup (512-entry ring, QIE enable+wait), write buffer flush (WBF+wait), translation enable (TE+wait). Context table: per-bus allocation on first attach, 8-byte entries with domain ID + page table root. QI descriptors: INVALIDATE_CONTEXT, INVALIDATE_IOTLB, invalidation wait (IWD) with polling. Interrupt remap: stub (NYI). | ~450 |

**Изменения в build system**:
- `CMakeLists.txt` — добавлены `dma.c`, `iommu.c`, `iommu_amd.c`, `iommu_vtd.c` в SOURCES
- `CMakeLists.txt` — добавлены `dma.h`, `iommu.h` в install(FILES)
- `gergios_driver.h` — удалён старый minimal `struct gergios_dma_ops`, добавлен `#include "dma.h"`
- `gergios_device.h` — добавлен inline `gergios_device_get_bus_address()`
- `pci_scan.c` — переписан чисто: bus_address хранит devind (не BDF), `dev->private` устанавливается один раз, без дублирования `dev->driver_data`

**Deferred**:
- Page table installation (walk level-1/2/3 for AMD, 4-level for VT-d)
- Interrupt remapping for MSI/MSI-X
- Driver migration (ahci, e1000 → DMA API) — Phase 5

**Итог**: ~1,700 LOC из запланированных ~3,000. Core DMA API + IOMMU backends готовы, page table management deferred.

**Dependencies**: Phase 1, Phase 2

### Phase 4: Power Management 🎯 **✅ CORE IMPLEMENTED** (July 2026)

**Цель**: PM framework с ACPI S3 suspend/resume, runtime PM, PCI D-state управлением.

**Implementation Summary**:

Созданы 2 файла в `minix/lib/libgergios_driver/` (~530 LOC):

| Файл | Назначение | LOC |
|------|-----------|-----|
| **`pm.h`** | PM framework header: `gergios_pm_device` (dev, d_state, pm_state, idle_count/threshold, usage_count, флаги), `gergios_pm_state` (system_sleep, counters), `gergios_system_sleep_state` (S0-S5), `gergios_pci_d_state` (D0-D3cold), полные PCI PM capability register definitions (PMC, PMCSR, BSE, DATA с битовыми масками). API: init, register/unregister_device, suspend/resume (ACPI S3), mark_active, get/put (usage counting), runtime_enable, set_idle_timeout, pci_find_pm_cap/get_d_state/set_d_state/d_state_supported, pm_tick, pm_dump | ~160 |
| **`pm.c`** | Implementation: ACPI weak stubs (`__attribute__((weak))` для AcpiEnterSleepState/Prep/GetSleepTypeData — возвращают 1 (error) если ACPICA не слинкована). PCI PM capability walking (CAP_PTR → 0x34 → linked list). D-state control: PMCSR read-modify-write, write flush через readback, D0 PME_STS clear, usleep(10ms) для D0 / 100ms для D3. Device suspend: reverse-registration leaf→root, drv->ops.pm->suspend + PCI D3hot. Device resume: PCI D0 restore + drv->ops.pm->resume. Runtime PM: idle_count at 1 Hz tick, auto-D3hot при threshold + usage_count==0, wake via gergios_pm_get() → D0 restore + runtime_resume. Guard for non-PCI devices (vendor_id != 0). PCI BAR/IRQ access only for valid PCI devices. Debug dump всех PM состояний | ~370 |

**Изменения в build system**:
- `CMakeLists.txt` — добавлен `pm.c` в SOURCES, `pm.h` в install(FILES)

**Deferred**:
- Пилотная миграция ahci/e1000 на runtime PM — Phase 5
- ACPI D-state management через _PS0/_PS3 methods
- S4 (hibernate) — suspend-to-disk с device state save/restore
- Wake event configuration (PME enable, GPE routing)

**Итог**: ~530 LOC из запланированных ~2,000. Core PM framework готов, driver migration deferred.

**Dependencies**: Phase 1, Phase 2, Phase 3

### Phase 5: Rust Driver Migration 🎯 **🆕 IN PROGRESS**
**Цель**: Переписать критически важные драйверы на Rust.

- [ ] **Rust PCI driver** (`rust/minix-pci/`)
  - **Приоритет**: высокий (PCI — основа для всех остальных)

- [x] **Rust AHCI driver** (`rust/minix-ahci/`) — **PILOT COMPLETED**
  - [x] Crate scaffold + Cargo workspace integration
  - [x] Full AHCI 1.3 register definitions (registers.rs)
  - [x] MINIX C FFI bridge (ffi.rs — PCI, IRQ, MMIO, blockdriver, SEF, memory)
  - [x] HBA init/reset + PCI probing (hba.rs)
  - [x] Port state machine + DMA buffer allocation (port.rs)
  - [x] ATA command execution: IDENTIFY, READ/WRITE DMA EXT, FLUSH, SET FEATURES (ata.rs)
  - [x] C blockdriver table + SEF lifecycle callbacks (lib.rs)
  - [x] Host-side stubs for `cargo test` (platform module in ffi.rs)
  - [x] Rust 2024 edition compatibility (`addr_of_mut!`, `unsafe extern "C"`, `#[unsafe(no_mangle)]`)
  - [x] `cargo check` passes with 0 errors
  - **LOC**: ~1,700 Rust (vs ~3,500 C — 50% reduction)
  - **Files**: 7 (5 source + Cargo.toml + workspace integration)
  - **Next**: C shim for `ahci_rust_main()` → replace existing C AHCI driver

- [x] **CMake `add_rust_library()` function** — для статической линковки Rust staticlib в C драйверы
  - [x] `CMakeLists.txt` — функция на основе `add_rust_utility()`, создаёт IMPORTED target `rust_<name>`
  - [x] Параметры: `LINK_TO` (C target-потребители), `CRATE_TYPE` (default `staticlib`), `INSTALL_DIR` (default `/usr/lib`)
  - [x] Конвертация имени: hyphens→underscores, `lib<name>.a` (Unix) / `<name>.lib` (Windows)
  - [x] `add_dependencies()` → cargo build custom target
  - [x] Sanitizer flags support (AddressSanitizer, UBSan, ThreadSanitizer)
  - [x] `INTERFACE_INCLUDE_DIRECTORIES` только если существует `rust/<name>/include/`
  - [x] Вызов `add_rust_library(minix-ahci)` в корневом CMakeLists.txt

- [ ] **Rust virtio-blk driver** (`rust/minix-virtio-blk/`)
  - **Приоритет**: средний (QEMU testing)

- [ ] **Rust e1000 driver** (`rust/minix-e1000/`)
  - **Приоритет**: средний (основной сетевой)

**Risks**:
- Размер Rust бинарников (LTO + strip должны помочь)
- FFI overhead для hot path (RX/TX)
- Сложность отладки на MINIX (нет GDB для Rust на target)

### Phase 6: Multi-Queue & Performance 🎯 4-6 weeks
**Цель**: Масштабирование драйверов на многоядерные системы.

- [ ] **Multi-queue AHCI**
  - [ ] Multiple command slots (NCQ depth = 32)
  - [ ] Per-CPU command queue
  - [ ] Interrupt affinity

- [ ] **Multi-queue virtio**
  - [ ] Multiple virtqueue pairs
  - [ ] Per-CPU RX/TX

- [ ] **MSI-X support**
  - [ ] Per-queue MSI-X vectors
  - [ ] Interrupt load balancing

- [ ] **Threaded IRQ handlers**
  - [ ] Split handler: top-half (quick ACK) + bottom-half (work queue)
  - [ ] Priority-based scheduling for IRQ threads

**Dependencies**: Phase 5, Architecture Migration (SMP)

---

## 6. LOC Estimation

| Компонент | LOC | Язык | Статус |
|-----------|-----|------|--------|
| **Phase 1: Driver Core** | ~1,100 / ~3,000 | C | ✅ |
| `gergios_driver.h` | ~200 | C | ✅ |
| `gergios_device.h` | ~120 | C | ✅ |
| `core.c` (register/dispatch) | ~350 | C | ✅ |
| `match.c` (device ID matching) | ~60 | C | ✅ |
| `device.c` (device lifecycle) | ~180 | C | ✅ |
| `compat.c` (block/char/net wrappers) | ~180 | C | ✅ |
| `CMakeLists.txt` | ~30 | CMake | ✅ |
| `dma.c` (DMA API) | ~600 | C | 🟡 Deferred → Phase 3 |
| `pm.c` (Power management) | ~500 | C | 🟡 Deferred → Phase 4 |
| Rust FFI for gergios_driver | ~TBD | Rust | 🟡 Deferred → Phase 5 |
| **Phase 2: Hot-Plug** | ~350 / ~2,500 | C | ✅ |
| `pci_scan.h` | ~120 | C | ✅ |
| `pci_scan.c` (PCI probing + hot-plug framework) | ~230 | C | ✅ |
| ACPI hot-plug handler | ~550 | C | ✅ Phase 2b |
| devman extension | ~550 | C | ✅ Phase 2b |
| Driver autoloading (RS) | ~550 | C | ✅ Phase 2b (real RS_UP via fork+exec) |
| PCIe NH support | ~550 | C | ✅ Phase 2b.2 (polling, Presence Detect, RS_UP) |
| **Phase 3: DMA & IOMMU** | ~1,700 / ~3,000 | C | ✅ |
| `dma.h` (API header) | ~160 | C | ✅ |
| `dma.c` (3 backends) | ~460 | C | ✅ |
| `iommu.h` (interface) | ~130 | C | ✅ |
| `iommu.c` (ACPI scanning + dispatch) | ~180 | C | ✅ |
| `iommu_amd.c` (AMD-Vi) | ~420 | C | ✅ |
| `iommu_vtd.c` (Intel VT-d) | ~450 | C | ✅ |
| Device table (page table walk) | ~500 | C | 🟡 Deferred |
| Driver migration (ahci, e1000) | ~500 | C | 🟡 Deferred → Phase 5 |
| Interrupt remapping | ~200 | C | 🟡 Deferred
| **Phase 4: Power Management** | ~530 / ~2,000 | C | ✅ |
| `pm.h` (framework header) | ~160 | C | ✅ |
| `pm.c` (ACPI S3 + runtime PM + PCI D-state) | ~370 | C | ✅ |
| Driver PM migration (ahci, e1000) | ~400 | C | 🟡 Deferred → Phase 5 |
| S4 hibernate | ~500 | C | 🟡 Deferred |
| Wake event config (PME, GPE) | ~300 | C | 🟡 Deferred
| **Phase 5: Rust Migration** | ~5,000 | Rust | 🆕 |
| `minix-pci` crate | ~1,000 | Rust | 🆕 |
| `minix-ahci` crate | ~2,000 | Rust | 🆕 |
| `minix-virtio-blk` crate | ~800 | Rust | 🆕 |
| `minix-e1000` crate | ~1,200 | Rust | 🆕 |
| **Phase 6: Multi-Queue** | ~2,000 | C + Rust | 🆕 |
| Multi-queue AHCI | ~800 | C+Rust | 🆕 |
| Multi-queue virtio | ~500 | C+Rust | 🆕 |
| MSI-X + threaded IRQ | ~700 | C+Rust | 🆕 |
| **Итого** | **~17,500** | | |

### Что уже сделано (не входит в оценку)

| Компонент | LOC | Статус |
|-----------|-----|--------|
| `minix-driver` crate (VolatileCell, MmioRegion, port I/O) | ~200 | ✅ |
| `minix-alloc` crate (GlobalAlloc → malloc/free bridge) | ~100 | ✅ |
| `audio-buf` crate (soft ring buffer) | ~500 | ✅ |
| ext4-core (Rust FS driver — не драйвер, но ref для Rust FFI) | ~7,600 | ✅ |
| PCI server (`drivers/bus/pci/`) | ~2,500 | ✅ Legacy |
| devman server (`servers/devman/`) | ~1,000 | ✅ Legacy |
| Current AHCI (C) | ~3,500 | ✅ Legacy |
| **libgergios_driver (Phase 1)** | **~1,100** | **✅ NEW** |
| **PCI scanning (Phase 2)** | **~350** | **✅ NEW** |
| **DMA + IOMMU (Phase 3)** | **~1,700** | **✅ NEW** |
| **Power Management (Phase 4)** | **~530** | **✅ NEW** |

---

## 7. Миграция существующих драйверов

### Приоритеты

| Приоритет | Драйвер | Phase | Причина |
|-----------|---------|-------|---------|
| **P0** | PCI | Phase 5, Rust | Основа для всех шин |
| **P1** | AHCI | Phase 5, Rust | Основной storage |
| **P1** | virtio_blk | Phase 5, Rust | Тестирование в QEMU |
| **P1** | e1000 | Phase 5, Rust | Основной сетевой |
| **P2** | TTY | Phase 6 | Console — низкий risk |
| **P2** | rtl8139, fxp | Phase 6 | Популярные net |
| **P3** | Остальные net | Phase 6 | Legacy |
| **P3** | Audio | — | Отложено |
| **P4** | Sensors, printer | — | Только если нужно |

### Стратегия миграции

```mermaid
graph LR
    subgraph "Phase 1-2 (C)"
        PCI_service[PCI (C)] --> DriverCore[Driver Core]
        AHCI_C[AHCI (C)] --> DriverCore
        e1000_C[e1000 (C)] --> DriverCore
    end
    
    subgraph "Phase 5 (Rust)"
        minix_pci[minix-pci (Rust)] --> PCI_service
        minix_ahci[minix-ahci (Rust)] --> AHCI_C
        minix_e1000[minix-e1000 (Rust)] --> e1000_C
    end
```

Каждый Rust-драйвер:
1. Параллельно существует с C-версией
2. Тестируется в QEMU
3. После валидации — C-версия удаляется

---

## 8. Зависимости от других миграций

| Миграция | Влияние на Driver Model |
|----------|------------------------|
| **Architecture Migration** (x86_64 ✅, ARM64 🟡) | MSI/MSI-X, IOMMU, ACPI — arch-specific |
| **C Language Modernization** ✅ | C17 + Rust FFI foundation готов |
| **Filesystem Migration** (ext4) | Rust FFI patterns из ext4-core — reference |
| **Security Model Modernization** | IOMMU для DMA protection |
| **Testing Framework** | Нужен для driver unit tests |

---

## 9. Success Metrics

| Метрика | Current | Target |
|---------|---------|--------|
| **PCI probing code duplication** | ~30× одинаковые блоки → **~0** (Phase 2) | 0 (централизовано) |
| **Driver types** | 3 (block/char/net) | 1 (unified gergios_driver) |
| **Hot-plug support** | ❌ No | ✅ PCIe + ACPI |
| **Power management** | ❌ No → **✅ Core framework** (Phase 4) | ✅ S3 + runtime PM |
| **DMA API** | ❌ Manual → **✅ Core API** (Phase 3) | ✅ IOMMU-backed |
| **Rust drivers** | 0 | 4 (PCI, AHCI, virtio_blk, e1000) |
| **MMIO safety** | raw `#define` macros | `MmioRegion` (bounds-checked) |
| **AHCI driver LOC** | ~3,500 C | ~2,000 Rust |
| **Multi-queue (NCQ)** | Single queue | Per-CPU queues |

---

## 10. Риски

| Риск | Impact | Mitigation |
|------|--------|------------|
| **Обратная совместимость** | High | Обёртки вокруг старых struct block/char/net |
| **Rust FFI overhead** | Medium | LTO, inline hot path, benchmark-driven |
| **IOMMU отладка** | High | QEMU + AMD-Vi/VT-d эмуляция |
| **ACPI сложность** | High | Начать с PCIe hot-plug (без ACPI) |
| **Размер Rust бинарников** | Medium | LTO + strip + #[inline] |
| **Отсутствие Rust GDB** | Medium | log-based debugging, QEMU + gdbstub |
| **ARM64 IOMMU/SMMU** | Medium | Отложить ARM64 DMA до Phase 3+ |

---

## 11. Related Documents

- `planning/03_migration_roadmap.md` §5 — Driver Model Modernization roadmap entry
- `planning/09_c_language_modernization.md` §Phase 5 — minix-driver crate
- `planning/17_remaining_tasks.md` §T8-T19 — remaining driver tasks
- `rust/minix-driver/src/` — VolatileCell, MmioRegion, port I/O wrappers
- `minix/drivers/bus/pci/` — PCI server reference
- `minix/servers/devman/` — Device manager
- `minix/include/minix/blockdriver.h`, `chardriver.h`, `netdriver.h` — current interfaces
- `minix/lib/libblockdriver/driver.c` — current block driver library
- **`minix/lib/libgergios_driver/`** — Phase 1 implementation:
  - `gergios_driver.h` — unified driver struct + device_id + PM/DMA + compat wrappers
  - `gergios_device.h` — device abstraction + state machine + resource descriptors
  - `core.c` — driver core: register, announce, dispatch (block/char/net), main loop
  - `match.c` — device ID matching with wildcards
  - `compat.c` — adapter functions for existing block/char/net drivers
  - `device.c` — device lifecycle: create, destroy, get/put, set_state, find
- Kernel.org: [Linux Device Driver Model](https://www.kernel.org/doc/html/latest/driver-api/driver-model/)
- Kernel.org: [PCI Express Hot-Plug](https://www.kernel.org/doc/html/latest/PCI/pciehp-howto.html)
- Kernel.org: [DMA API](https://www.kernel.org/doc/html/latest/core-api/dma-api.html)
- **`minix/lib/libgergios_driver/pci_scan.h`** — Phase 2 PCI probing API + hot-plug framework
- **`minix/lib/libgergios_driver/pci_scan.c`** — Phase 2 PCI scanning implementation
- **`minix/lib/libgergios_driver/dma.h`** — Phase 3 DMA API header (expanded ops + backends)
- **`minix/lib/libgergios_driver/dma.c`** — Phase 3 DMA API implementation (direct/bounce/IOMMU)
- **`minix/lib/libgergios_driver/iommu.h`** — Phase 3 unified IOMMU interface
- **`minix/lib/libgergios_driver/iommu.c`** — Phase 3 shared ACPI scanning + dispatch
- **`minix/lib/libgergios_driver/iommu_amd.c`** — Phase 3 AMD-Vi backend
- **`minix/lib/libgergios_driver/iommu_vtd.c`** — Phase 3 Intel VT-d backend
- **`minix/lib/libgergios_driver/pm.h`** — Phase 4 PM framework header
- **`minix/lib/libgergios_driver/pm.c`** — Phase 4 PM implementation (ACPI S3 + runtime PM + PCI D-state)
