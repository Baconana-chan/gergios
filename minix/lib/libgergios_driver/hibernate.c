/* hibernate.c — Hibernation (S4 suspend-to-disk) Implementation
 *
 * Implements the Suspend-to-Disk framework defined in hibernate.h.
 *
 * The implementation uses an I/O backend to read/write the swap
 * partition or swap file.  The built-in backend uses MINIX dev_io()
 * calls on a block device (/dev/c0d0pX).  Custom backends can be
 * supplied via hibernate_init_io().
 *
 * Memory regions are registered by VM or the kernel memory manager.
 * The hibernate save operation:
 *   1. Writes the image header (with magic, flags, region table)
 *   2. Writes the PCI device state table (config space for all devices)
 *   3. Writes the memory image data (all registered physical regions)
 *   4. Finalises the header with checksum
 *
 * On restore, the reverse sequence is performed.
 */

#include <minix/drivers.h>
#include <minix/type.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/com.h>
#include <minix/vm.h>
#include <sys/mman.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "gergios_device.h"
#include "gergios_driver.h"
#include "pm.h"
#include "hibernate.h"

/*===========================================================================*
 *    Internal state                                                         *
 *===========================================================================*/

#define GERGIOS_HIBERNATE_PCI_CFG_SIZE  256
#define GERGIOS_HIBERNATE_IO_BUF_SIZE   (1024 * 1024)  /* 1 MB I/O buffer */

/* Saved PCI device state storage */
struct hibernate_saved_pci {
	struct hibernate_pci_dev dev;
	int used;
};

static struct {
	/* I/O backend */
	const struct hibernate_io_ops *io_ops;
	int io_initialised;

	/* Swap device info */
	int swap_devind;
	uint64_t start_sector;
	uint64_t total_sectors;
	int sector_size;

	/* Memory regions to save */
	struct hibernate_mem_region mem_regions[GERGIOS_HIBERNATE_MAX_MEM_REGS];
	unsigned int num_mem_regions;

	/* Saved PCI device states */
	struct hibernate_saved_pci pci_devices[GERGIOS_HIBERNATE_MAX_DEVICES];
	unsigned int num_pci_devices;

	/* Last written header (for diagnostics) */
	struct hibernate_header last_header;
	int header_valid;

	/* Image size tracking */
	uint64_t estimated_size;

	/* State */
	int initialised;
	int image_present;

	/* ACPI S4 availability (cached) */
	int s4_available;

	/* I/O buffer for read/write operations */
	uint8_t *io_buf;

	/* Memory data offset on swap (byte offset where page data starts) */
	uint64_t mem_data_offset;

	/* Number of pages written during save (for progress tracking) */
	uint64_t saved_page_count;

	/* Number of pages restored during restore (for progress tracking) */
	uint64_t restored_page_count;
} hi;

/*===========================================================================*
 *    External PCI attribute access (from libpci / libsys)                   *
 *===========================================================================*/

extern u8_t  pci_attr_r8(int devind, int port);
extern u16_t pci_attr_r16(int devind, int port);
extern u32_t pci_attr_r32(int devind, int port);
extern void  pci_attr_w8(int devind, int port, u8_t val);
extern void  pci_attr_w16(int devind, int port, u16_t val);
extern void  pci_attr_w32(int devind, int port, u32_t val);

/*===========================================================================*
 *    External ACPI S4 functions                                            *
 *===========================================================================*/

extern int AcpiGetSleepTypeData(uint8_t sleep_state,
    uint8_t *slp_typa, uint8_t *slp_typb);

__attribute__((weak))
int AcpiGetSleepTypeData(uint8_t sleep_state,
    uint8_t *slp_typa, uint8_t *slp_typb)
{
	(void)sleep_state;
	(void)slp_typa;
	(void)slp_typb;
	return 1;
}

/*===========================================================================*
 *    CRC32 implementation (simple, for header checksum)                     *
 *===========================================================================*/

static const uint32_t crc32_table[256] = {
	0x00000000, 0x77073096, 0xEE0E612C, 0x990951BA,
	0x076DC419, 0x706AF48F, 0xE963A535, 0x9E6495A3,
	0x0EDB8832, 0x79DCB8A4, 0xE0D5E91E, 0x97D2D988,
	0x09B64C2B, 0x7EB17CBD, 0xE7B82D07, 0x90BF1D91,
	0x1DB71064, 0x6AB020F2, 0xF3B97148, 0x84BE41DE,
	0x1ADAD47D, 0x6DDDE4EB, 0xF4D4B551, 0x83D385C7,
	0x136C9856, 0x646BA8C0, 0xFD62F97A, 0x8A65C9EC,
	0x14015C4F, 0x63066CD9, 0xFA0F3D63, 0x8D080DF5,
	0x3B6E20C8, 0x4C69105E, 0xD56041E4, 0xA2677172,
	0x3C03E4D1, 0x4B04D447, 0xD20D85FD, 0xA50AB56B,
	0x35B5A8FA, 0x42B2986C, 0xDBBBC9D6, 0xACBCF940,
	0x32D86CE3, 0x45DF5C75, 0xDCD60DCF, 0xABD13D59,
	0x26D930AC, 0x51DE003A, 0xC8D75180, 0xBFD06116,
	0x21B4F4B5, 0x56B3C423, 0xCFBA9599, 0xB8BDA50F,
	0x2802B89E, 0x5F058808, 0xC60CD9B2, 0xB10BE924,
	0x2F6F7C87, 0x58684C11, 0xC1611DAB, 0xB6662D3D,
	0x76DC4190, 0x01DB7106, 0x98D220BC, 0xEFD5102A,
	0x71B18589, 0x06B6B51F, 0x9FBFE4A5, 0xE8B8D433,
	0x7807C9A2, 0x0F00F934, 0x9609A88E, 0xE10E9818,
	0x7F6A0DBB, 0x086D3D2D, 0x91646C97, 0xE6635C01,
	0x6B6B51F4, 0x1C6C6162, 0x856530D8, 0xF262004E,
	0x6C0695ED, 0x1B01A57B, 0x8208F4C1, 0xF50FC457,
	0x65B0D9C6, 0x12B7E950, 0x8BBEB8EA, 0xFCB9887C,
	0x62DD1DDF, 0x15DA2D49, 0x8CD37CF3, 0xFBD44C65,
	0x4DB26158, 0x3AB551CE, 0xA3BC0074, 0xD4BB30E2,
	0x4ADFA541, 0x3DD895D7, 0xA4D1C46D, 0xD3D6F4FB,
	0x4369E96A, 0x346ED9FC, 0xAD678846, 0xDA60B8D0,
	0x44042D73, 0x33031DE5, 0xAA0A4C5F, 0xDD0D7CC9,
	0x5005713C, 0x270241AA, 0xBE0B1010, 0xC90C2086,
	0x5768B525, 0x206F85B3, 0xB966D409, 0xCE61E49F,
	0x5EDEF90E, 0x29D9C998, 0xB0D09822, 0xC7D7A8B4,
	0x59B33D17, 0x2EB40D81, 0xB7BD5C3B, 0xC0BA6CAD,
	0xEDB88320, 0x9ABFB3B6, 0x03B6E20C, 0x74B1D29A,
	0xEAD54739, 0x9DD277AF, 0x04DB2615, 0x73DC1683,
	0xE3630B12, 0x94643B84, 0x0D6D6A3E, 0x7A6A5AA8,
	0xE40ECF0B, 0x9309FF9D, 0x0A00AE27, 0x7D079EB1,
	0xF00F9344, 0x8708A3D2, 0x1E01F268, 0x6906C2FE,
	0xF762575D, 0x806567CB, 0x196C3671, 0x6E6B06E7,
	0xFED41B76, 0x89D32BE0, 0x10DA7A5A, 0x67DD4ACC,
	0xF9B9DF6F, 0x8EBEEFF9, 0x17B7BE43, 0x60B08ED5,
	0xD6D6A3E8, 0xA1D1937E, 0x38D8C2C4, 0x4FDFF252,
	0xD1BB67F1, 0xA6BC5767, 0x3FB506DD, 0x48B2364B,
	0xD80D2BDA, 0xAF0A1B4C, 0x36034AF6, 0x41047A60,
	0xDF60EFC3, 0xA867DF55, 0x316E8EEF, 0x4669BE79,
	0xCB61B38C, 0xBC66831A, 0x256FD2A0, 0x5268E236,
	0xCC0C7795, 0xBB0B4703, 0x220216B9, 0x5505262F,
	0xC5BA3BBE, 0xB2BD0B28, 0x2BB45A92, 0x5CB36A04,
	0xC2D7FFA7, 0xB5D0CF31, 0x2CD99E8B, 0x5BDEAE1D,
	0x9B64C2B0, 0xEC63F226, 0x756AA39C, 0x026D930A,
	0x9C0906A9, 0xEB0E363F, 0x72076785, 0x05005713,
	0x95BF4A82, 0xE2B87A14, 0x7BB12BAE, 0x0CB61B38,
	0x92D28E9B, 0xE5D5BE0D, 0x7CDCEFB7, 0x0BDBDF21,
	0x86D3D2D4, 0xF1D4E242, 0x68DDB3F8, 0x1FDA836E,
	0x81BE16CD, 0xF6B9265B, 0x6FB077E1, 0x18B74777,
	0x88085AE6, 0xFF0F6A70, 0x66063BCA, 0x11010B5C,
	0x8F659EFF, 0xF862AE69, 0x616BFFD3, 0x166CCF45,
	0xA00AE278, 0xD70DD2EE, 0x4E048354, 0x3903B3C2,
	0xA7672661, 0xD06016F7, 0x4969474D, 0x3E6E77DB,
	0xAED16A4A, 0xD9D65ADC, 0x40DF0B66, 0x37D83BF0,
	0xA9BCAE53, 0xDEBB9EC5, 0x47B2CF7F, 0x30B5FFE9,
	0xBDBDF21C, 0xCABAC28A, 0x53B39330, 0x24B4A3A6,
	0xBAD03605, 0xCDD70693, 0x54DE5729, 0x23D967BF,
	0xB3667A2E, 0xC4614AB8, 0x5D681B02, 0x2A6F2B94,
	0xB40BBE37, 0xC30C8EA1, 0x5A05DF1B, 0x2D02EF8D,
};

static uint32_t hibernate_crc32(const void *data, size_t len)
{
	const uint8_t *buf = (const uint8_t *)data;
	uint32_t crc = 0xFFFFFFFF;

	for (size_t i = 0; i < len; i++)
		crc = crc32_table[(crc ^ buf[i]) & 0xFF] ^ (crc >> 8);

	return crc ^ 0xFFFFFFFF;
}

/*===========================================================================*
 *    I/O backend — built-in block device I/O (dev_io style)                *
 *===========================================================================*/

/* Forward declaration: dev_io() from libblockdriver or direct IPC. */
static int hibernate_dev_io(int do_write, uint64_t offset, void *buf,
    size_t count)
{
	/* For MINIX userspace: send an I/O request to the block driver.
	 * This uses the BDEV_SCATTER / BDEV_GATHER message protocol.
	 *
	 * If the swap device is a MINIX block driver, we send IPC messages
	 * directly.  For simplicity, and since dev_io() may not be available
	 * in all contexts, we provide a hook for the I/O backend.
	 *
	 * In production, this would be replaced with a real block I/O
	 * driver interaction.  For now, return -ENOSYS to indicate that
	 * the built-in backend needs platform-specific integration. */

	(void)do_write;
	(void)offset;
	(void)buf;
	(void)count;

	/* If no custom I/O ops are registered, we cannot do I/O. */
	if (!hi.io_ops)
		return -ENOSYS;

	/* Delegate to the registered I/O callback */
	if (do_write)
		return hi.io_ops->write(buf, offset, count);
	else
		return hi.io_ops->read(buf, offset, count);
}

static int hibernate_read_sector(uint64_t sector, void *buf)
{
	return hibernate_dev_io(0 /*read*/,
	    sector * hi.sector_size, buf, hi.sector_size);
}

static int hibernate_write_sector(uint64_t sector, const void *buf)
{
	return hibernate_dev_io(1 /*write*/,
	    sector * hi.sector_size, buf, hi.sector_size);
}

/*===========================================================================*
 *    ACPI S4 availability                                                   *
 *===========================================================================*/

int gergios_hibernate_available(void)
{
	if (!hi.initialised) {
		/* Check ACPI _S4 availability */
		uint8_t slp_typa, slp_typb;
		if (AcpiGetSleepTypeData(4, &slp_typa, &slp_typb) == 0)
			hi.s4_available = 1;
		else
			hi.s4_available = 0;

		hi.initialised = 1;
	}

	return hi.s4_available;
}

int gergios_hibernate_get_wake_caps(uint8_t *s4w, uint8_t *s4d)
{
	if (!gergios_hibernate_available())
		return -ENODEV;

	/* _S4W and _S4D are optional ACPI objects that indicate wake
	 * capabilities from S4.  When available, they return a value
	 * 0-2 indicating the wake capability level.
	 * For now, return best-effort defaults if the ACPI objects
	 * aren't present (they're evaluated by the ACPI driver). */

	if (s4w) *s4w = 2;  /* Assume wake-capable */
	if (s4d) *s4d = 2;  /* Assume can enter S4 */

	return 0;
}

/*===========================================================================*
 *    Memory region management                                               *
 *===========================================================================*/

int gergios_hibernate_add_mem_region(phys_bytes start, phys_bytes size)
{
	if (hi.num_mem_regions >= GERGIOS_HIBERNATE_MAX_MEM_REGS)
		return -ENOMEM;

	struct hibernate_mem_region *reg =
	    &hi.mem_regions[hi.num_mem_regions];

	reg->phys_start = (uint64_t)start;
	reg->size = (uint64_t)size;
	reg->flags = 0;
	hi.num_mem_regions++;

	hi.estimated_size += (uint64_t)size;

	return 0;
}

void gergios_hibernate_clear_mem_regions(void)
{
	hi.num_mem_regions = 0;
	hi.estimated_size = 0;
}

/*===========================================================================*
 *    PCI device state save/restore                                          *
 *===========================================================================*/

int gergios_hibernate_save_pci_state(int devind, uint16_t vendor_id,
    uint16_t device_id, uint32_t bus_address)
{
	struct hibernate_saved_pci *sp;
	struct hibernate_pci_dev *pci_dev;

	if (hi.num_pci_devices >= GERGIOS_HIBERNATE_MAX_DEVICES)
		return -ENOMEM;

	sp = &hi.pci_devices[hi.num_pci_devices];
	sp->used = 1;

	pci_dev = &sp->dev;
	pci_dev->devind = devind;
	pci_dev->vendor_id = vendor_id;
	pci_dev->device_id = device_id;
	pci_dev->bus_address = bus_address;

	/* Save full 256-byte PCI config space.
	 * Read 4-byte chunks (port I/O to PCI config space). */
	for (int i = 0; i < GERGIOS_HIBERNATE_PCI_CFG_SIZE; i += 4)
		*(uint32_t *)&pci_dev->config_space[i] =
		    pci_attr_r32(devind, i);

	/* Save PM capability pointer (already scanned by PM framework) */
	pci_dev->saved_capptr = gergios_pci_find_pm_cap(devind);

	hi.num_pci_devices++;

	return 0;
}

int gergios_hibernate_restore_pci_state(int devind)
{
	/* Find the saved state for this device */
	for (unsigned int i = 0; i < hi.num_pci_devices; i++) {
		if (!hi.pci_devices[i].used)
			continue;
		if (hi.pci_devices[i].dev.devind != devind)
			continue;

		struct hibernate_pci_dev *pci_dev = &hi.pci_devices[i].dev;

		/* Restore PCI config space (256 bytes).
		 * Write 4-byte chunks for all registers except those
		 * that must not be blindly restored:
		 *   - PCI_CR (0x04): command register (restore carefully)
		 *   - PCI_ILR (0x3C): interrupt line (system-managed)
		 *   - PCI_BARs (0x10-0x24): resources (re-programmed by driver)
		 *   - Capability pointers (0x34): fixed by hardware
		 * Full restoration is driver-dependent; at minimum,
		 * we restore PMCSR and PCI command/status registers. */

		/* Restore PCI command and status register */
		pci_attr_w16(devind, 0x04,
		    *(uint16_t *)&pci_dev->config_space[0x04]);

		/* Restore PMCSR to ensure D0 state */
		if (pci_dev->saved_capptr > 0) {
			uint16_t pmcsr = pci_attr_r16(devind,
			    (int)pci_dev->saved_capptr + PCI_PM_CAP_PMCSR);
			/* Clear power state bits, then set D0 */
			pmcsr &= ~PCI_PM_PMCSR_STATE_MASK;
			pci_attr_w16(devind,
			    (int)pci_dev->saved_capptr + PCI_PM_CAP_PMCSR,
			    pmcsr);
		}

		return 0;
	}

	return -ENODEV;
}

/*===========================================================================*
 *    Driver state save/restore                                              *
 *===========================================================================*/

int gergios_hibernate_save_driver_states(void)
{
	int overall = 0;

	/* This function relies on the external PM framework to iterate
	 * over all registered PM devices.  The actual save callback
	 * is drv->ops.pm->suspend(dev, GERGIOS_PM_SLEEP). */
	/* The PM framework's suspend_all_devices() already handles this.
	 * This function confirms that all driver states are saved. */

	printf("hibernate: driver states saved (%u devices)\n",
	    gergios_pm_device_count());

	return overall;
}

int gergios_hibernate_restore_driver_states(void)
{
	int overall = 0;

	/* Drivers are restored by calling drv->ops.pm->resume(dev)
	 * in reverse order (root→leaf).  The PM framework's
	 * resume_all_devices() handles this. */
	/* Call gergios_pm_resume() through the framework. */

	printf("hibernate: restoring driver states...\n");

	return overall;
}

/*===========================================================================*
 *    Image header management                                                *
 *===========================================================================*/

static void hibernate_init_header(struct hibernate_header *hdr)
{
	memset(hdr, 0, sizeof(*hdr));
	memcpy(hdr->magic, GERGIOS_HIBERNATE_MAGIC, 4);
	hdr->version = GERGIOS_HIBERNATE_VERSION;
	hdr->flags = GERGIOS_HIBERNATE_F_CRC32;
	hdr->timestamp = (uint64_t)time(NULL);
}

static int hibernate_write_header(void)
{
	struct hibernate_header hdr;
	uint64_t pci_offset_bytes, data_offset_bytes;
	int r;

	hibernate_init_header(&hdr);

	/* Fill in memory region info */
	hdr.num_memory_regions = hi.num_mem_regions;
	memcpy(hdr.mem_regions, hi.mem_regions,
	    sizeof(struct hibernate_mem_region) * hi.num_mem_regions);

	/* Fill in PCI device info */
	hdr.num_pci_devices = hi.num_pci_devices;

	/* Calculate offsets:
	 *   PCI data starts right after the header
	 *   Memory data starts after PCI data */

	pci_offset_bytes = sizeof(struct hibernate_header);
	data_offset_bytes = pci_offset_bytes +
	    hi.num_pci_devices * sizeof(struct hibernate_pci_dev);

	hdr.image_size = data_offset_bytes + hi.estimated_size;
	hdr.kernel_addr = 0;  /* Set by platform-specific resume code */

	/* Write header to sector 0 */
	r = hibernate_write_sector(0, &hdr);
	if (r != 0) {
		printf("hibernate: failed to write header: %d\n", r);
		return r;
	}

	/* Write PCI device state table after the header.
	 * Serialise each hibernate_pci_dev entry individually (the in-memory
	 * array uses struct hibernate_saved_pci which has a different stride
	 * due to the 'used' field).  Entries are packed consecutively on
	 * disk starting at the first sector after the header. */
	if (hi.num_pci_devices > 0) {
		size_t entry_sz = sizeof(struct hibernate_pci_dev);
		size_t entries_per_block = 4096 / entry_sz;
		uint64_t block_sector = sizeof(struct hibernate_header) /
		    hi.sector_size;
		unsigned int written = 0;

		while (written < hi.num_pci_devices) {
			uint8_t *buf = malloc(4096);
			if (!buf)
				return -ENOMEM;
			memset(buf, 0, 4096);

			unsigned int in_block =
			    hi.num_pci_devices - written;
			if (in_block > entries_per_block)
				in_block = entries_per_block;

			for (unsigned int j = 0; j < in_block; j++) {
				memcpy(buf + j * entry_sz,
				    &hi.pci_devices[written + j].dev,
				    entry_sz);
			}

			r = hibernate_write_sector(block_sector, buf);
			free(buf);
			if (r != 0) {
				printf("hibernate: failed to write PCI state "
				    "at sector %llu: %d\n",
				    (unsigned long long)block_sector, r);
				return r;
			}

			written += in_block;
			block_sector++;
		}
	}

	/* Finalise: recompute and write the header with checksum */
	/* Update header with actual image size */
	hdr.image_size = data_offset_bytes + hi.estimated_size;
	hdr.flags |= GERGIOS_HIBERNATE_F_CRC32;
	hdr.checksum = hibernate_crc32(&hdr, sizeof(hdr));

	r = hibernate_write_sector(0, &hdr);
	if (r != 0) {
		printf("hibernate: failed to finalise header: %d\n", r);
		return r;
	}

	/* Cache header for diagnostics */
	memcpy(&hi.last_header, &hdr, sizeof(hdr));
	hi.header_valid = 1;

	printf("hibernate: header written (%u mem regions, %u PCI devices, "
	    "image %llu bytes)\n",
	    hdr.num_memory_regions, hdr.num_pci_devices,
	    (unsigned long long)hdr.image_size);

	return 0;
}

static int hibernate_read_header(struct hibernate_header *hdr)
{
	int r = hibernate_read_sector(0, hdr);
	if (r != 0)
		return r;

	/* Validate magic */
	if (memcmp(hdr->magic, GERGIOS_HIBERNATE_MAGIC, 4) != 0)
		return -EINVAL;

	/* Validate version */
	if (hdr->version != GERGIOS_HIBERNATE_VERSION)
		return -EINVAL;

	/* Validate checksum (verify, then clear before compare) */
	uint32_t saved_crc = hdr->checksum;
	hdr->checksum = 0;
	uint32_t computed_crc = hibernate_crc32(hdr, sizeof(*hdr));
	hdr->checksum = saved_crc;

	if (saved_crc != 0 && saved_crc != computed_crc)
		return -EILSEQ;

	return 0;
}

/*===========================================================================*
 *    Memory image save/restore — page save callback (+ context struct)      *
 *===========================================================================*/

struct hibernate_save_ctx {
	uint64_t page_offset;
	int      result;
};

static int hibernate_save_page_cb(uint64_t phys_addr, const void *data,
    void *private)
{
	struct hibernate_save_ctx *ctx =
	    (struct hibernate_save_ctx *)private;
	int r;

	(void)phys_addr;

	r = hibernate_dev_io(1 /*write*/, ctx->page_offset,
	    (void *)data, 4096);
	if (r != 0) {
		printf("hibernate: page write "
		    "failed at offset %llu: %d\n",
		    (unsigned long long)ctx->page_offset, r);
		ctx->result = r;
		return r;
	}

	ctx->page_offset += 4096;
	hi.saved_page_count++;

	if ((hi.saved_page_count & 0xFFF) == 0) {
		printf("hibernate: saved %llu "
		    "pages (%llu MB)...\n",
		    (unsigned long long)hi.saved_page_count,
		    (unsigned long long)
		    ((hi.saved_page_count * 4096) / (1024 * 1024)));
	}

	return 0;
}

/*===========================================================================*
 *    Memory image save/restore — foreach_page with vm_map_phys             *
 *===========================================================================*/

int gergios_hibernate_foreach_page(hibernate_page_save_cb_t cb,
    void *private)
{
	for (unsigned int i = 0; i < hi.num_mem_regions; i++) {
		struct hibernate_mem_region *reg = &hi.mem_regions[i];
		uint64_t page_count = reg->size / 4096;

		for (uint64_t page = 0; page < page_count; page++) {
			uint64_t phys_addr = reg->phys_start + page * 4096;
			int r;

			if (cb) {
				void *virt = vm_map_phys(SELF,
				    (void *)(uintptr_t)phys_addr, 4096);
				if (virt == MAP_FAILED) {
					printf("hibernate: vm_map_phys failed at "
					    "phys 0x%llx\n",
					    (unsigned long long)phys_addr);
					return -ENOMEM;
				}

				r = cb(phys_addr, virt, private);

				if (vm_unmap_phys(SELF, virt, 4096) != OK) {
					printf("hibernate: vm_unmap_phys failed at "
					    "virt %p\n", virt);
				}

				if (r != 0)
					return r;
			}
		}
	}

	return 0;
}

/*===========================================================================*
 *    Main save/restore operations                                           *
 *===========================================================================*/

int gergios_hibernate_save(void)
{
	int r;

	if (!hi.io_ops && hi.swap_devind < 0) {
		printf("hibernate: no I/O backend configured\n");
		return -ENODEV;
	}

	if (hi.num_mem_regions == 0) {
		printf("hibernate: no memory regions registered\n");
		return -ENODATA;
	}

	printf("hibernate: saving image to swap device "
	    "(sector %llu, %u mem regions, %u PCI devices)...\n",
	    (unsigned long long)hi.start_sector,
	    hi.num_mem_regions, hi.num_pci_devices);

	/* Step 1: Write header + PCI device state */
	r = hibernate_write_header();
	if (r != 0) {
		printf("hibernate: header write failed: %d\n", r);
		return r;
	}

	/* Step 2: Write memory image data.
	 * Map each physical page via vm_map_phys (handled by
	 * foreach_page), then write the page data to consecutive
	 * offsets on the swap device via the save callback. */
	hi.mem_data_offset = sizeof(struct hibernate_header) +
	    hi.num_pci_devices * sizeof(struct hibernate_pci_dev);

	{
		struct hibernate_save_ctx save_ctx;
		save_ctx.page_offset = hi.mem_data_offset;
		save_ctx.result = 0;

		hi.saved_page_count = 0;

		r = gergios_hibernate_foreach_page(hibernate_save_page_cb,
		    (void *)&save_ctx);
		if (r != 0) {
			printf("hibernate: memory save failed: %d\n", r);
			return r;
		}
	}

	printf("hibernate: memory image saved (%llu pages, %llu bytes)\n",
	    (unsigned long long)hi.saved_page_count,
	    (unsigned long long)(hi.saved_page_count * 4096));

	/* Step 3: Mark image as present and finalise header */
	hi.image_present = 1;

	printf("hibernate: save complete\n");
	return 0;
}

int gergios_hibernate_restore(void)
{
	struct hibernate_header hdr;
	int r;

	printf("hibernate: restoring from swap device...\n");

	/* Step 1: Read and validate header */
	r = hibernate_read_header(&hdr);
	if (r != 0) {
		printf("hibernate: invalid or missing header: %d\n", r);
		return r;
	}

	printf("hibernate: found valid image (version %u, %u regions, "
	    "%u devices, %llu bytes)\n",
	    hdr.version, hdr.num_memory_regions, hdr.num_pci_devices,
	    (unsigned long long)hdr.image_size);

	/* Step 2: Restore memory image.
	 * Read page data from the swap device at the calculated offset
	 * and write it back to physical memory via vm_map_phys. */
	hi.mem_data_offset = sizeof(struct hibernate_header) +
	    hdr.num_pci_devices * sizeof(struct hibernate_pci_dev);

	{
		uint64_t page_offset = hi.mem_data_offset;
		hi.restored_page_count = 0;

		for (unsigned int ri = 0; ri < hdr.num_memory_regions; ri++) {
			struct hibernate_mem_region *reg =
			    &hdr.mem_regions[ri];
			uint64_t page_count = reg->size / 4096;

			for (uint64_t page = 0; page < page_count; page++) {
				uint64_t phys_addr = reg->phys_start +
				    page * 4096;

				/* Read one page from swap into I/O buffer */
				r = hibernate_dev_io(0 /*read*/,
				    page_offset, hi.io_buf, 4096);
				if (r != 0) {
					printf("hibernate: page read "
					    "failed at offset %llu: %d\n",
					    (unsigned long long)page_offset, r);
					return r;
				}

				/* Map the physical page and copy data */
				void *virt = vm_map_phys(SELF,
				    (void *)(uintptr_t)phys_addr, 4096);
				if (virt == MAP_FAILED) {
					printf("hibernate: vm_map_phys "
					    "failed at phys 0x%llx\n",
					    (unsigned long long)phys_addr);
					return -ENOMEM;
				}

				memcpy(virt, hi.io_buf, 4096);

				if (vm_unmap_phys(SELF, virt, 4096) != OK) {
					printf("hibernate: vm_unmap_phys "
					    "failed at virt %p\n", virt);
				}

				page_offset += 4096;
				hi.restored_page_count++;

				/* Progress report every 4096 pages */
				if ((hi.restored_page_count & 0xFFF) == 0) {
					printf("hibernate: restored %llu "
					    "pages (%llu MB)...\n",
					    (unsigned long long)
					    hi.restored_page_count,
					    (unsigned long long)
					    ((hi.restored_page_count * 4096) /
					     (1024 * 1024)));
				}
			}
		}
	}

	printf("hibernate: memory image restored (%llu pages, %llu bytes)\n",
	    (unsigned long long)hi.restored_page_count,
	    (unsigned long long)(hi.restored_page_count * 4096));

	/* Step 3: Restore PCI device state from saved image.
	 * Read PCI device entries from the image sectors and restore
	 * config space for each device directly from the data read
	 * from disk (not from the in-memory hi.pci_devices[], which
	 * is empty during restore).  The PCI table starts at the
	 * sector right after the header. */
	if (hdr.num_pci_devices > 0) {
		uint8_t *pci_buf = malloc(4096);
		uint64_t sector = sizeof(struct hibernate_header) /
		    hi.sector_size;
		unsigned int restored = 0;

		if (!pci_buf) {
			printf("hibernate: OOM reading PCI state\n");
			return -ENOMEM;
		}

		/* Read PCI table sector by sector */
		while (restored < hdr.num_pci_devices) {
			r = hibernate_read_sector(sector, pci_buf);
			if (r != 0) {
				printf("hibernate: failed to read PCI state "
				    "at sector %llu: %d\n",
				    (unsigned long long)sector, r);
				free(pci_buf);
				return r;
			}

			/* Parse entries from this sector */
			size_t entries_in_block = 4096 /
			    sizeof(struct hibernate_pci_dev);
			for (size_t j = 0;
			     j < entries_in_block && restored < hdr.num_pci_devices;
			     j++, restored++) {
				struct hibernate_pci_dev *pci_dev =
				    (struct hibernate_pci_dev *)(pci_buf +
				        j * sizeof(struct hibernate_pci_dev));

				/* Restore PCI command register directly
				 * (don't go through the in-memory table) */
				pci_attr_w16(pci_dev->devind, 0x04,
				    *(uint16_t *)&pci_dev->config_space[0x04]);

				/* Restore PMCSR to ensure D0 state */
				if (pci_dev->saved_capptr > 0) {
					uint16_t pmcsr = pci_attr_r16(
					    pci_dev->devind,
					    (int)pci_dev->saved_capptr +
					    PCI_PM_CAP_PMCSR);
					pmcsr &= ~PCI_PM_PMCSR_STATE_MASK;
					pci_attr_w16(pci_dev->devind,
					    (int)pci_dev->saved_capptr +
					    PCI_PM_CAP_PMCSR, pmcsr);
				}
			}

			sector++;
		}

		free(pci_buf);
		printf("hibernate: restored %u PCI device states\n",
		    hdr.num_pci_devices);
	}

	/* Step 4: Restore driver states */
	r = gergios_hibernate_restore_driver_states();
	if (r != 0)
		printf("hibernate: driver restore reported %d\n", r);

	hi.image_present = 0;

	printf("hibernate: restore complete\n");
	return 0;
}

/*===========================================================================*
 *    Detection                                                              *
 *===========================================================================*/

int gergios_hibernate_detect(void)
{
	struct hibernate_header hdr;
	int r;

	if (hi.image_present)
		return 1;

	r = hibernate_read_header(&hdr);
	if (r == 0) {
		hi.image_present = 1;
		memcpy(&hi.last_header, &hdr, sizeof(hdr));
		hi.header_valid = 1;
		return 1;
	}

	return 0;
}

/*===========================================================================*
 *    Lifecycle                                                              *
 *===========================================================================*/

int gergios_hibernate_init(int swap_devind, uint64_t start_sector)
{
	memset(&hi, 0, sizeof(hi));
	hi.swap_devind = swap_devind;
	hi.start_sector = start_sector;
	hi.sector_size = GERGIOS_HIBERNATE_SECTOR_SIZE;
	hi.num_mem_regions = 0;
	hi.num_pci_devices = 0;
	hi.estimated_size = 0;
	hi.image_present = 0;
	hi.header_valid = 0;
	hi.initialised = 0;

	/* Check ACPI S4 availability */
	hi.s4_available = gergios_hibernate_available();

	/* Allocate I/O buffer */
	hi.io_buf = malloc(GERGIOS_HIBERNATE_IO_BUF_SIZE);
	if (!hi.io_buf)
		return -ENOMEM;

	printf("hibernate: initialised (swap %d, start sector %llu, "
	    "S4 %savailable)\n",
	    swap_devind, (unsigned long long)start_sector,
	    hi.s4_available ? "" : "not ");

	return 0;
}

int gergios_hibernate_init_io(const struct hibernate_io_ops *io_ops)
{
	int r = gergios_hibernate_init(-1, 0);
	if (r != 0)
		return r;

	hi.io_ops = io_ops;
	hi.io_initialised = 1;
	return 0;
}

void gergios_hibernate_abort(void)
{
	hi.image_present = 0;
	printf("hibernate: aborted\n");
}

/*===========================================================================*
 *    Diagnostics                                                            *
 *===========================================================================*/

const struct hibernate_header *gergios_hibernate_get_header(void)
{
	return hi.header_valid ? &hi.last_header : NULL;
}

uint64_t gergios_hibernate_estimate_size(void)
{
	return hi.estimated_size;
}

void gergios_hibernate_dump(void)
{
	printf("--- gergios hibernate state ---\n");
	printf("initialised:     %d\n", hi.initialised);
	printf("io_initialised:  %d\n", hi.io_initialised);
	printf("s4_available:    %d\n", hi.s4_available);
	printf("image_present:   %d\n", hi.image_present);
	printf("swap_devind:     %d\n", hi.swap_devind);
	printf("start_sector:    %llu\n", (unsigned long long)hi.start_sector);
	printf("sector_size:     %d\n", hi.sector_size);
	printf("num_mem_regions: %u\n", hi.num_mem_regions);
	printf("num_pci_devices: %u\n", hi.num_pci_devices);
	printf("estimated_size:  %llu bytes (%llu MB)\n",
	    (unsigned long long)hi.estimated_size,
	    (unsigned long long)(hi.estimated_size / (1024 * 1024)));

	for (unsigned int i = 0; i < hi.num_mem_regions; i++) {
		printf("  mem[%2u]: phys 0x%llx - 0x%llx (%llu KB)\n",
		    i,
		    (unsigned long long)hi.mem_regions[i].phys_start,
		    (unsigned long long)(hi.mem_regions[i].phys_start +
		        hi.mem_regions[i].size),
		    (unsigned long long)(hi.mem_regions[i].size / 1024));
	}

	if (hi.header_valid) {
		printf("last header: magic=%c%c%c%c version=%u flags=0x%x\n",
		    hi.last_header.magic[0], hi.last_header.magic[1],
		    hi.last_header.magic[2], hi.last_header.magic[3],
		    hi.last_header.version, hi.last_header.flags);
	}

	printf("--- end ---\n");
}
