/*
pci_msix.c

Parse the PCI MSI-X capability structure and extract:
  - MSI-X table BAR (which BAR contains the table)
  - Table offset within that BAR
  - Number of table entries (table size)
  - PBA (Pending Bit Array) BAR and offset

Based on the PCI 3.0 / PCIe MSI-X capability specification.

Usage:
    struct pci_msix_info msix;
    if (pci_msix_parse(devind, &msix))
        printf("MSI-X: %d entries at BAR %d +0x%x\\n",
               msix.msix_table_size, msix.msix_table_bir,
               (unsigned)(msix.msix_table_offset * 8));
*/

#include "pci.h"
#include "syslib.h"
#include <minix/sysutil.h>

/* PCI MSI-X capability structure offsets (from capability pointer) */
#define MSIX_CAP_ID         0       /* capability ID (1 byte) = 0x11 */
#define MSIX_NEXT           1       /* next capability pointer (1 byte) */
#define MSIX_MSG_CTRL       2       /* message control (2 bytes) */
#define MSIX_TABLE          4       /* table BIR/offset (4 bytes) */
#define MSIX_PBA            8       /* PBA BIR/offset (4 bytes) */

/* Message Control Register bits */
#define MSIX_CTRL_TSIZE     0x7FF   /* table size mask (bits 0-10) */
#define MSIX_CTRL_FUNCMASK  (1 << 14) /* function mask */
#define MSIX_CTRL_ENABLE    (1 << 15) /* MSI-X enable */

/* PCI capability ID for MSI-X */
#define PCI_CAP_ID_MSIX     0x11

/*===========================================================================*
 *                             pci_find_msix                                 *
 *===========================================================================*/
int pci_find_msix(devind, cap_offset)
int devind;
int *cap_offset;
{
    return pci_find_cap(devind, PCI_CAP_ID_MSIX, cap_offset);
}

/*===========================================================================*
 *                             pci_msix_get_table_size                       *
 *===========================================================================*/
/* Returns the number of MSI-X table entries (actual count, not N-1). */
static int
pci_msix_get_table_size(devind, cap)
int devind;
int cap;
{
    u16_t ctrl;

    ctrl = pci_attr_r16(devind, cap + MSIX_MSG_CTRL);
    return (ctrl & MSIX_CTRL_TSIZE) + 1;
}

/*===========================================================================*
 *                             pci_msix_parse                                *
 *===========================================================================*/
int pci_msix_parse(devind, info)
int devind;
struct pci_msix_info *info;
{
    int cap;
    u32_t table_reg, pba_reg;

    if (!pci_find_msix(devind, &cap))
        return 0;

    /* Read table size from message control register */
    info->msix_table_size = pci_msix_get_table_size(devind, cap);

    /* Read table BAR and offset */
    table_reg = pci_attr_r32(devind, cap + MSIX_TABLE);
    info->msix_table_bir = table_reg & 0x7;
    info->msix_table_offset = (table_reg >> 3);  /* in QWORDs (8-byte units) */

    /* Read PBA BAR and offset */
    pba_reg = pci_attr_r32(devind, cap + MSIX_PBA);
    info->msix_pba_bir = pba_reg & 0x7;
    info->msix_pba_offset = (pba_reg >> 3);  /* in QWORDs */

    return 1;
}
