/* ============================================================
 * stubs.c — ARM64 kernel libc function stubs
 *
 * Provides minimal implementations of libc functions needed
 * by the kernel but unavailable with -nostdlib.
 *
 * memmove     — Used by utility.c (buffer management)
 * memset      — Used everywhere (zeroing memory)
 * strlcat     — Used by debug.c (safe string concatenation)
 * strlcpy     — Used by main.c, memory.c (safe string copy)
 * strncmp     — Used by board.h (board identification)
 * strncpy     — Used by do_getinfo.c, do_exec.c
 * strcat      — Used by do_fork.c
 * strlen      — Used by do_fork.c, stubs.c (strlcat)
 * read_tsc_64 — Used by proc.c (scheduling accounting)
 * get_bp      — Used by stacktrace.c (backtrace)
 * atoi        — Used by clock.c, main.c (env parsing)
 * vprintf     — Used by utility.c, libsys/kprintf.c
 * ============================================================ */

#include <stddef.h>
#include <string.h>
#include <stdarg.h>
#include <minix/u64.h>
#include <minix/type.h>
#include "direct_utils.h"

/* =========================================================================
 * memmove — Copy memory, handling overlapping regions
 *
 * Standard C library function. Copies n bytes from src to dst.
 * Unlike memcpy, memmove guarantees correct behavior when
 * src and dst overlap.
 *
 * Parameters:
 *   dst    Destination buffer
 *   src    Source buffer
 *   n      Number of bytes to copy
 *
 * Returns: dst
 * ========================================================================= */

void *memmove(void *dst, const void *src, size_t n)
{
	unsigned char *d = (unsigned char *)dst;
	const unsigned char *s = (const unsigned char *)src;

	if (d < s) {
		/* Copy forward */
		while (n--) {
			*d++ = *s++;
		}
	} else if (d > s) {
		/* Copy backward */
		d += n;
		s += n;
		while (n--) {
			*--d = *--s;
		}
	}
	/* If d == s, nothing to do */

	return dst;
}

/* =========================================================================
 * strlcat — Size-bounded string concatenation
 *
 * NetBSD/BSD safe string function. Appends src to dst, writing
 * at most dstsize - strlen(dst) - 1 characters, and always
 * NUL-terminating the result (if dstsize > 0).
 *
 * Parameters:
 *   dst      Destination buffer
 *   src      Source string to append
 *   dstsize  Size of destination buffer
 *
 * Returns:   Initial length of dst + length of src
 *            (if return >= dstsize, truncation occurred)
 * ========================================================================= */

size_t strlcat(char *dst, const char *src, size_t dstsize)
{
	const char *odst = dst;
	const char *osrc = src;
	size_t n = dstsize;
	size_t dlen;

	/* Find the end of dst */
	while (n-- != 0 && *dst != '\0')
		dst++;
	dlen = dst - odst;
	n = dstsize - dlen;

	if (n == 0)
		return dlen + strlen(src);
	while (*src != '\0') {
		if (n != 1)
			*dst++ = *src;
		src++;
		n--;
	}
	*dst = '\0';
	return dlen + (src - osrc);
}

/* =========================================================================
 * memset — Fill memory with a constant byte
 *
 * Standard C library function. Sets the first n bytes of the
 * memory area pointed to by s to the byte value c.
 *
 * Returns: s
 * ========================================================================= */

void *memset(void *s, int c, size_t n)
{
	unsigned char *p = (unsigned char *)s;
	while (n--)
		*p++ = (unsigned char)c;
	return s;
}

/* =========================================================================
 * memcpy — Copy memory (non-overlapping regions)
 *
 * Standard C library function. Copies n bytes from src to dst.
 * Does NOT handle overlapping regions (use memmove for that).
 *
 * Returns: dst
 * ========================================================================= */

void *memcpy(void *dst, const void *src, size_t n)
{
	unsigned char *d = (unsigned char *)dst;
	const unsigned char *s = (const unsigned char *)src;
	while (n--)
		*d++ = *s++;
	return dst;
}

/* =========================================================================
 * strlcpy — Size-bounded string copy
 *
 * NetBSD/BSD safe string function. Copies up to dstsize-1
 * characters from src to dst, always NUL-terminating.
 *
 * Returns: length of src
 * ========================================================================= */

size_t strlcpy(char *dst, const char *src, size_t dstsize)
{
	size_t srcsize = strlen(src);

	if (dstsize > 0) {
		size_t copylen = (srcsize >= dstsize) ? dstsize - 1 : srcsize;
		memcpy(dst, src, copylen);
		dst[copylen] = '\0';
	}

	return srcsize;
}

/* =========================================================================
 * read_tsc_64 — Read 64-bit timestamp counter
 *
 * ARM64: reads CNTPCT_EL0 (Physical Count Register of the Generic Timer).
 * This is a 64-bit monotonic counter, synchronized across CPUs.
 * Used by proc.c for scheduling accounting.
 * ========================================================================= */

void read_tsc_64(u64_t *t)
{
	uint64_t val;
	__asm__ volatile("mrs %0, cntpct_el0" : "=r"(val));
	*t = val;
}

/* =========================================================================
 * get_bp — Get current frame pointer (x29)
 *
 * Returns the value of the ARM64 frame pointer register (x29).
 * Used by stacktrace.c for backtrace generation.
 * ========================================================================= */

unsigned long get_bp(void)
{
	unsigned long bp;
	__asm__ volatile("mov %0, x29" : "=r"(bp));
	return bp;
}

/* =========================================================================
 * atoi — Convert ASCII string to integer
 *
 * Standard C library function. Parses a decimal integer from a
 * null-terminated string.
 * ========================================================================= */

int atoi(const char *s)
{
	int n = 0;
	int sign = 1;

	if (*s == '-') {
		sign = -1;
		s++;
	} else if (*s == '+') {
		s++;
	}

	while (*s >= '0' && *s <= '9') {
		n = n * 10 + (*s - '0');
		s++;
	}

	return n * sign;
}

/* =========================================================================
 * strlen — Get length of null-terminated string
 *
 * Standard C library function. Returns the number of characters
 * before the NUL terminator.
 * ========================================================================= */

size_t strlen(const char *s)
{
	size_t len = 0;
	while (*s++ != '\0')
		len++;
	return len;
}

/* =========================================================================
 * vprintf — Formatted print (va_list version)
 *
 * Minimal kernel implementation. Delegates to the kernel's built-in
 * printf()-compatible output path.
 *
 * NOTE: For Phase 2, this is a basic implementation that handles
 * %%s, %%d, %%u, %%x, %%p, %%c format specifiers. Full format
 * support (width, precision, flags) is TODO for Phase 3+.
 * ========================================================================= */

int vprintf(const char *fmt, va_list ap)
{
	int count = 0;
	char buf[12];
	const char *s;
	unsigned long u;
	size_t i;

	while (*fmt) {
		if (*fmt != '%') {
			direct_print_char(*fmt);
			count++;
			fmt++;
			continue;
		}

		fmt++; /* skip '%' */

		/* Handle length modifier 'l' (long) */
		int is_long = 0;
		if (*fmt == 'l') {
			is_long = 1;
			fmt++;
		}

		switch (*fmt) {
		case 's':
			s = va_arg(ap, const char *);
			if (!s) s = "(null)";
			while (*s) {
				direct_print_char(*s++);
				count++;
			}
			break;
		case 'd':
		case 'i': {
			long val = is_long ? va_arg(ap, long) : va_arg(ap, int);
			if (val < 0) {
				direct_print_char('-');
				count++;
				val = -val;
			}
			i = 0;
			if (val == 0) {
				buf[i++] = '0';
			} else {
				while (val > 0 && i < sizeof(buf)-1) {
					buf[i++] = '0' + (int)(val % 10);
					val /= 10;
				}
			}
			while (i > 0) {
				direct_print_char(buf[--i]);
				count++;
			}
			break;
		}
		case 'u': {
			unsigned long val = is_long ?
				va_arg(ap, unsigned long) :
				va_arg(ap, unsigned int);
			i = 0;
			if (val == 0) {
				buf[i++] = '0';
			} else {
				while (val > 0 && i < sizeof(buf)-1) {
					buf[i++] = '0' + (int)(val % 10);
					val /= 10;
				}
			}
			while (i > 0) {
				direct_print_char(buf[--i]);
				count++;
			}
			break;
		}
		case 'x':
		case 'X': {
			int hex_upper = (*fmt == 'X');
			unsigned long val = is_long ?
				va_arg(ap, unsigned long) :
				va_arg(ap, unsigned int);
			int nibble_count = is_long ? 16 : 8;
			for (i = 0; i < nibble_count && i < sizeof(buf)-1; i++) {
				int shift = (nibble_count - 1 - i) * 4;
				int nibble = (int)((val >> shift) & 0xF);
				if (nibble < 10)
					buf[i] = '0' + nibble;
				else if (hex_upper)
					buf[i] = 'A' + (nibble - 10);
				else
					buf[i] = 'a' + (nibble - 10);
			}
			buf[i] = '\0';
			for (s = buf; *s; s++) {
				direct_print_char(*s);
				count++;
			}
			break;
		}
		case 'p':
			direct_print_char('0');
			direct_print_char('x');
			count += 2;
			u = va_arg(ap, unsigned long);
			for (i = 0; i < 16 && i < sizeof(buf)-1; i++) {
				int nibble = (int)((u >> (60 - i*4)) & 0xF);
				buf[i] = (nibble < 10) ?
					'0' + nibble :
					'a' + (nibble - 10);
			}
			buf[i] = '\0';
			for (s = buf; *s; s++) {
				direct_print_char(*s);
				count++;
			}
			break;
		case 'c': {
			int ch = va_arg(ap, int);
			direct_print_char(ch);
			count++;
			break;
		}
		case '%':
			direct_print_char('%');
			count++;
			break;
		case '\0':
			goto done;
		default:
			direct_print_char('%');
			direct_print_char(*fmt);
			count += 2;
			break;
		}
		fmt++;
	}
done:
	return count;
}

/* =========================================================================
 * strncmp — Compare up to n characters of two strings
 *
 * Standard C library function. Compares at most n bytes of s1 and s2.
 * Returns 0 if equal, <0 if s1 < s2, >0 if s1 > s2.
 * ========================================================================= */

int strncmp(const char *s1, const char *s2, size_t n)
{
	while (n-- > 0) {
		if (*s1 != *s2)
			return (unsigned char)*s1 - (unsigned char)*s2;
		if (*s1 == '\0')
			return 0;
		s1++;
		s2++;
	}
	return 0;
}

/* =========================================================================
 * strncpy — Copy up to n characters from src to dst
 *
 * Standard C library function. Copies at most n characters from
 * src to dst. If src is shorter than n, pads with NUL. If src is
 * longer than n, dst is NOT NUL-terminated.
 *
 * Returns: dst
 * ========================================================================= */

char *strncpy(char *dst, const char *src, size_t n)
{
	size_t i;

	for (i = 0; i < n && src[i] != '\0'; i++)
		dst[i] = src[i];
	for (; i < n; i++)
		dst[i] = '\0';
	return dst;
}

/* =========================================================================
 * strcat — Concatenate src to dst
 *
 * Standard C library function. Appends src to dst, overwriting the
 * NUL terminator of dst and adding a new one.
 *
 * Returns: dst
 * ========================================================================= */

char *strcat(char *dst, const char *src)
{
	char *d = dst;

	while (*d != '\0')
		d++;
	while ((*d++ = *src++) != '\0')
		;
	return dst;
}

/* =========================================================================
 * cpuavg_init — Initialize per-process CPU average
 *
 * Called by do_fork.c when a new process is created.
 * Zeros out the cpuavg structure.
 * ========================================================================= */

void cpuavg_init(struct cpuavg *ca)
{
	ca->ca_base = 0;
	ca->ca_run = 0;
	ca->ca_last = 0;
	ca->ca_avg = 0;
}

/* =========================================================================
 * cpuavg_increment — Account a clock tick for a process
 *
 * Called on each clock tick that is charged to a process.
 * ========================================================================= */

void cpuavg_increment(struct cpuavg *ca, clock_t now, clock_t hz)
{
	if (ca->ca_base == 0)
		ca->ca_base = now;
	ca->ca_run++;
}

/* =========================================================================
 * get_randomness — Accumulate entropy from interrupt timing
 *
 * Uses CNTPCT_EL0 (ARM Generic Timer) to add entropy to the kernel pool.
 * ========================================================================= */

void get_randomness(struct k_randomness *rand, int source)
{
	u64_t tsc;

	source %= RANDOM_SOURCES;
	if (rand->bin[source].r_size >= RANDOM_ELEMENTS)
		return;

	read_tsc_64(&tsc);
	rand->bin[source].r_buf[rand->bin[source].r_next] = (rand_t)(tsc & 0xFFFF);
	if (rand->bin[source].r_size < RANDOM_ELEMENTS)
		rand->bin[source].r_size++;
	rand->bin[source].r_next = (rand->bin[source].r_next + 1) % RANDOM_ELEMENTS;
}

/* =========================================================================
 * printf — Formatted output to console
 *
 * Wrapper around vprintf. Called by interrupt.c, proc.c, and other
 * shared kernel code for diagnostic output.
 * ========================================================================= */

int printf(const char *fmt, ...)
{
	va_list ap;
	int n;

	va_start(ap, fmt);
	n = vprintf(fmt, ap);
	va_end(ap);

	return n;
}

/* =========================================================================
 * __assert13 — Assertion failure handler
 *
 * Called when an assert() macro fires. Prints diagnostic info
 * via direct_print_char and halts. Matches the MINIX libc ABI.
 * ========================================================================= */

void __assert13(const char *file, int line, const char *func, const char *expr)
{
	const char *msg = "assertion failed: ";
	const char *p;

	/* Print "assertion failed: <expr> <file>:<line> <func>\n" */
	for (p = msg; *p; p++) direct_print_char(*p);
	for (p = expr; *p; p++) direct_print_char(*p);
	direct_print_char(' ');
	for (p = file; *p; p++) direct_print_char(*p);
	direct_print_char(':');
	{
		char ln[12];
		int i, val = line;
		for (i = 11; i >= 0; i--) {
			ln[i] = '0' + (val % 10);
			val /= 10;
			if (val == 0) {
				int j;
				for (j = i; j <= 11; j++)
					direct_print_char(ln[j]);
				break;
			}
		}
	}
	direct_print_char(' ');
	for (p = func; *p; p++) direct_print_char(*p);
	direct_print_char('\n');

	/* Halt */
	for (;;)
		__asm__ volatile("wfi");
}

/* =========================================================================
 * util_stacktrace — Print kernel stack trace
 *
 * Walks the call stack using the frame pointer (x29) chain
 * and prints return addresses via direct_print_char.
 * ========================================================================= */

void util_stacktrace(void)
{
#if USE_SYSDEBUG
	unsigned long bp, pc, hbp;

	bp = get_bp();
	while (bp) {
		pc = ((unsigned long *)bp)[1];
		hbp = ((unsigned long *)bp)[0];
		/* Print hex address */
		{
			char buf[20];
			int i;
			buf[0] = '0';
			buf[1] = 'x';
			for (i = 0; i < 16; i++) {
				int nibble = (int)((pc >> (60 - i*4)) & 0xF);
				buf[2 + i] = (nibble < 10) ? ('0' + nibble) : ('a' + nibble - 10);
			}
			buf[18] = ' ';
			buf[19] = '\0';
			for (i = 0; buf[i]; i++)
				direct_print_char(buf[i]);
		}
		if (hbp != 0 && hbp <= bp)
			break;
		bp = hbp;
	}
	direct_print_char('\n');
#endif
}
