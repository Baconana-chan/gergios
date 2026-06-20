/* ============================================================
 * limine.c — Limine AAC64 boot protocol for AArch64
 *
 * Implements the Limine boot protocol request/response mechanism
 * for ARM64 (AArch64). The kernel declares global volatile request
 * structures that the bootloader scans and populates.
 *
 * On Limine AAC64 boot:
 *   - Kernel entered at EL1t with MMU enabled
 *   - Higher-half page tables provided by bootloader
 *   - x0 = 0 (no DTB in register — use LIMINE_DTB_REQUEST instead)
 *   - SP set to bootloader-allocated stack (>= 64KB)
 *   - All other registers zeroed
 *   - VBAR_EL1 undefined (kernel must set up its own vectors)
 *
 * The request structures are placed in .limine_requests section
 * (defined in kernel.lds) so the bootloader can scan them.
 *
 * PL011 UART output is self-contained (no dependency on startup.c).
 *
 * Phase 4: Limine AAC64 boot protocol support
 * ============================================================ */

#include <stdint.h>
#include <stddef.h>
#include <machine/limine.h>

/* =========================================================================
 * Minimal PL011 UART output (self-contained)
 *
 * The UART is hardcoded to the QEMU virt PL011 at 0x09000000,
 * matching startup.c's UART base. This allows early debug output
 * before the main UART driver is initialized.
 * ========================================================================= */

#define LIMINE_UART_BASE    0x09000000UL

#define UART_DR             (*(volatile uint32_t *)(LIMINE_UART_BASE + 0x000))
#define UART_FR             (*(volatile uint32_t *)(LIMINE_UART_BASE + 0x018))
#define UART_FR_TXFF        (1 << 5)

static void
limine_putc(char c)
{
    while (UART_FR & UART_FR_TXFF)
        ;
    UART_DR = (uint32_t)(unsigned char)c;
    if (c == '\n')
        limine_putc('\r');
}

static void
limine_puts(const char *str)
{
    while (*str)
        limine_putc(*str++);
}

static void
limine_put_hex(uint64_t val)
{
    const char *hex = "0123456789ABCDEF";
    int i;
    for (i = 60; i >= 0; i -= 4)
        limine_putc(hex[(val >> i) & 0xF]);
}

static void
limine_put_dec(uint64_t val)
{
    char buf[20];
    int i = 0;
    if (val == 0) { limine_putc('0'); return; }
    while (val > 0 && i < 20) {
        buf[i++] = '0' + (val % 10);
        val /= 10;
    }
    while (i > 0)
        limine_putc(buf[--i]);
}

/* =========================================================================
 * Limine request structures
 *
 * These MUST be placed in the .limine_requests section so the bootloader
 * can find them and populate the response pointers.
 *
 * All requests use revision 0 (backward compatible with Limine v8.x).
 * =========================================================================
 */

/* Declare base revision 1 */
LIMINE_BASE_REVISION(1);

/* Memory map request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_memmap_request _limine_memmap_req = {
    .id = LIMINE_MEMMAP_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Boot time request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_boot_time_request _limine_boot_time_req = {
    .id = LIMINE_BOOT_TIME_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Kernel address request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_kernel_address_request _limine_kern_addr_req = {
    .id = LIMINE_KERNEL_ADDRESS_REQUEST,
    .revision = 0,
    .response = NULL
};

/* HHDM request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_hhdm_request _limine_hhdm_req = {
    .id = LIMINE_HHDM_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Module request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_module_request _limine_module_req = {
    .id = LIMINE_MODULE_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Framebuffer request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_framebuffer_request _limine_fb_req = {
    .id = LIMINE_FRAMEBUFFER_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Bootloader info request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_bootloader_info_request _limine_bootloader_req = {
    .id = LIMINE_BOOTLOADER_INFO_REQUEST,
    .revision = 0,
    .response = NULL
};

/* RSDP (ACPI) request */
__attribute__((used, section(".limine_requests")))
volatile struct limine_rsdp_request _limine_rsdp_req = {
    .id = LIMINE_RSDP_REQUEST,
    .revision = 0,
    .response = NULL
};

/* DTB (Device Tree Blob) request — critical for AArch64!
 * On Limine AAC64, the DTB is provided via this request,
 * NOT in the x0 register (which is 0 at entry). */
__attribute__((used, section(".limine_requests")))
volatile struct limine_dtb_request _limine_dtb_req = {
    .id = LIMINE_DTB_REQUEST,
    .revision = 0,
    .response = NULL
};

/* Terminator for the request list */
__attribute__((used, section(".limine_requests")))
static volatile uint64_t _limine_requests_end[4] = { 0, 0, 0, 0 };

/* =========================================================================
 * limine_check_responses — verify which requests have responses
 *
 * Called early in boot to check if Limine is present and which
 * requests were fulfilled. Returns the DTB address if available,
 * or 0 if not booted via Limine.
 *
 * The request/response structures are in .limine_requests section,
 * placed in low physical memory before the kernel high mapping,
 * so they are accessible via the identity map during early boot.
 * ========================================================================= */

uint64_t
limine_check_responses(void)
{
    uint64_t dtb_address = 0;

    limine_puts("[LIMINE] Checking Limine responses...\r\n");

    /* Check DTB request — critical for AArch64 */
    if (_limine_dtb_req.response) {
        dtb_address = _limine_dtb_req.response->dtb_ptr;
        limine_puts("[LIMINE] DTB via Limine: 0x");
        limine_put_hex(dtb_address);
        limine_puts("\r\n");
    } else {
        limine_puts("[LIMINE] DTB: no response (not booted via Limine?)\r\n");
    }

    /* Check kernel address */
    if (_limine_kern_addr_req.response) {
        limine_puts("[LIMINE] Kernel phys: 0x");
        limine_put_hex(_limine_kern_addr_req.response->physical_base);
        limine_puts(", virt: 0x");
        limine_put_hex(_limine_kern_addr_req.response->virtual_base);
        limine_puts("\r\n");
    }

    /* Check memory map */
    if (_limine_memmap_req.response) {
        limine_puts("[LIMINE] Memory map: ");
        limine_put_dec(_limine_memmap_req.response->entry_count);
        limine_puts(" entries\r\n");
    }

    /* Check bootloader info */
    if (_limine_bootloader_req.response &&
        _limine_bootloader_req.response->name) {
        limine_puts("[LIMINE] Bootloader: ");
        limine_puts((const char *)_limine_bootloader_req.response->name);
        limine_puts("\r\n");
    }

    /* Check HHDM offset */
    if (_limine_hhdm_req.response) {
        limine_puts("[LIMINE] HHDM offset: 0x");
        limine_put_hex(_limine_hhdm_req.response->offset);
        limine_puts("\r\n");
    }

    /* Check modules */
    if (_limine_module_req.response) {
        limine_puts("[LIMINE] Modules: ");
        limine_put_dec(_limine_module_req.response->module_count);
        limine_puts("\r\n");
    }

    return dtb_address;
}

/* =========================================================================
 * limine_pre_init — main entry point for Limine boot path
 *
 * Called from the boot path when Limine AAC64 is detected.
 * Checks for Limine responses and returns the DTB address
 * for use by the FDT parser (fdt.c).
 *
 * Unlike x86_64, AArch64 uses the FDT parser to extract boot info
 * from the DTB (provided via LIMINE_DTB_REQUEST), rather than
 * directly parsing Limine memory map/module responses.
 *
 * @return  DTB physical address, or 0 if not booted via Limine
 * ========================================================================= */

uint64_t
limine_pre_init(void)
{
    limine_puts("\r\n========================================\r\n");
    limine_puts("  GergiOS ARM64 — Limine AAC64 Boot\r\n");
    limine_puts("  Phase 4: Limine Protocol\r\n");
    limine_puts("========================================\r\n");
    limine_puts("\r\n");

    return limine_check_responses();
}
