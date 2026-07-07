# Minimal IPsec Implementation — ESP Transport + AH for MINIX lwIP

> **Phase**: 4f (Security)
> **RFC**: 4301 (IPsec), 4302 (AH), 4303 (ESP), 4835 (Crypto Profile)
> **Phase**: 4f (Security)
> **RFC**: 4301 (IPsec), 4302 (AH), 4303 (ESP), 4835 (Crypto Profile)
> **Status**: ✅ Implemented
> **Implementation files**:
> - `minix/lib/liblwip/lib/lwip_ipsec.h` (~120 LOC)
> - `minix/lib/liblwip/lib/lwip_ipsec.c` (~950 LOC)
> - `minix/lib/liblwip/patches/0008-MINIX-3-only-add-IPsec-ESP-AH-hooks.patch`
> - `minix/net/lwip/ipsec_sysctl.h`, `ipsec_sysctl.c` (~80 LOC)
> **Modified**: `lwipopts.h`, `lib/Makefile`, `lwiphooks.h`, `tcpsock.c`, `lwip.c`, `lwip.h`
> **Parent**: `planning/25_network_stack_modernization.md` §4f

---

## 1. Overview

### 1.1 Goals

Реализовать минимальный IPsec стек для MINIX lwIP, покрывающий функции, недоступные в WireGuard:

1. **ESP Transport Mode (RFC 4303)** — шифрование + аутентификация IP payload (TCP/UDP segment)
2. **AH Transport Mode (RFC 4302)** — аутентификация IP payload + immutable IP header fields
3. **Manual SA management** — per-socket setsockopt для установки Security Associations
4. **Cryptographic algorithms**:
   - AES-GCM-128/256 (authenticated encryption)
   - AES-CBC-128/256 + HMAC-SHA256 (encryption + separate auth)
   - ChaCha20-Poly1305 (современный cipher, как в WireGuard)
   - HMAC-SHA1/SHA256 (для AH и ESP auth)

### 1.2 Non-goals

- ❌ IKEv2 — только manual keying
- ❌ Tunnel Mode — покрыт WireGuard
- ❌ ESN (Extended Sequence Numbers, RFC 4304)
- ❌ IPcomp (RFC 3173)
- ❌ IPv6 ESP/AH
- ❌ NAT-T (NAT Traversal, RFC 3947)
- ❌ SA expiry/автоматический teardown
- ❌ Transport mode for multicast

### 1.3 Architecture: BITS (Bump In The Stack)

```
Outbound path (per-socket SA):

  APP socket (TCP/UDP)
       ↓
  tcpsock_send() / udpsock_send()
       ↓
  lwIP tcp_write() / udp_sendto()
       ↓
  lwIP ip_output() / ip_output_if()
       ↓
  ┌──────────────────────────────────────┐
  │  LWIP_HOOK_IP_OUTPUT (new)          │
  │  → lwip_ipsec_output():             │
  │    if socket has SA policy:          │
  │      1. Look up outbound SA by dest  │
  │      2. Add ESP/AH header            │
  │      3. Encrypt/auth payload         │
  │      4. Adjust IP header (proto=50/51│
  │         + total_len)                 │
  └──────────────────────────────────────┘
       ↓
  ndev → driver → wire

Inbound path (global SADB lookup by SPI):

  wire → driver → ndev_input()
       ↓
  lwIP ip_input()
       ↓
  ┌──────────────────────────────────────┐
  │  LWIP_HOOK_IP_INPUT (new)           │
  │  → lwip_ipsec_input():              │
  │    if IP proto is 50 (ESP) or 51 (AH)│
  │      1. Look up SA by (dst, SPI)     │
  │      2. Verify/auth/decrypt          │
  │      3. Strip IPsec header           │
  │      4. Reinject into ip_input()      │
  │         with inner protocol          │
  └──────────────────────────────────────┘
       ↓
  TCP/UDP socket delivery
```

### 1.4 Module Structure

```
minix/lib/liblwip/
  lib/
    lwip_ipsec.h          — API: structs, SADB, hook declarations
    lwip_ipsec.c          — SADB + ESP/AH transforms + wolfCrypt glue
  patches/
    0008-MINIX-3-only-add-IPsec-ESP-AH-hooks.patch
                          — Hooks in ip4_input(), ip_output()

minix/net/lwip/
  tcpsock.c               — IP_IPSEC_SA setsockopt/getsockopt
  lwip.c                  — lwip_ipsec_init() call
```

---

## 2. API Design

### 2.1 Socket Option

```c
/* Per-socket SA configuration */
#define IP_IPSEC_SA  42   /* IPPROTO_IP level */

struct ipsec_sa {
    uint32_t    spi;              /* Security Parameter Index */
    uint8_t     flags;            /* IPSEC_SA_FLAG_* */
    uint8_t     esp_cipher;       /* ESP encryption algorithm */
    uint8_t     ah_auth;          /* AH authentication algorithm */
    uint8_t     pad[1];           /* padding */
    uint8_t     enc_key[64];      /* encryption key (up to 64 bytes) */
    uint8_t     enc_keylen;       /* key length */
    uint8_t     auth_key[64];     /* authentication key (HMAC) */
    uint8_t     auth_keylen;      /* key length */
    uint8_t     enc_iv[16];       /* initial IV (for CBC) / salt (for GCM) */
};
```

### 2.2 Algorithm Identifiers

```c
/* Flags */
#define IPSEC_SA_FLAG_ESP      0x01  /* Enable ESP */
#define IPSEC_SA_FLAG_AH       0x02  /* Enable AH */
#define IPSEC_SA_FLAG_OUTBOUND 0x04  /* Outbound SA (per-socket) */

/* ESP ciphers */
#define IPSEC_ESP_AES_GCM_16   1  /* AES-GCM, 16-byte ICV */
#define IPSEC_ESP_AES_GCM_8    2  /* AES-GCM, 8-byte ICV */
#define IPSEC_ESP_AES_CBC      3  /* AES-CBC + separate HMAC */
#define IPSEC_ESP_CHACHA20_POLY1305  4  /* ChaCha20-Poly1305 */

/* AH algorithms */
#define IPSEC_AH_HMAC_SHA1_96  1  /* HMAC-SHA1-96 (RFC 2404) */
#define IPSEC_AH_HMAC_SHA256_128 2  /* HMAC-SHA256-128 (RFC 4868) */
```

### 2.3 SADB Internal Structure

```c
#define IPSEC_SADB_MAX_ENTRIES  32  /* maximum SA entries */

struct ipsec_sadb_entry {
    int         used;
    ip_addr_t   dst_ip;           /* destination IP */
    uint32_t    spi;              /* Security Parameter Index */
    uint8_t     proto;            /* IPPROTO_ESP or IPPROTO_AH */
    uint8_t     flags;
    uint8_t     esp_cipher;
    uint8_t     ah_auth;
    uint8_t     enc_key[64];
    uint8_t     enc_keylen;
    uint8_t     auth_key[64];
    uint8_t     auth_keylen;
    uint8_t     salt[4];          /* for GCM/ChaCha20 */
    uint32_t    seq;              /* outbound sequence number */
    uint32_t    replay_window;    /* inbound anti-replay bitmap */
    uint64_t    replay_counter;   /* highest seen seq */
};
```

### 2.4 Hook Functions

```c
/* Called from ip_output() when socket has SA */
err_t lwip_ipsec_output(struct pbuf *p, struct netif *netif,
    ip_addr_t *dest, struct tcp_pcb *pcb);

/* Called from ip_input() when IP proto is ESP (50) or AH (51) */
err_t lwip_ipsec_input(struct pbuf *p, struct netif *netif,
    struct ip_hdr *iphdr);

/* SADB management (for socket option handler) */
int  lwip_ipsec_add_sa(const struct ipsec_sa *sa, ip_addr_t *dst);
int  lwip_ipsec_del_sa(uint32_t spi, ip_addr_t *dst);
int  lwip_ipsec_get_sa(uint32_t spi, ip_addr_t *dst, struct ipsec_sa *sa);
```

---

## 3. Packet Formats

### 3.1 ESP Transport Mode (RFC 4303)

```
Before: [IP hdr][TCP/UDP hdr][payload...]

After (AES-GCM):
  [IP hdr][SPI=4][Seq=4][IV/Salt=8][CT...][Pad+PadLen+NxtHdr][ICV=16]

After (AES-CBC + HMAC):
  [IP hdr][SPI=4][Seq=4][IV=16][CT...][Pad+PadLen+NxtHdr][HMAC=12-16]
```

ESP header fields:
- SPI (4 bytes): identifies the SA
- Seq (4 bytes): monotonically increasing sequence number
- IV/Salt (8 bytes for GCM, 16 for CBC): initialization vector
- Payload: encrypted original IP payload
- Padding: 0-255 bytes (to cipher block boundary)
- Pad Length (1 byte): length of padding
- Next Header (1 byte): inner protocol (TCP=6, UDP=17)
- ICV (12-16 bytes): Integrity Check Value

Overhead: ~36-48 bytes per packet (for GCM with 8-byte IV)

### 3.2 AH Transport Mode (RFC 4302)

```
Before: [IP hdr][TCP/UDP hdr][payload...]

After:
  [IP hdr][NH=1][Len=1][Rsv=2][SPI=4][Seq=4][ICV=12][TCP/UDP hdr][payload...]
```

AH header:
- Next Header (1 byte): inner protocol
- Length (1 byte): AH length in 32-bit words minus 2
- Reserved (2 bytes): zero
- SPI (4 bytes)
- Seq (4 bytes)
- ICV (12 bytes for HMAC-SHA1-96, 16 for HMAC-SHA256-128)

ICV covers (for AH):
1. IP immutable fields (version, header length, total length, identification,
   protocol, source IP, destination IP) — with mutable fields zeroed
2. AH header (with ICV=0)
3. Upper-layer data (TCP/UDP)

Overhead: ~24 bytes per packet

---

## 4. Crypto Operations

### 4.1 ESP AES-GCM Encapsulation

```
Input:  plaintext (original IP payload), SA
Output: ciphertext (ESP payload)

1. Generate IV: 8 bytes (4-byte salt XOR 4-byte seq, or random)
2. Encrypt: AES-GCM(plaintext + padding + pad_len + next_hdr)
3. AAD: SPI (4) + Seq (4)
4. Result: IV || ciphertext || ICV
```

### 4.2 ESP AES-CBC + HMAC-SHA256

```
Input:  plaintext, SA
Output: ciphertext + HMAC

1. Generate IV: 16 random bytes
2. Pad plaintext to AES block boundary (16 bytes)
3. Encrypt: AES-CBC(IV, padded_data)
4. Authenticate: HMAC-SHA256(ciphertext || pad_len || next_hdr)
5. Result: IV || ciphertext || truncated HMAC (12 bytes)
```

### 4.3 AH HMAC Computation

```
Input:  IP header + AH header (ICV=0) + payload
Output: ICV

1. Copy IP header, zero mutable fields (TOS, TTL, flags, checksum)
2. Concatenate: modified_IP || AH(ICV=0) || payload
3. Compute: HMAC-SHA256(data, auth_key)
4. Truncate to 16 bytes (HMAC-SHA256-128)
```

### 4.4 Anti-Replay

Standard sliding window (RFC 4301 §2.2):
- 32-bit sequence number
- Window size: 64 packets
- If seq <= last_seq - window: drop
- If seq > last_seq: advance window
- If seq in window and not duplicate: accept
- Otherwise: drop

---

## 5. lwIP Hook Integration

### 5.1 New Hooks Needed

Two new hooks in lwIP core:

**`ip4.c` — `ip_input()`**:
```c
#if LWIP_IPSEC
  if (IPH_PROTO(iphdr) == IP_PROTO_ESP ||
      IPH_PROTO(iphdr) == IP_PROTO_AH) {
    if (lwip_ipsec_input(p, netif, iphdr) == ERR_OK) {
      pbuf_free(p);
      return;
    }
  }
#endif
```

**`ip.c` — `ip_output_if()`** (after IP header build, before netif->output):
```c
#if LWIP_IPSEC
  if (pcb != NULL && lwip_ipsec_has_sa(pcb)) {
    if (lwip_ipsec_output(p, netif, dest, pcb) != ERR_OK) {
      return ERR_OK;  /* packet transformed, send via netif->output */
    }
  }
#endif
```

### 5.2 Patch Strategy

Single patch file `0008-MINIX-3-only-add-IPsec-ESP-AH-hooks.patch`:
- `ip4.c`: ~15 lines added in `ip_input()`
- `ip.c`: ~15 lines added in `ip_output_if()`
- `lwipopts.h` (in dist): add `LWIP_IPSEC` option

---

## 6. Error Handling

| Scenario | Action |
|----------|--------|
| SA not found for inbound ESP/AH | Drop packet, increment `lwip_ipsec_stats.sa_miss` |
| ICV mismatch | Drop packet, increment `lwip_ipsec_stats.auth_fail` |
| Anti-replay drop | Drop packet, increment `lwip_ipsec_stats.replay_drop` |
| Outbound SA not found | Allow packet to pass through (bypass) |
| OOM during transform | Drop packet, return ERR_MEM |
| Unknown algorithm | Drop packet, return ERR_ARG |
| HMAC verification failed | Drop packet, increment `lwip_ipsec_stats.hmac_fail` |
| ESP padding invalid | Drop packet, increment `lwip_ipsec_stats.pad_fail` |

---

## 7. Integration Points

### 7.1 `lwipopts.h`

```c
#define LWIP_IPSEC    1       /* enable IPsec ESP/AH support */
```

### 7.2 `lib/Makefile`

```makefile
SRCS += lwip_ipsec.c
```

### 7.3 `tcpsock.c`

```c
case IPPROTO_IP:
    switch (name) {
    case IP_IPSEC_SA: {
        struct ipsec_sa sa;
        /* copyin struct, validate, add to SADB */
        if (lwip_ipsec_add_sa(&sa, &pcb->remote_ip) != 0)
            return EINVAL;
        return OK;
    }
    }
```

### 7.4 `lwip.c`

```c
/* In init(): */
lwip_ipsec_init();
```

### 7.5 `lwiphooks.h`

```c
#if LWIP_IPSEC
/* IPsec hook declarations */
err_t lwip_ipsec_input(struct pbuf *p, struct netif *netif,
    struct ip_hdr *iphdr);
err_t lwip_ipsec_output(struct pbuf *p, struct netif *netif,
    ip_addr_t *dest, struct tcp_pcb *pcb);
#define LWIP_HOOK_IP_INPUT(p, netif, hdr)   lwip_ipsec_input(p, netif, hdr)
#define LWIP_HOOK_IP_OUTPUT(p, netif, dest, pcb) \
    lwip_ipsec_output(p, netif, dest, pcb)
#else
#define LWIP_HOOK_IP_INPUT(p, netif, hdr)    ERR_OK
#define LWIP_HOOK_IP_OUTPUT(p, netif, dest, pcb)  ERR_OK
#endif
```

---

## 8. File Sizes

| File | LOC | Description |
|------|-----|-------------|
| `lwip_ipsec.h` | ~100 | API: structs, SADB, hook decl, stubs |
| `lwip_ipsec.c` | ~1000 | SADB, ESP/AH transforms, wolfCrypt |
| `patches/0008-*.patch` | ~50 | lwIP hooks in ip4.c, ip.c |
| Modified: lwipopts.h | +2 | LWIP_IPSEC=1 |
| Modified: lib/Makefile | +1 | lwip_ipsec.c |
| Modified: tcpsock.c | +60 | IP_IPSEC_SA handler |
| Modified: lwip.c | +3 | init call |
| Modified: lwiphooks.h | +15 | hook defines |
| **Total** | **~1231** | |

---

## 9. Implementation Order

1. `lwip_ipsec.h` — header with structs and declarations
2. `patches/0008-*.patch` — lwIP hooks
3. `lwip_ipsec.c` — implementation (SADB + transforms + wolfCrypt)
4. `lwipopts.h` — LWIP_IPSEC=1
5. `lib/Makefile` — add source
6. `lwiphooks.h` — hook defines
7. `tcpsock.c` — socket option
8. `lwip.c` — init call
