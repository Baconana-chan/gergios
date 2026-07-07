/* SPDX-License-Identifier: BSD-3-Clause */
/*
 * virtio_net.h — Rust virtio-net driver FFI header
 *
 * This header declares the C-callable entry point exported by the
 * virtio-net Rust crate.
 */

#ifndef VIRTIO_NET_H
#define VIRTIO_NET_H

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Main entry point for the Rust virtio-net driver.
 * Called from a C shim or directly from the init system.
 */
int virtio_net_rust_main(int argc, char *argv[]);

#ifdef __cplusplus
}
#endif

#endif /* !VIRTIO_NET_H */
