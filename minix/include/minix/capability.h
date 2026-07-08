/* Capability definitions for the GergiOS Security Model.
 *
 * This file defines named capabilities that can be assigned to processes.
 * Capabilities are stored as bits in a uint64_t mask.
 *
 * Phase 2: Capability Model Refinement
 * See planning/26_security_model_modernization.md
 */
#ifndef _MINIX_CAPABILITY_H
#define _MINIX_CAPABILITY_H

#include <stdint.h>

/* Capability bit definitions (uint64_t mask).
 * A process needs a capability to perform privileged operations.
 * By default, processes have NO capabilities (base = 0).
 */
#define CAP_SYS_RAWIO    (1ULL << 0)   /* Direct I/O port access */
#define CAP_NET_RAW      (1ULL << 1)   /* Raw socket access (AF_PACKET) */
#define CAP_NET_BIND     (1ULL << 2)   /* Bind to privileged ports (<1024) */
#define CAP_NET_ADMIN    (1ULL << 3)   /* Network interface configuration */
#define CAP_SYS_ADMIN    (1ULL << 4)   /* System administration (RS control) */
#define CAP_SYS_BOOT     (1ULL << 5)   /* System reboot/shutdown */
#define CAP_IPC_OWNER    (1ULL << 6)   /* Bypass IPC send masks */
#define CAP_FS_MOUNT     (1ULL << 7)   /* Mount/umount filesystems */
#define CAP_FS_CHOWN     (1ULL << 8)   /* Change file ownership (chown) */
#define CAP_FS_DAC_OVERRIDE (1ULL << 9) /* Bypass file permission checks */
#define CAP_VM_MAP       (1ULL << 10)  /* Map physical memory */
#define CAP_IRQ_ALLOC    (1ULL << 11)  /* Allocate IRQ lines */
#define CAP_PCI_ACCESS   (1ULL << 12)  /* PCI configuration space access */

/* Total number of defined capabilities */
#define CAP_MAX          13

/* Predefined capability sets */
#define CAP_BASE        0ULL                          /* no capabilities */
#define CAP_SYSTEM      (CAP_NET_BIND | CAP_IPC_OWNER) /* system services */
#define CAP_DRIVER      (CAP_SYS_RAWIO | CAP_IRQ_ALLOC | CAP_VM_MAP)
#define CAP_NETWORK     (CAP_NET_RAW | CAP_NET_ADMIN | CAP_NET_BIND)
#define CAP_ADMIN       (CAP_SYS_ADMIN | CAP_SYS_BOOT | CAP_FS_MOUNT)
#define CAP_FULL        ((1ULL << CAP_MAX) - 1)       /* all capabilities */

/* SYS_CAPCTL subfunctions */
#define CAP_OP_GET            1    /* get effective capabilities */
#define CAP_OP_SET            2    /* set effective capabilities (can only drop) */
#define CAP_OP_BOUND_GET      3    /* get bounding set */
#define CAP_OP_BOUND_SET      4    /* set bounding set (SYS_ADMIN only) */
#define CAP_OP_LIST           5    /* list all defined capabilities */

/* String names for capability bits (for system.conf parsing) */
#define CAP_STR_SYS_RAWIO    "SYS_RAWIO"
#define CAP_STR_NET_RAW      "NET_RAW"
#define CAP_STR_NET_BIND     "NET_BIND"
#define CAP_STR_NET_ADMIN    "NET_ADMIN"
#define CAP_STR_SYS_ADMIN    "SYS_ADMIN"
#define CAP_STR_SYS_BOOT     "SYS_BOOT"
#define CAP_STR_IPC_OWNER    "IPC_OWNER"
#define CAP_STR_FS_MOUNT     "FS_MOUNT"
#define CAP_STR_FS_CHOWN     "FS_CHOWN"
#define CAP_STR_FS_DAC_OVERRIDE "FS_DAC_OVERRIDE"
#define CAP_STR_VM_MAP       "VM_MAP"
#define CAP_STR_IRQ_ALLOC    "IRQ_ALLOC"
#define CAP_STR_PCI_ACCESS   "PCI_ACCESS"

/* Predefined set string names */
#define CAP_STR_BASE         "BASE"
#define CAP_STR_SYSTEM       "SYSTEM"
#define CAP_STR_DRIVER       "DRIVER"
#define CAP_STR_NETWORK      "NETWORK"
#define CAP_STR_ADMIN        "ADMIN"
#define CAP_STR_NONE         "NONE"
#define CAP_STR_ALL          "ALL"

#endif /* _MINIX_CAPABILITY_H */
