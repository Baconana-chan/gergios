/*
pci_find_cap.c

Walk the PCI capability list starting at config offset 0x34 and find
the first capability matching the given cap_id.

Returns the config-space offset of the capability, or 0 if not found.
The cap_ptr argument returns the pointer to the next capability (for
iterating multiple instances), or 0 if there is none.

Usage:
    int cap;
    if (pci_find_cap(devind, PCI_CAP_ID_MSIX, &cap))
        printf("MSI-X at config offset 0x%x\\n", cap);
*/

#include "pci.h"
#include "syslib.h"
#include <minix/sysutil.h>

/* PCI config space registers for capability list traversal */
#define PCI_CAP_PTR         0x34    /* capabilities pointer (byte) */
#define PCI_STATUS          0x06    /* status register */
#define  PCI_STATUS_CAP_LIST 0x10   /* capabilities list bit */

/* Capability list entry layout */
#define PCI_CAP_LIST_ID     0       /* cap ID */
#define PCI_CAP_LIST_NEXT   1       /* next capability pointer */

/*===========================================================================*
 *                              pci_find_cap                                 *
 *===========================================================================*/
int pci_find_cap(devind, cap_id, cap_ptr)
int devind;
int cap_id;
int *cap_ptr;
{
    u16_t status;
    u8_t ptr, id;
    int max_caps = 48;  /* safety limit */

    /* Check if capability list is supported */
    status = pci_attr_r16(devind, PCI_STATUS);
    if (!(status & PCI_STATUS_CAP_LIST))
        return 0;

    /* Read pointer to first capability */
    ptr = pci_attr_r8(devind, PCI_CAP_PTR);
    if (ptr == 0)
        return 0;

    /* Walk the capability list */
    while (ptr != 0 && max_caps-- > 0) {
        id = pci_attr_r8(devind, ptr + PCI_CAP_LIST_ID);

        if (id == cap_id) {
            if (cap_ptr != NULL)
                *cap_ptr = ptr;
            return 1;
        }

        ptr = pci_attr_r8(devind, ptr + PCI_CAP_LIST_NEXT);
    }

    return 0;
}
