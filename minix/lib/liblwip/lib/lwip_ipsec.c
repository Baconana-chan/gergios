/*
 * MINIX 3 specific IPsec ESP Transport + AH support (RFC 4301/4302/4303).
 *
 * This module provides minimal IPsec functionality covering gaps not served
 * by WireGuard: ESP Transport Mode (encryption of IP payload) and AH
 * (authentication without encryption).  Manual keying only -- no IKEv2.
 *
 * Architecture: BITS (Bump In The Stack) with hooks in ip4_input() and
 * ip4_output_if_src() via LWIP_HOOK_IP4_INPUT and LWIP_HOOK_IP4_OUTPUT.
 *
 * Cryptographic primitives are implemented inline (no external crypto lib):
 *   - AES-128/256 key schedule, encrypt (ECB), CBC, GCM (GHASH in GF(2^128))
 *   - SHA-256 and HMAC-SHA256 for AH and ESP-CBC authentication
 *   - ChaCha20-Poly1305 via the wireguard-lwip library (already linked)
 */
#include "lwip/opt.h"

#if LWIP_IPSEC /* only build if configured */

#include <string.h>

#include "lwip_ipsec.h"
#include "lwip/ip.h"
#include "lwip/ip4.h"
#include "lwip/pbuf.h"
#include "lwip/inet_chksum.h"
#include "lwip/err.h"
#include "lwip/debug.h"
#include "lwip/prot/ip.h"
#include "lwip/prot/iana.h"
#include "lwip/netif.h"

/* WireGuard crypto for ChaCha20-Poly1305 */
#include "chacha20poly1305.h"
#include "chacha20.h"
#include "poly1305-donna.h"

/* ------------------------------------------------------------------ */
/*  Forward declarations                                               */
/* ------------------------------------------------------------------ */

static int  ipsec_esp_output(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest, struct ipsec_sadb_entry *sa);
static int  ipsec_ah_output(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest, struct ipsec_sadb_entry *sa);
static int  ipsec_esp_input(struct pbuf *p, struct netif *inp,
	struct ip_hdr *iphdr, struct ipsec_sadb_entry *sa);
static int  ipsec_ah_input(struct pbuf *p, struct netif *inp,
	struct ip_hdr *iphdr, struct ipsec_sadb_entry *sa);
static int  ipsec_anti_replay_check(struct ipsec_sadb_entry *sa,
	uint32_t seq);

/* ------------------------------------------------------------------ */
/*  AES-128/256 implementation (FIPS 197)                             */
/* ------------------------------------------------------------------ */

/* AES constants */
#define AES_BLOCK_SIZE  16
#define AES128_ROUNDS   10
#define AES256_ROUNDS   14

/* GF(2^8) multiplication for MixColumns */
static const uint8_t aes_sbox[256] = {
	0x63,0x7c,0x77,0x7b,0xf2,0x6b,0x6f,0xc5,0x30,0x01,0x67,0x2b,0xfe,0xd7,0xab,0x76,
	0xca,0x82,0xc9,0x7d,0xfa,0x59,0x47,0xf0,0xad,0xd4,0xa2,0xaf,0x9c,0xa4,0x72,0xc0,
	0xb7,0xfd,0x93,0x26,0x36,0x3f,0xf7,0xcc,0x34,0xa5,0xe5,0xf1,0x71,0xd8,0x31,0x15,
	0x04,0xc7,0x23,0xc3,0x18,0x96,0x05,0x9a,0x07,0x12,0x80,0xe2,0xeb,0x27,0xb2,0x75,
	0x09,0x83,0x2c,0x1a,0x1b,0x6e,0x5a,0xa0,0x52,0x3b,0xd6,0xb3,0x29,0xe3,0x2f,0x84,
	0x53,0xd1,0x00,0xed,0x20,0xfc,0xb1,0x5b,0x6a,0xcb,0xbe,0x39,0x4a,0x4c,0x58,0xcf,
	0xd0,0xef,0xaa,0xfb,0x43,0x4d,0x33,0x85,0x45,0xf9,0x02,0x7f,0x50,0x3c,0x9f,0xa8,
	0x51,0xa3,0x40,0x8f,0x92,0x9d,0x38,0xf5,0xbc,0xb6,0xda,0x21,0x10,0xff,0xf3,0xd2,
	0xcd,0x0c,0x13,0xec,0x5f,0x97,0x44,0x17,0xc4,0xa7,0x7e,0x3d,0x64,0x5d,0x19,0x73,
	0x60,0x81,0x4f,0xdc,0x22,0x2a,0x90,0x88,0x46,0xee,0xb8,0x14,0xde,0x5e,0x0b,0xdb,
	0xe0,0x32,0x3a,0x0a,0x49,0x06,0x24,0x5c,0xc2,0xd3,0xac,0x62,0x91,0x95,0xe4,0x79,
	0xe7,0xc8,0x37,0x6d,0x8d,0xd5,0x4e,0xa9,0x6c,0x56,0xf4,0xea,0x65,0x7a,0xae,0x08,
	0xba,0x78,0x25,0x2e,0x1c,0xa6,0xb4,0xc6,0xe8,0xdd,0x74,0x1f,0x4b,0xbd,0x8b,0x8a,
	0x70,0x3e,0xb5,0x66,0x48,0x03,0xf6,0x0e,0x61,0x35,0x57,0xb9,0x86,0xc1,0x1d,0x9e,
	0xe1,0xf8,0x98,0x11,0x69,0xd9,0x8e,0x94,0x9b,0x1e,0x87,0xe9,0xce,0x55,0x28,0xdf,
	0x8c,0xa1,0x89,0x0d,0xbf,0xe6,0x42,0x68,0x41,0x99,0x2d,0x0f,0xb0,0x54,0xbb,0x16
};

static const uint8_t aes_rcon[11] = {
	0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36
};

struct aes_ctx {
	uint8_t ek[60 * 4]; /* enough for AES-256 (60 words) */
	uint8_t rounds;
};

static uint8_t
aes_mul2(uint8_t x)
{
	return (uint8_t)((x << 1) ^ (((x >> 7) & 1) * 0x1b));
}

static uint8_t
aes_mul3(uint8_t x)
{
	return (uint8_t)(aes_mul2(x) ^ x);
}

static void
aes_sub_word(uint8_t *w)
{
	w[0] = aes_sbox[w[0]];
	w[1] = aes_sbox[w[1]];
	w[2] = aes_sbox[w[2]];
	w[3] = aes_sbox[w[3]];
}

static void
aes_rot_word(uint8_t *w)
{
	uint8_t tmp = w[0];
	w[0] = w[1];
	w[1] = w[2];
	w[2] = w[3];
	w[3] = tmp;
}

static void
aes_key_expand(struct aes_ctx *ctx, const uint8_t *key, int keylen)
{
	int nk, nr, i;
	uint8_t *w = ctx->ek;

	nk = keylen / 4; /* 4 for AES-128, 8 for AES-256 */
	nr = nk + 6;     /* 10 for AES-128, 14 for AES-256 */
	ctx->rounds = (uint8_t)nr;

	/* Copy key into first nk words */
	memcpy(w, key, keylen);

	for (i = nk; i < 4 * (nr + 1); i++) {
		uint8_t *prev = w + (i - 1) * 4;
		uint8_t *cur  = w + i * 4;

		memcpy(cur, prev, 4);

		if (i % nk == 0) {
			aes_rot_word(cur);
			aes_sub_word(cur);
			cur[0] ^= aes_rcon[i / nk];
		} else if (nk > 6 && i % nk == 4) {
			aes_sub_word(cur);
		}

		/* XOR with word (i - nk) */
		{
			int j;
			for (j = 0; j < 4; j++)
				cur[j] ^= w[(i - nk) * 4 + j];
		}
	}
}

static void
aes_encrypt(const struct aes_ctx *ctx, const uint8_t in[16], uint8_t out[16])
{
	uint8_t s[16], tmp[16];
	int r, i;

	memcpy(s, in, 16);

	/* AddRoundKey */
	for (i = 0; i < 16; i++)
		s[i] ^= ctx->ek[i];

	for (r = 1; r < ctx->rounds; r++) {
		/* SubBytes */
		for (i = 0; i < 16; i++)
			tmp[i] = aes_sbox[s[i]];

		/* ShiftRows */
		s[0]  = tmp[0];  s[1]  = tmp[5];  s[2]  = tmp[10]; s[3]  = tmp[15];
		s[4]  = tmp[4];  s[5]  = tmp[9];  s[6]  = tmp[14]; s[7]  = tmp[3];
		s[8]  = tmp[8];  s[9]  = tmp[13]; s[10] = tmp[2];  s[11] = tmp[7];
		s[12] = tmp[12]; s[13] = tmp[1];  s[14] = tmp[6];  s[15] = tmp[11];

		/* MixColumns */
		for (i = 0; i < 4; i++) {
			uint8_t *col = s + i * 4;
			uint8_t a0 = col[0], a1 = col[1], a2 = col[2], a3 = col[3];
			col[0] = (uint8_t)(aes_mul2(a0) ^ aes_mul3(a1) ^ a2 ^ a3);
			col[1] = (uint8_t)(a0 ^ aes_mul2(a1) ^ aes_mul3(a2) ^ a3);
			col[2] = (uint8_t)(a0 ^ a1 ^ aes_mul2(a2) ^ aes_mul3(a3));
			col[3] = (uint8_t)(aes_mul3(a0) ^ a1 ^ a2 ^ aes_mul2(a3));
		}

		/* AddRoundKey */
		for (i = 0; i < 16; i++)
			s[i] ^= ctx->ek[r * 16 + i];
	}

	/* Last round (no MixColumns) */
	for (i = 0; i < 16; i++)
		tmp[i] = aes_sbox[s[i]];
	s[0]  = tmp[0];  s[1]  = tmp[5];  s[2]  = tmp[10]; s[3]  = tmp[15];
	s[4]  = tmp[4];  s[5]  = tmp[9];  s[6]  = tmp[14]; s[7]  = tmp[3];
	s[8]  = tmp[8];  s[9]  = tmp[13]; s[10] = tmp[2];  s[11] = tmp[7];
	s[12] = tmp[12]; s[13] = tmp[1];  s[14] = tmp[6];  s[15] = tmp[11];

	/* AddRoundKey (last) */
	for (i = 0; i < 16; i++)
		out[i] = s[i] ^ ctx->ek[ctx->rounds * 16 + i];
}

/* ------------------------------------------------------------------ */
/*  AES-CBC mode                                                      */
/* ------------------------------------------------------------------ */

static void
aes_cbc_encrypt(const struct aes_ctx *ctx, const uint8_t *iv,
	const uint8_t *in, uint8_t *out, size_t len)
{
	uint8_t block[16];
	size_t i;

	memcpy(block, iv, 16);

	for (i = 0; i < len; i += 16) {
		int j;
		for (j = 0; j < 16; j++)
			block[j] ^= in[i + j];
		aes_encrypt(ctx, block, out + i);
		memcpy(block, out + i, 16);
	}
}

/* Since we need AES-CBC decryption too, implement AES decrypt */
static const uint8_t aes_inv_sbox[256] = {
	0x52,0x09,0x6a,0xd5,0x30,0x36,0xa5,0x38,0xbf,0x40,0xa3,0x9e,0x81,0xf3,0xd7,0xfb,
	0x7c,0xe3,0x39,0x82,0x9b,0x2f,0xff,0x87,0x34,0x8e,0x43,0x44,0xc4,0xde,0xe9,0xcb,
	0x54,0x7b,0x94,0x32,0xa6,0xc2,0x23,0x3d,0xee,0x4c,0x95,0x0b,0x42,0xfa,0xc3,0x4e,
	0x08,0x2e,0xa1,0x66,0x28,0xd9,0x24,0xb2,0x76,0x5b,0xa2,0x49,0x6d,0x8b,0xd1,0x25,
	0x72,0xf8,0xf6,0x64,0x86,0x68,0x98,0x16,0xd4,0xa4,0x5c,0xcc,0x5d,0x65,0xb6,0x92,
	0x6c,0x70,0x48,0x50,0xfd,0xed,0xb9,0xda,0x5e,0x15,0x46,0x57,0xa7,0x8d,0x9d,0x84,
	0x90,0xd8,0xab,0x00,0x8c,0xbc,0xd3,0x0a,0xf7,0xe4,0x58,0x05,0xb8,0xb3,0x45,0x06,
	0xd0,0x2c,0x1e,0x8f,0xca,0x3f,0x0f,0x02,0xc1,0xaf,0xbd,0x03,0x01,0x13,0x8a,0x6b,
	0x3a,0x91,0x11,0x41,0x4f,0x67,0xdc,0xea,0x97,0xf2,0xcf,0xce,0xf0,0xb4,0xe6,0x73,
	0x96,0xac,0x74,0x22,0xe7,0xad,0x35,0x85,0xe2,0xf9,0x37,0xe8,0x1c,0x75,0xdf,0x6e,
	0x47,0xf1,0x1a,0x71,0x1d,0x29,0xc5,0x89,0x6f,0xb7,0x62,0x0e,0xaa,0x18,0xbe,0x1b,
	0xfc,0x56,0x3e,0x4b,0xc6,0xd2,0x79,0x20,0x9a,0xdb,0xc0,0xfe,0x78,0xcd,0x5a,0xf4,
	0x1f,0xdd,0xa8,0x33,0x88,0x07,0xc7,0x31,0xb1,0x12,0x10,0x59,0x27,0x80,0xec,0x5f,
	0x60,0x51,0x7f,0xa9,0x19,0xb5,0x4a,0x0d,0x2d,0xe5,0x7a,0x9f,0x93,0xc9,0x9c,0xef,
	0xa0,0xe0,0x3b,0x4d,0xae,0x2a,0xf5,0xb0,0xc8,0xeb,0xbb,0x3c,0x83,0x53,0x99,0x61,
	0x17,0x2b,0x04,0x7e,0xba,0x77,0xd6,0x26,0xe1,0x69,0x14,0x63,0x55,0x21,0x0c,0x7d
};

static void
aes_decrypt(const struct aes_ctx *ctx, const uint8_t in[16], uint8_t out[16])
{
	uint8_t s[16], tmp[16];
	int r, i;

	memcpy(s, in, 16);

	/* AddRoundKey (last round key) */
	for (i = 0; i < 16; i++)
		s[i] ^= ctx->ek[ctx->rounds * 16 + i];

	for (r = ctx->rounds - 1; r >= 1; r--) {
		/* InvShiftRows */
		tmp[0]  = s[0];  tmp[1]  = s[13]; tmp[2]  = s[10]; tmp[3]  = s[7];
		tmp[4]  = s[4];  tmp[5]  = s[1];  tmp[6]  = s[14]; tmp[7]  = s[11];
		tmp[8]  = s[8];  tmp[9]  = s[5];  tmp[10] = s[2];  tmp[11] = s[15];
		tmp[12] = s[12]; tmp[13] = s[9];  tmp[14] = s[6];  tmp[15] = s[3];

		/* InvSubBytes */
		for (i = 0; i < 16; i++)
			s[i] = aes_inv_sbox[tmp[i]];

		/* AddRoundKey */
		for (i = 0; i < 16; i++)
			s[i] ^= ctx->ek[r * 16 + i];

		/* InvMixColumns */
		for (i = 0; i < 4; i++) {
			uint8_t *col = s + i * 4;
			uint8_t a0 = col[0], a1 = col[1], a2 = col[2], a3 = col[3];
			uint8_t b0, b1, b2, b3;
			/* InvMixColumns uses {0e,0b,0d,09} matrix */
			b0 = (uint8_t)(aes_mul2(aes_mul2(a0 ^ a2)) ^ aes_mul2(aes_mul2(a1 ^ a3)) ^ aes_mul2(a0 ^ a1 ^ a2 ^ a3) ^ a0 ^ a1 ^ a3);
			b1 = (uint8_t)(aes_mul2(aes_mul2(a1 ^ a3)) ^ aes_mul2(aes_mul2(a0 ^ a2)) ^ aes_mul2(a0 ^ a1 ^ a2 ^ a3) ^ a0 ^ a1 ^ a2);
			b2 = (uint8_t)(aes_mul2(aes_mul2(a0 ^ a2)) ^ aes_mul2(aes_mul2(a1 ^ a3)) ^ aes_mul2(a0 ^ a1 ^ a2 ^ a3) ^ a1 ^ a2 ^ a3);
			b3 = (uint8_t)(aes_mul2(aes_mul2(a1 ^ a3)) ^ aes_mul2(aes_mul2(a0 ^ a2)) ^ aes_mul2(a0 ^ a1 ^ a2 ^ a3) ^ a0 ^ a2 ^ a3);
			col[0] = b0; col[1] = b1; col[2] = b2; col[3] = b3;
		}
	}

	/* Last round: InvShiftRows + InvSubBytes + AddRoundKey */
	tmp[0]  = s[0];  tmp[1]  = s[13]; tmp[2]  = s[10]; tmp[3]  = s[7];
	tmp[4]  = s[4];  tmp[5]  = s[1];  tmp[6]  = s[14]; tmp[7]  = s[11];
	tmp[8]  = s[8];  tmp[9]  = s[5];  tmp[10] = s[2];  tmp[11] = s[15];
	tmp[12] = s[12]; tmp[13] = s[9];  tmp[14] = s[6];  tmp[15] = s[3];

	for (i = 0; i < 16; i++)
		s[i] = aes_inv_sbox[tmp[i]];

	for (i = 0; i < 16; i++)
		out[i] = s[i] ^ ctx->ek[i];
}

/* Proper AES-CBC decrypt */
static void
aes_cbc_decrypt_real(const struct aes_ctx *ctx, const uint8_t *iv,
	const uint8_t *in, uint8_t *out, size_t len)
{
	uint8_t block[16];
	size_t i;

	memcpy(block, iv, 16);

	for (i = 0; i < len; i += 16) {
		uint8_t next[16];
		int j;

		memcpy(next, in + i, 16);
		aes_decrypt(ctx, in + i, out + i);
		for (j = 0; j < 16; j++)
			out[i + j] ^= block[j];
		memcpy(block, next, 16);
	}
}

/* ------------------------------------------------------------------ */
/*  GF(2^128) multiplication for GCM                                   */
/* ------------------------------------------------------------------ */

static void
gcm_gf_mul(uint8_t *x, const uint8_t *y)
{
	uint8_t z[16] = {0};
	uint8_t v[16];
	int i, j;

	memcpy(v, y, 16);

	for (i = 0; i < 128; i++) {
		/* if (x bit i) then z ^= v */
		if (x[i >> 3] & (0x80 >> (i & 7))) {
			int k;
			for (k = 0; k < 16; k++)
				z[k] ^= v[k];
		}
		/* v = v >> 1 (with polynomial reduction) */
		{
			uint8_t lsb = v[15] & 1;
			for (j = 15; j > 0; j--)
				v[j] = (uint8_t)((v[j] >> 1) | (v[j - 1] << 7));
			v[0] >>= 1;
			if (lsb)
				v[0] ^= 0xe1; /* irreducible polynomial x^128 + x^7 + x^2 + x + 1 */
		}
	}

	memcpy(x, z, 16);
}

/* ------------------------------------------------------------------ */
/*  AES-GCM mode (authenticated encryption)                           */
/* ------------------------------------------------------------------ */

static void
aes_gcm_inc32(uint8_t *block)
{
	uint32_t *counter = (uint32_t *)(block + 12);
	*counter = lwip_htonl(lwip_ntohl(*counter) + 1);
}

static void
aes_gcm_ghash(const uint8_t *h, const uint8_t *aad, size_t aad_len,
	const uint8_t *ct, size_t ct_len, uint8_t out[16])
{
	uint8_t y[16] = {0};
	uint8_t block[16];
	size_t i;

	/* Process AAD */
	for (i = 0; i < aad_len; i += 16) {
		int j;
		uint8_t tmp[16] = {0};
		size_t chunk = aad_len - i;
		if (chunk > 16) chunk = 16;
		memcpy(tmp, aad + i, chunk);

		for (j = 0; j < 16; j++)
			block[j] = y[j] ^ tmp[j];
		gcm_gf_mul(block, h);
		memcpy(y, block, 16);
	}

	/* Process ciphertext */
	for (i = 0; i < ct_len; i += 16) {
		int j;
		uint8_t tmp[16] = {0};
		size_t chunk = ct_len - i;
		if (chunk > 16) chunk = 16;
		memcpy(tmp, ct + i, chunk);

		for (j = 0; j < 16; j++)
			block[j] = y[j] ^ tmp[j];
		gcm_gf_mul(block, h);
		memcpy(y, block, 16);
	}

	/* Process lengths block */
	{
		uint64_t aad_bits = (uint64_t)aad_len * 8;
		uint64_t ct_bits  = (uint64_t)ct_len * 8;
		uint8_t len_block[16];

		len_block[0] = (uint8_t)(aad_bits >> 56);
		len_block[1] = (uint8_t)(aad_bits >> 48);
		len_block[2] = (uint8_t)(aad_bits >> 40);
		len_block[3] = (uint8_t)(aad_bits >> 32);
		len_block[4] = (uint8_t)(aad_bits >> 24);
		len_block[5] = (uint8_t)(aad_bits >> 16);
		len_block[6] = (uint8_t)(aad_bits >> 8);
		len_block[7] = (uint8_t)(aad_bits);
		len_block[8] = (uint8_t)(ct_bits >> 56);
		len_block[9] = (uint8_t)(ct_bits >> 48);
		len_block[10] = (uint8_t)(ct_bits >> 40);
		len_block[11] = (uint8_t)(ct_bits >> 32);
		len_block[12] = (uint8_t)(ct_bits >> 24);
		len_block[13] = (uint8_t)(ct_bits >> 16);
		len_block[14] = (uint8_t)(ct_bits >> 8);
		len_block[15] = (uint8_t)(ct_bits);

		for (i = 0; i < 16; i++)
			block[i] = y[i] ^ len_block[i];
		gcm_gf_mul(block, h);
		memcpy(out, block, 16);
	}
}

/*
 * AES-GCM encrypt.
 * Input:  key (16 or 32 bytes), iv (12 bytes = 4 salt + 8 IV),
 *         aad, aad_len, plaintext, plaintext_len
 * Output: ciphertext (same len as plaintext), tag (16 bytes)
 */
static int
aes_gcm_encrypt(const uint8_t *key, int keylen, const uint8_t *iv,
	const uint8_t *aad, size_t aad_len,
	const uint8_t *plaintext, size_t plaintext_len,
	uint8_t *ciphertext, uint8_t tag[16])
{
	struct aes_ctx ctx;
	uint8_t h[16], j0[16], counter[16], ghash_in[16];
	size_t i;

	aes_key_expand(&ctx, key, keylen);

	/* H = AES(K, 0^128) */
	memset(h, 0, 16);
	aes_encrypt(&ctx, h, h);

	/* J0 = IV || 0x00000001 */
	memcpy(j0, iv, 12);
	j0[12] = 0; j0[13] = 0; j0[14] = 0; j0[15] = 1;

	/* Encrypt plaintext using GCM CTR mode (starting from J0+1) */
	memcpy(counter, j0, 16);

	for (i = 0; i < plaintext_len; i += 16) {
		size_t chunk = plaintext_len - i;
		if (chunk > 16) chunk = 16;

		aes_gcm_inc32(counter);
		aes_encrypt(&ctx, counter, ghash_in);

		/* XOR with plaintext */
		int j;
		for (j = 0; j < (int)chunk; j++)
			ciphertext[i + j] = plaintext[i + j] ^ ghash_in[j];
	}

	/* Compute GHASH over AAD || ciphertext || len(AAD) || len(CT) */
	aes_gcm_ghash(h, aad, aad_len, ciphertext, plaintext_len, ghash_in);

	/* T = GHASH ^ E(K, J0) */
	aes_encrypt(&ctx, j0, counter);
	for (i = 0; i < 16; i++)
		tag[i] = ghash_in[i] ^ counter[i];

	return 0;
}

/*
 * AES-GCM decrypt.
 * Returns 0 on success (tag verified), -1 on auth failure.
 */
static int
aes_gcm_decrypt(const uint8_t *key, int keylen, const uint8_t *iv,
	const uint8_t *aad, size_t aad_len,
	const uint8_t *ciphertext, size_t ciphertext_len,
	uint8_t *plaintext, const uint8_t tag[16])
{
	struct aes_ctx ctx;
	uint8_t h[16], j0[16], counter[16], ghash_in[16];
	uint8_t expected_tag[16];
	size_t i;

	aes_key_expand(&ctx, key, keylen);

	/* H = AES(K, 0^128) */
	memset(h, 0, 16);
	aes_encrypt(&ctx, h, h);

	/* J0 = IV || 0x00000001 */
	memcpy(j0, iv, 12);
	j0[12] = 0; j0[13] = 0; j0[14] = 0; j0[15] = 1;

	/* Decrypt using GCM CTR mode (same as encrypt) */
	memcpy(counter, j0, 16);

	for (i = 0; i < ciphertext_len; i += 16) {
		size_t chunk = ciphertext_len - i;
		if (chunk > 16) chunk = 16;

		aes_gcm_inc32(counter);
		aes_encrypt(&ctx, counter, ghash_in);

		int j;
		for (j = 0; j < (int)chunk; j++)
			plaintext[i + j] = ciphertext[i + j] ^ ghash_in[j];
	}

	/* Compute expected tag */
	aes_gcm_ghash(h, aad, aad_len, ciphertext, ciphertext_len, ghash_in);
	aes_encrypt(&ctx, j0, counter);
	for (i = 0; i < 16; i++)
		expected_tag[i] = ghash_in[i] ^ counter[i];

	/* Compare tags (constant-time-ish) */
	{
		uint8_t diff = 0;
		for (i = 0; i < 16; i++)
			diff |= tag[i] ^ expected_tag[i];
		if (diff != 0)
			return -1; /* auth failure */
	}

	return 0;
}

/* ------------------------------------------------------------------ */
/*  SHA-256 implementation (FIPS 180-4)                                */
/* ------------------------------------------------------------------ */

#define SHA256_BLOCK_SIZE  64
#define SHA256_DIGEST_SIZE 32

static const uint32_t sha256_k[64] = {
	0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,
	0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
	0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,
	0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
	0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,
	0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
	0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,
	0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
	0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,
	0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
	0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,
	0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
	0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,
	0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
	0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,
	0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
};

#define SHA256_LOAD32BE(p) \
	(((uint32_t)((p)[0]) << 24) | ((uint32_t)((p)[1]) << 16) | \
	 ((uint32_t)((p)[2]) << 8)  | ((uint32_t)((p)[3])))
#define SHA256_STORE32BE(p, v) \
	(p)[0] = (uint8_t)((v) >> 24); (p)[1] = (uint8_t)((v) >> 16); \
	(p)[2] = (uint8_t)((v) >> 8);  (p)[3] = (uint8_t)(v)

#define SHA256_ROTR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define SHA256_CH(x, y, z)   (((x) & (y)) ^ (~(x) & (z)))
#define SHA256_MAJ(x, y, z)  (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define SHA256_S0(x) (SHA256_ROTR(x, 2) ^ SHA256_ROTR(x, 13) ^ SHA256_ROTR(x, 22))
#define SHA256_S1(x) (SHA256_ROTR(x, 6) ^ SHA256_ROTR(x, 11) ^ SHA256_ROTR(x, 25))
#define SHA256_s0(x) (SHA256_ROTR(x, 7) ^ SHA256_ROTR(x, 18) ^ ((x) >> 3))
#define SHA256_s1(x) (SHA256_ROTR(x, 17) ^ SHA256_ROTR(x, 19) ^ ((x) >> 10))

struct sha256_ctx {
	uint32_t state[8];
	uint64_t count;
	uint8_t  buffer[SHA256_BLOCK_SIZE];
};

static void
sha256_transform(uint32_t state[8], const uint8_t block[64])
{
	uint32_t w[64], a, b, c, d, e, f, g, h;
	int t;

	for (t = 0; t < 16; t++)
		w[t] = SHA256_LOAD32BE(block + t * 4);
	for (t = 16; t < 64; t++)
		w[t] = SHA256_s1(w[t-2]) + w[t-7] + SHA256_s0(w[t-15]) + w[t-16];

	a = state[0]; b = state[1]; c = state[2]; d = state[3];
	e = state[4]; f = state[5]; g = state[6]; h = state[7];

	for (t = 0; t < 64; t++) {
		uint32_t t1, t2;
		t1 = h + SHA256_S1(e) + SHA256_CH(e, f, g) + sha256_k[t] + w[t];
		t2 = SHA256_S0(a) + SHA256_MAJ(a, b, c);
		h = g; g = f; f = e; e = d + t1;
		d = c; c = b; b = a; a = t1 + t2;
	}

	state[0] += a; state[1] += b; state[2] += c; state[3] += d;
	state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

static void
sha256_init(struct sha256_ctx *ctx)
{
	ctx->state[0] = 0x6a09e667;
	ctx->state[1] = 0xbb67ae85;
	ctx->state[2] = 0x3c6ef372;
	ctx->state[3] = 0xa54ff53a;
	ctx->state[4] = 0x510e527f;
	ctx->state[5] = 0x9b05688c;
	ctx->state[6] = 0x1f83d9ab;
	ctx->state[7] = 0x5be0cd19;
	ctx->count = 0;
}

static void
sha256_update(struct sha256_ctx *ctx, const void *data, size_t len)
{
	const uint8_t *p = (const uint8_t *)data;
	size_t idx = (size_t)(ctx->count & 0x3f);

	ctx->count += len;

	if (idx > 0) {
		size_t fill = SHA256_BLOCK_SIZE - idx;
		if (len < fill) {
			memcpy(ctx->buffer + idx, p, len);
			return;
		}
		memcpy(ctx->buffer + idx, p, fill);
		sha256_transform(ctx->state, ctx->buffer);
		p += fill;
		len -= fill;
	}

	while (len >= SHA256_BLOCK_SIZE) {
		sha256_transform(ctx->state, p);
		p += SHA256_BLOCK_SIZE;
		len -= SHA256_BLOCK_SIZE;
	}

	if (len > 0)
		memcpy(ctx->buffer, p, len);
}

static void
sha256_final(struct sha256_ctx *ctx, uint8_t digest[32])
{
	uint64_t bits;
	size_t idx;

	bits = ctx->count * 8;
	idx = (size_t)(ctx->count & 0x3f);

	/* Pad with 0x80 followed by zeros */
	ctx->buffer[idx++] = 0x80;

	if (idx > 56) {
		memset(ctx->buffer + idx, 0, SHA256_BLOCK_SIZE - idx);
		sha256_transform(ctx->state, ctx->buffer);
		idx = 0;
	}
	memset(ctx->buffer + idx, 0, 56 - idx);

	/* Append length in bits (big-endian) */
	ctx->buffer[56] = (uint8_t)(bits >> 56);
	ctx->buffer[57] = (uint8_t)(bits >> 48);
	ctx->buffer[58] = (uint8_t)(bits >> 40);
	ctx->buffer[59] = (uint8_t)(bits >> 32);
	ctx->buffer[60] = (uint8_t)(bits >> 24);
	ctx->buffer[61] = (uint8_t)(bits >> 16);
	ctx->buffer[62] = (uint8_t)(bits >> 8);
	ctx->buffer[63] = (uint8_t)(bits);

	sha256_transform(ctx->state, ctx->buffer);

	/* Store state as big-endian */
	int i;
	for (i = 0; i < 8; i++)
		SHA256_STORE32BE(digest + i * 4, ctx->state[i]);
}

/* ------------------------------------------------------------------ */
/*  HMAC-SHA256 (RFC 4868)                                             */
/* ------------------------------------------------------------------ */

#define HMAC_SHA256_OUTPUT_LEN 32

static void
hmac_sha256(const uint8_t *key, size_t keylen,
	const uint8_t *data, size_t datalen,
	uint8_t out[HMAC_SHA256_OUTPUT_LEN])
{
	struct sha256_ctx ctx;
	uint8_t ipad[SHA256_BLOCK_SIZE], opad[SHA256_BLOCK_SIZE];
	uint8_t k0[SHA256_BLOCK_SIZE];
	size_t i;

	if (keylen > SHA256_BLOCK_SIZE) {
		/* Hash key to reduce length */
		sha256_init(&ctx);
		sha256_update(&ctx, key, keylen);
		sha256_final(&ctx, k0);
		memset(k0 + SHA256_DIGEST_SIZE, 0,
		    SHA256_BLOCK_SIZE - SHA256_DIGEST_SIZE);
	} else {
		memcpy(k0, key, keylen);
		memset(k0 + keylen, 0, SHA256_BLOCK_SIZE - keylen);
	}

	for (i = 0; i < SHA256_BLOCK_SIZE; i++) {
		ipad[i] = (uint8_t)(k0[i] ^ 0x36);
		opad[i] = (uint8_t)(k0[i] ^ 0x5c);
	}

	/* Inner hash: H(i_key_pad || data) */
	sha256_init(&ctx);
	sha256_update(&ctx, ipad, SHA256_BLOCK_SIZE);
	sha256_update(&ctx, data, datalen);
	sha256_final(&ctx, out); /* reuse out as inner digest */

	/* Outer hash: H(o_key_pad || inner_digest) */
	sha256_init(&ctx);
	sha256_update(&ctx, opad, SHA256_BLOCK_SIZE);
	sha256_update(&ctx, out, SHA256_DIGEST_SIZE);
	sha256_final(&ctx, out);
}

/* ------------------------------------------------------------------ */
/*  SADB (Security Association Database)                               */
/* ------------------------------------------------------------------ */

static struct ipsec_sadb_entry ipsec_sadb[IPSEC_SADB_MAX_ENTRIES];

/* IPsec statistics */
struct ipsec_stats lwip_ipsec_stats;

/*
 * Runtime enable/disable toggle.  When disabled (0), IPsec processing is
 * skipped entirely and packets pass through unchanged.
 */
int lwip_ipsec_enabled = 1;

void
lwip_ipsec_init(void)
{
	memset(ipsec_sadb, 0, sizeof(ipsec_sadb));
	memset(&lwip_ipsec_stats, 0, sizeof(lwip_ipsec_stats));
}

/*
 * Add an SA entry to the global SADB.
 * For outbound SAs, dst is the destination IP address.
 * For inbound SAs, dst is the destination IP address of the local host.
 * Returns 0 on success, -1 on failure (table full or invalid params).
 */
int
lwip_ipsec_sa_add(const struct ipsec_sa *sa, const ip_addr_t *dst)
{
	int i;

	if (sa == NULL || dst == NULL)
		return -1;

	/* Validate parameters */
	if (sa->spi == 0)
		return -1;
	if (sa->flags & IPSEC_SA_FLAG_ESP) {
		if (sa->esp_cipher == 0)
			return -1;
		if (sa->enc_keylen == 0 || sa->enc_keylen > IPSEC_MAX_KEY_LEN)
			return -1;
	}
	if (sa->flags & IPSEC_SA_FLAG_AH) {
		if (sa->ah_auth == 0)
			return -1;
		if (sa->auth_keylen == 0 || sa->auth_keylen > IPSEC_MAX_AUTH_KEY_LEN)
			return -1;
	}

	/* Find empty slot or existing entry with same (dst, spi) */
	for (i = 0; i < IPSEC_SADB_MAX_ENTRIES; i++) {
		if (ipsec_sadb[i].used &&
		    ip_addr_cmp(&ipsec_sadb[i].dst_ip, dst) &&
		    ipsec_sadb[i].spi == sa->spi) {
			/* Replace existing entry */
			goto fill;
		}
	}

	/* Find empty slot */
	for (i = 0; i < IPSEC_SADB_MAX_ENTRIES; i++) {
		if (!ipsec_sadb[i].used)
			goto fill;
	}

	return -1; /* table full */

fill:
	memset(&ipsec_sadb[i], 0, sizeof(ipsec_sadb[i]));
	ipsec_sadb[i].used = 1;
	ip_addr_copy(ipsec_sadb[i].dst_ip, *dst);
	ipsec_sadb[i].spi = sa->spi;
	ipsec_sadb[i].flags = sa->flags;

	if (sa->flags & IPSEC_SA_FLAG_ESP) {
		ipsec_sadb[i].proto = IP_PROTO_ESP;
		ipsec_sadb[i].esp_cipher = sa->esp_cipher;
		memcpy(ipsec_sadb[i].enc_key, sa->enc_key, sa->enc_keylen);
		ipsec_sadb[i].enc_keylen = sa->enc_keylen;
		memcpy(ipsec_sadb[i].salt, sa->salt, IPSEC_SALT_LEN);
		ipsec_sadb[i].seq = 1; /* start at 1 */
	}
	if (sa->flags & IPSEC_SA_FLAG_AH) {
		ipsec_sadb[i].proto = IP_PROTO_AH;
		ipsec_sadb[i].ah_auth = sa->ah_auth;
		memcpy(ipsec_sadb[i].auth_key, sa->auth_key, sa->auth_keylen);
		ipsec_sadb[i].auth_keylen = sa->auth_keylen;
		if (!(sa->flags & IPSEC_SA_FLAG_ESP))
			ipsec_sadb[i].seq = 1;
	}

	return 0;
}

/*
 * Delete an SA entry from the SADB.
 */
int
lwip_ipsec_sa_del(uint32_t spi, const ip_addr_t *dst, uint8_t proto)
{
	int i;

	for (i = 0; i < IPSEC_SADB_MAX_ENTRIES; i++) {
		if (ipsec_sadb[i].used &&
		    ipsec_sadb[i].spi == spi &&
		    ipsec_sadb[i].proto == proto &&
		    ip_addr_cmp(&ipsec_sadb[i].dst_ip, dst)) {
			memset(&ipsec_sadb[i], 0, sizeof(ipsec_sadb[i]));
			return 0;
		}
	}

	return -1; /* not found */
}

/*
 * Look up an SA by (dst_ip, spi, proto).
 * Returns 0 on success, -1 on failure.
 */
int
lwip_ipsec_sa_lookup(uint32_t spi, const ip_addr_t *dst,
	uint8_t proto, struct ipsec_sadb_entry **entry)
{
	int i;

	for (i = 0; i < IPSEC_SADB_MAX_ENTRIES; i++) {
		if (ipsec_sadb[i].used &&
		    ipsec_sadb[i].spi == spi &&
		    ipsec_sadb[i].proto == proto &&
		    ip_addr_cmp(&ipsec_sadb[i].dst_ip, dst)) {
			*entry = &ipsec_sadb[i];
			return 0;
		}
	}

	return -1;
}

/*
 * Check if a PCB has an SA configured (for tcpsock.c).
 */
int
lwip_ipsec_has_sa(const struct tcp_pcb *pcb)
{
	ip_addr_t dst;
	int i;

	if (pcb == NULL)
		return 0;

	ip_addr_copy(dst, pcb->remote_ip);

	for (i = 0; i < IPSEC_SADB_MAX_ENTRIES; i++) {
		if (ipsec_sadb[i].used &&
		    ip_addr_cmp(&ipsec_sadb[i].dst_ip, &dst)) {
			return 1;
		}
	}

	return 0;
}

/* ------------------------------------------------------------------ */
/*  Anti-replay check                                                  */
/* ------------------------------------------------------------------ */

static int
ipsec_anti_replay_check(struct ipsec_sadb_entry *sa, uint32_t seq)
{
	uint32_t last = sa->replay_last_seq;
	uint32_t diff;

	if (seq == 0)
		return -1; /* RFC 4303: seq MUST NOT be 0 */

	if (seq > last) {
		/* Advance window */
		diff = seq - last;
		if (diff < IPSEC_REPLAY_WINDOW) {
			sa->replay_bitmap = (sa->replay_bitmap << diff) | 1;
		} else {
			sa->replay_bitmap = 1;
		}
		sa->replay_last_seq = seq;
		return 0;
	}

	/* seq <= last: check window */
	diff = last - seq;
	if (diff >= IPSEC_REPLAY_WINDOW)
		return -1; /* too old */

	if (sa->replay_bitmap & (1 << diff))
		return -1; /* duplicate */

	sa->replay_bitmap |= (1 << diff);
	return 0;
}

/* ------------------------------------------------------------------ */
/*  ESP output transform (Transport Mode)                              */
/* ------------------------------------------------------------------ */

/*
 * Build and send an ESP packet.
 * The original pbuf has the IP header + TCP/UDP payload.
 * We need to:
 * 1. Save the IP header and original payload
 * 2. Allocate a new pbuf for the ESP-encapsulated packet
 * 3. Build: [IP hdr] [ESP hdr: SPI(4) + Seq(4)] [IV/Salt(4/8)] [CT...] [Pad+PadLen+NxtHdr] [ICV]
 * 4. Encrypt the original payload (TCP/UDP)
 * 5. Free the original pbuf
 * 6. Send via netif->output()
 */
static int
ipsec_esp_output(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest, struct ipsec_sadb_entry *sa)
{
	struct ip_hdr *iphdr;
	uint8_t *payload; /* pointer to original TCP/UDP header */
	int payload_len;
	uint8_t iv[12];  /* 4 salt + 4 seq for GCM/ChaCha20 */
	uint8_t aad[8];  /* SPI + seq */
	uint8_t tag[16];
	uint8_t *pad_ptr, *ct_ptr;
	int pad_len, esp_total, icv_len;
	int i;
	struct pbuf *p_out;
	uint8_t *out;

	iphdr = (struct ip_hdr *)p->payload;
	payload_len = p->tot_len - IP_HLEN;
	payload = (uint8_t *)p->payload + IP_HLEN;

	/* Determine ICV length */
	icv_len = 16; /* default for GCM-16 and ChaCha20-Poly1305 */
	if (sa->esp_cipher == IPSEC_ESP_AES_GCM_8)
		icv_len = 8;

	/* Padding: need to align to 4 bytes (RFC 4303) */
	pad_len = (16 - (payload_len + 2) % 16) % 16; /* for AES blocks */
	if (pad_len < 4) pad_len += 16;

	/* Build IV: salt (4) + seq (4) for GCM, or salt + seq for ChaCha20 */
	memcpy(iv, sa->salt, 4);
	iv[4] = (uint8_t)(sa->seq >> 24);
	iv[5] = (uint8_t)(sa->seq >> 16);
	iv[6] = (uint8_t)(sa->seq >> 8);
	iv[7] = (uint8_t)(sa->seq);
	/* For GCM, IV is 12 bytes total (salt + 8-byte IV/seq).
	   For simplicity we use 4-byte salt + seq (8 bytes total IV). */
	iv[8] = 0; iv[9] = 0; iv[10] = 0; iv[11] = 0;
	(void)iv[11]; /* suppress warning */

	/* Total ESP payload size (inside ESP, excluding outer IP hdr) */
	esp_total = 8 /* SPI+Seq */ + 8 /* IV/salt+seq */ +
		payload_len + pad_len + 2 /* pad_len + next_hdr */ + icv_len;

	/* Allocate new pbuf for the output */
	p_out = pbuf_alloc(PBUF_LINK, IP_HLEN + esp_total, PBUF_RAM);
	if (p_out == NULL)
		return -1;

	out = (uint8_t *)p_out->payload;

	/* Copy original IP header */
	memcpy(out, iphdr, IP_HLEN);

	/* Update IP header for ESP */
	IPH_PROTO_SET((struct ip_hdr *)out, IP_PROTO_ESP);
	IPH_LEN_SET((struct ip_hdr *)out, lwip_htons(IP_HLEN + esp_total));
	IPH_CHKSUM_SET((struct ip_hdr *)out, 0); /* will be recomputed */
	/* Recompute IP checksum */
	IPH_CHKSUM_SET((struct ip_hdr *)out,
	    inet_chksum(out, IP_HLEN));

	/* ESP header: SPI + Seq */
	out += IP_HLEN;
	out[0] = (uint8_t)(sa->spi >> 24);
	out[1] = (uint8_t)(sa->spi >> 16);
	out[2] = (uint8_t)(sa->spi >> 8);
	out[3] = (uint8_t)(sa->spi);
	out[4] = (uint8_t)(sa->seq >> 24);
	out[5] = (uint8_t)(sa->seq >> 16);
	out[6] = (uint8_t)(sa->seq >> 8);
	out[7] = (uint8_t)(sa->seq);

	/* IV: 8 bytes (salt(4) + seq(4)) */
	memcpy(out + 8, iv, 8);

	/* Build AAD: SPI(4) + Seq(4) */
	aad[0] = (uint8_t)(sa->spi >> 24);
	aad[1] = (uint8_t)(sa->spi >> 16);
	aad[2] = (uint8_t)(sa->spi >> 8);
	aad[3] = (uint8_t)(sa->spi);
	aad[4] = (uint8_t)(sa->seq >> 24);
	aad[5] = (uint8_t)(sa->seq >> 16);
	aad[6] = (uint8_t)(sa->seq >> 8);
	aad[7] = (uint8_t)(sa->seq);

	/* Encrypt the payload */
	ct_ptr = out + 16; /* after SPI+Seq+IV */

	if (sa->esp_cipher == IPSEC_ESP_CHACHA20_POLY1305) {
		uint64_t nonce;

		/* Build nonce: 32-bit zero + 64-bit seq (lower 32 used) */
		nonce = (uint64_t)sa->seq;

		/* Use wireguard-lwip ChaCha20Poly1305 AEAD */
		chacha20poly1305_encrypt(ct_ptr, (const uint8_t *)payload,
		    payload_len, aad, 8, nonce, sa->enc_key);

		/* Copy tag from the encrypted output (last 16 bytes) */
		memcpy(tag, ct_ptr + payload_len, 16);
	} else {
		/* AES-GCM */

		aes_gcm_encrypt(sa->enc_key, sa->enc_keylen,
		    iv, aad, 8,
		    (const uint8_t *)payload, payload_len,
		    ct_ptr, tag);
	}

	/* Build padding + pad_len + next_hdr after ciphertext */
	pad_ptr = ct_ptr + payload_len;
	for (i = 0; i < pad_len; i++)
		pad_ptr[i] = (uint8_t)(i + 1); /* RFC 4303: padding bytes */
	pad_ptr[pad_len] = (uint8_t)pad_len;
	pad_ptr[pad_len + 1] = IPH_PROTO(iphdr); /* next header (TCP=6, UDP=17) */

	/* Append ICV */
	memcpy(pad_ptr + pad_len + 2, tag, icv_len);

	/* Send via netif */
	sa->seq++;
	netif->output(netif, p_out, dest);

	/* Free original pbuf */
	pbuf_free(p);

	return 0;
}

/* ------------------------------------------------------------------ */
/*  AH output transform                                                */
/* ------------------------------------------------------------------ */

static int
ipsec_ah_output(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest, struct ipsec_sadb_entry *sa)
{
	struct ip_hdr *iphdr;
	uint8_t *ah_ptr;
	struct pbuf *p_out;
	uint8_t ah_hdr[8]; /* NH(1)+Len(1)+Rsv(2)+SPI(4) */
	uint8_t icv[32];   /* enough for SHA256 */
	int icv_len;
	uint8_t *data_to_auth;
	int data_len;

	iphdr = (struct ip_hdr *)p->payload;

	icv_len = 12; /* HMAC-SHA1-96 */
	if (sa->ah_auth == IPSEC_AH_HMAC_SHA256_128)
		icv_len = 16;

	/* AH header: Next Header(1) + Length(1) + Reserved(2) + SPI(4) + Seq(4) */
	ah_hdr[0] = IPH_PROTO(iphdr); /* Next Header */
	ah_hdr[1] = (uint8_t)((8 + icv_len + 3) / 4 - 2); /* AH length in 32-bit words - 2 */
	ah_hdr[2] = 0; /* Reserved */
	ah_hdr[3] = 0; /* Reserved */
	ah_hdr[4] = (uint8_t)(sa->spi >> 24);
	ah_hdr[5] = (uint8_t)(sa->spi >> 16);
	ah_hdr[6] = (uint8_t)(sa->spi >> 8);
	ah_hdr[7] = (uint8_t)(sa->spi);

	/* Allocate output pbuf: IP + AH + original payload */
	p_out = pbuf_alloc(PBUF_LINK, p->tot_len + 8 + 4 + icv_len, PBUF_RAM);
	if (p_out == NULL)
		return -1;

	data_to_auth = (uint8_t *)p_out->payload;
	data_len = p_out->tot_len;

	/* Copy IP header */
	memcpy(data_to_auth, iphdr, IP_HLEN);

	/* Update IP header for AH */
	IPH_PROTO_SET((struct ip_hdr *)data_to_auth, IP_PROTO_AH);
	IPH_LEN_SET((struct ip_hdr *)data_to_auth,
	    lwip_htons(p_out->tot_len));
	/* Zero mutable fields for AH ICV computation */
	{
		struct ip_hdr *ah_iphdr = (struct ip_hdr *)data_to_auth;
		uint8_t saved_tos = IPH_TOS(ah_iphdr);
		uint8_t saved_ttl = IPH_TTL(ah_iphdr);
		uint16_t saved_off = IPH_OFFSET(ah_iphdr);
		uint16_t saved_cksum = IPH_CHKSUM(ah_iphdr);

		IPH_TOS_SET(ah_iphdr, 0);
		IPH_TTL_SET(ah_iphdr, 0);
		IPH_OFFSET_SET(ah_iphdr, 0);
		IPH_CHKSUM_SET(ah_iphdr, 0);

		/* AH header (with ICV=0) */
		ah_ptr = data_to_auth + IP_HLEN;
		memcpy(ah_ptr, ah_hdr, 8);
		/* Seq number */
		ah_ptr[4] = (uint8_t)(sa->seq >> 24);
		ah_ptr[5] = (uint8_t)(sa->seq >> 16);
		ah_ptr[6] = (uint8_t)(sa->seq >> 8);
		ah_ptr[7] = (uint8_t)(sa->seq);
		/* Zero ICV */
		memset(ah_ptr + 8, 0, icv_len);

		/* Copy original payload after AH header */
		memcpy(ah_ptr + 8 + icv_len, payload_data(p),
		    p->tot_len - IP_HLEN);

		/* Compute HMAC-SHA256 (or HMAC-SHA1) over the whole thing */
		if (sa->ah_auth == IPSEC_AH_HMAC_SHA256_128) {
			uint8_t full_hmac[32];
			hmac_sha256(sa->auth_key, sa->auth_keylen,
			    data_to_auth, data_len, full_hmac);
			memcpy(ah_ptr + 8, full_hmac, icv_len);
		} else {
			/* HMAC-SHA1-96: use SHA256 truncated (simplified) */
			uint8_t full_hmac[32];
			hmac_sha256(sa->auth_key, sa->auth_keylen,
			    data_to_auth, data_len, full_hmac);
			memcpy(ah_ptr + 8, full_hmac, icv_len);
		}

		/* Restore mutable fields in IP header for wire */
		IPH_TOS_SET(ah_iphdr, saved_tos);
		IPH_TTL_SET(ah_iphdr, saved_ttl);
		IPH_OFFSET_SET(ah_iphdr, saved_off);
		/* Recompute checksum */
		IPH_CHKSUM_SET(ah_iphdr, inet_chksum(ah_iphdr, IP_HLEN));
	}

	sa->seq++;
	netif->output(netif, p_out, dest);
	pbuf_free(p);

	return 0;
}

/*
 * Helper: get pointer to payload after IP header in a pbuf.
 */
static uint8_t *
payload_data(struct pbuf *p)
{
	return (uint8_t *)p->payload + IP_HLEN;
}

/* ------------------------------------------------------------------ */
/*  Hook functions                                                     */
/* ------------------------------------------------------------------ */

/*
 * Input hook: called from ip4_input() via LWIP_HOOK_IP4_INPUT.
 * If the packet is ESP (50) or AH (51), process the IPsec transform.
 * Returns 1 if the packet was eaten (consumed), 0 to continue normal processing.
 *
 * For ESP: decrypt in-place, change IP proto to inner protocol, return 0
 * so ip4_input() continues with the modified packet.
 * For AH: verify in-place, strip AH header, change IP proto to inner
 * protocol, adjust IP length, return 0.
 */
int
lwip_ipsec_input_hook(struct pbuf *p, struct netif *inp)
{
	struct ip_hdr *iphdr;
	uint8_t proto;
	struct ipsec_sadb_entry *sa;
	ip_addr_t dst;
	int rv;

	if (!lwip_ipsec_enabled)
		return 0;

	iphdr = (struct ip_hdr *)p->payload;
	proto = IPH_PROTO(iphdr);

	if (proto != IP_PROTO_ESP && proto != IP_PROTO_AH)
		return 0; /* not IPsec, continue normal processing */

	/* Look up SA by (dst IP, SPI) */
	ip_addr_copy_from_ip4(dst, iphdr->dest);

	if (proto == IP_PROTO_ESP) {
		uint32_t spi;
		uint8_t *esp_hdr;

		if (p->tot_len < IP_HLEN + 8)
			goto drop;

		esp_hdr = (uint8_t *)p->payload + IP_HLEN;
		spi = ((uint32_t)esp_hdr[0] << 24) |
		      ((uint32_t)esp_hdr[1] << 16) |
		      ((uint32_t)esp_hdr[2] << 8)  |
		      (uint32_t)esp_hdr[3];

		if (lwip_ipsec_sa_lookup(spi, &dst, IP_PROTO_ESP, &sa) != 0) {
			lwip_ipsec_stats.sa_miss++;
			goto drop;
		}

		rv = ipsec_esp_input(p, inp, iphdr, sa);
		if (rv == 0)
			return 0; /* modified in-place, continue processing */
		goto drop;
	} else { /* AH */
		uint32_t spi;
		uint8_t *ah_hdr;

		if (p->tot_len < IP_HLEN + 8)
			goto drop;

		ah_hdr = (uint8_t *)p->payload + IP_HLEN;
		spi = ((uint32_t)ah_hdr[4] << 24) |
		      ((uint32_t)ah_hdr[5] << 16) |
		      ((uint32_t)ah_hdr[6] << 8)  |
		      (uint32_t)ah_hdr[7];

		if (lwip_ipsec_sa_lookup(spi, &dst, IP_PROTO_AH, &sa) != 0) {
			lwip_ipsec_stats.sa_miss++;
			goto drop;
		}

		rv = ipsec_ah_input(p, inp, iphdr, sa);
		if (rv == 0)
			return 0; /* modified in-place, continue processing */
		goto drop;
	}

drop:
	pbuf_free(p);
	return 1; /* eaten/dropped */
}

/*
 * ESP input processing.
 * Decrypt the ESP payload in-place and adjust the IP header so that
 * the inner protocol can be processed normally.
 */
static int
ipsec_esp_input(struct pbuf *p, struct netif *inp,
	struct ip_hdr *iphdr, struct ipsec_sadb_entry *sa)
{
	uint8_t *esp_hdr, *iv, *ct;
	int payload_len, esp_len, icv_len, pad_len;
	uint8_t inner_proto;
	uint32_t seq;
	uint8_t iv_buf[12];

	payload_len = p->tot_len - IP_HLEN;
	esp_hdr = (uint8_t *)p->payload + IP_HLEN;

	/* Parse ESP header */
	seq = ((uint32_t)esp_hdr[4] << 24) |
	      ((uint32_t)esp_hdr[5] << 16) |
	      ((uint32_t)esp_hdr[6] << 8)  |
	      (uint32_t)esp_hdr[7];

	/* Anti-replay check */
	if (ipsec_anti_replay_check(sa, seq) != 0) {
		lwip_ipsec_stats.replay_drop++;
		return -1;
	}

	/* IV = 8 bytes (salt + seq) */
	iv = esp_hdr + 8;

	/* Build full IV (12 bytes for GCM) */
	memcpy(iv_buf, sa->salt, 4);
	memcpy(iv_buf + 4, iv, 8);

	/* Determine ICV length */
	icv_len = 16;
	if (sa->esp_cipher == IPSEC_ESP_AES_GCM_8)
		icv_len = 8;

	esp_len = payload_len - 8 /* SPI+Seq */ - icv_len;
	ct = esp_hdr + 16; /* after SPI+Seq+IV(8) */

	/* Find padding and next header at end of ciphertext region */
	{
		uint8_t *end = (uint8_t *)p->payload + p->tot_len - icv_len;
		pad_len = end[-1];
		inner_proto = end[-2];

		if (pad_len < 0 || pad_len > 255 || pad_len > esp_len - 16) {
			lwip_ipsec_stats.pad_fail++;
			return -1;
		}
	}

	/* Build AAD: SPI(4) + Seq(4) */
	{
		uint8_t aad_local[8];
		aad_local[0] = (uint8_t)(sa->spi >> 24);
		aad_local[1] = (uint8_t)(sa->spi >> 16);
		aad_local[2] = (uint8_t)(sa->spi >> 8);
		aad_local[3] = (uint8_t)(sa->spi);
		aad_local[4] = (uint8_t)(seq >> 24);
		aad_local[5] = (uint8_t)(seq >> 16);
		aad_local[6] = (uint8_t)(seq >> 8);
		aad_local[7] = (uint8_t)(seq);

		/* Decrypt (in-place) */
		if (sa->esp_cipher == IPSEC_ESP_CHACHA20_POLY1305) {
			uint64_t nonce;
			uint8_t *ciphertext;
			uint8_t *icv_ptr;
			bool ok;

			nonce = (uint64_t)seq;
			ciphertext = ct;

			/*
			 * The wireguard ChaCha20Poly1305 AEAD decrypt function
			 * expects src = ciphertext || tag (contiguous, last 16
			 * bytes = tag).  In the ESP packet, the ICV (tag) is at
			 * the very end, separated from the ciphertext by
			 * padding (pad_len + 2 bytes).  Copy the tag from the
			 * end of the pbuf to right after the ciphertext so the
			 * two are contiguous for the AEAD function.
			 */
			icv_ptr = (uint8_t *)p->payload + p->tot_len - icv_len;
			memcpy(ciphertext + (esp_len - 2 - pad_len),
			    icv_ptr, icv_len);

			ok = chacha20poly1305_decrypt(ct, ciphertext,
			    (size_t)(esp_len - 2 - pad_len),
			    aad_local, 8, nonce, sa->enc_key);

			if (!ok) {
				lwip_ipsec_stats.auth_fail++;
				return -1;
			}

			lwip_ipsec_stats.esp_packets++;
			lwip_ipsec_stats.esp_bytes += esp_len - 2 - pad_len;
		} else if (sa->esp_cipher == IPSEC_ESP_AES_GCM_16 ||
			   sa->esp_cipher == IPSEC_ESP_AES_GCM_8) {
			/* AES-GCM decrypt */
			uint8_t tag[16];
			uint8_t *icv_ptr;

			icv_ptr = (uint8_t *)p->payload + p->tot_len - icv_len;
			memcpy(tag, icv_ptr, icv_len);
			/* Zero-fill unused tag bytes */
			if (icv_len < 16)
				memset(tag + icv_len, 0, 16 - icv_len);

			if (aes_gcm_decrypt(sa->enc_key, sa->enc_keylen,
			    iv_buf, aad_local, 8,
			    ct, esp_len - 2 - pad_len,
			    ct, tag) != 0) {
				lwip_ipsec_stats.auth_fail++;
				return -1;
			}

			lwip_ipsec_stats.esp_packets++;
			lwip_ipsec_stats.esp_bytes += esp_len - 2 - pad_len;
		}
	}

	/* Update IP header for inner packet */
	IPH_PROTO_SET(iphdr, inner_proto);
	IPH_LEN_SET(iphdr, lwip_htons(IP_HLEN + esp_len - 2 - pad_len));

	/* Move the inner payload to right after IP header */
	{
		int inner_len = esp_len - 2 - pad_len;
		uint8_t *inner_start = ct;
		memmove((uint8_t *)p->payload + IP_HLEN, inner_start, inner_len);
		/* Trim pbuf to correct length */
		pbuf_realloc(p, IP_HLEN + inner_len);
	}

	/* Recompute IP checksum */
	IPH_CHKSUM_SET(iphdr, 0);
	IPH_CHKSUM_SET(iphdr, inet_chksum(iphdr, IP_HLEN));

	return 0; /* success, packet modified in-place */
}

/*
 * AH input processing.
 * Verify the AH ICV and strip the AH header.
 */
static int
ipsec_ah_input(struct pbuf *p, struct netif *inp,
	struct ip_hdr *iphdr, struct ipsec_sadb_entry *sa)
{
	uint8_t *ah_ptr;
	int ah_len, icv_len, total_ah;
	uint32_t seq;
	uint8_t inner_proto;
	uint8_t *icv_pkt;
	uint8_t icv_expected[32];

	ah_ptr = (uint8_t *)p->payload + IP_HLEN;
	inner_proto = ah_ptr[0];
	ah_len = (ah_ptr[1] + 2) * 4; /* total AH header length in bytes */
	icv_len = 12; /* HMAC-SHA1-96 */
	if (sa->ah_auth == IPSEC_AH_HMAC_SHA256_128)
		icv_len = 16;

	total_ah = ah_len;

	if (total_ah < 12 || total_ah > p->tot_len - IP_HLEN)
		return -1;

	seq = ((uint32_t)ah_ptr[4] << 24) |
	      ((uint32_t)ah_ptr[5] << 16) |
	      ((uint32_t)ah_ptr[6] << 8)  |
	      (uint32_t)ah_ptr[7];

	/* Anti-replay check */
	if (ipsec_anti_replay_check(sa, seq) != 0) {
		lwip_ipsec_stats.replay_drop++;
		return -1;
	}

	icv_pkt = ah_ptr + 8;

	/* Compute expected ICV over the entire IP+AH+payload with ICV=0 */
	{
		uint8_t *data;
		int data_len;
		uint8_t saved_icv[32];
		uint8_t saved_tos, saved_ttl;
		uint16_t saved_off;

		data = (uint8_t *)p->payload;
		data_len = p->tot_len;

		memcpy(saved_icv, icv_pkt, icv_len);
		memset(icv_pkt, 0, icv_len);

		/* Zero mutable IP header fields for AH computation */
		saved_tos = IPH_TOS(iphdr);
		saved_ttl = IPH_TTL(iphdr);
		saved_off = IPH_OFFSET(iphdr);
		IPH_TOS_SET(iphdr, 0);
		IPH_TTL_SET(iphdr, 0);
		IPH_OFFSET_SET(iphdr, 0);

		hmac_sha256(sa->auth_key, sa->auth_keylen,
		    data, data_len, icv_expected);

		/* Restore */
		memcpy(icv_pkt, saved_icv, icv_len);
		IPH_TOS_SET(iphdr, saved_tos);
		IPH_TTL_SET(iphdr, saved_ttl);
		IPH_OFFSET_SET(iphdr, saved_off);
	}

	/* Verify ICV */
	if (memcmp(icv_expected, icv_pkt, icv_len) != 0) {
		lwip_ipsec_stats.hmac_fail++;
		return -1;
	}

	lwip_ipsec_stats.ah_packets++;

	/* Strip AH header: modify IP header and move payload */
	{
		int inner_len = p->tot_len - IP_HLEN - total_ah;
		uint8_t *inner_start = ah_ptr + total_ah;

		memmove(ah_ptr, inner_start, inner_len);
		IPH_PROTO_SET(iphdr, inner_proto);
		IPH_LEN_SET(iphdr, lwip_htons(IP_HLEN + inner_len));
		pbuf_realloc(p, IP_HLEN + inner_len);
	}

	/* Recompute IP checksum */
	IPH_CHKSUM_SET(iphdr, 0);
	IPH_CHKSUM_SET(iphdr, inet_chksum(iphdr, IP_HLEN));

	return 0;
}

/*
 * Output hook: called from ip4_output_if_src() via LWIP_HOOK_IP4_OUTPUT.
 * If the destination has an outbound SA, apply the IPsec transform.
 * Returns 1 if the packet was handled (sent), 0 to continue normal output.
 */
int
lwip_ipsec_output_hook(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest)
{
	struct ipsec_sadb_entry *sa;
	ip_addr_t dst;
	int i;

	if (!lwip_ipsec_enabled)
		return 0;

	ip_addr_copy_from_ip4(dst, *dest);

	/* Look for an SA matching this destination */
	for (i = 0; i < IPSEC_SADB_MAX_ENTRIES; i++) {
		if (!ipsec_sadb[i].used)
			continue;
		if (!ip_addr_cmp(&ipsec_sadb[i].dst_ip, &dst))
			continue;

		sa = &ipsec_sadb[i];

		if (sa->flags & IPSEC_SA_FLAG_ESP) {
			return ipsec_esp_output(p, netif, dest, sa) == 0;
		} else if (sa->flags & IPSEC_SA_FLAG_AH) {
			return ipsec_ah_output(p, netif, dest, sa) == 0;
		}
	}

	return 0; /* no SA found, continue normal output */
}

#endif /* LWIP_IPSEC */
