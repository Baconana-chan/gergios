/*
 * wg-quick — WireGuard interface auto-configuration for MINIX 3.
 *
 * Reads a WireGuard configuration file from /etc/wireguard/<ifname>.conf
 * and configures the interface via the minix.lwip.wireguard.cfg sysctl
 * node and ifconfig.
 *
 * Usage:  wg-quick up   <ifname>   (bring up interface)
 *         wg-quick down <ifname>   (tear down interface)
 *
 * Config file format (standard WireGuard):
 *
 *   [Interface]
 *   PrivateKey = <base64>
 *   ListenPort = <port>
 *   Address = <CIDR>
 *
 *   [Peer]
 *   PublicKey = <base64>
 *   PresharedKey = <base64>    (optional)
 *   Endpoint = <host>:<port>
 *   AllowedIPs = <CIDR[,CIDR]>
 *   PersistentKeepalive = <sec> (optional)
 */

#include <sys/types.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <sys/sysctl.h>

#include <net/if.h>
#include <netinet/in.h>
#include <arpa/inet.h>

#include <ctype.h>
#include <err.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <unistd.h>

/* ------------------------------------------------------------------ */
/*  WireGuard sysctl command interface (mirrors kernel struct)         */
/* ------------------------------------------------------------------ */

#define WG_IFNAME_MAX		16
#define WG_ADDR_STRLEN		48
#define WG_KEY_LEN		32

#define WG_CMD_CONFIGURE	1
#define WG_CMD_ADD_PEER		2
#define WG_CMD_REMOVE_PEER	3
#define WG_CMD_CONNECT		4
#define WG_CMD_DISCONNECT	5

struct wg_sysctl_req {
	char		ifname[WG_IFNAME_MAX];
	uint32_t	cmd;
	uint8_t		private_key[WG_KEY_LEN];
	uint16_t	listen_port;
	uint8_t		peer_index;
	uint8_t		public_key[WG_KEY_LEN];
	uint8_t		preshared_key[WG_KEY_LEN];
	char		endpoint_addr[WG_ADDR_STRLEN];
	uint16_t	endpoint_port;
	char		allowed_addr[WG_ADDR_STRLEN];
	char		allowed_mask[WG_ADDR_STRLEN];
	uint16_t	keep_alive;
};

/* ------------------------------------------------------------------ */
/*  Base64 decoding (RFC 4648)                                         */
/* ------------------------------------------------------------------ */

static const char b64_chars[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static int
b64_decode(const char *in, uint8_t *out, size_t outlen)
{
	int i, j, pad;
	uint32_t v;
	unsigned char d[4];
	const char *p;

	if (in == NULL || out == NULL)
		return -1;

	i = 0;
	j = 0;
	pad = 0;

	while (in[i] != '\0') {
		/* Skip whitespace. */
		if (in[i] == ' ' || in[i] == '\t' || in[i] == '\n' ||
		    in[i] == '\r') {
			i++;
			continue;
		}

		/* Read 4 characters. */
		for (j = 0; j < 4; j++) {
			if (in[i] == '=') {
				pad++;
				d[j] = 0;
				i++;
			} else if (in[i] == '\0') {
				if (j == 0)
					goto done;
				/* Premature end — pad with zeros. */
				d[j] = 0;
				pad = 4 - j;
			} else {
				p = strchr(b64_chars, in[i]);
				if (p == NULL)
					return -1;
				d[j] = (unsigned char)(p - b64_chars);
				i++;
			}
		}

		v = (d[0] << 18) | (d[1] << 12) | (d[2] << 6) | d[3];

		if (outlen >= 3) {
			*out++ = (v >> 16) & 0xFF;
			outlen--;
		}
		if (pad < 2 && outlen >= 1) {
			*out++ = (v >> 8) & 0xFF;
			outlen--;
		}
		if (pad < 1 && outlen >= 1) {
			*out++ = v & 0xFF;
			outlen--;
		}
	}

done:
	return 0;
}

/* ------------------------------------------------------------------ */
/*  Config file parser                                                 */
/* ------------------------------------------------------------------ */

#define MAX_LINE_LEN	256

/*
 * Parse a CIDR address string (e.g. "10.0.0.1/24") into address and
 * netmask strings suitable for the wg sysctl interface.
 * Returns 0 on success, -1 on failure.
 */
static int
parse_cidr(const char *str, char *addr_out, size_t addr_size,
    char *mask_out, size_t mask_size)
{
	struct in_addr in;
	char cidr_str[MAX_LINE_LEN];
	char *slash;
	int prefix;
	uint32_t nm;

	strncpy(cidr_str, str, sizeof(cidr_str) - 1);
	cidr_str[sizeof(cidr_str) - 1] = '\0';

	slash = strchr(cidr_str, '/');
	if (slash == NULL) {
		/* No prefix length — just the address. */
		if (inet_aton(cidr_str, &in) == 0)
			return -1;
		strncpy(addr_out, cidr_str, addr_size);
		strncpy(mask_out, "255.255.255.255", mask_size);
		return 0;
	}

	*slash = '\0';
	prefix = atoi(slash + 1);
	if (prefix < 0 || prefix > 32)
		return -1;

	if (inet_aton(cidr_str, &in) == 0)
		return -1;

	strncpy(addr_out, cidr_str, addr_size);

	/* Convert prefix length to netmask. */
	nm = (prefix == 0) ? 0 : htonl(~((1U << (32 - prefix)) - 1));
	snprintf(mask_out, mask_size, "%d.%d.%d.%d",
	    (nm >> 24) & 0xFF, (nm >> 16) & 0xFF,
	    (nm >> 8) & 0xFF, nm & 0xFF);

	return 0;
}

/*
 * Parse a config file for the given interface name.
 * Path: /etc/wireguard/<ifname>.conf
 *
 * Returns 0 on success, -1 on failure.
 */
#define WG_MAX_ADDRESSES	8
#define WG_MAX_PEERS		8

struct wg_config {
	uint8_t		private_key[32];
	uint16_t	listen_port;
	int		has_private_key;
	int		has_listen_port;
	int		num_addresses;
	char		addresses[WG_MAX_ADDRESSES][WG_ADDR_STRLEN];
	char		address_masks[WG_MAX_ADDRESSES][WG_ADDR_STRLEN];
	int		num_peers;

	struct wg_peer_config {
		int		has_public_key;
		uint8_t		public_key[32];
		uint8_t		preshared_key[32];
		int		has_preshared_key;
		char		endpoint_host[WG_ADDR_STRLEN];
		uint16_t	endpoint_port;
		char		allowed_addrs[WG_MAX_ADDRESSES][WG_ADDR_STRLEN];
		char		allowed_masks[WG_MAX_ADDRESSES][WG_ADDR_STRLEN];
		int		num_allowed_ips;
		int		keep_alive;
	} peers[WG_MAX_PEERS];
};

static int
wg_config_read(const char *ifname, struct wg_config *cfg)
{
	char path[256];
	char line[MAX_LINE_LEN];
	FILE *fp;
	int section = 0; /* 0=none, 1=Interface, 2=Peer */
	int peer_idx = -1;

	snprintf(path, sizeof(path), "/etc/wireguard/%s.conf", ifname);

	fp = fopen(path, "r");
	if (fp == NULL) {
		warn("cannot open %s", path);
		return -1;
	}

	memset(cfg, 0, sizeof(*cfg));
	cfg->listen_port = 51820; /* default */

	while (fgets(line, sizeof(line), fp) != NULL) {
		char *p, *key, *val;
		size_t len;

		/* Strip trailing newline/CR. */
		len = strlen(line);
		while (len > 0 && (line[len - 1] == '\n' ||
		    line[len - 1] == '\r'))
			line[--len] = '\0';

		p = line;

		/* Skip leading whitespace. */
		while (*p == ' ' || *p == '\t')
			p++;

		/* Skip empty lines and comments. */
		if (*p == '\0' || *p == '#')
			continue;

		/* Section header. */
		if (*p == '[') {
			char *end = strchr(p, ']');
			if (end == NULL) {
				warnx("invalid section header: %s", p);
				fclose(fp);
				return -1;
			}
			*end = '\0';
			p++;

			if (strcasecmp(p, "Interface") == 0) {
				section = 1;
			} else if (strcasecmp(p, "Peer") == 0) {
				section = 2;
				peer_idx++;
				if (peer_idx >= 8) {
					warnx("too many peers (max 8)");
					fclose(fp);
					return -1;
				}
				cfg->num_peers++;
			} else {
				warnx("unknown section: %s", p);
				fclose(fp);
				return -1;
			}
			continue;
		}

		/* Key = Value */
		key = p;
		val = strchr(p, '=');
		if (val == NULL) {
			warnx("invalid config line: %s", line);
			fclose(fp);
			return -1;
		}
		*val++ = '\0';

		/* Strip trailing whitespace from key. */
		{
			char *end = key + strlen(key) - 1;
			while (end >= key && (*end == ' ' || *end == '\t'))
				*end-- = '\0';
		}

		/* Strip leading whitespace from value. */
		while (*val == ' ' || *val == '\t')
			val++;

		/* Strip trailing whitespace from value. */
		{
			char *end = val + strlen(val) - 1;
			while (end >= val && (*end == ' ' || *end == '\t'))
				*end-- = '\0';
		}

		switch (section) {
		case 1: /* Interface */
			if (strcasecmp(key, "PrivateKey") == 0) {
				if (b64_decode(val, cfg->private_key,
				    sizeof(cfg->private_key)) != 0) {
					warnx("invalid PrivateKey: %s", val);
					fclose(fp);
					return -1;
				}
				cfg->has_private_key = 1;
			} else if (strcasecmp(key, "ListenPort") == 0) {
				cfg->listen_port = (uint16_t)atoi(val);
				cfg->has_listen_port = 1;
			} else if (strcasecmp(key, "Address") == 0) {
				char a[WG_ADDR_STRLEN], m[WG_ADDR_STRLEN];
				int n;

				if (parse_cidr(val, a, sizeof(a),
				    m, sizeof(m)) != 0) {
					warnx("invalid Address: %s", val);
				} else if (cfg->num_addresses <
				    WG_MAX_ADDRESSES) {
					n = cfg->num_addresses++;
					strncpy(cfg->addresses[n], a,
					    sizeof(cfg->addresses[n]));
					strncpy(cfg->address_masks[n], m,
					    sizeof(cfg->address_masks[n]));
				}
			}
			/* Other Interface keys are ignored (handled by
			 * ifconfig: Address, DNS, MTU, Table, etc.) */
			break;

		case 2: /* Peer */
			if (peer_idx < 0 || peer_idx >= 8)
				break;

			if (strcasecmp(key, "PublicKey") == 0) {
				if (b64_decode(val,
				    cfg->peers[peer_idx].public_key,
				    sizeof(cfg->peers[peer_idx].public_key))
				    != 0) {
					warnx("invalid PublicKey: %s", val);
					fclose(fp);
					return -1;
				}
				cfg->peers[peer_idx].has_public_key = 1;
			} else if (strcasecmp(key, "PresharedKey") == 0) {
				if (b64_decode(val,
				    cfg->peers[peer_idx].preshared_key,
				    sizeof(cfg->peers[peer_idx].preshared_key))
				    != 0) {
					warnx("invalid PresharedKey: %s", val);
					fclose(fp);
					return -1;
				}
				cfg->peers[peer_idx].has_preshared_key = 1;
			} else if (strcasecmp(key, "Endpoint") == 0) {
				char *colon = strrchr(val, ':');
				if (colon == NULL) {
					warnx("invalid Endpoint "
					    "(need host:port): %s", val);
					fclose(fp);
					return -1;
				}
				*colon++ = '\0';
				/* Handle IPv6 addresses in brackets:
				 * [::1]:51820 */
				if (val[0] == '[') {
					char *bracket = strchr(val, ']');
					if (bracket == NULL) {
						warnx("invalid Endpoint "
						    "(unmatched bracket)");
						fclose(fp);
						return -1;
					}
					*bracket = '\0';
					strncpy(
					    cfg->peers[peer_idx].endpoint_host,
					    val + 1,
					    sizeof(cfg->peers[peer_idx]
					        .endpoint_host) - 1);
				} else {
					strncpy(
					    cfg->peers[peer_idx].endpoint_host,
					    val,
					    sizeof(cfg->peers[peer_idx]
					        .endpoint_host) - 1);
				}
				cfg->peers[peer_idx].endpoint_port =
				    (uint16_t)atoi(colon);
			} else if (strcasecmp(key, "AllowedIPs") == 0) {
				char *tok, *save;
				char *copy = strdup(val);
				if (copy == NULL) {
					warn("strdup");
					fclose(fp);
					return -1;
				}
				tok = strtok_r(copy, ",", &save);
				while (tok != NULL) {
					int n = cfg->peers[peer_idx]
					    .num_allowed_ips;
					if (n >= 8)
						break;
					if (parse_cidr(tok,
					    cfg->peers[peer_idx]
					        .allowed_addrs[n],
					    sizeof(cfg->peers[peer_idx]
					        .allowed_addrs[n]),
					    cfg->peers[peer_idx]
					        .allowed_masks[n],
					    sizeof(cfg->peers[peer_idx]
					        .allowed_masks[n])) != 0) {
						warnx("invalid AllowedIPs"
						    " CIDR: %s", tok);
					} else {
						cfg->peers[peer_idx]
						    .num_allowed_ips++;
					}
					tok = strtok_r(NULL, ",", &save);
				}
				free(copy);
			} else if (strcasecmp(key, "PersistentKeepalive")
			    == 0) {
				cfg->peers[peer_idx].keep_alive =
				    atoi(val);
			}
			break;
		}
	}

	fclose(fp);

	if (!cfg->has_private_key) {
		warnx("missing PrivateKey in [Interface] section");
		return -1;
	}

	return 0;
}

/* ------------------------------------------------------------------ */
/*  Sysctl configuration helpers                                       */
/* ------------------------------------------------------------------ */

/*
 * Send a configuration command to the WireGuard sysctl node.
 * Returns 0 on success, -1 on failure.
 */
static int
wg_sysctl_cmd(struct wg_sysctl_req *req)
{
	int mib[5];
	size_t miblen = 5;
	size_t reqlen = sizeof(*req);

	/* Translate "minix.lwip.wireguard.cfg" to MIB. */
	if (sysctlnametomib("minix.lwip.wireguard.cfg", mib, &miblen) != 0) {
		warn("sysctlnametomib(minix.lwip.wireguard.cfg)");
		return -1;
	}

	if (sysctl(mib, miblen, NULL, 0, req, reqlen) != 0) {
		warn("sysctl(minix.lwip.wireguard.cfg)");
		return -1;
	}

	return 0;
}

/*
 * Create (bring up) a WireGuard interface.
 * Returns 0 on success, -1 on failure.
 */
static int
wg_up(const char *ifname)
{
	struct wg_config cfg;
	struct wg_sysctl_req req;
	char cmd[256];
	int i;

	/* Read config file. */
	if (wg_config_read(ifname, &cfg) != 0)
		return -1;

	/*
	 * Step 1: Create the interface.
	 * ifconfig wg0 create
	 */
	snprintf(cmd, sizeof(cmd), "/sbin/ifconfig %s create", ifname);
	if (system(cmd) != 0) {
		warnx("failed to create interface %s", ifname);
		warnx("  (command: %s)", cmd);
		return -1;
	}

	/*
	 * Step 2: Configure private key and listen port.
	 */
	memset(&req, 0, sizeof(req));
	strncpy(req.ifname, ifname, sizeof(req.ifname) - 1);
	req.cmd = WG_CMD_CONFIGURE;
	memcpy(req.private_key, cfg.private_key, 32);
	req.listen_port = cfg.listen_port;

	if (wg_sysctl_cmd(&req) != 0) {
		warnx("failed to configure %s", ifname);
		return -1;
	}

	/*
	 * Step 3: Add each peer.
	 */
	for (i = 0; i < cfg.num_peers; i++) {
		/* Skip peers without a PublicKey (empty section). */
		if (!cfg.peers[i].has_public_key)
			continue;

		memset(&req, 0, sizeof(req));
		strncpy(req.ifname, ifname, sizeof(req.ifname) - 1);
		req.cmd = WG_CMD_ADD_PEER;
		memcpy(req.public_key, cfg.peers[i].public_key, 32);
		if (cfg.peers[i].has_preshared_key)
			memcpy(req.preshared_key,
			    cfg.peers[i].preshared_key, 32);
		strncpy(req.endpoint_addr, cfg.peers[i].endpoint_host,
		    sizeof(req.endpoint_addr) - 1);
		req.endpoint_port = cfg.peers[i].endpoint_port;
		req.keep_alive = (uint16_t)cfg.peers[i].keep_alive;

		/* Use the first allowed IP for the peer config. */
		if (cfg.peers[i].num_allowed_ips > 0) {
			strncpy(req.allowed_addr,
			    cfg.peers[i].allowed_addrs[0],
			    sizeof(req.allowed_addr) - 1);
			strncpy(req.allowed_mask,
			    cfg.peers[i].allowed_masks[0],
			    sizeof(req.allowed_mask) - 1);
		}

		if (wg_sysctl_cmd(&req) != 0) {
			warnx("failed to add peer %d to %s", i, ifname);
			return -1;
		}
	}

	/*
	 * Step 4: Assign tunnel IP addresses via ifconfig.
	 */
	for (i = 0; i < cfg.num_addresses; i++) {
		snprintf(cmd, sizeof(cmd),
		    "/sbin/ifconfig %s inet %s netmask %s",
		    ifname, cfg.addresses[i], cfg.address_masks[i]);
		if (system(cmd) != 0)
			warnx("ifconfig address failed: %s", cmd);
	}

	/*
	 * Step 5: Bring the interface up.
	 */
	snprintf(cmd, sizeof(cmd), "/sbin/ifconfig %s up", ifname);
	if (system(cmd) != 0) {
		warnx("failed to bring up %s", ifname);
		return -1;
	}

	printf("WireGuard interface %s is up.\n", ifname);
	return 0;
}

/*
 * Tear down (destroy) a WireGuard interface.
 * Returns 0 on success, -1 on failure.
 */
static int
wg_down(const char *ifname)
{
	char cmd[256];

	snprintf(cmd, sizeof(cmd), "/sbin/ifconfig %s destroy", ifname);
	if (system(cmd) != 0) {
		warnx("failed to destroy %s", ifname);
		return -1;
	}

	printf("WireGuard interface %s is down.\n", ifname);
	return 0;
}

/* ------------------------------------------------------------------ */
/*  Main entry point                                                   */
/* ------------------------------------------------------------------ */

static void
usage(void)
{

	fprintf(stderr,
	    "Usage: wg-quick up <ifname>\n"
	    "       wg-quick down <ifname>\n"
	    "       wg-quick list\n");
}

int
main(int argc, char *argv[])
{

	if (argc < 2) {
		usage();
		return 1;
	}

	if (strcmp(argv[1], "up") == 0) {
		if (argc != 3) {
			usage();
			return 1;
		}
		return (wg_up(argv[2]) != 0) ? 1 : 0;
	}

	if (strcmp(argv[1], "down") == 0) {
		if (argc != 3) {
			usage();
			return 1;
		}
		return (wg_down(argv[2]) != 0) ? 1 : 0;
	}

	if (strcmp(argv[1], "list") == 0) {
		/* List all available configs in /etc/wireguard/. */
		/* Not implemented — just list files. */
		char cmd[128];
		snprintf(cmd, sizeof(cmd),
		    "ls -1 /etc/wireguard/*.conf 2>/dev/null | "
		    "sed 's|.*/||; s|\\.conf$||'");
		return system(cmd);
	}

	usage();
	return 1;
}
