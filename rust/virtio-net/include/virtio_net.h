/* rust/virtio-net/include/virtio_net.h
 *
 * Virtio-net / Legacy Virtio Device Constants
 * — C header for driver FFI tests.
 *
 * All constants are verified against the Rust implementation in
 * rust/virtio-net/src/device.rs, queue.rs, and net.rs.
 * C FFI test functions are exported from the virtio_net staticlib.
 *
 * Usage:
 *   #include <virtio_net.h>
 *   // link against: -lvirtio_net (libvirtio_net.a)
 */

#ifndef VIRTIO_NET_CORE_H
#define VIRTIO_NET_CORE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===================================================================
 * Version
 * =================================================================== */

/** Return version code: 0x00010000 (legacy virtio 0.9.5). */
uint32_t virtio_test_version(void);

/* ===================================================================
 * Register Byte Offsets (Legacy I/O Port BAR)
 * =================================================================== */

/** Return register offset by numeric ID. */
uint32_t virtio_test_reg_offset(uint32_t reg_id);

/* Register ID constants for virtio_test_reg_offset(). */
#define VIRTIO_REG_HOST_F_OFF      0   /* Host features (0x0000) */
#define VIRTIO_REG_GUEST_F_OFF     1   /* Guest features (0x0004) */
#define VIRTIO_REG_QADDR_OFF       2   /* Queue PFN (0x0008) */
#define VIRTIO_REG_QSIZE_OFF       3   /* Queue size (0x000C) */
#define VIRTIO_REG_QSEL_OFF        4   /* Queue select (0x000E) */
#define VIRTIO_REG_QNOTIFY_OFF     5   /* Queue notify (0x0010) */
#define VIRTIO_REG_DEV_STATUS_OFF  6   /* Device status (0x0012) */
#define VIRTIO_REG_ISR_STATUS_OFF  7   /* ISR status (0x0013) */
#define VIRTIO_REG_DEV_SPECIFIC_OFF 8  /* Device config (0x0014) */

/* Expected byte offsets. */
#define VIRTIO_HOST_F_OFF      0x0000
#define VIRTIO_GUEST_F_OFF     0x0004
#define VIRTIO_QADDR_OFF       0x0008
#define VIRTIO_QSIZE_OFF       0x000C
#define VIRTIO_QSEL_OFF        0x000E
#define VIRTIO_QNOTIFY_OFF     0x0010
#define VIRTIO_DEV_STATUS_OFF  0x0012
#define VIRTIO_ISR_STATUS_OFF  0x0013
#define VIRTIO_DEV_SPECIFIC_OFF 0x0014

/* ===================================================================
 * Bitfield / Constant Constants
 * =================================================================== */

/** Return constant value by numeric ID. */
uint32_t virtio_test_bitfield(uint32_t bf_id);

/* Bitfield IDs for virtio_test_bitfield(). */
#define VIRTIO_BF_STATUS_ACK          0   /* Device status: ACK (0x01) */
#define VIRTIO_BF_STATUS_DRV          1   /* Device status: DRV (0x02) */
#define VIRTIO_BF_STATUS_DRV_OK       2   /* Device status: DRV_OK (0x04) */
#define VIRTIO_BF_STATUS_FAIL         3   /* Device status: FAIL (0x80) */
#define VIRTIO_BF_F_INDIRECT_DESC     4   /* Feature: INDIRECT_DESC (28) */
#define VIRTIO_BF_VRING_DESC_F_NEXT   5   /* Vring: NEXT flag (1) */
#define VIRTIO_BF_VRING_DESC_F_WRITE  6   /* Vring: WRITE flag (2) */
#define VIRTIO_BF_VRING_DESC_F_INDIRECT 7 /* Vring: INDIRECT flag (4) */
#define VIRTIO_BF_VRING_DESC_SIZE     8   /* VringDesc struct size (16) */
#define VIRTIO_BF_VRING_USED_ELEM_SIZE 9  /* VringUsedElem size (8) */
#define VIRTIO_BF_NET_F_CSUM         10   /* Net: CSUM feature (0) */
#define VIRTIO_BF_NET_F_GUEST_CSUM   11   /* Net: GUEST_CSUM (1) */
#define VIRTIO_BF_NET_F_MAC          12   /* Net: MAC feature (5) */
#define VIRTIO_BF_NET_F_GSO          13   /* Net: GSO feature (6) */
#define VIRTIO_BF_NET_F_STATUS       14   /* Net: STATUS feature (16) */
#define VIRTIO_BF_NET_F_CTRL_VQ      15   /* Net: CTRL_VQ (17) */
#define VIRTIO_BF_NET_F_MRG_RXBUF    16   /* Net: MRG_RXBUF (15) */
#define VIRTIO_BF_NET_S_LINK_UP      17   /* Net: LINK_UP status (1) */
#define VIRTIO_BF_NET_S_ANNOUNCE     18   /* Net: ANNOUNCE (2) */
#define VIRTIO_BF_VIRTIO_NET_HDR_SIZE      19  /* VirtioNetHdr size (10) */
#define VIRTIO_BF_VIRTIO_NET_HDR_MRG_SIZE  20  /* VirtioNetHdrMrgRxbuf (12) */
#define VIRTIO_BF_HDR_F_NEEDS_CSUM   21   /* Hdr flag: NEEDS_CSUM (1) */
#define VIRTIO_BF_HDR_F_DATA_VALID   22   /* Hdr flag: DATA_VALID (2) */
#define VIRTIO_BF_HDR_GSO_NONE       23   /* GSO: NONE (0) */
#define VIRTIO_BF_HDR_GSO_TCPV4      24   /* GSO: TCPV4 (1) */
#define VIRTIO_BF_HDR_GSO_TCPV6      25   /* GSO: TCPV6 (4) */
#define VIRTIO_BF_HDR_GSO_ECN        26   /* GSO: ECN (0x80) */
#define VIRTIO_BF_RX_Q               27   /* Queue: RX (0) */
#define VIRTIO_BF_TX_Q               28   /* Queue: TX (1) */
#define VIRTIO_BF_CTRL_Q             29   /* Queue: CTRL (2) */
#define VIRTIO_BF_BUF_PACKETS        30   /* Driver: BUF_PACKETS (64) */
#define VIRTIO_BF_MAX_PACK_SIZE      31   /* Driver: MAX_PACK_SIZE (1514) */

/* Expected values. */
#define VIRTIO_STATUS_ACK              0x01
#define VIRTIO_STATUS_DRV              0x02
#define VIRTIO_STATUS_DRV_OK           0x04
#define VIRTIO_STATUS_FAIL             0x80
#define VIRTIO_F_INDIRECT_DESC         28
#define VIRTIO_VRING_DESC_F_NEXT         1u
#define VIRTIO_VRING_DESC_F_WRITE        2u
#define VIRTIO_VRING_DESC_F_INDIRECT     4u
#define VIRTIO_VRING_DESC_SIZE           16
#define VIRTIO_VRING_USED_ELEM_SIZE      8
#define VIRTIO_NET_F_CSUM                0
#define VIRTIO_NET_F_GUEST_CSUM          1
#define VIRTIO_NET_F_MAC                 5
#define VIRTIO_NET_F_GSO                 6
#define VIRTIO_NET_F_STATUS             16
#define VIRTIO_NET_F_CTRL_VQ            17
#define VIRTIO_NET_F_MRG_RXBUF          15
#define VIRTIO_NET_S_LINK_UP             1u
#define VIRTIO_NET_S_ANNOUNCE            2u
#define VIRTIO_VIRTIO_NET_HDR_SIZE       10
#define VIRTIO_VIRTIO_NET_HDR_MRG_SIZE   12
#define VIRTIO_HDR_F_NEEDS_CSUM          1u
#define VIRTIO_HDR_F_DATA_VALID          2u
#define VIRTIO_HDR_GSO_NONE              0
#define VIRTIO_HDR_GSO_TCPV4             1
#define VIRTIO_HDR_GSO_TCPV6             4
#define VIRTIO_HDR_GSO_ECN            0x80
#define VIRTIO_RX_Q                      0
#define VIRTIO_TX_Q                      1
#define VIRTIO_CTRL_Q                    2
#define VIRTIO_BUF_PACKETS              64
#define VIRTIO_MAX_PACK_SIZE          1514

#ifdef __cplusplus
}
#endif

#endif /* VIRTIO_NET_CORE_H */
