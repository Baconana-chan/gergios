/* rust/minix-ahci/include/ahci.h
 *
 * AHCI 1.3 Register Constants — C header for driver FFI tests.
 *
 * All constants are verified against the Rust implementation in
 * rust/minix-ahci/src/registers.rs. C FFI test functions are
 * exported from the minix_ahci staticlib via #[no_mangle].
 *
 * Usage:
 *   #include <ahci.h>
 *   // link against: -lminix_ahci (libminix_ahci.a)
 */

#ifndef AHCI_CORE_H
#define AHCI_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===================================================================
 * Version
 * =================================================================== */

/** Return the AHCI spec version (0x01030000 = AHCI 1.3). */
uint32_t ahci_test_version(void);

/* ===================================================================
 * HBA Register Byte Offsets
 *
 * Each register is a u32 at a byte offset from the HBA MMIO base.
 * Byte offset = register_index * 4.
 * =================================================================== */

/** Return byte offset of an AHCI register by numeric ID (see below). */
uint32_t ahci_test_reg_offset(uint32_t reg_id);

/* Register ID constants for ahci_test_reg_offset(). */
#define AHCI_REG_HBA_CAP    0   /* Host Capabilities (0x00) */
#define AHCI_REG_HBA_GHC    1   /* Global Host Control (0x04) */
#define AHCI_REG_HBA_IS     2   /* Interrupt Status (0x08) */
#define AHCI_REG_HBA_PI     3   /* Ports Implemented (0x0C) */
#define AHCI_REG_HBA_VS     4   /* Version (0x10) */
#define AHCI_REG_HBA_CAP2   5   /* Capabilities Extended (0x24) */

#define AHCI_REG_PORT_CLB   8   /* Command List Base (0x00) */
#define AHCI_REG_PORT_CLBU  9   /* Command List Base Upper (0x04) */
#define AHCI_REG_PORT_FB    10  /* FIS Base (0x08) */
#define AHCI_REG_PORT_FBU   11  /* FIS Base Upper (0x0C) */
#define AHCI_REG_PORT_IS    12  /* Interrupt Status (0x10) */
#define AHCI_REG_PORT_IE    13  /* Interrupt Enable (0x14) */
#define AHCI_REG_PORT_CMD   14  /* Command and Status (0x18) */
#define AHCI_REG_PORT_TFD   15  /* Task File Data (0x20) */
#define AHCI_REG_PORT_SIG   16  /* Signature (0x24) */
#define AHCI_REG_PORT_SSTS  17  /* Serial ATA Status (0x28) */
#define AHCI_REG_PORT_SCTL  18  /* Serial ATA Control (0x2C) */
#define AHCI_REG_PORT_SERR  19  /* Serial ATA Error (0x30) */
#define AHCI_REG_PORT_SACT  20  /* Serial ATA Active (0x34) */
#define AHCI_REG_PORT_CI    21  /* Command Issue (0x38) */

/* Expected byte offsets for verification. */
#define AHCI_HBA_CAP_OFFSET     0x00
#define AHCI_HBA_GHC_OFFSET     0x04
#define AHCI_HBA_IS_OFFSET      0x08
#define AHCI_HBA_PI_OFFSET      0x0C
#define AHCI_HBA_VS_OFFSET      0x10
#define AHCI_HBA_CAP2_OFFSET    0x24

#define AHCI_PORT_CLB_OFFSET    0x00
#define AHCI_PORT_CLBU_OFFSET   0x04
#define AHCI_PORT_FB_OFFSET     0x08
#define AHCI_PORT_FBU_OFFSET    0x0C
#define AHCI_PORT_IS_OFFSET     0x10
#define AHCI_PORT_IE_OFFSET     0x14
#define AHCI_PORT_CMD_OFFSET    0x18
#define AHCI_PORT_TFD_OFFSET    0x20
#define AHCI_PORT_SIG_OFFSET    0x24
#define AHCI_PORT_SSTS_OFFSET   0x28
#define AHCI_PORT_SCTL_OFFSET   0x2C
#define AHCI_PORT_SERR_OFFSET   0x30
#define AHCI_PORT_SACT_OFFSET   0x34
#define AHCI_PORT_CI_OFFSET     0x38

/* ===================================================================
 * Bitfield Constants
 *
 * Verified via ahci_test_bitfield() function.
 * =================================================================== */

/** Return a key AHCI bitfield constant by numeric ID. */
uint32_t ahci_test_bitfield(uint32_t id);

/* Bitfield IDs for ahci_test_bitfield(). */
#define AHCI_BF_CAP_SNCQ       0   /* CAP.SNCQ (bit 30) */
#define AHCI_BF_CAP_SCLO       1   /* CAP.SCLO (bit 24) */
#define AHCI_BF_CAP_NCS_MASK   2   /* CAP.NCS mask (bits 8-12) */
#define AHCI_BF_GHC_AE         3   /* GHC.AHCI Enable (bit 31) */
#define AHCI_BF_GHC_HR         4   /* GHC.HBA Reset (bit 0) */

#define AHCI_BF_IS_TFES        5   /* IS.Task File Error (bit 30) */
#define AHCI_BF_IS_PRCS        6   /* IS.PhyRdy Change (bit 22) */
#define AHCI_BF_IS_PCS         7   /* IS.Port Connect Change (bit 6) */
#define AHCI_BF_IS_DHRS        8   /* IS.D2H Register FIS (bit 0) */

#define AHCI_BF_CMD_ST         9   /* CMD.Start (bit 0) */
#define AHCI_BF_CMD_FRE       10   /* CMD.FIS Receive Enable (bit 4) */
#define AHCI_BF_CMD_FR        11   /* CMD.FIS Receive Running (bit 14) */
#define AHCI_BF_CMD_CR        12   /* CMD.Command List Running (bit 15) */
#define AHCI_BF_CMD_SUD       13   /* CMD.Spin-Up Device (bit 1) */

#define AHCI_BF_TFD_BSY       14   /* TFD.Busy (bit 7) */
#define AHCI_BF_TFD_DRQ       15   /* TFD.Data Request (bit 3) */
#define AHCI_BF_TFD_ERR       16   /* TFD.Error (bit 0) */

#define AHCI_BF_SSTS_DET_PHY  17   /* SSTS.DET PHY (0x0003) */
#define AHCI_BF_SSTS_DET_NONE 18   /* SSTS.DET None (0x0000) */

#define AHCI_BF_SERR_DIAG_X   19   /* SERR.DIAG Exchanged (bit 26) */
#define AHCI_BF_SERR_DIAG_N   20   /* SERR.DIAG PhyRdy Change (bit 16) */

#define AHCI_BF_FIS_TYPE_H2D  21   /* FIS type: Host-to-Device (0x27) */
#define AHCI_BF_FIS_DEV_LBA   22   /* FIS DEV: LBA bit (0x40) */

#define AHCI_BF_ATA_SECTOR    23   /* ATA sector size (512) */
#define AHCI_BF_MAX_PORTS     24   /* Max AHCI ports (32) */
#define AHCI_BF_MAX_CMDS      25   /* Max commands per port (32) */

/* Expected bitfield values (from AHCI 1.3 spec). */
#define AHCI_CAP_SNCQ         (1u << 30)
#define AHCI_CAP_SCLO         (1u << 24)
#define AHCI_CAP_NCS_MASK     0x1F
#define AHCI_GHC_AE           (1u << 31)
#define AHCI_GHC_HR           (1u << 0)

#define AHCI_IS_TFES          (1u << 30)
#define AHCI_IS_PRCS          (1u << 22)
#define AHCI_IS_PCS           (1u << 6)
#define AHCI_IS_DHRS          (1u << 0)

#define AHCI_CMD_ST           (1u << 0)
#define AHCI_CMD_FRE          (1u << 4)
#define AHCI_CMD_FR           (1u << 14)
#define AHCI_CMD_CR           (1u << 15)
#define AHCI_CMD_SUD          (1u << 1)

#define AHCI_TFD_BSY          (1u << 7)
#define AHCI_TFD_DRQ          (1u << 3)
#define AHCI_TFD_ERR          (1u << 0)

#define AHCI_SSTS_DET_PHY     0x0003
#define AHCI_SSTS_DET_NONE    0x0000

#define AHCI_SERR_DIAG_X      (1u << 26)
#define AHCI_SERR_DIAG_N      (1u << 16)

#define AHCI_ATA_SECTOR_SIZE  512
#define AHCI_MAX_PORTS        32
#define AHCI_MAX_CMDS         32

/* ===================================================================
 * Memory Layout Constants
 * =================================================================== */

/** HBA MMIO base region size (before port registers). */
#define AHCI_MEM_BASE_SIZE    0x100

/** Size of each port's MMIO region. */
#define AHCI_MEM_PORT_SIZE    0x80

/** FIS DMA buffer size. */
#define AHCI_FIS_SIZE         256

/** Command List size (32 entries × 32 bytes). */
#define AHCI_CL_SIZE          1024

/** Command Table size. */
#define AHCI_CT_SIZE          (128 + 66 * 16)

#ifdef __cplusplus
}
#endif

#endif /* AHCI_CORE_H */
