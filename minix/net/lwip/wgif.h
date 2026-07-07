/*
 * MINIX 3 WireGuard interface — ifdev glue for wireguard-lwip.
 *
 * This module creates a virtual WireGuard network interface using MINIX's
 * ifdev abstraction layer, wrapping the wireguard-lwip library's netif-based
 * interface.
 */
#ifndef MINIX_NET_LWIP_WGIF_H
#define MINIX_NET_LWIP_WGIF_H

#include <stdint.h>

struct wgif;

void wgif_init(void);

/*
 * Look up a WireGuard interface by name (e.g. "wg0").
 * If name is NULL, return the first configured interface.
 * Returns NULL if not found.
 */
struct wgif *wgif_find_by_name(const char *name);

/*
 * Configure a WireGuard interface with a private key and listen port.
 */
int wgif_configure(struct wgif *wgif, const uint8_t private_key[32],
    uint16_t listen_port);

/*
 * Add a peer to a WireGuard interface.
 */
int wgif_add_peer(struct wgif *wgif,
    const struct wireguardif_peer *peer);

/*
 * Remove a peer from a WireGuard interface.
 */
void wgif_remove_peer(struct wgif *wgif, uint8_t peer_index);

/*
 * Get the ifdev structure associated with a WireGuard interface.
 * The ifdev is always the first field of struct wgif for cast safety.
 */
struct ifdev *wgif_get_ifdev(struct wgif *wgif);

#endif /* !MINIX_NET_LWIP_WGIF_H */
