/*
 * MINIX 3 platform adaptation for wireguard-lwip.
 *
 * This header provides the platform-specific functions required by the
 * wireguard-lwip protocol core: system clock, random bytes, TAI64N time,
 * and load indicator.
 */
#ifndef WIREGUARD_PLATFORM_H
#define WIREGUARD_PLATFORM_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#include "lwip/sys.h"

/*
 * Maximum number of WireGuard peers supported by a single interface.
 * Setting this to 4 allows for typical BGP/ VPN setups.
 */
#define WIREGUARD_MAX_PEERS               4

/*
 * Maximum number of allowed source IPs per peer.
 */
#define WIREGUARD_MAX_SRC_IPS             8

/*
 * Maximum number of initiation messages per second (DoS protection).
 */
#define MAX_INITIATIONS_PER_SECOND        2

/*
 * Return the number of milliseconds since system boot.
 * Implemented in wgif.c (maps to lwIP's sys_now()).
 */
uint32_t wireguard_sys_now(void);

/*
 * Fill a buffer with cryptographically secure random bytes.
 * Implemented in wireguard-platform.c.
 * Uses a ChaCha20 DRBG seeded from the kernel entropy pool via
 * sys_getrandomness().  Reseeds automatically after 1MB of output.
 */
void wireguard_random_bytes(void *bytes, size_t size);

/*
 * Get the current time in TAI64N format (12 bytes: 8-byte seconds,
 * 4-byte nanoseconds).  Used for handshake replay protection.
 * Falls back to system uptime if wall clock is not available.
 */
void wireguard_tai64n_now(uint8_t *output);

/*
 * Return TRUE if the system is currently under heavy load.
 * When under load, cookie reply messages are generated in response
 * to initiation requests to prevent handshake amplification attacks.
 */
bool wireguard_is_under_load(void);

#endif /* !WIREGUARD_PLATFORM_H */
