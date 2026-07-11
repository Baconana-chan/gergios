/* rust/e1000/include/e1000.h
 *
 * Intel PRO/1000 Gigabit Ethernet Register Constants
 * — C header for driver FFI tests.
 *
 * All constants are verified against the Rust implementation in
 * rust/e1000/src/reg.rs. C FFI test functions are exported from
 * the e1000 staticlib via #[no_mangle].
 *
 * Usage:
 *   #include <e1000.h>
 *   // link against: -le1000 (libe1000.a)
 */

#ifndef E1000_CORE_H
#define E1000_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===================================================================
 * Version
 * =================================================================== */

/** Return e1000 version code: 0x1008254X (device ID family). */
uint32_t e1000_test_version(void);

/* ===================================================================
 * Register Byte Offsets
 *
 * Each register is a u32 at a specific byte offset from MMIO base.
 * =================================================================== */

/** Return byte offset of an e1000 register by numeric ID (see below). */
uint32_t e1000_test_reg_offset(uint32_t reg_id);

/* Register ID constants for e1000_test_reg_offset(). */
#define E1000_REG_CTRL      0   /* Device Control (0x00000) */
#define E1000_REG_STATUS    1   /* Device Status (0x00008) */
#define E1000_REG_EERD      2   /* EEPROM Read (0x00014) */
#define E1000_REG_FCAL      3   /* Flow Ctrl Addr Low (0x00028) */
#define E1000_REG_FCAH      4   /* Flow Ctrl Addr High (0x0002C) */
#define E1000_REG_FCT       5   /* Flow Control Type (0x00030) */
#define E1000_REG_FCTTV     6   /* Flow Ctrl Timer (0x00170) */
#define E1000_REG_ICR       7   /* Int Cause Read (0x000C0) */
#define E1000_REG_IMS       8   /* Int Mask Set (0x000D0) */
#define E1000_REG_RCTL      9   /* Receive Control (0x00100) */
#define E1000_REG_TCTL     10   /* Transmit Control (0x00400) */
#define E1000_REG_RDBAL    11   /* Rx Desc Base Low (0x02800) */
#define E1000_REG_RDBAH    12   /* Rx Desc Base High (0x02804) */
#define E1000_REG_RDLEN    13   /* Rx Desc Length (0x02808) */
#define E1000_REG_RDH      14   /* Rx Desc Head (0x02810) */
#define E1000_REG_RDT      15   /* Rx Desc Tail (0x02818) */
#define E1000_REG_TDBAL    16   /* Tx Desc Base Low (0x03800) */
#define E1000_REG_TDBAH    17   /* Tx Desc Base High (0x03804) */
#define E1000_REG_TDLEN    18   /* Tx Desc Length (0x03808) */
#define E1000_REG_TDH      19   /* Tx Desc Head (0x03810) */
#define E1000_REG_TDT      20   /* Tx Desc Tail (0x03818) */
#define E1000_REG_CRCERRS  21   /* CRC Error Count (0x04000) */
#define E1000_REG_RXERRC   22   /* Rx Error Count (0x0400C) */
#define E1000_REG_MPC      23   /* Missed Packets Count (0x04010) */
#define E1000_REG_COLC     24   /* Collision Count (0x04028) */
#define E1000_REG_TPR      25   /* Total Pkts Rx (0x040D0) */
#define E1000_REG_TPT      26   /* Total Pkts Tx (0x040D4) */
#define E1000_REG_RAL      27   /* Receive Addr Low (0x05400) */
#define E1000_REG_RAH      28   /* Receive Addr High (0x05404) */
#define E1000_REG_MTA      29   /* Multicast Array (0x05200) */
#define E1000_REG_IVAR     30   /* Int Vector Alloc (0x00E00) */
#define E1000_REG_EICR     31   /* Ext Int Cause Read (0x01580) */
#define E1000_REG_EIAC     32   /* Ext Int Auto Clear (0x0158C) */
#define E1000_REG_EIMS     33   /* Ext Int Mask Set (0x01524) */
#define E1000_REG_EIMC     34   /* Ext Int Mask Clear (0x01528) */

/* Expected byte offsets (must match Rust constants). */
#define E1000_CTRL_OFFSET     0x00000
#define E1000_STATUS_OFFSET   0x00008
#define E1000_EERD_OFFSET     0x00014
#define E1000_FCAL_OFFSET     0x00028
#define E1000_FCAH_OFFSET     0x0002C
#define E1000_FCT_OFFSET      0x00030
#define E1000_FCTTV_OFFSET    0x00170
#define E1000_ICR_OFFSET      0x000C0
#define E1000_IMS_OFFSET      0x000D0
#define E1000_RCTL_OFFSET     0x00100
#define E1000_TCTL_OFFSET     0x00400
#define E1000_RDBAL_OFFSET    0x02800
#define E1000_RDBAH_OFFSET    0x02804
#define E1000_RDLEN_OFFSET    0x02808
#define E1000_RDH_OFFSET      0x02810
#define E1000_RDT_OFFSET      0x02818
#define E1000_TDBAL_OFFSET    0x03800
#define E1000_TDBAH_OFFSET    0x03804
#define E1000_TDLEN_OFFSET    0x03808
#define E1000_TDH_OFFSET      0x03810
#define E1000_TDT_OFFSET      0x03818
#define E1000_CRCERRS_OFFSET  0x04000
#define E1000_RXERRC_OFFSET   0x0400C
#define E1000_MPC_OFFSET      0x04010
#define E1000_COLC_OFFSET     0x04028
#define E1000_TPR_OFFSET      0x040D0
#define E1000_TPT_OFFSET      0x040D4
#define E1000_RAL_OFFSET      0x05400
#define E1000_RAH_OFFSET      0x05404
#define E1000_MTA_OFFSET      0x05200
#define E1000_IVAR_OFFSET     0x00E00
#define E1000_EICR_OFFSET     0x01580
#define E1000_EIAC_OFFSET     0x0158C
#define E1000_EIMS_OFFSET     0x01524
#define E1000_EIMC_OFFSET     0x01528

/* ===================================================================
 * Bitfield Constants
 *
 * Verified via e1000_test_bitfield() function.
 * =================================================================== */

/** Return a bitfield/constant value by numeric ID. */
uint32_t e1000_test_bitfield(uint32_t bf_id);

/* Bitfield IDs for e1000_test_bitfield(). */
#define E1000_BF_CTRL_LRST      0   /* CTRL.Link Reset (bit 3) */
#define E1000_BF_CTRL_ASDE      1   /* CTRL.Auto-Speed (bit 5) */
#define E1000_BF_CTRL_SLU       2   /* CTRL.Set Link Up (bit 6) */
#define E1000_BF_CTRL_ILOS      3   /* CTRL.Invert Loss (bit 7) */
#define E1000_BF_CTRL_RST       4   /* CTRL.Device Reset (bit 26) */
#define E1000_BF_CTRL_VME       5   /* CTRL.VLAN Mode (bit 30) */
#define E1000_BF_CTRL_PHY_RST   6   /* CTRL.PHY Reset (bit 31) */

#define E1000_BF_STATUS_FD            7   /* STATUS.Full Duplex (bit 0) */
#define E1000_BF_STATUS_LU            8   /* STATUS.Link Up (bit 1) */
#define E1000_BF_STATUS_TXOFF         9   /* STATUS.Tx Paused (bit 4) */
#define E1000_BF_STATUS_SPEED        10   /* STATUS.Speed mask (bits 6:7) */
#define E1000_BF_STATUS_SPEED_10     11   /* STATUS.Speed 10 Mbps */
#define E1000_BF_STATUS_SPEED_100    12   /* STATUS.Speed 100 Mbps */
#define E1000_BF_STATUS_SPEED_1000_A 13   /* STATUS.Speed 1000 Mbps (A) */
#define E1000_BF_STATUS_SPEED_1000_B 14   /* STATUS.Speed 1000 Mbps (B) */

#define E1000_BF_EERD_START   15   /* EERD.Start Read (bit 0) */
#define E1000_BF_EERD_DONE    16   /* EERD.Read Done (bit 4) */
#define E1000_BF_EERD_ADDR    17   /* EERD.Address mask (bits 8-15) */
#define E1000_BF_EERD_DATA    18   /* EERD.Data mask (bits 16-31) */

#define E1000_BF_ICR_TXDW     19   /* ICR.Tx Desc Written (bit 0) */
#define E1000_BF_ICR_TXQE     20   /* ICR.Tx Queue Empty (bit 1) */
#define E1000_BF_ICR_LSC      21   /* ICR.Link Status (bit 2) */
#define E1000_BF_ICR_RXO      22   /* ICR.Rx Overrun (bit 6) */
#define E1000_BF_ICR_RXT      23   /* ICR.Rx Timer (bit 7) */

#define E1000_BF_RCTL_EN      24   /* RCTL.Rx Enable (bit 1) */
#define E1000_BF_RCTL_UPE     25   /* RCTL.Unicast Promisc (bit 3) */
#define E1000_BF_RCTL_MPE     26   /* RCTL.Multicast Promisc (bit 4) */
#define E1000_BF_RCTL_BAM     27   /* RCTL.Broadcast Accept (bit 15) */
#define E1000_BF_RCTL_BSIZE   28   /* RCTL.Buffer Size (bits 17:16) */
#define E1000_BF_RCTL_BSEX    29   /* RCTL.Buf Size Ext (bit 25) */

#define E1000_BF_TCTL_EN      30   /* TCTL.Tx Enable (bit 1) */
#define E1000_BF_TCTL_PSP     31   /* TCTL.Pad Short Pkts (bit 3) */

#define E1000_BF_RAH_AV       32   /* RAH.Address Valid (bit 31) */

#define E1000_BF_IVAR_VALID   33   /* IVAR entry valid (bit 15) */

#define E1000_BF_EICR_RX0     34   /* EICR.RX queue 0 (bit 0) */
#define E1000_BF_EICR_TX0     35   /* EICR.TX queue 0 (bit 1) */
#define E1000_BF_EICR_OTHER   36   /* EICR.Other (bit 2) */

/* Config constants */
#define E1000_BF_RXDESC_NR        37   /* Number of Rx desc (256) */
#define E1000_BF_TXDESC_NR        38   /* Number of Tx desc (256) */
#define E1000_BF_IOBUF_SIZE       39   /* IO buffer size (16384) */
#define E1000_BF_EERD_READ_TIMEOUT 40  /* EERD poll timeout (100000) */

/* IVAR entry offsets */
#define E1000_BF_IVAR_RX0     41   /* IVAR RX queue 0 offset (0) */
#define E1000_BF_IVAR_TX0     42   /* IVAR TX queue 0 offset (2) */
#define E1000_BF_IVAR_RX1     43   /* IVAR RX queue 1 offset (4) */
#define E1000_BF_IVAR_TX1     44   /* IVAR TX queue 1 offset (6) */
#define E1000_BF_IVAR_OTHER   45   /* IVAR other offset (8) */

/* Expected bitfield values (from Intel PRO/1000 manual). */
#define E1000_CTRL_LRST        (1u << 3)
#define E1000_CTRL_ASDE        (1u << 5)
#define E1000_CTRL_SLU         (1u << 6)
#define E1000_CTRL_ILOS        (1u << 7)
#define E1000_CTRL_RST         (1u << 26)
#define E1000_CTRL_VME         (1u << 30)
#define E1000_CTRL_PHY_RST     (1u << 31)

#define E1000_STATUS_FD        (1u << 0)
#define E1000_STATUS_LU        (1u << 1)
#define E1000_STATUS_TXOFF     (1u << 4)
#define E1000_STATUS_SPEED     0xC0u
#define E1000_STATUS_SPEED_10         0x00u
#define E1000_STATUS_SPEED_100        0x40u
#define E1000_STATUS_SPEED_1000_A     0x80u
#define E1000_STATUS_SPEED_1000_B     0xC0u

#define E1000_EERD_START       (1u << 0)
#define E1000_EERD_DONE        (1u << 4)
#define E1000_EERD_ADDR        0xFF00u
#define E1000_EERD_DATA        0xFFFF0000u

#define E1000_ICR_TXDW         (1u << 0)
#define E1000_ICR_TXQE         (1u << 1)
#define E1000_ICR_LSC          (1u << 2)
#define E1000_ICR_RXO          (1u << 6)
#define E1000_ICR_RXT          (1u << 7)

#define E1000_RCTL_EN          (1u << 1)
#define E1000_RCTL_UPE         (1u << 3)
#define E1000_RCTL_MPE         (1u << 4)
#define E1000_RCTL_BAM         (1u << 15)
#define E1000_RCTL_BSIZE       0x00030000u
#define E1000_RCTL_BSEX        (1u << 25)

#define E1000_TCTL_EN          (1u << 1)
#define E1000_TCTL_PSP         (1u << 3)

#define E1000_RAH_AV           (1u << 31)

#define E1000_IVAR_VALID       (1u << 15)

#define E1000_EICR_RX0         (1u << 0)
#define E1000_EICR_TX0         (1u << 1)
#define E1000_EICR_OTHER       (1u << 2)

#define E1000_RXDESC_NR         256
#define E1000_TXDESC_NR         256
#define E1000_IOBUF_SIZE        16384
#define E1000_EERD_READ_TIMEOUT 100000

#define E1000_IVAR_RX0     0
#define E1000_IVAR_TX0     2
#define E1000_IVAR_RX1     4
#define E1000_IVAR_TX1     6
#define E1000_IVAR_OTHER   8

#ifdef __cplusplus
}
#endif

#endif /* E1000_CORE_H */
