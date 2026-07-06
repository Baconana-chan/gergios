/* hibernate.h — Hibernation (S4 suspend-to-disk) Framework for GergiOS
 *
 * Provides the infrastructure for Suspend-to-Disk (ACPI S4):
 *
 * 1. Memory image management: save/restore all physical RAM to/from
 *    a swap partition (or swap file).
 * 2. Device state persistence: save/restore PCI config space and
 *    driver-specific state for all registered devices.
 * 3. Boot-time resume detection: header signature + boot parameter
 *    integration for automatic resume on next boot.
 *
 * Lifecycle:
 *
 *   Hibernate (triggered by user/PM):
 *     gergios_pm_suspend(GERGIOS_SLEEP_S4)
 *       → pm.c suspends all devices (drv->ops.pm->suspend, D3hot)
 *       → gergios_hibernate_save()
 *           → save PCI config space for all PM-registered devices
 *           → save memory image to swap (pages + metadata)
 *           → write hibernate header
 *       → AcpiEnterSleepStatePrep(S4)
 *       → AcpiEnterSleepState(S4)  ← system powers off
 *
 *   Resume (boot path):
 *     gergios_hibernate_detect() — bootloader/kernel startup
 *       → scan swap partition for "G4HI" signature
 *     gergios_hibernate_restore()
 *       → read header + memory image back into RAM
 *       → restore PCI config space for all devices
 *       → call gergios_pm_resume() → drv->ops.pm->resume
 *
 * The actual block I/O (reading/writing to the swap partition) is
 * abstracted through the hibernate_io_ops structure, allowing
 * different backends (raw block device, file system, bootloader).
 */

#ifndef _GERGIOS_HIBERNATE_H
#define _GERGIOS_HIBERNATE_H

#include <minix/config.h>
#include <minix/type.h>
#include <minix/endpoint.h>
#include <stdint.h>
#include <time.h>

/*===========================================================================*
 *    Hibernate Image Header (on-disk format, 4096 bytes)                   *
 *===========================================================================*/

#define GERGIOS_HIBERNATE_MAGIC   "G4HI"       /* 4-byte signature */
#define GERGIOS_HIBERNATE_VERSION  1            /* Format version */
#define GERGIOS_HIBERNATE_SECTOR_SIZE 512       /* Default sector size */

/* Flags */
#define GERGIOS_HIBERNATE_F_COMPRESSED   (1 << 0)  /* Image is compressed */
#define GERGIOS_HIBERNATE_F_ENCRYPTED    (1 << 1)  /* Image is encrypted */
#define GERGIOS_HIBERNATE_F_CRC32        (1 << 2)  /* CRC32 checksum present */
#define GERGIOS_HIBERNATE_F_VERIFIED     (1 << 3)  /* Image was verified */

#define GERGIOS_HIBERNATE_MAX_MEM_REGS   64   /* Max physical memory regions */
#define GERGIOS_HIBERNATE_MAX_DEVICES    64   /* Max tracked devices */

/* Physical memory region descriptor */
struct hibernate_mem_region {
	uint64_t phys_start;   /* Physical start address */
	uint64_t size;         /* Region size in bytes */
	uint32_t flags;        /* Region flags (reserved) */
	uint32_t reserved;
};

/* PCI device state descriptor (saved in image header) */
struct hibernate_pci_dev {
	int      devind;               /* MINIX PCI device index */
	uint16_t vendor_id;            /* PCI vendor ID */
	uint16_t device_id;            /* PCI device ID */
	uint32_t bus_address;          /* BDF or bus address */
	uint8_t  config_space[256];    /* Full PCI config space save */
	uint32_t saved_capptr;         /* PM capability pointer */
} __attribute__((packed));

/* On-disk header (first sector of swap partition).
 * Total: 4096 bytes (8 × 512-byte sectors, or 1 × 4096-byte sector). */
struct hibernate_header {
	/* Fixed fields (first 64 bytes) */
	char     magic[4];                    /* "G4HI" */
	uint32_t version;                     /* Format version */
	uint64_t image_size;                  /* Total image size in bytes */
	uint32_t num_memory_regions;          /* Entries in region table */
	uint32_t num_pci_devices;             /* Entries in device table */
	uint32_t flags;                       /* GERGIOS_HIBERNATE_F_* */
	uint32_t checksum;                    /* CRC32 of entire image */
	uint64_t timestamp;                   /* Time of hibernation */
	uint64_t kernel_addr;                 /* Kernel entry for resume */
	uint64_t resume_jump;                 /* Resume vector address */

	/* Reserved for future expansion */
	uint8_t  reserved[128];

	/* Memory region table (64 entries × 16 bytes = 1024 bytes) */
	struct hibernate_mem_region mem_regions[GERGIOS_HIBERNATE_MAX_MEM_REGS];

	/* PCI device state table (64 entries × 268 bytes = 17152 bytes).
	 * Spans multiple sectors after the header. */
	/* struct hibernate_pci_dev pci_devices[...]; stored after header */

	/* Padding to ensure total struct fits within 4096 bytes.
	 * The PCI device state table (struct hibernate_pci_dev[]) is stored
	 * in sectors following the header, not inline.  The header itself
	 * occupies exactly sizeof(struct hibernate_header) bytes, which
	 * is verified by _Static_assert to be <= 4096. */
	uint8_t  padding[2816 - sizeof(struct hibernate_mem_region) *
	    GERGIOS_HIBERNATE_MAX_MEM_REGS];
} __attribute__((packed));

_Static_assert(sizeof(struct hibernate_header) <= 4096,
    "hibernate_header must fit in 4096 bytes");

/*===========================================================================*
 *    Image sector layout                                                    *
 *===========================================================================*
 *
 * Sector 0:      hibernate_header (with magic, flags, mem_regions table)
 * Sector 1..N:   pci_devices[] table (if num_pci_devices > 0)
 *                 Each entry = 268 bytes, ~15 entries per 4096-byte block
 * Sector N+1..M: Memory image data (compressed or raw pages)
 *
 * The PCI device table starts at sector offset:
 *   pci_offset = 1 (right after header)
 *
 * The memory image data starts at sector offset:
 *   data_offset = 1 + ceil(num_pci_devices * sizeof(hibernate_pci_dev) / 4096)
 */

/*===========================================================================*
 *    I/O abstraction for reading/writing the swap device                    *
 *===========================================================================*/

struct hibernate_io_ops {
	/* Read count bytes from the swap device starting at byte offset.
	 * Returns 0 on success, negative errno on failure. */
	int (*read)(void *buf, uint64_t offset, size_t count);

	/* Write count bytes to the swap device starting at byte offset.
	 * Returns 0 on success, negative errno on failure. */
	int (*write)(const void *buf, uint64_t offset, size_t count);

	/* Get the total size of the swap device (in bytes).
	 * Returns size, or 0 if unknown. */
	uint64_t (*get_size)(void);

	/* Get the sector size of the swap device.
	 * Returns sector size (default 512), or negative on error. */
	int (*get_sector_size)(void);
};

/*===========================================================================*
 *    Public API                                                             *
 *===========================================================================*/

/* --- Initialisation ----------------------------------------------------- */

/* Initialise the hibernation subsystem.
 * Sets up the I/O backend to access the swap partition.
 * io_ops can be NULL if using built-in block device I/O (platform-dependent).
 * Swap device is specified by devind (PCI block device index) and
 * start_sector (LBA of the swap partition start).
 * Returns 0 on success, negative errno on failure. */
int gergios_hibernate_init(int swap_devind, uint64_t start_sector);

/* Initialise with custom I/O callbacks (for non-standard backends). */
int gergios_hibernate_init_io(const struct hibernate_io_ops *io_ops);

/* --- Availability ------------------------------------------------------- */

/* Check if ACPI S4 (hibernate) is available on this system.
 * Returns 1 if S4 is available, 0 if not. */
int gergios_hibernate_available(void);

/* Get S4 wake capabilities (S4W and S4D values from ACPI).
 * Returns 0 on success, negative if not available. */
int gergios_hibernate_get_wake_caps(uint8_t *s4w, uint8_t *s4d);

/* --- Memory Region Management ------------------------------------------- */

/* Register a physical memory region to be saved during hibernation.
 * Called by VM or kernel memory manager during init.
 * Returns 0 on success, negative errno on failure. */
int gergios_hibernate_add_mem_region(phys_bytes start, phys_bytes size);

/* Clear all registered memory regions. */
void gergios_hibernate_clear_mem_regions(void);

/* --- Device State Save/Restore ------------------------------------------ */

/* Save the full PCI config space (256 bytes) for a device.
 * Called during hibernate save.
 * Returns 0 on success, negative errno on failure. */
int gergios_hibernate_save_pci_state(int devind, uint16_t vendor_id,
    uint16_t device_id, uint32_t bus_address);

/* Restore the full PCI config space for a device.
 * Called during hibernate restore.
 * Returns 0 on success, negative errno on failure. */
int gergios_hibernate_restore_pci_state(int devind);

/* Save driver-specific PM state (calls dev->driver->ops.pm->suspend).
 * Called during hibernate for all registered PM devices. */
int gergios_hibernate_save_driver_states(void);

/* Restore driver-specific PM state (calls dev->driver->ops.pm->resume).
 * Called during hibernate restore for all registered PM devices. */
int gergios_hibernate_restore_driver_states(void);

/* --- Main Hibernate operations ------------------------------------------ */

/* Save the hibernation image to the swap device.
 * This is the main save operation, called after all devices are suspended:
 *   1. Write header + PCI state table
 *   2. Write memory image data (all registered regions)
 *   3. Finalise header (checksum, flags)
 * Returns 0 on success, negative errno on failure. */
int gergios_hibernate_save(void);

/* Restore the hibernation image from the swap device.
 * Called during boot to resume from hibernation:
 *   1. Read and validate header
 *   2. Restore memory image
 *   3. Restore PCI device state
 *   4. Resume driver states
 * Returns 0 on success, negative errno on failure. */
int gergios_hibernate_restore(void);

/* Detect if a valid hibernation image exists on the swap device.
 * Scans for the "G4HI" magic signature at sector 0 (or start_sector).
 * Returns 1 if found, 0 if not, negative on error. */
int gergios_hibernate_detect(void);

/* Abort a hibernate operation (clean up temporary state). */
void gergios_hibernate_abort(void);

/* Get a pointer to the last written header (for diagnostics). */
const struct hibernate_header *gergios_hibernate_get_header(void);

/* --- Debug / Diagnostics ------------------------------------------------ */

/* Print the current hibernate state for debugging. */
void gergios_hibernate_dump(void);

/* Returns the total image size estimate in bytes (0 if unknown). */
uint64_t gergios_hibernate_estimate_size(void);

/* --- Memory Page Saving Callback (for page-level save/restore) ---------- */

/* Callback type for processing individual memory pages during save.
 * Called for each page in a memory region.
 * phys_addr: physical address of the page
 * data: pointer to the page data (kernel-mapped virtual address)
 * private: user-supplied pointer from the save call
 * Returns 0 on success, negative to abort. */
typedef int (*hibernate_page_save_cb_t)(uint64_t phys_addr,
    const void *data, void *private);

/* Callback type for processing individual memory pages during restore.
 * Called for each page that needs to be restored.
 * phys_addr: physical address to restore to
 * data: pointer to the restored page data
 * private: user-supplied pointer from the restore call
 * Returns 0 on success, negative to abort. */
typedef int (*hibernate_page_restore_cb_t)(uint64_t phys_addr,
    void *data, void *private);

/* Iterate all registered memory regions and call the save callback
 * for each physical page within them. */
int gergios_hibernate_foreach_page(hibernate_page_save_cb_t cb,
    void *private);

#endif /* _GERGIOS_HIBERNATE_H */
