/*
 * MINIX 3 WireGuard sysctl interface.
 *
 * Provides sysctl entries under minix.lwip.wireguard for configuring
 * WireGuard interfaces and peers at runtime.
 */
#ifndef MINIX_NET_LWIP_WG_SYSCTL_H
#define MINIX_NET_LWIP_WG_SYSCTL_H

/*
 * Maximum length of an IP address string (including IPv6 and NUL).
 */
#define WG_ADDR_STRLEN	48

/*
 * WireGuard sysctl commands.
 */
#define WG_CMD_CONFIGURE	1	/* set private key + listen port */
#define WG_CMD_ADD_PEER		2	/* add a peer */
#define WG_CMD_REMOVE_PEER	3	/* remove a peer by index */
#define WG_CMD_CONNECT		4	/* connect to a peer */
#define WG_CMD_DISCONNECT	5	/* disconnect from a peer */

/*
 * Sysctl request structure.
 *
 * To perform a command, write this struct to the minix.lwip.wireguard.cfg
 * sysctl node with the relevant fields filled in.
 *
 * Example usage (from userland C):
 *
 *   struct wg_sysctl_req req;
 *
 *   // Configure interface wg0
 *   memset(&req, 0, sizeof(req));
 *   strcpy(req.ifname, "wg0");
 *   req.cmd = WG_CMD_CONFIGURE;
 *   hex2bin("01234567...", req.private_key, 32);
 *   req.listen_port = 51820;
 *   sysctlbyname("minix.lwip.wireguard.cfg", NULL, 0, &req, sizeof(req));
 *
 *   // Add a peer
 *   memset(&req, 0, sizeof(req));
 *   strcpy(req.ifname, "wg0");
 *   req.cmd = WG_CMD_ADD_PEER;
 *   hex2bin("abcdef...", req.public_key, 32);
 *   strcpy(req.endpoint_addr, "192.168.1.100");
 *   req.endpoint_port = 51820;
 *   strcpy(req.allowed_addr, "10.0.0.0");
 *   strcpy(req.allowed_mask, "255.255.255.0");
 *   req.keep_alive = 25;
 *   sysctlbyname("minix.lwip.wireguard.cfg", NULL, 0, &req, sizeof(req));
 */
struct wg_sysctl_req {
	/* Interface name (e.g. "wg0"), must be set for all commands. */
	char		ifname[IFNAMSIZ];

	/* Command to execute (WG_CMD_*). */
	uint32_t	cmd;

	/* --- WG_CMD_CONFIGURE --- */
	uint8_t		private_key[32];
	uint16_t	listen_port;

	/* --- WG_CMD_REMOVE_PEER / CONNECT / DISCONNECT --- */
	uint8_t		peer_index;

	/* --- WG_CMD_ADD_PEER --- */
	uint8_t		public_key[32];
	uint8_t		preshared_key[32];
	char		endpoint_addr[WG_ADDR_STRLEN];
	uint16_t	endpoint_port;
	char		allowed_addr[WG_ADDR_STRLEN];
	char		allowed_mask[WG_ADDR_STRLEN];
	uint16_t	keep_alive;
};

/*
 * Initialize the WireGuard sysctl tree and register it with the MIB module.
 * Must be called before mibtree_init().
 */
void wg_sysctl_init(void);

#endif /* !MINIX_NET_LWIP_WG_SYSCTL_H */
