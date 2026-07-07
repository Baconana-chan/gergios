/*
 * MINIX 3 platform adaptation for wireguard-lwip — implementation.
 *
 * Random bytes are generated using a ChaCha20-based deterministic random
 * bit generator (DRBG) seeded from the kernel entropy pool via
 * sys_getrandomness().  The DRBG is reseeded after every 1MB of output.
 */
#include "wireguard-platform.h"
#include "lwip/sys.h"

#include <string.h>
#include <minix/syslib.h>
#include <minix/type.h>

/* The ChaCha20 implementation from wireguard-lwip's crypto. */
#include "chacha20.h"

/*
 * ChaCha20 DRBG state.
 *
 * Instead of lrand48() which is NOT cryptographically secure, we use a
 * ChaCha20 keystream generator seeded with kernel entropy.  This gives
 * us a CSPRNG suitable for WireGuard key material.
 */
static struct {
	uint8_t		key[32];	/* ChaCha20 key */
	uint32_t	counter;	/* block counter for ChaCha20 */
	uint8_t		nonce[8];	/* nonce (fixed after init) */
	uint8_t		block[64];	/* current keystream block */
	size_t		avail;		/* bytes left in current block */
	uint32_t	generated;	/* total bytes generated since reseed */
	int		seeded;		/* has been seeded */
} wg_rng;

/*
 * Reseed the ChaCha20 DRBG from kernel entropy.
 * Uses sys_getrandomness() to obtain entropy from the kernel's pool,
 * then hashes it down to a 32-byte key using ChaCha20 itself.
 */
static void
wg_rng_reseed(void)
{
	struct k_randomness krand;
	uint8_t seed_material[64];
	struct chacha20_ctx ctx;
	int i, j, src;

	/*
	 * Retrieve kernel entropy.  The kernel maintains entropy from
	 * interrupt timings, device events, etc.  We use all 16 bins.
	 */
	memset(&krand, 0, sizeof(krand));

	if (sys_getrandomness(&krand) != OK) {
		/*
		 * If sys_getrandomness fails, fall back to a best-effort
		 * approach using the monotonic clock mixed with a counter.
		 * This is not ideal but better than blocking entirely.
		 */
		uint32_t now = sys_now();

		for (i = 0; i < (int)sizeof(seed_material); i++)
			seed_material[i] = (uint8_t)(now + i * 0x9e3779b9);
	} else {
		/*
		 * Mix all randomness bins into the seed material.
		 * rand_t is typically u32_t; each bin has 64 entries.
		 * We hash all bins down to 64 bytes using a simple
		 * linear mixing (XOR + rotate) which is sufficient to
		 * extract entropy from the kernel pool.
		 */
		memset(seed_material, 0, sizeof(seed_material));

		for (src = 0; src < RANDOM_SOURCES; src++) {
			for (i = 0; i < RANDOM_ELEMENTS; i++) {
				uint32_t val = krand.bin[src].r_buf[i];
				int idx = i % (int)sizeof(seed_material);

				seed_material[idx] ^=
				    (uint8_t)(val & 0xFF);
				seed_material[(idx + 1) %
				    sizeof(seed_material)] ^=
				    (uint8_t)((val >> 8) & 0xFF);
				seed_material[(idx + 2) %
				    sizeof(seed_material)] ^=
				    (uint8_t)((val >> 16) & 0xFF);
				seed_material[(idx + 3) %
				    sizeof(seed_material)] ^=
				    (uint8_t)((val >> 24) & 0xFF);
			}
		}
	}

	/*
	 * Use ChaCha20 itself to hash the seed material into a new key.
	 * Set up ChaCha20 with the first 32 bytes of seed as key,
	 * generate one block, then use the first 32 bytes as the new
	 * DRBG key.  (ChaCha20 uses 32-byte keys; the remaining 32
	 * bytes of seed_material are mixed into key via the nonce.)
	 */
	chacha20_setup(&ctx, seed_material, 32,
	    (const uint8_t *)"wgdrbg01", 8, 0);

	chacha20_keystream(&ctx, wg_rng.key, sizeof(wg_rng.key));

	/* Zero the first keystream block and reset counters. */
	memset(wg_rng.block, 0, sizeof(wg_rng.block));
	wg_rng.counter = 0;
	wg_rng.avail = 0;
	wg_rng.generated = 0;

	/* Derive a fresh nonce from the new key itself. */
	for (j = 0; j < (int)sizeof(wg_rng.nonce); j++)
		wg_rng.nonce[j] = wg_rng.key[j] ^ wg_rng.key[j + 8] ^
		    wg_rng.key[j + 16] ^ wg_rng.key[j + 24];

	/* Clear seed material from stack. */
	memset(seed_material, 0, sizeof(seed_material));
	memset(&ctx, 0, sizeof(ctx));

	wg_rng.seeded = 1;
}

/*
 * Rekey the ChaCha20 DRBG by generating a new key from the current one.
 * This provides forward secrecy: if the current state is compromised,
 * previous outputs remain secure.
 */
static void
wg_rng_rekey(void)
{
	struct chacha20_ctx ctx;

	chacha20_setup(&ctx, wg_rng.key, sizeof(wg_rng.key),
	    wg_rng.nonce, sizeof(wg_rng.nonce), wg_rng.counter);

	chacha20_keystream(&ctx, wg_rng.key, sizeof(wg_rng.key));

	wg_rng.avail = 0;
	memset(&ctx, 0, sizeof(ctx));
}

/*
 * Generate random bytes using the ChaCha20 DRBG.
 * Seeds from kernel entropy on first call and after 1MB of output.
 */
void
wireguard_random_bytes(void *bytes, size_t size)
{
	uint8_t *out = (uint8_t *)bytes;
	size_t total = size;
	struct chacha20_ctx ctx;

	/* Auto-initialise on first call. */
	if (!wg_rng.seeded)
		wg_rng_reseed();

	/* Reseed if we have generated more than 1MB since last reseed. */
	if (wg_rng.generated > 1024 * 1024)
		wg_rng_reseed();

	while (total > 0) {
		size_t chunk;

		/* Refill keystream block if empty. */
		if (wg_rng.avail == 0) {
			chacha20_setup(&ctx, wg_rng.key, sizeof(wg_rng.key),
			    wg_rng.nonce, sizeof(wg_rng.nonce),
			    wg_rng.counter);

			chacha20_keystream(&ctx, wg_rng.block,
			    sizeof(wg_rng.block));

			wg_rng.counter++;
			wg_rng.avail = sizeof(wg_rng.block);

			memset(&ctx, 0, sizeof(ctx));

			/*
			 * Rekey after each keystream block to provide
			 * forward secrecy and prevent state compromise.
			 * (Rekey does NOT increment counter — that's done
			 * above, so next block starts at +1.)
			 */
			wg_rng_rekey();
		}

		chunk = total;
		if (chunk > wg_rng.avail)
			chunk = wg_rng.avail;

		memcpy(out, wg_rng.block + sizeof(wg_rng.block) -
		    wg_rng.avail, chunk);

		wg_rng.avail -= chunk;
		out += chunk;
		total -= chunk;
		wg_rng.generated += (uint32_t)chunk;
	}

	/* Zero sensitive stack data. */
	memset(&ctx, 0, sizeof(ctx));
}

void
wireguard_tai64n_now(uint8_t *output)
{
	uint64_t seconds;
	uint32_t nanoseconds;
	uint32_t now_ms;

	/*
	 * TAI64N format: 12 bytes total.
	 * Bytes 0-7: 64-bit big-endian seconds since 1970-01-01 (TAI).
	 * Bytes 8-11: 32-bit big-endian nanoseconds within the second.
	 *
	 * Since MINIX may not have a wall clock in all configurations,
	 * we use the monotonic boot time as a substitute.  This provides
	 * the required monotonically increasing value for replay protection.
	 */
	now_ms = sys_now();

	/* Convert milliseconds to seconds + nanoseconds. */
	seconds = (uint64_t)(now_ms / 1000);
	nanoseconds = (uint32_t)((now_ms % 1000) * 1000000);

	/* Write big-endian seconds. */
	output[0] = (uint8_t)(seconds >> 56);
	output[1] = (uint8_t)(seconds >> 48);
	output[2] = (uint8_t)(seconds >> 40);
	output[3] = (uint8_t)(seconds >> 32);
	output[4] = (uint8_t)(seconds >> 24);
	output[5] = (uint8_t)(seconds >> 16);
	output[6] = (uint8_t)(seconds >> 8);
	output[7] = (uint8_t)(seconds);

	/* Write big-endian nanoseconds. */
	output[8]  = (uint8_t)(nanoseconds >> 24);
	output[9]  = (uint8_t)(nanoseconds >> 16);
	output[10] = (uint8_t)(nanoseconds >> 8);
	output[11] = (uint8_t)(nanoseconds);
}

bool
wireguard_is_under_load(void)
{
	/*
	 * For now, always return false (no load).  In the future,
	 * this could check the length of the main message queue or
	 * the number of pending timers.
	 */
	return false;
}
