/*
 * MINIX 3 WireGuard sysctl interface — implementation.
 *
 * Provides runtime configuration of WireGuard interfaces via the
 * minix.lwip.wireguard.* sysctl tree.
 */
#include "lwip.h"

#if LWIP_WIREGUARD

#include "wg_sysctl.h"
#include "wgif.h"

#include <string.h>

#include "wireguardif.h"
#include "wireguard.h"

/*
 * Global WireGuard enable toggle.  When disabled, all WireGuard interfaces
 * stop processing packets but retain their configuration.
 */
int lwip_wireguard_enabled = 1;

/* ------------------------------------------------------------------ */
/*  Helper: parse an IP address string into an ip_addr_t               */
/* ------------------------------------------------------------------ */

/*
 * Parse a dotted-decimal (IPv4) or colon-hex (IPv6) address string into
 * an ip_addr_t.  Returns 0 on success, -1 on failure.
 */
static int
wg_parse_addr(const char *str, ip_addr_t *addr)
{

	if (str == NULL || str[0] == '\0') {
		ip_addr_set_any(LWIP_IPV6, addr);
		return 0;
	}

	if (ipaddr_aton(str, addr))
		return 0;

	return -1;
}

/* ------------------------------------------------------------------ */
/*  Convert ip_addr_t back to string (for status read)                 */
/* ------------------------------------------------------------------ */

static void
wg_addr_to_str(const ip_addr_t *addr, char *buf, size_t bufsize)
{

	if (ip_addr_isany(addr)) {
		buf[0] = '\0';
		return;
	}

	ipaddr_ntoa_r(addr, buf, (int)bufsize);
}

/* ------------------------------------------------------------------ */
/*  Sysctl handler — all-in-one command dispatch                       */
/* ------------------------------------------------------------------ */

/*
 * Handle read/write on the minix.lwip.wireguard.cfg sysctl node.
 *
 * Write:  pass a struct wg_sysctl_req with the desired command.
 * Read:   returns a snapshot of the first wg interface's peer status.
 */
static ssize_t
wg_sysctl_cfg(struct rmib_call *call __unused,
    struct rmib_node *node __unused,
    struct rmib_oldp *oldp, struct rmib_newp *newp)
{
	struct wg_sysctl_req req;
	struct wgif *wgif;
	int r;

	/*
	 * If writing (newp != NULL), copy in the request and dispatch.
	 */
	if (newp != NULL) {
		if ((r = rmib_copyin(newp, &req, sizeof(req))) != OK)
			return r;

		/* Find the WireGuard interface by name. */
		wgif = wgif_find_by_name(req.ifname);
		if (wgif == NULL)
			return ENOENT;

		switch (req.cmd) {
		case WG_CMD_CONFIGURE:
			r = wgif_configure(wgif, req.private_key,
			    req.listen_port);
			break;

		case WG_CMD_ADD_PEER: {
			struct wireguardif_peer peer;
			ip_addr_t endpoint, allowed_ip, allowed_mask;
			char b64[48];
			size_t b64len;
			int psk_nonzero, pk_i;

			wireguardif_peer_init(&peer);

			/*
			 * wireguard-lwip's wireguardif_add_peer() expects the
			 * public key as a base64-encoded string.  We accept raw
			 * 32-byte keys here and convert inline.
			 * ceil(32 / 3) * 4 + 1 = 45 bytes needed for b64.
			 * b64 lives in the outer block (same scope as peer)
			 * so that peer.public_key remains valid.
			 */
			b64len = sizeof(b64);
			wireguard_base64_encode(req.public_key,
			    sizeof(req.public_key), b64, &b64len);
			peer.public_key = b64;

			/*
			 * Optional pre-shared key.  If the user provided a non-
			 * zero preshared_key, pass a pointer to it.  Otherwise
			 * wireguardif_peer_init() already set it to NULL.
			 */
			psk_nonzero = 0;
			for (pk_i = 0; pk_i < 32; pk_i++) {
				if (req.preshared_key[pk_i] != 0) {
					psk_nonzero = 1;
					break;
				}
			}
			if (psk_nonzero)
				peer.preshared_key = req.preshared_key;

			/* Parse endpoint, allowed IPs from strings. */
			if (wg_parse_addr(req.endpoint_addr,
			    &endpoint) != 0) {
				ip_addr_set_any(LWIP_IPV6, &endpoint);
			}
			peer.endpoint_ip = endpoint;
			peer.endpoint_port = req.endpoint_port;

			if (wg_parse_addr(req.allowed_addr,
			    &allowed_ip) != 0) {
				ip_addr_set_any(LWIP_IPV6, &allowed_ip);
			}
			peer.allowed_ip = allowed_ip;

			if (wg_parse_addr(req.allowed_mask,
			    &allowed_mask) != 0) {
				ip_addr_set_any(LWIP_IPV6, &allowed_mask);
			}
			peer.allowed_mask = allowed_mask;

			peer.keep_alive = req.keep_alive;

			r = wgif_add_peer(wgif, &peer);
			break;
		}

		case WG_CMD_REMOVE_PEER:
			wgif_remove_peer(wgif, req.peer_index);
			r = OK;
			break;

		case WG_CMD_CONNECT:
		case WG_CMD_DISCONNECT: {
			struct netif *netif;

			netif = ifdev_get_netif(wgif_get_ifdev(wgif));
			if (netif == NULL) {
				r = EINVAL;
				break;
			}

			if (req.cmd == WG_CMD_CONNECT)
				r = wireguardif_connect(netif,
				    req.peer_index);
			else
				r = wireguardif_disconnect(netif,
				    req.peer_index);
			break;
		}

		default:
			r = EINVAL;
			break;
		}

		return r;
	}

	/*
	 * If reading (oldp != NULL), we do not currently implement a full
	 * status read (the struct is write-only).  Return EOPNOTSUPP so that
	 * the caller knows this is a write-only command interface.
	 * TODO: implement read to return peer status.
	 */
	if (oldp != NULL)
		return EOPNOTSUPP;

	/* Both oldp and newp are NULL — just return size. */
	return sizeof(req);
}

/* ------------------------------------------------------------------ */
/*  RMIB tree registration                                            */
/* ------------------------------------------------------------------ */

static struct rmib_node minix_lwip_wireguard_table[] = {
	[0] = RMIB_INTPTR(RMIB_RW, &lwip_wireguard_enabled,
	    "enabled", "Enable WireGuard VPN interfaces"),
	[1] = RMIB_FUNC(RMIB_RW, sizeof(struct wg_sysctl_req),
	    wg_sysctl_cfg, "cfg",
	    "WireGuard interface configuration command"),
};

static struct rmib_node minix_lwip_wireguard_node =
    RMIB_NODE(RMIB_RO, minix_lwip_wireguard_table, "wireguard",
    "WireGuard VPN settings");

/* ------------------------------------------------------------------ */
/*  Initialisation                                                     */
/* ------------------------------------------------------------------ */

void
wg_sysctl_init(void)
{

	mibtree_register_lwip(&minix_lwip_wireguard_node);
}

#else /* !LWIP_WIREGUARD */

void
wg_sysctl_init(void)
{
}

#endif /* LWIP_WIREGUARD */
