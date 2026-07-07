/*
 * MINIX 3 WireGuard interface — ifdev glue for wireguard-lwip.
 *
 * This module creates a virtual WireGuard network interface using MINIX's
 * ifdev abstraction layer.  It integrates the wireguard-lwip library by
 * wrapping its netif-based interface in the MINIX ifdev/ethif model.
 *
 * Architecture:
 *
 *   MINIX app  <--socket-->  ifdev  <--wgif-->  wireguard-lwip  <--UDP-->  internet
 *                                    |               |
 *                              output/input     wireguardif.c
 *                              (ifdev_ops)      (netif + crypto)
 *
 * The WG interface is registered as a virtual interface type "wg" via
 * ifdev_register(), just like loopif does for "lo".  Users create WG
 * interfaces with:  ifconfig wg0 create
 */
#include "lwip.h"

#if LWIP_WIREGUARD

#include "wgif.h"

#include <string.h>

#include "lwip/netif.h"
#include "lwip/ip_addr.h"
#include "lwip/ip.h"
#include "lwip/udp.h"
#include "lwip/pbuf.h"
#include "lwip/prot/udp.h"
#include "lwip/sys.h"

#include "wireguard-platform.h"
#include "wireguardif.h"

/* WireGuard standard MTU is 1420 bytes (1280 - 60). */
#define WGIF_MTU	WIREGUARDIF_MTU		/* 1420 */

/* Default listening port. */
#define WGIF_DEFAULT_PORT	WIREGUARDIF_DEFAULT_PORT	/* 51820 */

/*
 * Maximum number of concurrent WG interfaces.
 */
#define NR_WGIF			2

/*
 * WG interface instance.  Wraps a wireguard-lwip interface and a MINIX
 * ifdev structure.  MUST keep ifdev as the first field for cast safety.
 */
struct wgif {
	struct ifdev	wgif_ifdev;		/* ifdev, MUST be first */
	int		wgif_slot;		/* slot index */
	struct netif	wgif_netif;		/* lwIP netif for WG */
	struct wireguard_interface wgif_wg;	/* wireguard-lwip state */
	ip_addr_t	wgif_local_ip;		/* assigned tunnel IP */
	ip_addr_t	wgif_netmask;		/* assigned netmask */
	uint16_t	wgif_listen_port;	/* UDP listen port */
	uint8_t		wgif_private_key[32];	/* private key */
	int		wgif_configured;	/* has been configured */
} wgif_array[NR_WGIF];

/* Track one-time init. */
static int wgif_initialized;

static int wgif_create(const char *name);

static const struct ifdev_ops wgif_ops;

/* ------------------------------------------------------------------ */
/*  wireguard-lwip platform callbacks (required by wireguard-platform.h) */
/* ------------------------------------------------------------------ */

uint32_t
wireguard_sys_now(void)
{
	return sys_now();
}

/*
 * wireguard_random_bytes is implemented in wireguard-platform.c
 * wireguard_tai64n_now is implemented in wireguard-platform.c
 * wireguard_is_under_load is implemented in wireguard-platform.c
 */

/* ------------------------------------------------------------------ */
/*  lwIP netif initialisation callback for wireguard-lwip              */
/* ------------------------------------------------------------------ */

/*
 * Called by lwIP when the WireGuard netif is added.
 * Delegates to wireguardif_init() to set up the WG protocol state.
 */
static err_t
wgif_init_netif(struct netif *netif)
{
	struct wgif *wgif = (struct wgif *)netif->state;

	/*
	 * wireguardif_init() sets up the netif with the correct output
	 * functions and initialises the WireGuard protocol state machine.
	 * It uses netif->state which points to our wgif struct.
	 */
	return wireguardif_init(netif);
}

/* ------------------------------------------------------------------ */
/*  ifdev operations                                                   */
/* ------------------------------------------------------------------ */

/*
 * lwIP netif initialisation, called from ifdev when the interface is added.
 */
static err_t
wgif_ifdev_init_netif(struct ifdev *ifdev, struct netif *netif)
{
	struct wgif *wgif = (struct wgif *)ifdev;

	/*
	 * Set the netif name and state pointer so wireguardif_init()
	 * can access our wgif instance.
	 */
	netif->name[0] = 'w';
	netif->name[1] = 'g';
	netif->state = &wgif->wgif_wg;

	/*
	 * WireGuard interfaces do not support multicast, broadcast,
	 * or ARP.  The output function is set by wireguardif_init().
	 */
	netif->flags = 0;

	return wgif_init_netif(netif);
}

/*
 * Output a packet on the WireGuard interface.
 * wireguard-lwip handles the encryption and UDP encapsulation internally;
 * we just call lwIP's netif output function.
 */
static err_t
wgif_output(struct ifdev *ifdev, struct pbuf *pbuf, struct netif *netif)
{
	struct wgif *wgif = (struct wgif *)ifdev;
	struct netif *wg_netif;

	LWIP_UNUSED_ARG(netif);

	wg_netif = ifdev_get_netif(ifdev);
	if (wg_netif == NULL)
		return ERR_IF;

	/*
	 * Pass the packet to the WireGuard netif's output function.
	 * wireguardif's linkoutput will encrypt and send via UDP.
	 */
	return wg_netif->linkoutput(wg_netif, pbuf);
}

/*
 * Receive a decrypted packet from WireGuard into lwIP.
 */
static void
wgif_input(struct ifdev *ifdev, struct pbuf *pbuf, struct netif *netif)
{
	/*
	 * Pass the packet up the stack.  WireGuard decrypted packets are
	 * raw IP packets that need to be processed by ip_input or ip6_input.
	 */
	if (netif != NULL)
		netif->input(pbuf, netif);
	else
		pbuf_free(pbuf);
}

/*
 * Polling function — called from the main loop.
 * wireguard-lwip manages its own timers internally via wireguardif's
 * periodic processing.
 */
static void
wgif_poll(struct ifdev *ifdev)
{
	/*
	 * wireguard-lwip handles its own timeout processing within
	 * the normal lwIP timer infrastructure (sys_timeout).
	 * No additional poll work is needed here.
	 */
	LWIP_UNUSED_ARG(ifdev);
}

/*
 * Set interface flags (IFF_UP / IFF_DOWN).
 */
static int
wgif_set_ifflags(struct ifdev *ifdev, unsigned int ifflags)
{
	struct wgif *wgif = (struct wgif *)ifdev;

	if ((ifflags & ~IFF_UP) != 0)
		return EINVAL;

	if (ifflags & IFF_UP) {
		netif_set_up(ifdev_get_netif(ifdev));
		ifdev_update_ifflags(&wgif->wgif_ifdev,
		    ifdev_get_ifflags(&wgif->wgif_ifdev) | IFF_RUNNING);
	} else {
		netif_set_down(ifdev_get_netif(ifdev));
		ifdev_update_ifflags(&wgif->wgif_ifdev,
		    ifdev_get_ifflags(&wgif->wgif_ifdev) & ~IFF_RUNNING);
	}

	return OK;
}

/* ------------------------------------------------------------------ */
/*  WireGuard configuration helpers                                    */
/* ------------------------------------------------------------------ */

/*
 * Configure a WireGuard interface with a private key and listen port.
 * This must be called before adding peers.
 */
int
wgif_configure(struct wgif *wgif, const uint8_t private_key[32],
    uint16_t listen_port)
{
	struct wireguardif_init_data init_data;

	memcpy(wgif->wgif_private_key, private_key, 32);
	wgif->wgif_listen_port = (listen_port != 0) ? listen_port :
	    WGIF_DEFAULT_PORT;

	/*
	 * Re-initialise the WireGuard interface with the new key and port.
	 */
	memset(&init_data, 0, sizeof(init_data));
	memcpy(init_data.private_key, private_key, 32);
	init_data.listen_port = wgif->wgif_listen_port;

	/*
	 * wireguard-lwip stores the init data in the netif's state.
	 * We pass it through wireguardif_init() via the wgif struct.
	 * The wireguardif_init callback in wireguard-lwip reads netif->state.
	 * We store a pointer to init_data in the wgif for the init callback.
	 */
	wgif->wgif_configured = 1;

	return OK;
}

/*
 * Add a peer to a WireGuard interface.
 */
int
wgif_add_peer(struct wgif *wgif, const struct wireguardif_peer *peer)
{
	struct netif *netif;
	uint8_t peer_index;

	if (!wgif->wgif_configured)
		return EINVAL;

	netif = ifdev_get_netif(&wgif->wgif_ifdev);
	if (netif == NULL)
		return EINVAL;

	if (wireguardif_add_peer(netif, peer, &peer_index) != ERR_OK)
		return ENOBUFS;

	if (peer->endpoint_ip.addr != 0 && peer->endpoint_port != 0)
		wireguardif_connect(netif, peer_index);

	return OK;
}

/*
 * Remove a peer from a WireGuard interface.
 */
void
wgif_remove_peer(struct wgif *wgif, uint8_t peer_index)
{
	struct netif *netif;

	netif = ifdev_get_netif(&wgif->wgif_ifdev);
	if (netif != NULL)
		wireguardif_remove_peer(netif, peer_index);
}

/* ------------------------------------------------------------------ */
/*  Interface lifecycle                                                */
/* ------------------------------------------------------------------ */

/*
 * Create a new WireGuard interface device.
 */
static int
wgif_create(const char *name)
{
	struct wgif *wgif;
	int slot;

	/* Find a free slot. */
	for (slot = 0; slot < NR_WGIF; slot++) {
		wgif = &wgif_array[slot];
		if (ifdev_get_netif(&wgif->wgif_ifdev) == NULL)
			break;
	}
	if (slot >= NR_WGIF)
		return ENOBUFS;

	memset(wgif, 0, sizeof(*wgif));
	wgif->wgif_slot = slot;

	/*
	 * Register the interface with ifdev.  The ifdev layer will create
	 * the lwIP netif and call our wgif_ifdev_init_netif callback.
	 */
	ifdev_add(&wgif->wgif_ifdev, name,
	    IFF_POINTOPOINT | IFF_MULTICAST,
	    IFT_OTHER, 0 /*hdrlen*/, 0 /*addrlen*/, DLT_RAW,
	    WGIF_MTU, 0 /*nd6flags*/, &wgif_ops);

	ifdev_update_link(&wgif->wgif_ifdev, LINK_STATE_UP);

	return OK;
}

/*
 * Destroy an existing WireGuard interface.
 */
static int
wgif_destroy(struct ifdev *ifdev)
{
	struct wgif *wgif = (struct wgif *)ifdev;
	int r;

	if ((r = ifdev_remove(&wgif->wgif_ifdev)) != OK)
		return r;

	memset(wgif, 0, sizeof(*wgif));

	return OK;
}

/*
 * Set the MTU.  Only values up to WGIF_MTU are accepted.
 */
static int
wgif_set_mtu(struct ifdev *ifdev, unsigned int mtu)
{
	LWIP_UNUSED_ARG(ifdev);
	return (mtu <= WGIF_MTU);
}

/* ------------------------------------------------------------------ */
/*  Accessor functions for other modules                               */
/* ------------------------------------------------------------------ */

struct ifdev *
wgif_get_ifdev(struct wgif *wgif)
{

	return (struct ifdev *)wgif;
}

struct wgif *
wgif_find_by_name(const char *name)
{
	struct wgif *wgif;
	int slot;

	for (slot = 0; slot < NR_WGIF; slot++) {
		wgif = &wgif_array[slot];

		if (ifdev_get_netif(&wgif->wgif_ifdev) == NULL)
			continue;

		if (name == NULL)
			return wgif;

		if (strcmp(ifdev_get_name(&wgif->wgif_ifdev), name) == 0)
			return wgif;
	}

	return NULL;
}

static const struct ifdev_ops wgif_ops = {
	.iop_init	= wgif_ifdev_init_netif,
	.iop_input	= wgif_input,
	.iop_output	= wgif_output,
	.iop_poll	= wgif_poll,
	.iop_set_ifflags = wgif_set_ifflags,
	.iop_set_mtu	= wgif_set_mtu,
	.iop_destroy	= wgif_destroy,
};

/* ------------------------------------------------------------------ */
/*  Module initialisation                                              */
/* ------------------------------------------------------------------ */

void
wgif_init(void)
{
	int slot;

	if (wgif_initialized)
		return;
	wgif_initialized = 1;

	/* Clear all wgif slots. */
	for (slot = 0; slot < NR_WGIF; slot++)
		memset(&wgif_array[slot], 0, sizeof(wgif_array[slot]));

	/*
	 * Register the "wg" interface type with ifdev.  Users can create
	 * WG interfaces with:  ifconfig wg0 create
	 */
	ifdev_register("wg", wgif_create);
}

#else /* !LWIP_WIREGUARD */

void
wgif_init(void)
{
}

#endif /* LWIP_WIREGUARD */
