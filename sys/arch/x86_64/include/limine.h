/* Limine Boot Protocol definitions for GergiOS x86_64. */

#ifndef _X86_64_LIMINE_H_
#define _X86_64_LIMINE_H_

#include <stdint.h>

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

#define LIMINE_BOOTLOADER_INFO_REQUEST \
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

#define LIMINE_MEMMAP_REQUEST \
    { 0x67cf3d9d378a806f, 0xe304acdfc50c3c62, 0, 0 }

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

#define LIMINE_BOOT_TIME_REQUEST \
    { 0x502746e184c088aa, 0xfbc5ec83e6327893, 0, 0 }

struct limine_boot_time_response {
    int64_t boot_time;
};

struct limine_boot_time_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_boot_time_response *response;
};

#define LIMINE_KERNEL_ADDRESS_REQUEST \
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

#define LIMINE_HHDM_REQUEST \
    { 0x48dcf1cb8ad2b852, 0x63984e959a98244b, 0, 0 }

struct limine_hhdm_response {
    uint64_t offset;
};

struct limine_hhdm_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_hhdm_response *response;
};

#define LIMINE_MODULE_REQUEST \
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

#define LIMINE_FRAMEBUFFER_REQUEST \
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

#define LIMINE_SMP_REQUEST \
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

#define LIMINE_RSDP_REQUEST \
    { 0xc5e77b6b397e7b43, 0x27637845accdcf3c, 0, 0 }

struct limine_rsdp_response {
    uint64_t address;
};

struct limine_rsdp_request {
    uint64_t id[4];
    uint64_t revision;
    volatile struct limine_rsdp_response *response;
};

#endif /* _X86_64_LIMINE_H_ */
