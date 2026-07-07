/*
 * MINIX 3 specific TCP MD5 signature support (RFC 2385) for BGP peering.
 *
 * This implementation uses lwIP's tcp_ext_arg API to store per-connection
 * MD5 keys, and the LWIP_HOOK_TCP_* hooks to intercept outgoing and
 * incoming TCP segments for MD5 option processing.
 *
 * The MD5 digest is computed over:
 *   TCP pseudo-header (IP src + dst + protocol + TCP length) +
 *   TCP header (except options, with checksum set to 0) +
 *   TCP segment data +
 *   shared secret (key)
 */
#include "lwip/opt.h"

#if LWIP_TCP_MD5SIG /* only build if configured */

#include <string.h>

#include "lwip_tcp_md5.h"
#include "lwip/tcp.h"
#include "lwip/priv/tcp_priv.h"
#include "lwip/ip.h"
#include "lwip/ip6.h"
#include "lwip/pbuf.h"
#include "lwip/inet_chksum.h"
#include "lwip/err.h"
#include "lwip/debug.h"

/* ------------------------------------------------------------------ */
/*  Minimal MD5 implementation (RFC 1321, public domain)              */
/* ------------------------------------------------------------------ */

/*
 * MD5 constants
 */
#define MD5_S11 7
#define MD5_S12 12
#define MD5_S13 17
#define MD5_S14 22
#define MD5_S21 5
#define MD5_S22 9
#define MD5_S23 14
#define MD5_S24 20
#define MD5_S31 4
#define MD5_S32 11
#define MD5_S33 16
#define MD5_S34 23
#define MD5_S41 6
#define MD5_S42 10
#define MD5_S43 15
#define MD5_S44 21

static const uint8_t md5_padding[64] = {
  0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
};

#define MD5_F(x, y, z) (((x) & (y)) | ((~x) & (z)))
#define MD5_G(x, y, z) (((x) & (z)) | ((y) & (~z)))
#define MD5_H(x, y, z) ((x) ^ (y) ^ (z))
#define MD5_I(x, y, z) ((y) ^ ((x) | (~z)))

#define MD5_ROTATE_LEFT(x, n) (((x) << (n)) | ((x) >> (32 - (n))))

#define MD5_FF(a, b, c, d, x, s, ac) { \
  (a) += MD5_F((b), (c), (d)) + (x) + (uint32_t)(ac); \
  (a) = MD5_ROTATE_LEFT((a), (s)); \
  (a) += (b); \
}

#define MD5_GG(a, b, c, d, x, s, ac) { \
  (a) += MD5_G((b), (c), (d)) + (x) + (uint32_t)(ac); \
  (a) = MD5_ROTATE_LEFT((a), (s)); \
  (a) += (b); \
}

#define MD5_HH(a, b, c, d, x, s, ac) { \
  (a) += MD5_H((b), (c), (d)) + (x) + (uint32_t)(ac); \
  (a) = MD5_ROTATE_LEFT((a), (s)); \
  (a) += (b); \
}

#define MD5_II(a, b, c, d, x, s, ac) { \
  (a) += MD5_I((b), (c), (d)) + (x) + (uint32_t)(ac); \
  (a) = MD5_ROTATE_LEFT((a), (s)); \
  (a) += (b); \
}

struct lwip_md5_ctx {
  uint32_t state[4];
  uint32_t count[2];
  uint8_t  buffer[64];
};

static void
lwip_md5_init(struct lwip_md5_ctx *ctx)
{
  ctx->state[0] = 0x67452301UL;
  ctx->state[1] = 0xefcdab89UL;
  ctx->state[2] = 0x98badcfeUL;
  ctx->state[3] = 0x10325476UL;
  ctx->count[0] = 0;
  ctx->count[1] = 0;
}

static void
lwip_md5_transform(uint32_t state[4], const uint8_t block[64])
{
  uint32_t a, b, c, d, x[16];
  int i;

  for (i = 0; i < 16; i++) {
    x[i] = (uint32_t)block[i * 4] |
           ((uint32_t)block[i * 4 + 1] << 8) |
           ((uint32_t)block[i * 4 + 2] << 16) |
           ((uint32_t)block[i * 4 + 3] << 24);
  }

  a = state[0]; b = state[1]; c = state[2]; d = state[3];

  MD5_FF(a, b, c, d, x[ 0], MD5_S11, 0xd76aa478UL);
  MD5_FF(d, a, b, c, x[ 1], MD5_S12, 0xe8c7b756UL);
  MD5_FF(c, d, a, b, x[ 2], MD5_S13, 0x242070dbUL);
  MD5_FF(b, c, d, a, x[ 3], MD5_S14, 0xc1bdceeeUL);
  MD5_FF(a, b, c, d, x[ 4], MD5_S11, 0xf57c0fafUL);
  MD5_FF(d, a, b, c, x[ 5], MD5_S12, 0x4787c62aUL);
  MD5_FF(c, d, a, b, x[ 6], MD5_S13, 0xa8304613UL);
  MD5_FF(b, c, d, a, x[ 7], MD5_S14, 0xfd469501UL);
  MD5_FF(a, b, c, d, x[ 8], MD5_S11, 0x698098d8UL);
  MD5_FF(d, a, b, c, x[ 9], MD5_S12, 0x8b44f7afUL);
  MD5_FF(c, d, a, b, x[10], MD5_S13, 0xffff5bb1UL);
  MD5_FF(b, c, d, a, x[11], MD5_S14, 0x895cd7beUL);
  MD5_FF(a, b, c, d, x[12], MD5_S11, 0x6b901122UL);
  MD5_FF(d, a, b, c, x[13], MD5_S12, 0xfd987193UL);
  MD5_FF(c, d, a, b, x[14], MD5_S13, 0xa679438eUL);
  MD5_FF(b, c, d, a, x[15], MD5_S14, 0x49b40821UL);

  MD5_GG(a, b, c, d, x[ 1], MD5_S21, 0xf61e2562UL);
  MD5_GG(d, a, b, c, x[ 6], MD5_S22, 0xc040b340UL);
  MD5_GG(c, d, a, b, x[11], MD5_S23, 0x265e5a51UL);
  MD5_GG(b, c, d, a, x[ 0], MD5_S24, 0xe9b6c7aaUL);
  MD5_GG(a, b, c, d, x[ 5], MD5_S21, 0xd62f105dUL);
  MD5_GG(d, a, b, c, x[10], MD5_S22, 0x02441453UL);
  MD5_GG(c, d, a, b, x[15], MD5_S23, 0xd8a1e681UL);
  MD5_GG(b, c, d, a, x[ 4], MD5_S24, 0xe7d3fbc8UL);
  MD5_GG(a, b, c, d, x[ 9], MD5_S21, 0x21e1cde6UL);
  MD5_GG(d, a, b, c, x[14], MD5_S22, 0xc33707d6UL);
  MD5_GG(c, d, a, b, x[ 3], MD5_S23, 0xf4d50d87UL);
  MD5_GG(b, c, d, a, x[ 8], MD5_S24, 0x455a14edUL);
  MD5_GG(a, b, c, d, x[13], MD5_S21, 0xa9e3e905UL);
  MD5_GG(d, a, b, c, x[ 2], MD5_S22, 0xfcefa3f8UL);
  MD5_GG(c, d, a, b, x[ 7], MD5_S23, 0x676f02d9UL);
  MD5_GG(b, c, d, a, x[12], MD5_S24, 0x8d2a4c8aUL);

  MD5_HH(a, b, c, d, x[ 5], MD5_S31, 0xfffa3942UL);
  MD5_HH(d, a, b, c, x[ 8], MD5_S32, 0x8771f681UL);
  MD5_HH(c, d, a, b, x[11], MD5_S33, 0x6d9d6122UL);
  MD5_HH(b, c, d, a, x[14], MD5_S34, 0xfde5380cUL);
  MD5_HH(a, b, c, d, x[ 1], MD5_S31, 0xa4beea44UL);
  MD5_HH(d, a, b, c, x[ 4], MD5_S32, 0x4bdecfa9UL);
  MD5_HH(c, d, a, b, x[ 7], MD5_S33, 0xf6bb4b60UL);
  MD5_HH(b, c, d, a, x[10], MD5_S34, 0xbebfbc70UL);
  MD5_HH(a, b, c, d, x[13], MD5_S31, 0x289b7ec6UL);
  MD5_HH(d, a, b, c, x[ 0], MD5_S32, 0xeaa127faUL);
  MD5_HH(c, d, a, b, x[ 3], MD5_S33, 0xd4ef3085UL);
  MD5_HH(b, c, d, a, x[ 6], MD5_S34, 0x04881d05UL);
  MD5_HH(a, b, c, d, x[ 9], MD5_S31, 0xd9d4d039UL);
  MD5_HH(d, a, b, c, x[12], MD5_S32, 0xe6db99e5UL);
  MD5_HH(c, d, a, b, x[15], MD5_S33, 0x1fa27cf8UL);
  MD5_HH(b, c, d, a, x[ 2], MD5_S34, 0xc4ac5665UL);

  MD5_II(a, b, c, d, x[ 0], MD5_S41, 0xf4292244UL);
  MD5_II(d, a, b, c, x[ 7], MD5_S42, 0x432aff97UL);
  MD5_II(c, d, a, b, x[14], MD5_S43, 0xab9423a7UL);
  MD5_II(b, c, d, a, x[ 5], MD5_S44, 0xfc93a039UL);
  MD5_II(a, b, c, d, x[12], MD5_S41, 0x655b59c3UL);
  MD5_II(d, a, b, c, x[ 3], MD5_S42, 0x8f0ccc92UL);
  MD5_II(c, d, a, b, x[10], MD5_S43, 0xffeff47dUL);
  MD5_II(b, c, d, a, x[ 1], MD5_S44, 0x85845dd1UL);
  MD5_II(a, b, c, d, x[ 8], MD5_S41, 0x6fa87e4fUL);
  MD5_II(d, a, b, c, x[15], MD5_S42, 0xfe2ce6e0UL);
  MD5_II(c, d, a, b, x[ 6], MD5_S43, 0xa3014314UL);
  MD5_II(b, c, d, a, x[13], MD5_S44, 0x4e0811a1UL);
  MD5_II(a, b, c, d, x[ 4], MD5_S41, 0xf7537e82UL);
  MD5_II(d, a, b, c, x[11], MD5_S42, 0xbd3af235UL);
  MD5_II(c, d, a, b, x[ 2], MD5_S43, 0x2ad7d2bbUL);
  MD5_II(b, c, d, a, x[ 9], MD5_S44, 0xeb86d391UL);

  state[0] += a;
  state[1] += b;
  state[2] += c;
  state[3] += d;
}

static void
lwip_md5_update(struct lwip_md5_ctx *ctx, const void *data, size_t len)
{
  const uint8_t *input = (const uint8_t *)data;
  size_t i, index, part_len;

  index = (size_t)((ctx->count[0] >> 3) & 0x3F);

  if ((ctx->count[0] += (uint32_t)(len << 3)) < (uint32_t)(len << 3))
    ctx->count[1]++;
  ctx->count[1] += (uint32_t)(len >> 29);

  part_len = 64 - index;

  if (len >= part_len) {
    memcpy(&ctx->buffer[index], input, part_len);
    lwip_md5_transform(ctx->state, ctx->buffer);

    for (i = part_len; i + 63 < len; i += 64)
      lwip_md5_transform(ctx->state, &input[i]);

    index = 0;
  } else
    i = 0;

  memcpy(&ctx->buffer[index], &input[i], len - i);
}

static void
lwip_md5_final(struct lwip_md5_ctx *ctx, uint8_t digest[16])
{
  uint8_t bits[8];
  size_t index, pad_len;
  int i;

  bits[0] = (uint8_t)(ctx->count[0] & 0xFF);
  bits[1] = (uint8_t)((ctx->count[0] >> 8) & 0xFF);
  bits[2] = (uint8_t)((ctx->count[0] >> 16) & 0xFF);
  bits[3] = (uint8_t)((ctx->count[0] >> 24) & 0xFF);
  bits[4] = (uint8_t)(ctx->count[1] & 0xFF);
  bits[5] = (uint8_t)((ctx->count[1] >> 8) & 0xFF);
  bits[6] = (uint8_t)((ctx->count[1] >> 16) & 0xFF);
  bits[7] = (uint8_t)((ctx->count[1] >> 24) & 0xFF);

  index = (size_t)((ctx->count[0] >> 3) & 0x3F);
  pad_len = (index < 56) ? (56 - index) : (120 - index);

  lwip_md5_update(ctx, md5_padding, pad_len);
  lwip_md5_update(ctx, bits, 8);

  for (i = 0; i < 4; i++) {
    digest[i * 4]     = (uint8_t)(ctx->state[i] & 0xFF);
    digest[i * 4 + 1] = (uint8_t)((ctx->state[i] >> 8) & 0xFF);
    digest[i * 4 + 2] = (uint8_t)((ctx->state[i] >> 16) & 0xFF);
    digest[i * 4 + 3] = (uint8_t)((ctx->state[i] >> 24) & 0xFF);
  }
}

/* ------------------------------------------------------------------ */
/*  TCP MD5 key storage (tcp_ext_arg API)                             */
/* ------------------------------------------------------------------ */

/* The ext_arg ID allocated at init time. */
static u8_t tcp_md5_ext_arg_id;

/*
 * Runtime enable/disable toggle.  Exposed via sysctl as
 * net.inet.tcp.md5sig.  Default enabled (1).
 */
int lwip_tcp_md5_enabled = 1;

/*
 * Destroy callback: free key data when PCB is destroyed.
 */
static void
tcp_md5_extarg_destroy(u8_t id, void *data)
{
  struct tcp_md5_key *key;

  LWIP_UNUSED_ARG(id);

  key = (struct tcp_md5_key *)data;

  if (key != NULL) {
    memset(key, 0, sizeof(*key));
    mem_free(key);
  }
}

/*
 * Passive-open callback: inherit MD5 key from listener to new connection.
 */
static err_t
tcp_md5_extarg_passive_open(u8_t id, struct tcp_pcb_listen *lpcb,
    struct tcp_pcb *cpcb)
{
  const struct tcp_md5_key *lkey;
  struct tcp_md5_key *ckey;

  LWIP_UNUSED_ARG(id);

  lkey = (const struct tcp_md5_key *)
      tcp_ext_arg_get((const struct tcp_pcb *)lpcb, tcp_md5_ext_arg_id);

  if (lkey == NULL)
    return ERR_OK;

  /* If the listener has a specific peer address, only inherit if it matches. */
  if (!ip_addr_isany(&lkey->peer_addr)) {
    if (!ip_addr_cmp(&lkey->peer_addr, &cpcb->remote_ip))
      return ERR_OK;
  }

  /* Allocate and copy the key to the new connection PCB. */
  ckey = (struct tcp_md5_key *)mem_malloc(sizeof(struct tcp_md5_key));
  if (ckey == NULL)
    return ERR_MEM;

  SMEMCPY(ckey, lkey, sizeof(struct tcp_md5_key));
  tcp_ext_arg_set(cpcb, tcp_md5_ext_arg_id, ckey);

  return ERR_OK;
}

static const struct tcp_ext_arg_callbacks tcp_md5_ext_arg_callbacks = {
  tcp_md5_extarg_destroy,
  tcp_md5_extarg_passive_open
};

/* ------------------------------------------------------------------ */
/*  Public API                                                         */
/* ------------------------------------------------------------------ */

void
lwip_tcp_md5_init(void)
{
  tcp_md5_ext_arg_id = tcp_ext_arg_alloc_id();

  LWIP_ASSERT("tcp_md5_ext_arg_id != LWIP_TCP_PCB_NUM_EXT_ARG_ID_INVALID",
      tcp_md5_ext_arg_id != LWIP_TCP_PCB_NUM_EXT_ARG_ID_INVALID);
}

err_t
lwip_tcp_md5_set_key(struct tcp_pcb *pcb, const struct tcp_md5sig *md5sig)
{
  struct tcp_md5_key *key;
  const struct sockaddr_in *sin;
  const struct sockaddr_in6 *sin6;
  struct tcp_md5_key *old_key;

  if (pcb == NULL || md5sig == NULL)
    return ERR_ARG;

  if (md5sig->tcpm_keylen > TCP_MD5SIG_MAXKEYLEN)
    return ERR_ARG;

  /* Allocate new key structure. */
  key = (struct tcp_md5_key *)mem_malloc(sizeof(struct tcp_md5_key));
  if (key == NULL)
    return ERR_MEM;

  memset(key, 0, sizeof(*key));

  /* Extract peer address from sockaddr_storage. */
  if (md5sig->tcpm_addr.ss_family == AF_INET) {
    sin = (const struct sockaddr_in *)&md5sig->tcpm_addr;
    ip_addr_set_ip4_u32(&key->peer_addr, sin->sin_addr.s_addr);
#if LWIP_IPV6
  } else if (md5sig->tcpm_addr.ss_family == AF_INET6) {
    sin6 = (const struct sockaddr_in6 *)&md5sig->tcpm_addr;
    memcpy(ip_2_ip6(&key->peer_addr)->addr, sin6->sin6_addr.s6_addr, 16);
#endif /* LWIP_IPV6 */
  } else if (md5sig->tcpm_addr.ss_family == AF_UNSPEC) {
    /* Wildcard address -- all zeros, ip_addr_isany() will return true. */
    ip_addr_set_any(LWIP_IPV6, &key->peer_addr);
  } else {
    mem_free(key);
    return ERR_ARG;
  }

  /* Copy the key. */
  SMEMCPY(key->key, md5sig->tcpm_key, md5sig->tcpm_keylen);
  key->keylen = md5sig->tcpm_keylen;

  /* Replace any existing key. */
  old_key = (struct tcp_md5_key *)tcp_ext_arg_get(pcb, tcp_md5_ext_arg_id);
  if (old_key != NULL) {
    memset(old_key, 0, sizeof(*old_key));
    mem_free(old_key);
  }

  tcp_ext_arg_set_callbacks(pcb, tcp_md5_ext_arg_id,
      &tcp_md5_ext_arg_callbacks);
  tcp_ext_arg_set(pcb, tcp_md5_ext_arg_id, key);

  return ERR_OK;
}

err_t
lwip_tcp_md5_get_key(const struct tcp_pcb *pcb, struct tcp_md5sig *md5sig)
{
  const struct tcp_md5_key *key;
  struct sockaddr_in *sin;
  struct sockaddr_in6 *sin6;

  if (pcb == NULL || md5sig == NULL)
    return ERR_ARG;

  key = (const struct tcp_md5_key *)
      tcp_ext_arg_get(pcb, tcp_md5_ext_arg_id);

  if (key == NULL)
    return ERR_ARG;

  memset(md5sig, 0, sizeof(*md5sig));

  /* Encode peer address. */
  if (IP_IS_V6_VAL(key->peer_addr)) {
#if LWIP_IPV6
    sin6 = (struct sockaddr_in6 *)&md5sig->tcpm_addr;
    sin6->sin6_family = AF_INET6;
    sin6->sin6_len = sizeof(*sin6);
    memcpy(sin6->sin6_addr.s6_addr, ip_2_ip6(&key->peer_addr)->addr, 16);
#endif
  } else {
    sin = (struct sockaddr_in *)&md5sig->tcpm_addr;
    sin->sin_family = AF_INET;
    sin->sin_len = sizeof(*sin);
    sin->sin_addr.s_addr = ip_2_ip4(&key->peer_addr)->addr;
  }

  SMEMCPY(md5sig->tcpm_key, key->key, key->keylen);
  md5sig->tcpm_keylen = key->keylen;

  return ERR_OK;
}

void
lwip_tcp_md5_clear_key(struct tcp_pcb *pcb)
{
  struct tcp_md5_key *key;

  if (pcb == NULL)
    return;

  key = (struct tcp_md5_key *)tcp_ext_arg_get(pcb, tcp_md5_ext_arg_id);
  if (key != NULL) {
    tcp_ext_arg_set(pcb, tcp_md5_ext_arg_id, NULL);
    memset(key, 0, sizeof(*key));
    mem_free(key);
  }
}

int
lwip_tcp_md5_has_key(const struct tcp_pcb *pcb)
{
  if (pcb == NULL)
    return 0;

  return (tcp_ext_arg_get(pcb, tcp_md5_ext_arg_id) != NULL) ? 1 : 0;
}

/* ------------------------------------------------------------------ */
/*  MD5 digest computation for TCP segment                             */
/* ------------------------------------------------------------------ */

/*
 * Compute the TCP-MD5 digest for a segment according to RFC 2385.
 *
 * The digest covers:
 *   TCP pseudo-header: IP src, IP dst, protocol (0x06=TCP), TCP length
 *   TCP header (except options), with checksum set to zero
 *   TCP segment data
 *   shared secret (key)
 *
 * Parameters:
 *   pcb     - the TCP PCB (for the shared secret)
 *   hdr     - pointer to the TCP header
 *   p       - the pbuf chain (points to data after TCP header)
 *   digest  - output buffer (16 bytes)
 */
static void
tcp_md5_compute_digest(const struct tcp_pcb *pcb, struct tcp_hdr *hdr,
    struct pbuf *p, uint8_t digest[16])
{
  struct lwip_md5_ctx ctx;
  struct tcp_md5_key *key;
  uint16_t tcp_len;
  uint8_t phdr[12];  /* IPv4 pseudo-header */
#if LWIP_IPV6
  uint8_t phdr6[40]; /* IPv6 pseudo-header */
#endif

  key = (struct tcp_md5_key *)tcp_ext_arg_get(pcb, tcp_md5_ext_arg_id);
  if (key == NULL) {
    memset(digest, 0, 16);
    return;
  }

  lwip_md5_init(&ctx);

  /* TCP segment length = total pbuf length */
  tcp_len = p->tot_len;

  if (IP_IS_V6_VAL(pcb->local_ip)) {
#if LWIP_IPV6
    /*
     * IPv6 pseudo-header: src (16) + dst (16) + upper-layer length (4) +
     * zero (3) + next header (1)
     */
    memcpy(phdr6, ip_2_ip6(&pcb->local_ip)->addr, 16);
    memcpy(phdr6 + 16, ip_2_ip6(&pcb->remote_ip)->addr, 16);
    phdr6[32] = (uint8_t)(tcp_len >> 24);
    phdr6[33] = (uint8_t)(tcp_len >> 16);
    phdr6[34] = (uint8_t)(tcp_len >> 8);
    phdr6[35] = (uint8_t)(tcp_len);
    phdr6[36] = 0;
    phdr6[37] = 0;
    phdr6[38] = 0;
    phdr6[39] = IP_PROTO_TCP;

    lwip_md5_update(&ctx, phdr6, 40);
#endif /* LWIP_IPV6 */
  } else {
    /* IPv4 pseudo-header: src (4) + dst (4) + zeros (1) + protocol (1) + length (2) */
    phdr[0] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->local_ip)) >> 24);
    phdr[1] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->local_ip)) >> 16);
    phdr[2] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->local_ip)) >> 8);
    phdr[3] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->local_ip)));
    phdr[4] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->remote_ip)) >> 24);
    phdr[5] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->remote_ip)) >> 16);
    phdr[6] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->remote_ip)) >> 8);
    phdr[7] = (uint8_t)(ip4_addr_get_u32(ip_2_ip4(&pcb->remote_ip)));
    phdr[8] = 0;
    phdr[9] = IP_PROTO_TCP;
    phdr[10] = (uint8_t)(tcp_len >> 8);
    phdr[11] = (uint8_t)(tcp_len);

    lwip_md5_update(&ctx, phdr, 12);
  }

  /*
   * TCP header (excluding options), with checksum set to zero.
   * We need to hash from the start of the TCP header up to the options.
   * The tcp_hdr struct is exactly TCP_HLEN bytes (20 bytes without options).
   * We save and clear the checksum, then restore it after the hash update.
   */
  {
    uint16_t saved_cksum;

    saved_cksum = hdr->chksum;
    hdr->chksum = 0;

    lwip_md5_update(&ctx, hdr, TCP_HLEN);

    hdr->chksum = saved_cksum;
  }

  /*
   * Hash the segment data.  Data starts right after the TCP header at
   * the current position in 'p'.  Note: for outgoing segments, p->payload
   * points to the beginning of the pbuf chain after the TCP header.
   * For incoming segments, p points to the data portion.
   */
  {
    const struct pbuf *q;
    const uint8_t *payload;
    u16_t offset;
    u16_t chunk;

    offset = 0;
    q = p;
    while (q != NULL) {
      /* Determine what portion of this pbuf contains actual data. */
      if (offset < q->len) {
        payload = (const uint8_t *)q->payload;
        chunk = q->len - offset;
        lwip_md5_update(&ctx, payload + offset, chunk);
        offset = 0;
      } else {
        offset -= q->len;
      }
      q = q->next;
    }
  }

  /* Hash the shared secret (key). */
  lwip_md5_update(&ctx, key->key, key->keylen);

  /* Finalise and store the digest. */
  lwip_md5_final(&ctx, digest);
}

/* ------------------------------------------------------------------ */
/*  Hook: reserve space for MD5 option in outgoing segments            */
/* ------------------------------------------------------------------ */

u8_t
lwip_tcp_md5_out_tcpopt_length(const struct tcp_pcb *pcb,
    u8_t internal_option_length)
{
  if (pcb != NULL && lwip_tcp_md5_enabled && lwip_tcp_md5_has_key(pcb))
    return (u8_t)(internal_option_length + TCP_MD5SIG_OPT_LEN);

  return internal_option_length;
}

/* ------------------------------------------------------------------ */
/*  Hook: write MD5 option into outgoing segment                       */
/* ------------------------------------------------------------------ */

u32_t *
lwip_tcp_md5_add_tcpopts(struct pbuf *p, struct tcp_hdr *hdr,
    const struct tcp_pcb *pcb, u32_t *opts)
{
  uint8_t digest[16];
  uint8_t *optptr;

  if (pcb == NULL || !lwip_tcp_md5_enabled || !lwip_tcp_md5_has_key(pcb))
    return opts;

  /* Write the MD5 option: Kind=19, Length=18, 16-byte digest. */
  optptr = (uint8_t *)opts;
  optptr[0] = TCP_MD5SIG_OPT_KIND;
  optptr[1] = TCP_MD5SIG_OPT_LEN;

  /* Compute the digest over the TCP segment + pseudo-header + key. */
  tcp_md5_compute_digest(pcb, hdr, p, digest);

  memcpy(&optptr[2], digest, TCP_MD5SIG_DIGEST_LEN);

  return (u32_t *)(optptr + TCP_MD5SIG_OPT_LEN);
}

/* ------------------------------------------------------------------ */
/*  Hook: validate MD5 option on incoming segments                     */
/* ------------------------------------------------------------------ */

err_t
lwip_tcp_md5_inpacket(struct tcp_pcb *pcb, struct tcp_hdr *hdr,
    u16_t optlen, u16_t opt1len, u8_t *opt2, struct pbuf *p)
{
  uint8_t pkt_digest[16];
  uint8_t exp_digest[16];
  uint8_t *optdata;
  u16_t optidx;
  u16_t opt_remaining;
  int md5_found;
  int md5_valid;
  u8_t kind, len;

  if (pcb == NULL)
    return ERR_OK;

  /* If MD5 is disabled globally, skip validation entirely. */
  if (!lwip_tcp_md5_enabled)
    return ERR_OK;

  /* If no key is configured, skip MD5 validation entirely. */
  if (!lwip_tcp_md5_has_key(pcb))
    return ERR_OK;

  /*
   * Parse TCP options to find the MD5 option (Kind=19).
   * The options may be split across two pbufs (opt1len + opt2).
   */
  md5_found = 0;
  optidx = 0;
  opt_remaining = optlen;

  while (opt_remaining > 0) {
    /* Read option kind.  May be split across two buffers. */
    if (optidx < opt1len) {
      /* Option byte is in the first part (directly after TCP header). */
      kind = *(const uint8_t *)((const uint8_t *)(hdr + 1) + optidx);
    } else if (opt2 != NULL) {
      /* Option byte is in the second part. */
      kind = opt2[optidx - opt1len];
    } else {
      break;
    }

    if (kind == 0) {
      /* End of Options List. */
      break;
    }

    if (kind == 1) {
      /* No-Operation. */
      optidx++;
      opt_remaining--;
      continue;
    }

    /* Read option length.  Must be at least 2 bytes. */
    if (opt_remaining < 2)
      break;

    if (optidx + 1 < opt1len) {
      len = *(const uint8_t *)((const uint8_t *)(hdr + 1) + optidx + 1);
    } else if (opt2 != NULL) {
      len = opt2[optidx + 1 - opt1len];
    } else {
      break;
    }

    if (len < 2 || len > opt_remaining)
      break;

    if (kind == TCP_MD5SIG_OPT_KIND) {
      if (len == TCP_MD5SIG_OPT_LEN) {
        /*
         * Found the MD5 option.  Extract the 16-byte digest.
         * We don't break here because we want to know if there's
         * at most one MD5 option (RFC requires exactly one).
         */
        int j;

        if (md5_found) {
          LWIP_DEBUGF(TCP_INPUT_DEBUG,
              ("tcp_md5_inpacket: multiple MD5 options, dropping\n"));
          return ERR_VAL;
        }

        /* Extract the digest from the option. */
        if (optidx + 2 + TCP_MD5SIG_DIGEST_LEN <= opt1len) {
          /* All in first part. */
          optdata = (uint8_t *)(hdr + 1) + optidx + 2;
        } else {
          /* May span both parts.  Do a safe copy. */
          for (j = 0; j < TCP_MD5SIG_DIGEST_LEN; j++) {
            if (optidx + 2 + j < opt1len)
              pkt_digest[j] = *((const uint8_t *)(hdr + 1) + optidx + 2 + j);
            else
              pkt_digest[j] = opt2[optidx + 2 + j - opt1len];
          }
          optdata = pkt_digest;
        }

        if (optdata != pkt_digest)
          memcpy(pkt_digest, optdata, TCP_MD5SIG_DIGEST_LEN);

        md5_found = 1;
      } else {
        LWIP_DEBUGF(TCP_INPUT_DEBUG,
            ("tcp_md5_inpacket: bad MD5 option length %d\n", len));
        return ERR_VAL;
      }
    }

    optidx += len;
    opt_remaining -= len;
  }

  if (!md5_found) {
    /*
     * RFC 2385: if MD5 option is configured but not present on an
     * incoming segment, the segment MUST be dropped.
     */
    LWIP_DEBUGF(TCP_INPUT_DEBUG,
        ("tcp_md5_inpacket: missing MD5 option on configured PCB\n"));
    return ERR_VAL;
  }

  /*
   * Compute expected digest.
   * We have the TCP header in hdr and the data in p.
   * The MD5 option in the TCP header must have its digest zeroed out
   * before computing the expected digest (since the option is part
   * of the TCP header but the digest field itself is not hashed).
   */
  {
    uint8_t *opt_in_hdr;
    uint8_t saved[TCP_MD5SIG_DIGEST_LEN];
    int j;

    /*
     * The MD5 option should be somewhere in the TCP header options area.
     * We need to zero out the digest bytes before computing our digest.
     * Find the MD5 option in the header again.
     */
    optidx = 0;
    opt_remaining = optlen;
    while (opt_remaining > 0) {
      if (optidx < opt1len)
        kind = *(const uint8_t *)((const uint8_t *)(hdr + 1) + optidx);
      else if (opt2 != NULL)
        kind = opt2[optidx - opt1len];
      else
        break;

      if (kind == 0)
        break;
      if (kind == 1) {
        optidx++;
        opt_remaining--;
        continue;
      }
      if (opt_remaining < 2)
        break;

      if (optidx + 1 < opt1len)
        len = *(const uint8_t *)((const uint8_t *)(hdr + 1) + optidx + 1);
      else if (opt2 != NULL)
        len = opt2[optidx + 1 - opt1len];
      else
        break;

      if (len < 2 || len > opt_remaining)
        break;

      if (kind == TCP_MD5SIG_OPT_KIND && len == TCP_MD5SIG_OPT_LEN) {
        /* Found it.  Save and zero the digest bytes in the header. */
        if (optidx + 2 + TCP_MD5SIG_DIGEST_LEN <= opt1len) {
          opt_in_hdr = (uint8_t *)(hdr + 1) + optidx + 2;
          memcpy(saved, opt_in_hdr, TCP_MD5SIG_DIGEST_LEN);
          memset(opt_in_hdr, 0, TCP_MD5SIG_DIGEST_LEN);
        } else {
          for (j = 0; j < TCP_MD5SIG_DIGEST_LEN; j++) {
            if (optidx + 2 + j < opt1len)
              saved[j] = *((uint8_t *)(hdr + 1) + optidx + 2 + j);
            else
              saved[j] = opt2[optidx + 2 + j - opt1len];
          }
          /* Zero out in both buffers. */
          for (j = 0; j < TCP_MD5SIG_DIGEST_LEN; j++) {
            if (optidx + 2 + j < opt1len)
              *((uint8_t *)(hdr + 1) + optidx + 2 + j) = 0;
            else
              opt2[optidx + 2 + j - opt1len] = 0;
          }
        }

        tcp_md5_compute_digest(pcb, hdr, p, exp_digest);

        /* Restore the digest in the header. */
        if (optidx + 2 + TCP_MD5SIG_DIGEST_LEN <= opt1len) {
          opt_in_hdr = (uint8_t *)(hdr + 1) + optidx + 2;
          memcpy(opt_in_hdr, saved, TCP_MD5SIG_DIGEST_LEN);
        } else {
          for (j = 0; j < TCP_MD5SIG_DIGEST_LEN; j++) {
            if (optidx + 2 + j < opt1len)
              *((uint8_t *)(hdr + 1) + optidx + 2 + j) = saved[j];
            else
              opt2[optidx + 2 + j - opt1len] = saved[j];
          }
        }

        break;
      }

      optidx += len;
      opt_remaining -= len;
    }
  }

  md5_valid = (memcmp(pkt_digest, exp_digest, TCP_MD5SIG_DIGEST_LEN) == 0);

  if (!md5_valid) {
    LWIP_DEBUGF(TCP_INPUT_DEBUG,
        ("tcp_md5_inpacket: MD5 digest mismatch, dropping segment\n"));
    return ERR_VAL;
  }

  return ERR_OK;
}

#endif /* LWIP_TCP_MD5SIG */
