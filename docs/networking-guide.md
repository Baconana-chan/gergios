# Networking Guide — GergiOS

> **Last updated**: July 2026
> **Related**: `docs/network-architecture.md`, `docs/network-performance.md`,
>   `docs/network-security.md`, `man ifconfig(8)`, `man netstat(8)`

## Table of Contents

1. [Overview](#1-overview)
2. [Network Interfaces](#2-network-interfaces)
3. [IP Configuration](#3-ip-configuration)
4. [Routing](#4-routing)
5. [DNS Configuration](#5-dns-configuration)
6. [DHCP](#6-dhcp)
7. [WireGuard VPN](#7-wireguard-vpn)
8. [Monitoring and Diagnostics](#8-monitoring-and-diagnostics)
9. [Configuration Examples](#9-configuration-examples)
10. [Troubleshooting](#10-troubleshooting)

---

## 1. Overview

GergiOS uses the **lwIP 2.2.1** TCP/IP stack running as a system service
(`minix/net/lwip/`). Networking is fully IPv4/IPv6 dual-stack.

**Key features:**

| Feature | Status | Notes |
|---------|--------|-------|
| TCP/IPv4 | ✅ | Full socket API |
| UDP/IPv4 | ✅ | Connected/unconnected sockets |
| IPv6 dual-stack | ✅ | `PF_INET6`, V6ONLY, IPv4-mapped |
| Loopback | ✅ | `lo0` interface |
| Ethernet drivers | ✅ | e1000, rtl8139, fxp, lance, dp8390 |
| DHCP client | ✅ | Automatic configuration |
| DNS resolver | ✅ | Via `resolv.conf` |
| Packet capture | ✅ | BPF, `tcpdump`, `pcapng`, Wireshark (rpcap) |
| WireGuard VPN | ✅ | Kernel-level tunnel interface |
| IPsec/DTLS | ✅ | ESP, AH transport mode, DTLS over UDP |

---

## 2. Network Interfaces

### 2.1 Interface Types

| Type | Driver | Name pattern | Description |
|------|--------|-------------|-------------|
| Ethernet | e1000 | `e0`, `e1` | Intel PRO/1000 |
| Ethernet | rtl8139 | `e0`, `e1` | Realtek 8139 |
| Ethernet | fxp | `e0`, `e1` | Intel EtherExpress PRO/100 |
| Ethernet | lance | `e0`, `e1` | AMD LANCE |
| Ethernet | dp8390 | `e0`, `e1` | National Semiconductor |
| Loopback | — | `lo0` | Local loopback (always present) |
| WireGuard | — | `wg0`, `wg1` | VPN tunnel interface |

### 2.2 Listing Interfaces

```sh
# Show all interfaces:
ifconfig -a

# Show a specific interface:
ifconfig e0

# Compact listing with statistics:
netstat -i
```

### 2.3 Interface Names

Ethernet interface names are assigned at boot time based on driver probe order.
The first e1000 driver becomes `e0`, the second `e1`, etc.

To determine which driver owns an interface:
```sh
# Check driver label and endpoint:
sysctl minix.lwip.drivers.info
```

### 2.4 Bringing Interfaces Up/Down

```sh
# Bring up an interface:
ifconfig e0 up

# Bring down:
ifconfig e0 down

# Set IP address and bring up in one command:
ifconfig e0 192.168.1.100 netmask 255.255.255.0 up
```

### 2.5 Jumbo Frames

To enable jumbo frames (MTU up to 9000):

```sh
ifconfig e0 mtu 9000
```

**Note**: Jumbo frames require the driver (e.g., e1000) and the network
infrastructure to support them.  Default MTU is 1500.

### 2.6 Promiscuous Mode

Promiscuous mode is enabled automatically when a BPF device is attached
(e.g., when `tcpdump` is running).  Manual override:

```sh
# Enable:
ifconfig e0 promisc

# Disable:
ifconfig e0 -promisc
```

---

## 3. IP Configuration

### 3.1 Static IPv4

```sh
# Set static IP:
ifconfig e0 inet 192.168.1.100 netmask 255.255.255.0

# Add a default gateway:
route add default 192.168.1.1

# Verify:
ifconfig e0
netstat -r
```

### 3.2 Static IPv6

```sh
# Set static IPv6 (link-local is auto-configured by default):
ifconfig e0 inet6 2001:db8::100/64

# Verify:
ifconfig e0
netstat -r -f inet6
```

### 3.3 IPv6 Auto-Configuration

Link-local IPv6 addresses (`fe80::`) are automatically assigned when an
interface is brought up.  Stateless address autoconfiguration (SLAAC) via
Router Advertisements is supported but not enabled by default.

To enable SLAAC:
```sh
# Accept router advertisements:
ndp -I e0 -flags accept_rtadv
```

### 3.4 Multiple IP Addresses

Multiple addresses can be assigned to the same interface:

```sh
# Primary address:
ifconfig e0 inet 192.168.1.100 netmask 255.255.255.0

# Secondary address:
ifconfig e0 inet 192.168.1.101 netmask 255.255.255.0 alias

# Remove secondary:
ifconfig e0 inet 192.168.1.101 -alias
```

---

## 4. Routing

### 4.1 Viewing the Routing Table

```sh
# Full routing table:
netstat -r

# IPv4 only:
netstat -r -f inet

# IPv6 only:
netstat -r -f inet6
```

### 4.2 Adding Routes

```sh
# Default gateway:
route add default 192.168.1.1

# Specific subnet:
route add -net 10.0.0.0/8 192.168.1.1

# Host route:
route add -host 10.0.0.5 192.168.1.1

# Reject route (blackhole):
route add -net 10.0.0.0/8 -reject
```

### 4.3 Deleting Routes

```sh
route delete default
route delete -net 10.0.0.0/8
```

### 4.4 IP Forwarding

IP forwarding between interfaces is disabled by default.  Enable at runtime:

```sh
# Enable forwarding:
sysctl -w net.inet.ip.forwarding=1
sysctl -w net.inet6.ip6.forwarding=1
```

**Note**: GergiOS is not designed as a router.  Forwarding performance
is limited by the single-threaded lwIP design.

---

## 5. DNS Configuration

### 5.1 /etc/resolv.conf

```sh
# Edit resolver configuration:
cat > /etc/resolv.conf <<EOF
nameserver 8.8.8.8
nameserver 8.8.4.4
nameserver 2001:4860:4860::8888
search example.com
EOF
```

### 5.2 Testing DNS

```sh
# Use getent (if available):
getent hosts google.com

# Or use the DNS client directly:
dnsquery google.com
```

---

## 6. DHCP

### 6.1 Automatic Configuration

```sh
# Obtain IP via DHCP on the first Ethernet interface:
dhcpcd e0
```

### 6.2 DHCP with Fallback to Static

Create `/etc/dhcpcd.conf`:

```
interface e0
fallback static_eth0

profile static_eth0
static ip_address=192.168.1.100/24
static routers=192.168.1.1
static domain_name_servers=8.8.8.8
```

---

## 7. WireGuard VPN

WireGuard is integrated as a kernel-level tunnel interface (`wgX`).

### 7.1 Creating a WireGuard Interface

```sh
# Create a WireGuard interface:
ifconfig wg0 create

# Configure private key and listen port:
cat /etc/wireguard/wg0.conf
[Interface]
PrivateKey = gN6R0pY0...b3d4=
ListenPort = 51820

[Peer]
PublicKey = xTIBA...Uw0=
AllowedIPs = 10.0.0.0/24
Endpoint = 192.168.1.200:51820
```

### 7.2 Using wg-quick

```sh
# Auto-configure from conf file:
wg-quick up wg0

# Tear down:
wg-quick down wg0
```

### 7.3 Manual Configuration via sysctl

```sh
# Set private key:
sysctl -w minix.lwip.wireguard.cfg=CONFIGURE,private_key=<hex>,port=51820

# Add a peer:
sysctl -w minix.lwip.wireguard.cfg=ADD_PEER,public_key=<hex>,endpoint=1.2.3.4:51820,allowed_ips=10.0.0.0/24
```

---

## 8. Monitoring and Diagnostics

### 8.1 netstat

```sh
# TCP connections with congestion metrics:
netstat                    # Shows cwnd, rtt, rto, retransmissions

# Interface statistics:
netstat -i                 # Packets, errors, drops, collisions

# Protocol statistics:
netstat -s                 # TCP/UDP/IP buffer sizes, forwarding

# Driver information:
netstat -d                 # Per-driver state and queue depths

# Active sockets:
sockstat                   # All open sockets
```

### 8.2 sysctl

```sh
# Full sysctl tree:
sysctl -a | grep net

# Interface statistics:
sysctl minix.lwip.ifaces

# TCP extended metrics:
sysctl minix.lwip.tcp_ext

# Latency histograms:
sysctl minix.lwip.latency

# Performance alert thresholds:
sysctl minix.lwip.alerts
```

### 8.3 Packet Capture

```sh
# Capture on a specific interface:
tcpdump -i e0

# Capture on all interfaces:
tcpdump -i any

# Write to pcapng file:
pcapng -i e0 -w capture.pcapng

# Remote capture (Wireshark):
# On MINIX: rpcapd -d
# In Wireshark: rpcap://<minix-ip>/
```

---

## 9. Configuration Examples

### 9.1 Static IP + DHCP Fallback

```sh
# /etc/rc.d/network:
ifconfig e0 up
dhcpcd -t 10 e0 || ifconfig e0 192.168.1.100 netmask 255.255.255.0
route add default 192.168.1.1 || true
```

### 9.2 Bridge Setup

**Note**: GergiOS does not support bridging at this time.  For connecting
to a QEMU tap network, use the host bridge:

```sh
# On Linux host:
sudo ip link add br0 type bridge
sudo ip link set tap0 master br0
sudo ip link set eth0 master br0
sudo ip addr add 192.168.100.1/24 dev br0
sudo ip link set br0 up
```

### 9.3 Multi-Homing

```sh
# Two interfaces, two subnets:
ifconfig e0 192.168.1.100 netmask 255.255.255.0
ifconfig e1 10.0.0.100 netmask 255.255.255.0
route add default 192.168.1.1
```

---

## 10. Troubleshooting

### 10.1 Interface Not Appearing

```sh
# Check driver status:
sysctl minix.lwip.drivers.info

# Expected output shows endpoint and queue depths:
#   e1000 (endpt 12345) [active]
#   SendQ[0]: 0/4  SendQ[1]: 0/4
#   RecvQ:    0/4

# If no driver is listed, check the boot log:
dmesg | grep e1000
```

### 10.2 No Network Connectivity

```sh
# 1. Is the interface up?
ifconfig e0

# 2. Is there a default route?
netstat -r

# 3. Can we reach the gateway?
ping 192.168.1.1

# 4. Is DNS working?
# Check /etc/resolv.conf

# 5. Check interface errors:
netstat -i
```

### 10.3 Slow Network Performance

See `docs/network-performance.md` for detailed tuning guidance.

```sh
# Quick checks:
netstat      # Check for retransmissions (nrtx column)
netstat -i   # Check for errors, collisions, drops
sysctl minix.lwip.alerts  # Check for performance alerts
```

### 10.4 IPv6 Issues

```sh
# Check IPv6 addresses:
ifconfig e0 inet6

# Verify NDP:
ndp -a

# Test reachability:
ping6 -c 3 2001:db8::1

# Check ingress filtering (may block certain addresses):
sysctl net.inet6.ip6.ingress_filter
```

### 10.5 WireGuard Issues

```sh
# Verify interface exists and is up:
ifconfig wg0

# Check configuration:
sysctl minix.lwip.wireguard

# Re-start:
wg-quick down wg0 && wg-quick up wg0
```

### 10.6 Memory Pressure

TCP socket buffers are statically allocated.  If you see `ENOBUFS` errors:

```sh
# Increase per-socket buffers:
sysctl -w net.inet.tcp.sendspace=65536
sysctl -w net.inet.tcp.recvspace=65536

# Increase UDP buffers:
sysctl -w net.inet.udp.sendspace=16384
sysctl -w net.inet.udp.recvspace=65536
```

---

> **See also**: `docs/network-architecture.md` for internal architecture,
> `docs/network-performance.md` for tuning, `docs/network-security.md`
> for security features.
