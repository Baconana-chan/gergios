/* pcapng.c — PCAP Next Generation (pcapng) writer implementation */

#include "pcapng.h"

#include <string.h>
#include <errno.h>

/*
 * Pad a value up to the next 4-byte boundary.
 */
static inline uint32_t
pcapng_pad32(uint32_t len)
{

	return (len + 3) & ~(uint32_t)3;
}

/*
 * Write raw bytes to the file, checking for errors.
 */
static int
pcapng_write(FILE *fp, const void *buf, size_t len)
{

	if (fwrite(buf, 1, len, fp) != len)
		return -1;

	return 0;
}

/*
 * Write a 32-bit value in host byte order (native).
 */
static int
pcapng_write_u32(FILE *fp, uint32_t val)
{
	uint32_t v = val;

	return pcapng_write(fp, &v, sizeof(v));
}

/*
 * Write a 16-bit value in host byte order.
 */
static int
pcapng_write_u16(FILE *fp, uint16_t val)
{
	uint16_t v = val;

	return pcapng_write(fp, &v, sizeof(v));
}

/*
 * Write a 64-bit value in host byte order.
 */
static int
pcapng_write_u64(FILE *fp, uint64_t val)
{
	uint64_t v = val;

	return pcapng_write(fp, &v, sizeof(v));
}

/*
 * Write a null-terminated string as a padded option value.
 * Returns the padded length (without the null terminator in padding).
 */
static int
pcapng_write_opt_string(FILE *fp, uint16_t code, const char *str)
{
	uint16_t len;
	int pad;

	len = (uint16_t)strlen(str);
	pad = (4 - (len % 4)) % 4;

	if (pcapng_write_u16(fp, code) != 0 ||
	    pcapng_write_u16(fp, len) != 0)
		return -1;

	if (len > 0 && pcapng_write(fp, str, len) != 0)
		return -1;

	/* Write padding bytes (zeros). */
	if (pad > 0) {
		static const uint8_t zero[3] = { 0, 0, 0 };

		if (pcapng_write(fp, zero, pad) != 0)
			return -1;
	}

	return 0;
}

/*
 * Write an end-of-options marker (opt_code=0, opt_len=0, no value).
 */
static int
pcapng_write_opt_end(FILE *fp)
{

	return pcapng_write_u32(fp, 0);  /* code=0, len=0 as one u32 */
}

/*
 * Write a block total length (duplicate the opening length).
 */
static int
pcapng_write_totlen(FILE *fp, uint32_t totlen)
{

	return pcapng_write_u32(fp, totlen);
}

/*
 * Write a Section Header Block.
 */
int
pcapng_write_shb(FILE *fp, const char *os, const char *app)
{
	uint32_t totlen;
	int r;

	/*
	 * Calculate total block size:
	 *   4 (type) + 4 (totlen) + 4 (magic) + 2 (major) + 2 (minor)
	 *   + 8 (section_len) + opt(os) + opt(app) + opt_end + 4 (totlen dup)
	 *
	 *   SHB fixed part: 24 bytes
	 *   Options: os string + app string + end-of-opt (4)
	 *   Trailing totlen: 4 bytes
	 */
	uint32_t fixed = 24;		/* type + totlen + magic + ver + section_len */
	uint32_t os_len = os ? (uint32_t)strlen(os) : 0;
	uint32_t app_len = app ? (uint32_t)strlen(app) : 0;
	uint32_t opt_size = 4;		/* end-of-opt */

	if (os_len > 0)
		opt_size += 4 + pcapng_pad32(os_len);	/* code(2)+len(2)+value(padded) */
	if (app_len > 0)
		opt_size += 4 + pcapng_pad32(app_len);

	totlen = fixed + opt_size + 4;	/* + trailing totlen */

	/* Write block type and total length. */
	r = pcapng_write_u32(fp, PCAPNG_SHB);
	if (r != 0) return r;
	r = pcapng_write_u32(fp, totlen);
	if (r != 0) return r;

	/* Write mandatory fields. */
	r = pcapng_write_u32(fp, PCAPNG_BYTE_ORDER_MAGIC);
	if (r != 0) return r;
	r = pcapng_write_u16(fp, PCAPNG_MAJOR_VERSION);
	if (r != 0) return r;
	r = pcapng_write_u16(fp, PCAPNG_MINOR_VERSION);
	if (r != 0) return r;
	r = pcapng_write_u64(fp, (uint64_t)-1);	/* section length = unspecified */
	if (r != 0) return r;

	/* Write options. */
	if (os_len > 0) {
		r = pcapng_write_opt_string(fp, PCAPNG_SHB_OS, os);
		if (r != 0) return r;
	}
	if (app_len > 0) {
		r = pcapng_write_opt_string(fp, PCAPNG_SHB_USERAPPL, app);
		if (r != 0) return r;
	}
	r = pcapng_write_opt_end(fp);
	if (r != 0) return r;

	/* Write trailing total length. */
	return pcapng_write_totlen(fp, totlen);
}

/*
 * Write an Interface Description Block.
 */
int
pcapng_write_idb(FILE *fp, uint16_t linktype, const char *ifname,
    uint32_t mtu, uint32_t snaplen)
{
	uint32_t totlen;
	int r;

	/*
	 * Fixed part: 16 bytes (type + totlen + linktype + reserved + snaplen)
	 * Options: if_name + tsresol + end-of-opt
	 * Trailing totlen: 4 bytes
	 */
	uint32_t fixed = 16;
	uint32_t name_len = ifname ? (uint32_t)strlen(ifname) : 0;
	uint32_t opt_size = 4 + 4 + 4 + 4;	/* name(4+pad) + name code(4) + tsresol(4+4) + end(4) */

	if (name_len > 0)
		opt_size += pcapng_pad32(name_len);

	totlen = fixed + opt_size + 4;

	r = pcapng_write_u32(fp, PCAPNG_IDB);
	if (r != 0) return r;
	r = pcapng_write_u32(fp, totlen);
	if (r != 0) return r;
	r = pcapng_write_u16(fp, linktype);
	if (r != 0) return r;
	r = pcapng_write_u16(fp, 0);		/* reserved */
	if (r != 0) return r;
	r = pcapng_write_u32(fp, snaplen);
	if (r != 0) return r;

	/* Write if_name option. */
	if (name_len > 0) {
		r = pcapng_write_opt_string(fp, PCAPNG_IDB_NAME, ifname);
		if (r != 0) return r;
	}

	/* Write if_tsresol option: 1 byte value = 6 (microsecond resolution). */
	{
		uint8_t tsresol = PCAPNG_TSRESOL_MICRO;
		uint16_t opt_code = PCAPNG_IDB_TSRESOL;
		uint16_t opt_len = 1;

		r = pcapng_write_u16(fp, opt_code);
		if (r != 0) return r;
		r = pcapng_write_u16(fp, opt_len);
		if (r != 0) return r;
		r = pcapng_write(fp, &tsresol, 1);
		if (r != 0) return r;
		/* Pad to 4 bytes (3 bytes of zeros after the 1-byte value). */
		{
			static const uint8_t pad3[3] = { 0, 0, 0 };
			r = pcapng_write(fp, pad3, 3);
			if (r != 0) return r;
		}
	}

	r = pcapng_write_opt_end(fp);
	if (r != 0) return r;

	return pcapng_write_totlen(fp, totlen);
}

/*
 * Write an Enhanced Packet Block.
 */
int
pcapng_write_epb(FILE *fp, uint32_t iface_id, uint32_t ts_sec,
    uint32_t ts_usec, const void *data, uint32_t caplen, uint32_t origlen)
{
	/*
	 * EPB fixed header: type(4) + totlen(4) + iface_id(4) + ts_high(4)
	 * + ts_low(4) + caplen(4) + origlen(4) = 28 bytes.
	 * Trailing totlen adds another 4 bytes.
	 */
	uint32_t fixed = 28;
	uint32_t padded = pcapng_pad32(caplen);
	uint32_t totlen;
	int r;

	totlen = fixed + padded + 4;		/* + trailing totlen */

	r = pcapng_write_u32(fp, PCAPNG_EPB);
	if (r != 0) return r;
	r = pcapng_write_u32(fp, totlen);
	if (r != 0) return r;
	r = pcapng_write_u32(fp, iface_id);
	if (r != 0) return r;
	r = pcapng_write_u32(fp, ts_sec);	/* timestamp high = seconds */
	if (r != 0) return r;
	r = pcapng_write_u32(fp, ts_usec);	/* timestamp low = microseconds */
	if (r != 0) return r;
	r = pcapng_write_u32(fp, caplen);
	if (r != 0) return r;
	r = pcapng_write_u32(fp, origlen);
	if (r != 0) return r;

	/* Write packet data. */
	if (caplen > 0) {
		r = pcapng_write(fp, data, caplen);
		if (r != 0) return r;
	}

	/* Write padding bytes (zeros). */
	{
		uint32_t pad = padded - caplen;
		if (pad > 0) {
			static const uint8_t zero[3] = { 0, 0, 0 };
			r = pcapng_write(fp, zero, pad);
			if (r != 0) return r;
		}
	}

	/* No options for EPB (just end-of-opt would add 4 bytes).
	 * We skip options entirely for minimal blocks. */

	return pcapng_write_totlen(fp, totlen);
}
