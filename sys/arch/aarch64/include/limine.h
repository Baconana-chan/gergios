/* Limine Boot Protocol definitions for GergiOS AArch64.
 *
 * This is a minimum set of Limine protocol structures needed for
 * booting the kernel on AArch64. Based on the Limine Boot Protocol v8.x.
 *
 * Reference: https://github.com/limine-bootloader/limine-protocol
 *
 * The protocol uses a request/response mechanism:
 *   1. Kernel defines global volatile request structures with magic IDs
 *   2. Bootloader scans the ELF for these magic IDs
 *   3. Bootloader populates the response pointer in each request
 *   4. Kernel reads the response after boot
 *
 * Request structures must be placed in the ".limine_requests" section
 * so the bootloader can find them.
 *
 * On Limine AAC64:
 *   - Entry at EL1t with MMU enabled (higher-half page tables)
 *   - All registers zeroed except SP (bootloader stack, >= 64KB)
 *   - VBAR_EL1 undefined (kernel must set up its own)
 *   - Boot info via request/response structures only
 *   - DTB provided via LIMINE_DTB request (not in x0)
 */

#ifndef _LIMINE_H_
#define _LIMINE_H_

#include <stdint.h>

/* =========================================================================
 * Base revision mechanism
 * =========================================================================
 */

#define LIMINE_BASE_REVISION_ID \
    { 0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 0, 0 }

struct limine_base_revision {
    uint64_t id[4];
    uint64_t revision;
};

#define LIMINE_BASE_REVISION(N) \
    __attribute__((used, section(".limine_requests"))) \
    static volatile struct limine_base_revision _limine_base_revision = { \
        .id = LIMINE_BASE_REVISION_ID, \
        .revision = (N) \
    }

/* =========================================================================
 * Bootloader info
 * =========================================================================
 */

#define LIMINE_BOOTLOADER_INFO_REQUEST \
    { 0xf55038d8e2a1202f, 0x279426fcf5f59740, 0, 0 }
#define LIMINE_BOOTLOADER_INFO_RESPONSE \
    { 0xf55038d8e2a1202f, 0x279426fcf5f59740, 0, 0 }

struct limine_bootloader_info_response {
    char *name;
    char *version;
};

struct limine_bootloader_info_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_bootloader_info_response *response;
};

/* =========================================================================
 * Memory map
 * =========================================================================
 */

#define LIMINE_MEMMAP_REQUEST \
    { 0x67cf3d9d378a806f, 0xe304acdfc50c3c62, 0, 0 }
#define LIMINE_MEMMAP_RESPONSE \
    { 0x67cf3d9d378a806f, 0xe304acdfc50c3c62, 0, 0 }

/* Memory map entry types */
#define LIMINE_MEMMAP_USABLE                 0
#define LIMINE_MEMMAP_RESERVED               1
#define LIMINE_MEMMAP_ACPI_RECLAIMABLE       2
#define LIMINE_MEMMAP_ACPI_NVS               3
#define LIMINE_MEMMAP_BAD_MEMORY             4
#define LIMINE_MEMMAP_BOOTLOADER_RECLAIMABLE 5
#define LIMINE_MEMMAP_KERNEL_AND_MODULES     6
#define LIMINE_MEMMAP_FRAMEBUFFER            7

struct limine_memmap_entry {
    uint64_t base;
    uint64_t length;
    uint64_t type;
};

struct limine_memmap_response {
    uint64_t entry_count;
    struct limine_memmap_entry *entries[];
};

struct limine_memmap_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_memmap_response *response;
};

/* =========================================================================
 * Boot time
 * =========================================================================
 */

#define LIMINE_BOOT_TIME_REQUEST \
    { 0x502746e184c088aa, 0xfbc5ec83e6327893, 0, 0 }
#define LIMINE_BOOT_TIME_RESPONSE \
    { 0x502746e184c088aa, 0xfbc5ec83e6327893, 0, 0 }

struct limine_boot_time_response {
    int64_t boot_time;
};

struct limine_boot_time_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_boot_time_response *response;
};

/* =========================================================================
 * Kernel address
 * =========================================================================
 */

#define LIMINE_KERNEL_ADDRESS_REQUEST \
    { 0x71ba76863cc55f63, 0xb2644a48c516a487, 0, 0 }
#define LIMINE_KERNEL_ADDRESS_RESPONSE \
    { 0x71ba76863cc55f63, 0xb2644a48c516a487, 0, 0 }

struct limine_kernel_address_response {
    uint64_t physical_base;
    uint64_t virtual_base;
};

struct limine_kernel_address_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_kernel_address_response *response;
};

/* =========================================================================
 * HHDM (Higher Half Direct Map)
 * =========================================================================
 */

#define LIMINE_HHDM_REQUEST \
    { 0x48dcf1cb8ad2b852, 0x63984e959a98244b, 0, 0 }
#define LIMINE_HHDM_RESPONSE \
    { 0x48dcf1cb8ad2b852, 0x63984e959a98244b, 0, 0 }

struct limine_hhdm_response {
    uint64_t offset;
};

struct limine_hhdm_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_hhdm_response *response;
};

/* =========================================================================
 * Modules
 * =========================================================================
 */

#define LIMINE_MODULE_REQUEST \
    { 0x3e7e279702be32af, 0xca1c4f3bd1280cee, 0, 0 }
#define LIMINE_MODULE_RESPONSE \
    { 0x3e7e279702be32af, 0xca1c4f3bd1280cee, 0, 0 }

struct limine_file {
    uint64_t address;
    uint64_t size;
    char *path;
    char *cmdline;
    uint64_t media_size;
    uint64_t unused[6];
};

struct limine_module_response {
    uint64_t module_count;
    struct limine_file *modules[];
};

struct limine_module_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_module_response *response;
};

/* =========================================================================
 * Framebuffer
 * =========================================================================
 */

#define LIMINE_FRAMEBUFFER_REQUEST \
    { 0x9d5827dcd881dd75, 0xa3148604f6fab11b, 0, 0 }
#define LIMINE_FRAMEBUFFER_RESPONSE \
    { 0x9d5827dcd881dd75, 0xa3148604f6fab11b, 0, 0 }

#define LIMINE_FRAMEBUFFER_RGB      1
#define LIMINE_FRAMEBUFFER_RGBX     2
#define LIMINE_FRAMEBUFFER_BGR      3
#define LIMINE_FRAMEBUFFER_BGRX     4
#define LIMINE_FRAMEBUFFER_XBGR     5
#define LIMINE_FRAMEBUFFER_XRGB     6
#define LIMINE_FRAMEBUFFER_RGBA     7
#define LIMINE_FRAMEBUFFER_BGRA     8
#define LIMINE_FRAMEBUFFER_ARGB     9
#define LIMINE_FRAMEBUFFER_ABGR    10

struct limine_framebuffer {
    uint64_t address;
    uint16_t width;
    uint16_t height;
    uint16_t pitch;
    uint16_t bpp;
    uint8_t  memory_model;
    uint8_t  red_mask_size;
    uint8_t  red_mask_shift;
    uint8_t  green_mask_size;
    uint8_t  green_mask_shift;
    uint8_t  blue_mask_size;
    uint8_t  blue_mask_shift;
    uint8_t  reserved[7];
};

struct limine_framebuffer_response {
    uint64_t framebuffer_count;
    struct limine_framebuffer *framebuffers[];
};

struct limine_framebuffer_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_framebuffer_response *response;
};

/* =========================================================================
 * SMP
 * =========================================================================
 */

#define LIMINE_SMP_REQUEST \
    { 0x95a67b819a0b95db, 0xb08334b614a9a8c8, 0, 0 }
#define LIMINE_SMP_RESPONSE \
    { 0x95a67b819a0b95db, 0xb08334b614a9a8c8, 0, 0 }

struct limine_smp_info {
    uint32_t processor_id;
    uint32_t lapic_id;
    uint64_t reserved;
    void (*goto_address)(void);
    uint64_t extra_argument;
};

struct limine_smp_response {
    uint32_t cpu_count;
    uint32_t flags;
    struct limine_smp_info *cpus[];
};

struct limine_smp_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_smp_response *response;
};

/* =========================================================================
 * RSDP (ACPI Root System Description Pointer)
 * =========================================================================
 */

#define LIMINE_RSDP_REQUEST \
    { 0xc5e77b6b397e7b43, 0x27637845accdcf3c, 0, 0 }
#define LIMINE_RSDP_RESPONSE \
    { 0xc5e77b6b397e7b43, 0x27637845accdcf3c, 0, 0 }

struct limine_rsdp_response {
    uint64_t address;
};

struct limine_rsdp_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_rsdp_response *response;
};

/* =========================================================================
 * DTB (Device Tree Blob - critical for AArch64/Limine AAC64)
 *
 * On AArch64, the bootloader provides the Device Tree Blob via this
 * request instead of passing it in x0 (which is 0 at Limine entry).
 * =========================================================================
 */

#define LIMINE_DTB_REQUEST \
    { 0xb40dddc48d1e0508, 0x27a337c6182a11a5, 0, 0 }
#define LIMINE_DTB_RESPONSE \
    { 0xb40dddc48d1e0508, 0x27a337c6182a11a5, 0, 0 }

struct limine_dtb_response {
    uint64_t dtb_ptr;
};

struct limine_dtb_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_dtb_response *response;
};

#endif /* _LIMINE_H_ */
