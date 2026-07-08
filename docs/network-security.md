# Network Security — GergiOS

> **Last updated**: July 2026
> **Related**: `docs/networking-guide.md`, `docs/network-architecture.md`,
>   `docs/network-performance.md`, `planning/25_network_stack_modernization.md`

## Table of Contents

1. [Overview](#1-overview)
2. [SYN Cookies (RFC 4987)](#2-syn-cookies-rfc-4987)
3. [TCP MD5 Signature (RFC 2385)](#3-tcp-md5-signature-rfc-2385)
4. [Rate Limiting](#4-rate-limiting)
5. [Ingress Filtering (BCP 38)](#5-ingress-filtering-bcp-38)
6. [WireGuard VPN](#6-wireguard-vpn)
7. [IPsec (ESP / AH)](#7-ipsec-esp--ah)
8. [DTLS over UDP](#8-dtls-over-udp)
9. [IPC Security Audit](#9-ipc-security-audit)

---

## 1. Overview

GergiOS implements a layered security model for the network stack:

| Layer | Feature | RFC | Status |
|-------|---------|-----|--------|
| **Transport** | SYN Cookies | 4987 | ✅ Default on |
| **Transport** | TCP MD5 Signature | 2385 | ✅ Default on |
| **Transport** | DTLS 1.2/1.3 | 6347/9147 | ✅ Optional |
| **Network** | Ingress Filtering | BCP 38 | ✅ Off by default |
| **Network** | IPsec ESP Transport | 4303 | ✅ Off by default |
| **Network** | IPsec AH Transport | 4302 | ✅ Off by default |
| **Link** | WireGuard VPN | — | ✅ Off by default |
| **Protocol** | Rate Limiting | — | ✅ Default on |
| **Protocol** | SYN Flood Protection | — | ✅ Default on |

---

## 2. SYN Cookies (RFC 4987)

### 2.1 Purpose

Protects against **SYN flood** denial-of-service attacks. Instead of
allocating connection state for every incoming SYN, the server encodes
connection parameters into the SYN-ACK sequence number.

### 2.2 Implementation

```
Client                     Server
  │                          │
  │── SYN ──────────────────▶│
  │                          │  SYN cookie generated:
  │                          │  ISN = (timestamp << 27)
  │                          │      | (mss_idx << 24)
  │                          │      | sha256_hash[0..23]
  │                          │
  │◀── SYN-ACK (cookie) ─────┤
  │                          │  No PCB allocated yet!
  │                          │
  │── ACK ──────────────────▶│
  │                          │  Cookie validated:
  │                          │  SHA256(4-tuple + secret)
  │                          │  matches ISN[0..23]?
  │                          │  → If yes: allocate PCB,
  │                          │     restore MSS, enter ESTABLISHED
  │                          │  → If no: drop
```

### 2.3 Cookie Format

```
31              27 26 24 23                          0
┌────────────────┬────┬───────────────────────────────┐
│   timestamp    │mss │   SHA256(4-tuple + secret)     │
│   (5 bits)    │(3) │   truncated to 24 bits          │
└────────────────┴────┴───────────────────────────────┘
```

- **Timestamp**: 5 bits, wraps every ~64 seconds at 2-second granularity
- **MSS index**: 3 bits, encodes one of 8 MSS values
- **Hash**: 24-bit SHA256(4-tuple + secret), where secret rotates every ~64s

### 2.4 Configuration

```sh
# Enable (default):
sysctl -w net.inet.tcp.syncookies=1

# Disable:
sysctl -w net.inet.tcp.syncookies=0

# Verify:
sysctl net.inet.tcp.syncookies
```

### 2.5 Limitations

- **Limited MSS values**: Only 8 possible MSS values (536, 1100, 1200,
  1300, 1400, 1460, 2500, 8958 bytes)
- **No TCP options**: SACK, window scaling, timestamps are not encoded
  in the cookie
- **Secret rotation**: Secret changes every ~64 seconds (with 2 secrets
  for overlap during rotation)

---

## 3. TCP MD5 Signature (RFC 2385)

### 3.1 Purpose

Adds a **MD5 digest** to TCP segments to authenticate the peer.
Primarily used for **BGP session protection** (RFC 2385, RFC 5925).

### 3.2 Implementation

The MD5 digest is computed over:

```
TCP pseudo-header (IPv4 or IPv6) +
TCP header (with checksum field = 0) +
TCP data +
TCP MD5 option (kind=19, len=18, digest) +
Key
```

### 3.3 Configuration

```c
// Per-socket enable:
struct tcp_md5sig md5sig;
strlcpy(md5sig.tcpm_addr.sin_addr, "192.168.1.1", sizeof(md5sig.tcpm_addr));
md5sig.tcpm_keylen = 16;
memcpy(md5sig.tcpm_key, "my-secret-key-16", 16);
setsockopt(fd, IPPROTO_TCP, TCP_MD5SIG, &md5sig, sizeof(md5sig));
```

```sh
# Global enable (default):
sysctl -w net.inet.tcp.md5sig=1

# Global disable:
sysctl -w net.inet.tcp.md5sig=0
```

### 3.4 Supported Options

- **Per-socket key**: Each socket can have a different MD5 key
- **Connection validation**: Only peers with matching keys can connect
- **IPv4 and IPv6**: Pseudo-header covers both address families
- **RFC 2385 compatible**: Wire format matches BGP MD5 requirement

---

## 4. Rate Limiting

### 4.1 Purpose

Prevents protocol-level abuse by rate-limiting ICMP/ARP/NDP responses.

### 4.2 Token Bucket Algorithm

```
For each protocol:
  bucket = min(bucket + rate_per_sec * delta_time, burst)

  if bucket > 0:
    allow packet
    bucket -= 1
  else:
    drop packet
```

### 4.3 Default Limits

| Protocol | Rate | Burst | Effect |
|----------|------|-------|--------|
| ICMP errors | 10/sec | 20 | Limits ping flood response |
| ARP | 50/sec | 100 | Limits ARP scan response |
| NDP | 50/sec | 100 | Limits neighbor discovery flood |

### 4.4 Callback Integration

Rate limit drops trigger an optional callback. In GergiOS, this callback
invokes `perf_alerts_rate_limit()` (via `lwip_ratelimit_set_alert_cb`):

```
lwip_ratelimit_icmp_error() → rate limit hit → callback(0) → perf alert
lwip_ratelimit_arp()       → rate limit hit → callback(1) → perf alert
lwip_ratelimit_ndp()       → rate limit hit → callback(2) → perf alert
```

---

## 5. Ingress Filtering (BCP 38)

### 5.1 Purpose

Prevents **IP spoofing** by rejecting packets with source addresses that
could not legitimately originate from the incoming interface.

### 5.2 Filters

| Filter | Address | Interface |
|--------|---------|-----------|
| IPv4 loopback | `127.0.0.0/8` | All (except loopback) |
| IPv4 link-local | `169.254.0.0/16` | All |
| IPv6 loopback | `::1` | All (except loopback) |

**DHCP exception**: Link-local filtering is bypassed during DHCP
(`192.168.1.100 → 255.255.255.255` is allowed).

### 5.3 Configuration

```sh
# Enable ingress filtering:
sysctl -w net.inet.ip.ingress_filter=1

# For IPv6:
sysctl -w net.inet6.ip6.ingress_filter=1

# Verify:
sysctl net.inet.ip.ingress_filter
```

---

## 6. WireGuard VPN

### 6.1 Purpose

Provides **kernel-level VPN** using the WireGuard protocol
(ChaCha20-Poly1305 encryption, Curve25519 key exchange).

### 6.2 Architecture

```
┌─────────────────────────────────────────────┐
│                GergiOS                       │
│                                               │
│  Application                                 │
│    │                                          │
│    ▼                                          │
│  Socket layer (AF_INET/AF_INET6)             │
│    │                                          │
│    ▼                                          │
│  lwIP routing → wg0 interface                │
│    │                                          │
│    ▼                                          │
│  WireGuard tunnel interface (wgif)           │
│    │                                          │
│    ├── Encrypt: ChaCha20-Poly1305 (data)     │
│    ├── Encrypt: Curve25519 (key exchange)    │
│    ├── Hash: BLAKE2s (handshake)            │
│    └── CSPRNG: ChaCha20 DRBG (entropy)      │
│    │                                          │
│    ▼                                          │
│  UDP socket → peer IP:51820                  │
│    │                                          │
│    ▼                                          │
│  Physical interface (e0) → Internet          │
└─────────────────────────────────────────────┘
```

### 6.3 Interface Types

| Interface | Type | Purpose |
|-----------|------|---------|
| `wg0`, `wg1` | WireGuard tunnel | VPN to remote networks |

### 6.4 Configuration

#### Using wg-quick

Create `/etc/wireguard/wg0.conf`:

```ini
[Interface]
PrivateKey = gN6R0pY0...b3d4=
Address = 10.0.0.1/24
ListenPort = 51820

[Peer]
PublicKey = xTIBA...Uw0=
AllowedIPs = 10.0.0.0/24, 192.168.2.0/24
Endpoint = 203.0.113.5:51820
PersistentKeepalive = 25
```

Then:

```sh
wg-quick up wg0
```

#### Using sysctl

```sh
# Configure interface:
sysctl -w minix.lwip.wireguard.cfg=CONFIGURE,private_key=<hex>,port=51820

# Add a peer:
sysctl -w minix.lwip.wireguard.cfg=ADD_PEER,public_key=<hex>,endpoint=203.0.113.5:51820,allowed_ips=10.0.0.0/24

# Connect:
sysctl -w minix.lwip.wireguard.cfg=CONNECT

# Remove peer:
sysctl -w minix.lwip.wireguard.cfg=REMOVE_PEER,public_key=<hex>

# Disconnect / remove interface:
sysctl -w minix.lwip.wireguard.cfg=DISCONNECT
```

### 6.5 CSPRNG

WireGuard uses a **ChaCha20 DRBG** (Deterministic Random Bit Generator)
as its cryptographically secure pseudo-random number generator:

- **Seed**: Obtained from `sys_getrandomness()` (kernel entropy pool)
- **Reseed**: Every 1MB of output
- **Rekey**: After each block (forward secrecy)
- **Fallback**: `sys_now()` if kernel entropy unavailable (less secure)

### 6.6 Global Toggle

```sh
# Enable (default):
sysctl -w minix.lwip.wireguard.enabled=1

# Disable (tears down all WG interfaces):
sysctl -w minix.lwip.wireguard.enabled=0
```

---

## 7. IPsec (ESP / AH)

### 7.1 Purpose

Provides **network-layer encryption and authentication** using IPsec
in transport mode (RFC 4301).

### 7.2 Supported Protocols

| Protocol | RFC | Mode | Auth | Encryption |
|----------|-----|------|------|------------|
| ESP | 4303 | Transport | AES-GCM-128/256 (AEAD) | AES-GCM |
| ESP | 4303 | Transport | AES-CBC+HMAC-SHA256 | AES-CBC |
| ESP | 4303 | Transport | ChaCha20-Poly1305 | ChaCha20 |
| AH | 4302 | Transport | HMAC-SHA1-96 | — |
| AH | 4302 | Transport | HMAC-SHA256-128 | — |

### 7.3 Architecture

```
┌─────────────────────────────────────────────┐
│              lwIP IP Layer                   │
│                                               │
│  ip4_output()                                │
│    │                                          │
│    ├── LWIP_HOOK_IP4_OUTPUT                  │
│    │   └── ipsec_output_check()              │
│    │       └── Match SA → ESP/AH processing  │
│    │           ├── ESP: encrypt + auth        │
│    │           └── AH:  auth only             │
│    │                                          │
│    ▼                                          │
│  ethif_output() → driver                     │
│                                               │
│  ip4_input()                                 │
│    │                                          │
│    ├── LWIP_HOOK_IP4_INPUT                   │
│    │   └── ipsec_input_check()               │
│    │       ├── ESP proto → decrypt + verify  │
│    │       └── AH proto → verify auth        │
│    │       └── Match SA? → strip header      │
│    │       └── No match? → drop              │
│    │                                          │
│    ▼                                          │
│  tcp_input() / udp_input()                   │
└─────────────────────────────────────────────┘
```

### 7.4 Security Association Database (SADB)

- **Global SADB**: 32 entries maximum
- **Per-socket SA**: Attach via socket option: `IP_IPSEC_SA`
- **Anti-replay**: 64-bit sliding window (RFC 4303 §3.4.3)
- **Lookup key**: (SPI, destination IP, protocol)

### 7.5 Configuration

#### Adding an SA via sysctl

```sh
# Add ESP SA (AES-GCM-128):
sysctl -w minix.lwip.ipsec.sadb=ADD,spi=0x1234,dst=203.0.113.5,proto=esp,\
  cipher=aes-gcm-128,key=<hex-key>,auth_key=<hex-auth-key>

# Add AH SA (HMAC-SHA256):
sysctl -w minix.lwip.ipsec.sadb=ADD,spi=0x5678,dst=203.0.113.5,proto=ah,\
  auth=hmac-sha256,auth_key=<hex-key>

# Delete SA:
sysctl -w minix.lwip.ipsec.sadb=DEL,spi=0x1234

# List SAs:
sysctl minix.lwip.ipsec.sadb

# View stats:
sysctl minix.lwip.ipsec.stats
```

#### Per-Socket SA

```c
// Attach IPsec SA to a specific socket:
int sa_spi = 0x1234;
setsockopt(fd, IPPROTO_IP, IP_IPSEC_SA, &sa_spi, sizeof(sa_spi));
```

### 7.6 Global Toggle

```sh
sysctl -w minix.lwip.ipsec.enabled=1  # Enable
sysctl -w minix.lwip.ipsec.enabled=0  # Disable
```

### 7.7 Limitations

- **Transport mode only**: No tunnel mode (used in VPN scenarios)
- **Manual keying only**: No IKE (Internet Key Exchange) support
- **No ESN**: Extended Sequence Number (RFC 4303) not supported
- **No PMTU discovery**: Fragmentation handled by lwIP but may interact
  poorly with IPsec headers
- **Single-threaded**: Crypto operations run on the lwIP event loop

---

## 8. DTLS over UDP

### 8.1 Purpose

Provides **Transport Layer Security for UDP datagrams** (Datagram TLS),
allowing secure communication for DNS-over-DTLS, SIP, and custom protocols.

### 8.2 Supported Versions

| Version | RFC | wolfSSL API |
|---------|-----|-------------|
| DTLS 1.2 | 6347 | `wolfDTLSv1_2_client_method()` / `server_method()` |
| DTLS 1.3 | 9147 | `wolfDTLSv1_3_client_method()` / `server_method()` |

### 8.3 Architecture

```
┌─────────────────────────────────────────────┐
│              UDP Socket (udpsock)            │
│                                               │
│  udpsock_setsockopt(UDP_DTLS, 1)             │
│    └── Enable DTLS on this socket            │
│                                               │
│  udpsock_send()                              │
│    └── Non-blocking DTLS handshake           │
│    └── Encrypt plaintext via wolfSSL         │
│    └── Send encrypted datagram               │
│                                               │
│  udpsock_recv()                              │
│    └── Receive encrypted datagram            │
│    └── Decrypt via wolfSSL                   │
│    └── Return plaintext                      │
└─────────────────────────────────────────────┘
```

### 8.4 State Machine

```
NONE → INIT → HANDSHAKE → ESTABLISHED → CLOSING → FAILED
                          │
                          └── Retransmit handshake if no response
                          └── Queue pending datagrams during handshake
```

### 8.5 Configuration

```c
// On UDP socket:
int dtls = 1;
setsockopt(fd, IPPROTO_UDP, UDP_DTLS, &dtls, sizeof(dtls));
```

```sh
# Global enable:
sysctl -w minix.lwip.dtls.enabled=1

# View stats:
sysctl minix.lwip.dtls.stats
```

### 8.6 Certificate-Based Auth

DTLS uses **wolfSSL** certificate-based authentication:

```c
// Required (via wolfSSL API, not exposed through setsockopt):
// 1. Load CA certificate:
wolfSSL_CTX_load_verify_buffer(ctx, ca_cert, ca_len, SSL_FILETYPE_ASN1);

// 2. Load client certificate:
wolfSSL_CTX_use_certificate_buffer(ctx, cert, cert_len, SSL_FILETYPE_ASN1);

// 3. Load private key:
wolfSSL_CTX_use_PrivateKey_buffer(ctx, key, key_len, SSL_FILETYPE_ASN1);
```

### 8.7 Limitations

- **Per-socket**: DTLS is enabled per-socket, not per-interface
- **Certificate management**: Not exposed via sysctl; certificates must
  be loaded programmatically
- **Performance**: DTLS adds per-datagram encryption overhead (~1-5%
  depending on datagram size)
- **Non-blocking handshake**: During handshake, datagrams are queued
  (up to 4) and sent after handshake completes
- **wolfSSL dependency**: Requires wolfSSL built with DTLS support
  (`WOLFSSL_DTLS`, `WOLFSSL_DTLS13`)

---

## 9. IPC Security Audit

### 9.1 Audit Scope

All IPC handlers in the lwIP service have been audited for security:

| Handler | File | Status |
|---------|------|--------|
| `rmib_process()` | `lwip.c` | ✅ Source endpoint validated |
| `sockevent_process()` | `lwip.c` | ✅ Message type validated |
| `bpfdev_process()` | `bpfdev.c` | ✅ Buffer sizes validated |
| `ndev_process()` | `ndev.c` | ✅ Source + type validated |

### 9.2 Findings

| Area | Finding | Status |
|------|---------|--------|
| Source validation | All handlers verify source endpoint | ✅ |
| Message type | `IS_SDEV_RQ`, `IS_CDEV_RQ`, `IS_NDEV_RS` macros used | ✅ |
| Buffer size | All copy-in operations have bounds checks | ✅ |
| Null termination | String copy uses `strlcpy` with max length | ✅ |
| Grant validation | `GRANT_VALID` checked before memory access | ✅ |
| Error handling | All errors return appropriate errno | ✅ |
| Unknown messages | `default:` case logs via `printf` | ✅ (Phase 4h) |

### 9.3 Improvement (Phase 4h)

Added `default:` case in `ndev_process()` to log unknown message types
from known drivers. This helps detect:
- Stale or corrupted messages
- Driver bugs
- Potential attacks

---

## Summary: Security Checklist

| Feature | Configuration | Status |
|---------|--------------|--------|
| SYN Cookies | `sysctl net.inet.tcp.syncookies` | ✅ On by default |
| TCP MD5 Signature | `sysctl net.inet.tcp.md5sig` | ✅ On by default |
| ICMP Rate Limit | Built-in (10/sec) | ✅ Always on |
| ARP Rate Limit | Built-in (50/sec) | ✅ Always on |
| NDP Rate Limit | Built-in (50/sec) | ✅ Always on |
| Ingress Filtering | `sysctl net.inet.ip.ingress_filter` | 🔲 Off by default |
| IPsec ESP/AH | `sysctl minix.lwip.ipsec.enabled` | 🔲 Off by default |
| WireGuard | `sysctl minix.lwip.wireguard.enabled` | 🔲 Off by default |
| DTLS | Per-socket (`UDP_DTLS`) | 🔲 Off by socket |
| IPC Validation | Built-in | ✅ Always on |

---

> **See also**: `docs/networking-guide.md` for practical setup,
> `docs/network-architecture.md` for internal architecture,
> `docs/network-performance.md` for performance tuning.
