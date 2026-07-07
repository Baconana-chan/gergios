# DTLS (Datagram TLS) Integration for MINIX lwIP

> **Phase**: 4g (Security)
> **RFC**: 6347 (DTLS 1.2), 9147 (DTLS 1.3)
> **Status**: 📋 Plan (design complete, ready for implementation)
> **Parent**: `planning/25_network_stack_modernization.md` §4g

---

## 1. Overview

### 1.1 Goals

Реализовать DTLS (Datagram TLS) для MINIX lwIP, обеспечивающий шифрование и аутентификацию для UDP-based протоколов.

1. **DTLS 1.2 (RFC 6347)** — стабильный, широко поддерживаемый протокол
2. **DTLS 1.3 (RFC 9147)** — современный, более быстрый handshake (1-RTT vs 2-RTT)
3. **Per-socket DTLS** — включение DTLS через `setsockopt` на UDP сокете
4. **wolfSSL backend** — использует существующую wolfSSL библиотеку (уже в проекте для syslogd)
5. **Non-blocking handshake** — асинхронный DTLS handshake (совместим с lwIP event loop)

### 1.2 Non-goals

- ❌ TLS over TCP — покрыт lwIP ALTCP, не требуется для MINIX сейчас
- ❌ Mutual TLS (mTLS) — опционально, не в первой реализации
- ❌ Session resumption — опционально, может быть добавлено позже
- ❌ DTLS over IPv6 — будет работать через существующие IPv6 пути lwIP
- ❌ OCSP stapling — не требуется для embedded use case

### 1.3 Architecture: DTLS Shim Layer over UDP

DTLS не может использовать lwIP `ALTCP` (который спроектирован только для TCP). Вместо этого используется **DTLS shim layer**, который встраивается между приложением и lwIP UDP сокетом.

```
Outbound (DTLS encrypt):

  APP sendto(sock, plaintext, len, ...)
       ↓
  udpsock_sendto()
       ↓
  ┌────────────────────────────────┐
  │  if socket has DTLS enabled:  │
  │  1. Lookup DTLS session       │
  │  2. wolfSSL_write(ssl, pt, ln)│
  │  3. Get encrypted datagram    │
  │     from wolfSSL output buffer │
  │  4. Send via UDP pbuf         │
  └────────────────────────────────┘
       ↓
  lwIP udp_sendto() → IP → wire

Inbound (DTLS decrypt):

  wire → IP → lwIP udp_input()
       ↓
  udpsock_recvfrom()
       ↓
  ┌────────────────────────────────┐
  │  if socket has DTLS enabled:  │
  │  1. Get encrypted datagram    │
  │  2. Inject into wolfSSL       │
  │     (wolfSSL_read(ssl, buf))  │
  │  3. Decrypt, verify, return   │
  │     plaintext to application  │
  └────────────────────────────────┘
       ↓
  APP recvfrom(sock, plaintext, ...)
```

### 1.4 Почему не ALTCP

lwIP `ALTCP` — это **TCP-only** абстракция:
- Ориентирована на stream (TCP), не на datagrams (UDP)
- Требует `altcp_pcb` с `listen/accept` — семантика отсутствует в lwIP UDP
- Нельзя "завернуть" UDP pcb в ALTCP слой

Вместо этого DTLS интегрируется **на уровне udpsock.c**, где `udp_pcb` уже существует и обрабатывает датаграммы.

### 1.5 Module Structure

```
minix/lib/liblwip/
  lib/
    lwip_dtls.h            — API: DTLS session, config, declarations
    lwip_dtls.c            — DTLS session management + wolfSSL glue

minix/net/lwip/
  udpsock.c                — UDP_DTLS socket option, DTLS wrap/unwrap

crypto/external/gpl2/wolfssl/
  (wolfSSL library — DTLS 1.2/1.3 уже поддерживается)
```

---

## 2. API Design

### 2.1 Socket Option

```c
/* Socket option for enabling DTLS on a UDP socket */
#define UDP_DTLS  43   /* SOL_SOCKET or IPPROTO_UDP level */

/* DTLS configuration structure passed via setsockopt */
struct lwip_dtls_config {
    uint8_t     enable;           /* 1 = enable DTLS, 0 = disable */
    uint8_t     dtls_version;     /* LWIP_DTLS_1_2 or LWIP_DTLS_1_3 */
    uint8_t     role;             /* LWIP_DTLS_CLIENT or LWIP_DTLS_SERVER */
    uint8_t     pad;
    /* Certificate and key material (PEM strings or pointers) */
    const char *ca_cert;          /* CA certificate (PEM) */
    const char *cert;             /* Own certificate (PEM) — optional for client */
    const char *key;              /* Private key (PEM) — optional for client */
    size_t      ca_cert_len;
    size_t      cert_len;
    size_t      key_len;
    /* PSK mode (alternative to certificates) */
    const char *psk_identity;     /* PSK identity */
    const char *psk_key;          /* PSK key (hex) */
    size_t      psk_identity_len;
    size_t      psk_key_len;
};
```

### 2.2 DTLS Session State Machine

```c
/* Per-socket DTLS state */
enum lwip_dtls_state {
    LWIP_DTLS_NONE      = 0,   /* DTLS not configured */
    LWIP_DTLS_INIT      = 1,   /* wolfSSL initialized, not connected */
    LWIP_DTLS_HANDSHAKE = 2,   /* Handshake in progress (non-blocking) */
    LWIP_DTLS_ESTABLISHED = 3, /* Handshake complete, ready for data */
    LWIP_DTLS_CLOSING   = 4,   /* Closure in progress */
    LWIP_DTLS_FAILED    = 5,   /* Handshake or crypto failure */
};

/* Internal DTLS session (per UDP socket) */
struct lwip_dtls_session {
    uint8_t     state;            /* LWIP_DTLS_* state */
    uint8_t     version;          /* DTLS version in use */
    uint8_t     role;             /* client or server */
    uint8_t     pad;
    WOLFSSL_CTX *ctx;             /* wolfSSL context (shared) */
    WOLFSSL     *ssl;             /* wolfSSL session object */
    struct udp_pcb *pcb;          /* owning UDP PCB */
    ip_addr_t   peer_addr;        /* DTLS peer address */
    uint16_t    peer_port;        /* DTLS peer port */
    /* Buffers for non-blocking I/O */
    uint8_t     *pending_data;    /* Decrypted data waiting for read */
    size_t      pending_len;
    size_t      pending_offset;
};
```

### 2.3 Key Functions

```c
/* Initialize DTLS subsystem (must call wolfSSL_Init()) */
void lwip_dtls_init(void);

/* Attach DTLS to a UDP PCB */
int lwip_dtls_attach(struct udp_pcb *pcb,
    const struct lwip_dtls_config *config);

/* Detach DTLS from a UDP PCB (cleanup) */
void lwip_dtls_detach(struct udp_pcb *pcb);

/* Process incoming DTLS datagram (called from udp_recv callback) */
/* Returns ERR_OK if packet was consumed by DTLS layer */
err_t lwip_dtls_input(struct udp_pcb *pcb, struct pbuf *p,
    const ip_addr_t *addr, u16_t port);

/* Encrypt outgoing datagram (called before udp_sendto) */
/* Returns pbuf with DTLS-encapsulated data, or NULL on error */
struct pbuf *lwip_dtls_output(struct udp_pcb *pcb, struct pbuf *p,
    const ip_addr_t *addr, u16_t port);

/* Read decrypted data from DTLS session */
int lwip_dtls_read(struct udp_pcb *pcb, void *buf, size_t len);

/* Continue pending handshake (called from timer/event loop) */
err_t lwip_dtls_poll(struct udp_pcb *pcb);

/* Check if DTLS is established on this PCB */
int lwip_dtls_is_established(struct udp_pcb *pcb);

/* Get underlying wolfSSL object for advanced config */
WOLFSSL *lwip_dtls_get_ssl(struct udp_pcb *pcb);
```

---

## 3. WolfSSL Integration

### 3.1 WolfSSL Build Configuration

wolfSSL уже интегрирован в проект (`crypto/external/gpl2/wolfssl/`). Для DTLS необходимы изменения в `crypto/Makefile.wolfssl`:

```makefile
# Текущие проблемные defines (блокируют DTLS):
# CPPFLAGS+=\t-DNO_WOLFSSL_CLIENT    ← УДАЛИТЬ (блокирует wolfSSL_connect)
# CPPFLAGS+=\t-DNO_WOLFSSL_SERVER    ← УДАЛИТЬ (блокирует wolfSSL_accept)

# Добавить для DTLS:
CPPFLAGS+=\t-DWOLFSSL_DTLS          # Enable DTLS support
CPPFLAGS+=\t-DWOLFSSL_DTLS13        # Enable DTLS 1.3 (RFC 9147)
CPPFLAGS+=\t-DHAVE_SECRET_CALLBACK  # For debugging/tracing
```

Также может потребоваться `WOLFSSL_LWIP` для lwIP-specific оптимизаций.

Так как замена defines в глобальном `crypto/Makefile.wolfssl` может затронуть syslogd и другие компоненты, более безопасный подход — создать **отдельный набор defines** только для lwIP сборки. Либо через `lwipopts.h`, либо через локальный `Makefile.inc` в `minix/lib/liblwip/lib/`.

### 3.2 WolfSSL DTLS API Usage

```c
/* Initialization */
wolfSSL_Init();

/* Create DTLS context */
WOLFSSL_CTX *ctx;
ctx = wolfSSL_CTX_new(wolfDTLSv1_3_server_method());  /* server */
/* или */
ctx = wolfSSL_CTX_new(wolfDTLSv1_3_client_method());  /* client */

/* Load certificates */
wolfSSL_CTX_use_certificate_buffer(ctx, cert_pem, cert_len, SSL_FILETYPE_PEM);
wolfSSL_CTX_use_PrivateKey_buffer(ctx, key_pem, key_len, SSL_FILETYPE_PEM);
wolfSSL_CTX_load_verify_buffer(ctx, ca_pem, ca_len, SSL_FILETYPE_PEM);

/* Set DTLS peer (for connection-oriented DTLS over UDP) */
wolfSSL_dtls_set_peer(ssl, &peer_addr_sa, sizeof(peer_addr_sa));

/* Non-blocking DTLS handshake */
wolfSSL_set_using_nonblock(ssl, 1);
int ret = wolfSSL_connect(ssl);   /* или wolfSSL_accept(ssl) */
if (ret != SSL_SUCCESS) {
    int err = wolfSSL_get_error(ssl, ret);
    if (err == SSL_ERROR_WANT_READ || err == SSL_ERROR_WANT_WRITE) {
        /* Need to poll/schedule retry */
        return ERR_INPROGRESS;
    }
    /* Fatal error */
    return ERR_ABRT;
}

/* Data transfer (DTLS over UDP) */
int written = wolfSSL_write(ssl, plaintext, len);
int read = wolfSSL_read(ssl, buffer, bufsize);

/* Custom I/O callbacks (instead of socket fd) */
wolfSSL_SetIORecv(ssl, lwip_dtls_recv_cb);
wolfSSL_SetIOSend(ssl, lwip_dtls_send_cb);
```

### 3.3 Custom I/O Callbacks

Так как lwIP использует pbuf-based API вместо POSIX sockets, нужны кастомные I/O callbacks:

```c
/* Callback: wolfSSL wants to send data */
static int
lwip_dtls_send_cb(WOLFSSL *ssl, char *buf, int sz, void *ctx)
{
    struct lwip_dtls_session *session = (struct lwip_dtls_session *)ctx;
    struct pbuf *p;
    err_t err;

    p = pbuf_alloc(PBUF_TRANSPORT, sz, PBUF_RAM);
    if (p == NULL) return WOLFSSL_CBIO_ERR_GENERAL;

    memcpy(p->payload, buf, sz);

    err = udp_sendto(session->pcb, p,
        &session->peer_addr, session->peer_port);
    pbuf_free(p);

    return (err == ERR_OK) ? sz : WOLFSSL_CBIO_ERR_GENERAL;
}

/* Callback: wolfSSL wants to receive data */
static int
lwip_dtls_recv_cb(WOLFSSL *ssl, char *buf, int sz, void *ctx)
{
    struct lwip_dtls_session *session = (struct lwip_dtls_session *)ctx;
    /* Data is already in session->input_buf from udp_recv */
    /* (managed by lwip_dtls_input() which stores the pbuf) */
    int avail = session->input_len - session->input_offset;
    if (avail <= 0) return WOLFSSL_CBIO_ERR_WANT_READ;

    int copy = (avail < sz) ? avail : sz;
    memcpy(buf, session->input_buf + session->input_offset, copy);
    session->input_offset += copy;
    return copy;
}
```

---

## 4. UDP Socket Integration

### 4.1 Integration Points in udpsock.c

```c
/* In udpsock_setsockopt(): */
case IPPROTO_UDP:
    switch (name) {
    case UDP_DTLS: {
        struct lwip_dtls_config config;
        /* copyin config from userspace */
        if (lwip_dtls_attach(pcb, &config) != 0)
            return EINVAL;
        return OK;
    }
    }

/* In udpsock_sendto(): */
/* Before calling udp_sendto, check if DTLS is enabled */
if (lwip_dtls_is_established(pcb)) {
    p = lwip_dtls_output(pcb, p, addr, port);
    if (p == NULL)
        return EIO;  /* DTLS error */
}

/* In udpsock_recvfrom() / udp_recv callback: */
/* Before delivering to user, check if DTLS is enabled */
if (lwip_dtls_is_established(pcb)) {
    /* Hand packet to DTLS layer for decryption */
    lwip_dtls_input(pcb, p, addr, port);
    /* Return decrypted data on next recvfrom call */
}
```

### 4.2 DTLS Session Lifecycle

```
setsockopt(UDP_DTLS) → lwip_dtls_attach()
  ↓
[ DTLS init: wolfSSL_CTX_new, wolfSSL_new, load certs ]
  ↓
udp_sendto() → DTLS client → wolfSSL_connect() (non-blocking)
  ├─ SSL_ERROR_WANT_READ  → schedule dtls_poll timer
  ├─ SSL_ERROR_WANT_WRITE → schedule dtls_poll timer
  └─ SSL_SUCCESS          → LWIP_DTLS_ESTABLISHED

[ Established: data flows through encrypt/decrypt ]

close() / shutdown() → lwip_dtls_detach()
  ├─ wolfSSL_shutdown()
  ├─ wolfSSL_free()
  └─ wolfSSL_CTX_free()
```

### 4.3 Non-Blocking Handshake with Event Loop

DTLS handshake может требовать нескольких round-trips. В lwIP однопоточном сервисе, handshake должен быть асинхронным:

```c
/* After initial connect/accept attempt returns WANT_READ/WANT_WRITE: */
/* Schedule a timer to retry the handshake */
sys_timeout(DTLS_HANDSHAKE_RETRY_MS, lwip_dtls_handshake_retry, pcb);

/* The retry callback polls the handshake progress */
static void
lwip_dtls_handshake_retry(void *arg)
{
    struct udp_pcb *pcb = (struct udp_pcb *)arg;

    if (lwip_dtls_poll(pcb) == ERR_OK) {
        /* Handshake complete — socket is ready */
    } else {
        /* Need more time — schedule another retry */
        sys_timeout(DTLS_HANDSHAKE_RETRY_MS,
            lwip_dtls_handshake_retry, pcb);
    }
}
```

---

## 5. File Budget

| File | LOC | Description |
|------|-----|-------------|
| `minix/lib/liblwip/lib/lwip_dtls.h` | ~100 | API, config struct, session struct, declarations |
| `minix/lib/liblwip/lib/lwip_dtls.c` | ~800 | DTLS session management, wolfSSL glue, I/O callbacks |
| Modified: `minix/net/lwip/udpsock.c` | +80 | UDP_DTLS setsockopt/getsockopt, dtls_output/dtls_input hooks |
| Modified: `crypto/Makefile.wolfssl` | ±2 | Remove NO_WOLFSSL_CLIENT/SERVER, add WOLFSSL_DTLS/DTLS13 |
| Modified: `minix/lib/liblwip/lib/lwipopts.h` | +2 | `LWIP_DTLS=1` |
| Modified: `minix/lib/liblwip/lib/Makefile` | +1 | `lwip_dtls.c` |
| **Total** | **~985** | |

---

## 6. Supported Algorithms

| Algorithm | DTLS 1.2 | DTLS 1.3 |
|-----------|----------|----------|
| TLS_AES_128_GCM_SHA256 | ✅ | ✅ |
| TLS_AES_256_GCM_SHA384 | ✅ | ✅ |
| TLS_CHACHA20_POLY1305_SHA256 | — | ✅ (wolfSSL) |
| TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 | ✅ | — |
| TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 | ✅ | — |
| ECDHE key exchange | ✅ | ✅ |
| PSK (Pre-Shared Key) | ✅ | ✅ |
| Certificate-based auth | ✅ | ✅ |

---

## 7. Error Handling

| Scenario | Action |
|----------|--------|
| DTLS handshake timeout | Drop session, return ETIMEDOUT to application |
| Certificate verification failure | Drop session, return EACCES/EPERM |
| Decryption failure (bad record MAC) | Drop datagram, continue, increment stats |
| Replay (duplicate seq) | Drop silently (DTLS built-in anti-replay) |
| OOM during DTLS operation | Drop packet, return ERR_MEM |
| Unknown cipher suite | Return EINVAL at setsockopt time |

---

## 8. Integration Points

### 8.1 `lwipopts.h`

```c
#define LWIP_DTLS     1       /* Enable DTLS over UDP support */
```

### 8.2 `lib/Makefile`

```makefile
SRCS += lwip_dtls.c
```

### 8.3 `crypto/Makefile.wolfssl` (changes)

```makefile
# Удалить эти строки:
# CPPFLAGS+=\t-DNO_WOLFSSL_CLIENT
# CPPFLAGS+=\t-DNO_WOLFSSL_SERVER

# Добавить эти строки (только для lwIP):
# CPPFLAGS+=\t-DWOLFSSL_DTLS
# CPPFLAGS+=\t-DWOLFSSL_DTLS13
```

**Примечание**: `NO_WOLFSSL_CLIENT` и `NO_WOLFSSL_SERVER` могут быть нужны для syslogd. Лучше создать отдельную конфигурацию wolfSSL для lwIP (через локальные defines в `lwipopts.h` через `-include` или через отдельную сборку), чтобы не сломать существующие компоненты.

### 8.4 `lwip.c`

```c
/* In init(), after lwip_ipsec_init(): */
lwip_dtls_init();
```

---

## 9. Implementation Order

1. Update `crypto/Makefile.wolfssl` (or create lwIP-specific wolfSSL config)
2. Create `lwip_dtls.h` — structures and API declarations
3. Create `lwip_dtls.c` — wolfSSL DTLS integration (session management, I/O callbacks, handshake)
4. Update `lwipopts.h` — `LWIP_DTLS=1`
5. Update `lib/Makefile` — add source
6. Update `udpsock.c` — `UDP_DTLS` socket option, hook DTLS encrypt/decrypt in sendto/recv paths
7. Update `lwip.c` — call `lwip_dtls_init()`
8. Test with DTLS client/server utilities

---

## 10. Testing Strategy

| Test | Tool/Method | Criteria |
|------|-------------|----------|
| DTLS 1.2 handshake | openssl s_server/s_client (dtls1_2) | Successful handshake |
| DTLS 1.3 handshake | wolfSSL example client/server | Successful handshake |
| Data transfer | echo server | Correct round-trip |
| Non-blocking handshake | Simulated delay | Completes within timeout |
| Certificate verification | Self-signed cert chain | Correct accept/reject |
| PSK mode | PSK-configured client/server | Successful handshake |
| Large datagrams (1472 bytes) | Stress test | No fragmentation errors |
| Error recovery | Corrupt packets | Graceful degradation |
| Regression | test94 (UDP tests) | All PASS |
