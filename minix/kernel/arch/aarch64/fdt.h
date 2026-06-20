/* ============================================================
 * fdt.h — Flattened Device Tree Blob parser for AArch64 boot
 *
 * Minimal self-contained DTB parser for early kernel boot.
 * Parses the Device Tree Blob provided by the bootloader
 * (QEMU -kernel, U-Boot, or Limine AAC64) to extract:
 *   - Memory layout (RAM base and size)
 *   - CPU core count
 *   - Boot command line (/chosen/bootargs)
 *   - Console UART (/chosen/stdout-path or specific node)
 *
 * This is NOT a full libfdt replacement. It only implements
 * the subset needed for MINIX kernel bootstrap.
 *
 * DTB format reference: https://devicetree.org/specifications/
 * ARM64 boot convention: DTB address passed in x0 register
 *
 * Phase 3: FDT parser for AArch64 boot
 * ============================================================ */

#ifndef _AARCH64_FDT_H
#define _AARCH64_FDT_H

#include <stdint.h>

/* =========================================================================
 * DTB header format (big-endian multi-byte fields)
 * =========================================================================
 *
 * All multi-byte values in the DTB are stored in big-endian byte order.
 * The structure block uses tokens to encode the device tree structure.
 */

struct fdt_header {
    uint32_t magic;              /* Magic number: 0xD00DFEED */
    uint32_t totalsize;          /* Total size of DTB (including header) */
    uint32_t off_dt_struct;      /* Offset to structure block */
    uint32_t off_dt_strings;     /* Offset to strings block */
    uint32_t off_mem_rsvmap;     /* Offset to memory reservation block */
    uint32_t version;            /* DTB version */
    uint32_t last_comp_version;  /* Last compatible version */
    uint32_t boot_cpuid_phys;    /* Physical CPU ID to boot from */
    uint32_t size_dt_strings;    /* Size of strings block */
    uint32_t size_dt_struct;     /* Size of structure block */
};

/* Memory reservation entry */
struct fdt_reserve_entry {
    uint64_t address;            /* Physical address of reserved region */
    uint64_t size;               /* Size of reserved region */
};

/* =========================================================================
 * DTB structure block tokens (big-endian)
 * =========================================================================
 *
 * The structure block is a sequence of tokens followed by token-specific
 * data. All tokens are 32-bit big-endian values.
 */

#define FDT_TOKEN_BEGIN_NODE     0x00000001  /* Node start: followed by node name */
#define FDT_TOKEN_END_NODE       0x00000002  /* Node end */
#define FDT_TOKEN_PROP           0x00000003  /* Property: followed by prop header + value */
#define FDT_TOKEN_NOP            0x00000004  /* No-op (skip) */
#define FDT_TOKEN_END            0x00000009  /* End of structure block */

/* Property header (follows FDT_TOKEN_PROP) */
struct fdt_prop_header {
    uint32_t len;                /* Length of property value in bytes */
    uint32_t nameoff;            /* Offset into strings block for property name */
    /* Followed by 'len' bytes of value data, padded to 4 bytes */
};

/* =========================================================================
 * Endian conversion helpers (constexpr for constant folding)
 * =========================================================================
 *
 * DTB is big-endian; AArch64 is little-endian.
 * These macros handle the conversion at compile or runtime.
 */

#ifndef __ORDER_LITTLE_ENDIAN__
#error "AArch64 requires little-endian mode"
#endif

/* Read a big-endian 32-bit value */
static inline uint32_t
fdt32_to_cpu(const uint32_t *p)
{
    const uint8_t *b = (const uint8_t *)p;
    return ((uint32_t)b[0] << 24) | ((uint32_t)b[1] << 16) |
           ((uint32_t)b[2] << 8)  | (uint32_t)b[3];
}

/* Read a big-endian 64-bit value */
static inline uint64_t
fdt64_to_cpu(const uint64_t *p)
{
    const uint8_t *b = (const uint8_t *)p;
    return ((uint64_t)b[0] << 56) | ((uint64_t)b[1] << 48) |
           ((uint64_t)b[2] << 40) | ((uint64_t)b[3] << 32) |
           ((uint64_t)b[4] << 24) | ((uint64_t)b[5] << 16) |
           ((uint64_t)b[6] << 8)  | (uint64_t)b[7];
}

/* =========================================================================
 * Public FDT parser API
 * =========================================================================
 *
 * All functions take a pointer to the DTB (as passed by the bootloader)
 * as the first argument. Functions return 0 on success, -1 on error,
 * unless otherwise noted.
 */

/**
 * fdt_validate - Validate that a memory region contains a valid DTB
 *
 * @fdt:  Pointer to the DTB
 * @size: Maximum size to check (0 = no check)
 *
 * Returns: 0 if valid, -1 if not
 */
int fdt_validate(const void *fdt, uint32_t size);

/**
 * fdt_total_size - Return the total size of the DTB
 *
 * @fdt: Pointer to the DTB
 *
 * Returns: Total DTB size in bytes, or 0 on invalid input
 */
uint32_t fdt_total_size(const void *fdt);

/**
 * fdt_get_memory - Parse /memory node and return RAM information
 *
 * @fdt:      Pointer to the DTB
 * @reg_addr: [out] Physical base address of RAM (or 0 for first region)
 * @reg_size: [out] Size of RAM region in bytes
 *
 * Scans for a /memory node with a "device_type = \"memory\"" property.
 * Returns the first reg entry's address and size.
 *
 * Returns: 1 if memory found, 0 if not found, -1 on error
 */
int fdt_get_memory(const void *fdt, uint64_t *reg_addr, uint64_t *reg_size);

/**
 * fdt_get_cpu_count - Count CPU cores from /cpus node
 *
 * @fdt: Pointer to the DTB
 *
 * Returns: Number of CPU cores, or 0 on error/not found
 */
int fdt_get_cpu_count(const void *fdt);

/**
 * fdt_get_chosen_bootargs - Get bootargs string from /chosen node
 *
 * @fdt:   Pointer to the DTB
 *
 * Returns: Pointer to bootargs string, or NULL if not found
 */
const char *fdt_get_chosen_bootargs(const void *fdt);

/**
 * fdt_get_chosen_stdout - Get stdout-path string from /chosen node
 *
 * @fdt: Pointer to the DTB
 *
 * Returns: Pointer to stdout-path string, or NULL if not found
 */
const char *fdt_get_chosen_stdout(const void *fdt);

/**
 * fdt_get_uart_info - Find UART base address from stdout-path or known devices
 *
 * @fdt:       Pointer to the DTB
 * @uart_addr: [out] Physical base address of UART
 *
 * Tries to locate the boot console UART by:
 *   1. Parsing stdout-path from /chosen
 *   2. Resolving aliases via /aliases node
 *   3. Walking to the UART node and reading its reg property
 *   4. Falling back to known QEMU virt platform addresses (0x09000000)
 *
 * Returns: 1 if UART found via DTB, 0 if using fallback, -1 on error
 */
int fdt_get_uart_info(const void *fdt, uint64_t *uart_addr);

/**
 * fdt_resolve_alias - Resolve an alias name to its full path
 *
 * @fdt:       Pointer to the DTB
 * @alias:     Alias name to resolve (e.g. "serial0")
 * @resolved:  [out] Buffer for resolved path (e.g. "/pl011@9000000")
 * @max_len:   Size of resolved buffer
 *
 * Looks up the /aliases node for a property matching the alias name.
 * The property value should be a path reference to the target node.
 *
 * Returns: Length of resolved path (positive), or -1 if not found/error
 */
int fdt_resolve_alias(const void *fdt, const char *alias,
                      char *resolved, int max_len);

/**
 * fdt_get_node_reg - Get the reg property of a node at a given path
 *
 * @fdt:        Pointer to the DTB
 * @path:       Absolute node path (e.g. "/pl011@9000000" or "/soc/uart@...")
 * @addr_cells: #address-cells of the parent node (-1 = auto-detect)
 * @size_cells: #size-cells of the parent node (-1 = auto-detect)
 * @reg_addr:   [out] First reg entry address
 * @reg_size:   [out] First reg entry size
 *
 * Walks the DTB to the specified node and parses its reg property.
 * If addr_cells/size_cells are -1, they are auto-detected from the
 * parent node's #address-cells and #size-cells properties.
 *
 * Returns: 1 if node and reg found, 0 if node not found, -1 on error
 */
int fdt_get_node_reg(const void *fdt, const char *path,
                     int addr_cells, int size_cells,
                     uint64_t *reg_addr, uint64_t *reg_size);

/**
 * fdt_dump - Debug: print DTB summary via direct_putc callback
 *
 * @fdt:    Pointer to the DTB
 * @putc:   Character output function
 *
 * Useful for early boot debugging before full console init.
 * Only available when FDT_VERBOSE is defined.
 */
void fdt_dump(const void *fdt, void (*putc)(char));

#endif /* _AARCH64_FDT_H */
