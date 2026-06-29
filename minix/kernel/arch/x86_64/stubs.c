/* ============================================================
 * stubs.c — x86_64 kernel libc function stubs
 *
 * Provides minimal implementations of libc functions needed
 * by the kernel but unavailable with -nostdlib.
 *
 * strcmp            — Used by limine.c, pre_init.c
 * strcpy            — Used by pre_init.c
 * strlen            — Used by strlcpy
 * strlcpy           — Used by main.c, memory.c, protect.c (safe string copy)
 * strncmp           — Used by _cpufeature.c, board.h
 * strlcat           — Used by libexec (exec_general.c)
 * strncpy           — Used by pre_init.c
 * strcat            — Used by pre_init.c
 * snprintf          — Used by limine.c, utility.c (kputc)
 * read_tsc          — Used by get_randomness.c
 * read_tsc_64       — Used by arch_clock.c, proc.c (scheduling)
 * get_bp            — Used by stacktrace.c
 * memcpy            — Used by _cpufeature.c, memory.c
 * memset            — Used by memory.c, pre_init.c
 * memmove           — Used by libexec
 * atoi              — Used by memory.c (string to int)
 * vprintf           — Used by printf (indirectly through libsys)
 * _minix_ipcvecs    — Used by libexec (IPC inline functions)
 * minix_mmap_for    — Used by libexec (exec_general.c)
 * usermapped_offset — Used by memory.c (IPC user-mapped offset)
 * ============================================================ */

#include <stddef.h>
#include <sys/types.h>
#include <stdarg.h>

/*
 * Local helper: convert unsigned integer to string buffer, return length.
 */
static size_t utostr(char *buf, size_t bufsz, unsigned long val)
{
	char tmp[40];
	size_t i = 0;
	if (val == 0) {
		if (bufsz > 0) buf[0] = '0';
		return 1;
	}
	while (val > 0 && i < sizeof(tmp) - 1) {
		tmp[i++] = '0' + (val % 10);
		val /= 10;
	}
	/* Reverse into output buffer */
	while (i > 0 && bufsz > 0) {
		*buf++ = tmp[--i];
		bufsz--;
	}
	return i;
}

/* =========================================================================
 * _vsnprintf — Core formatting with va_list (internal helper)
 *
 * Simplified kernel implementation for %s, %d, %u, %x, %X, %lx, %lu, %c, %% only.
 * ========================================================================= */

static int _vsnprintf(char *buf, size_t size, const char *fmt, va_list ap)
{
	int count = 0;
	char tmp[40];
	const char *s;
	size_t i;
	unsigned long u;

	if (size == 0)
		return 0;

	while (*fmt && count < (int)(size - 1)) {
		if (*fmt != '%') {
			buf[count++] = *fmt++;
			continue;
		}
		fmt++;
		switch (*fmt) {
		case 's':
			s = va_arg(ap, const char *);
			if (!s) s = "(null)";
			while (*s && count < (int)(size - 1))
				buf[count++] = *s++;
			break;
		case 'd': {
			int val = va_arg(ap, int);
			if (val < 0) {
				if (count < (int)(size - 1))
					buf[count++] = '-';
				val = -val;
			}
			count += (int)utostr(buf + count, size - count, (unsigned long)val);
			break;
		}
		case 'u':
			count += (int)utostr(buf + count, size - count,
				va_arg(ap, unsigned int));
			break;
		case 'l': {
			/* Handle %lx, %lu */
			if (*(fmt+1) == 'u') {
				fmt++;
				count += (int)utostr(buf + count, size - count,
					va_arg(ap, unsigned long));
			} else if (*(fmt+1) == 'x' || *(fmt+1) == 'X') {
				fmt++;
				u = va_arg(ap, unsigned long);
				{
					int hex_upper = (*fmt == 'X');
					int nibble_count = 16;
					for (i = 0; i < (size_t)nibble_count && i < sizeof(tmp)-1; i++) {
						int shift = (nibble_count - 1 - (int)i) * 4;
						size_t nibble = (u >> shift) & 0xF;
						if (nibble < 10)
							tmp[i] = '0' + nibble;
						else if (hex_upper)
							tmp[i] = 'A' + (nibble - 10);
						else
							tmp[i] = 'a' + (nibble - 10);
					}
					tmp[i] = '\0';
					for (s = tmp; *s && count < (int)(size - 1); s++)
						buf[count++] = *s;
				}
			} else {
				break;
			}
			break;
		}
		case 'x':
		case 'X': {
			unsigned int val = va_arg(ap, unsigned int);
			for (i = 0; i < 8 && i < sizeof(tmp)-1; i++) {
				int nibble = (int)((val >> (28 - (int)i*4)) & 0xF);
				if (nibble < 10)
					tmp[i] = '0' + nibble;
				else
					tmp[i] = 'a' + (nibble - 10);
			}
			tmp[i] = '\0';
			for (s = tmp; *s && count < (int)(size - 1); s++)
				buf[count++] = *s;
			break;
		}
		case 'c': {
			int ch = va_arg(ap, int);
			if (count < (int)(size - 1))
				buf[count++] = ch;
			break;
		}
		case '%':
			if (count < (int)(size - 1))
				buf[count++] = '%';
			break;
		case '\0':
			goto done;
		}
		fmt++;
	}
done:
	buf[count] = '\0';
	return count;
}

/* =========================================================================
 * snprintf — Format a string with size limit
 *
 * Wrapper around _vsnprintf. Used by limine.c and utility.c (kputc).
 * ========================================================================= */

int snprintf(char *buf, size_t size, const char *fmt, ...)
{
	va_list ap;
	int n;

	va_start(ap, fmt);
	n = _vsnprintf(buf, size, fmt, ap);
	va_end(ap);

	return n;
}

/* =========================================================================
 * strcmp — Compare two strings
 *
 * Standard C library function. Returns 0 if equal, <0 if s1 < s2, >0 if s1 > s2.
 * ========================================================================= */

int strcmp(const char *s1, const char *s2)
{
	while (*s1 != '\0' && *s1 == *s2) {
		s1++;
		s2++;
	}
	return (unsigned char)*s1 - (unsigned char)*s2;
}

/* =========================================================================
 * strcpy — Copy string
 *
 * Standard C library function. Copies the null-terminated string from src to dst.
 * Returns: dst
 * ========================================================================= */

char *strcpy(char *dst, const char *src)
{
	char *d = dst;
	while ((*d++ = *src++) != '\0')
		;
	return dst;
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
 * strlcpy — Size-bounded string copy
 *
 * NetBSD/BSD safe string function. Copies up to dstsize-1
 * characters from src to dst, always NUL-terminating.
 *
 * Returns: length of src
 * ========================================================================= */

size_t strlcpy(char *dst, const char *src, size_t dstsize)
{
	size_t i;

	if (dstsize == 0)
		return 0;

	/* Copy up to dstsize-1 characters, count src length */
	for (i = 0; i < dstsize - 1 && src[i] != '\0'; i++)
		dst[i] = src[i];
	dst[i] = '\0';

	/* Continue counting src length */
	while (src[i] != '\0')
		i++;

	return i;
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
 * read_tsc — Read the timestamp counter (32-bit high/low interface)
 *
 * Used by get_randomness.c for entropy gathering. On x86_64, reads the
 * TSC via RDTSC instruction and splits into high/low 32-bit halves.
 * Uses explicit "=a"/"=d" constraints ("=A" is 64-bit RAX only on x86_64).
 * ========================================================================= */

void read_tsc(u32_t *high, u32_t *low)
{
	u32_t lo, hi;
	__asm__ volatile("rdtsc" : "=a"(lo), "=d"(hi));
	*high = hi;
	*low  = lo;
}

/* =========================================================================
 * minix_ipcvecs — IPC vector tables for user-mapped dispatch
 *
 * These are referenced by memory.c's arch_phys_map_reply() which
 * selects the appropriate IPC dispatch style (sysenter, syscall,
 * or softint) based on CPU feature flags.
 *
 * On x86_64, the usermapped_* functions are NOT defined (they are
 * i386-specific assembly in mpx.S). The x86_64 kernel uses direct
 * SYSCALL dispatch instead. These tables exist only to satisfy the
 * linker — the MKF_I386_INTEL_SYSENTER flag is never set on x86_64
 * (Intel SYSENTER is 32-bit only), and MKF_I386_AMD_SYSCALL triggers
 * the SYSCALL path which points to these null tables (the real
 * dispatch is handled by the kernel call mechanism, not via these
 * user-mapped vectors on x86_64).
 * ========================================================================= */

#include <minix/ipc.h>

struct minix_ipcvecs minix_ipcvecs_sysenter = { NULL };
struct minix_ipcvecs minix_ipcvecs_syscall = { NULL };
struct minix_ipcvecs minix_ipcvecs_softint = { NULL };

/* =========================================================================
 * usermapped_offset — Offset for user-mapped kernel data
 *
 * Used by memory.c's arch_phys_map_reply() to calculate the
 * FIXEDPTR macro offset for user-space IPC vector pointers.
 *
 * Defined as u64_t (unsigned 64-bit), matching the x86_64
 * extern declaration in memory.c:
 *   extern u64_t usermapped_offset;
 * ========================================================================= */

u64_t usermapped_offset = 0;

/* =========================================================================
 * memcpy — Copy memory region
 *
 * Standard C library function. Copies n bytes from src to dst.
 * Uses simple byte loop to avoid -fno-builtin issues with __builtin_memcpy.
 * ========================================================================= */

void *memcpy(void *dst, const void *src, size_t n)
{
	unsigned char *d = dst;
	const unsigned char *s = src;
	while (n-- > 0)
		*d++ = *s++;
	return dst;
}

/* =========================================================================
 * memmove — Copy memory region (may overlap)
 *
 * Standard C library function. Copies n bytes from src to dst,
 * handling overlapping memory correctly.
 * ========================================================================= */

void *memmove(void *dst, const void *src, size_t n)
{
	unsigned char *d = dst;
	const unsigned char *s = src;

	if (d < s) {
		while (n-- > 0)
			*d++ = *s++;
	} else {
		d += n;
		s += n;
		while (n-- > 0)
			*--d = *--s;
	}
	return dst;
}

/* =========================================================================
 * memset — Fill memory with constant byte
 *
 * Standard C library function. Sets the first n bytes of s to c.
 * ========================================================================= */

void *memset(void *s, int c, size_t n)
{
	unsigned char *p = s;
	while (n-- > 0)
		*p++ = (unsigned char)c;
	return s;
}

/* =========================================================================
 * atoi — Convert ASCII string to integer
 *
 * Standard C library function. Parses an optional sign and digits
 * to produce an int. Skips leading whitespace.
 * ========================================================================= */

int atoi(const char *s)
{
	int sign = 1, val = 0;

	while (*s == ' ' || *s == '\t' || *s == '\n')
		s++;
	if (*s == '-') {
		sign = -1;
		s++;
	} else if (*s == '+') {
		s++;
	}
	while (*s >= '0' && *s <= '9')
		val = val * 10 + (*s++ - '0');
	return sign * val;
}

/* =========================================================================
 * vprintf — Print formatted output to console
 *
 * Formats output using _vsnprintf into a stack buffer,
 * then calls kputc for each character.
 * ========================================================================= */

int vprintf(const char *fmt, va_list ap)
{
	char buf[256];
	int n;
	extern void kputc(int c);

	n = _vsnprintf(buf, sizeof(buf), fmt, ap);
	if (n > 0) {
		int i;
		for (i = 0; i < n && i < (int)sizeof(buf) - 1; i++)
			kputc(buf[i]);
	}
	return n;
}

/* =========================================================================
 * printf — Print formatted output to console
 *
 * Wrapper around vprintf. Used by utility.c and other kernel code.
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
 * and halts. Matches the MINIX libc assertion ABI.
 * ========================================================================= */

void __assert13(const char *file, int line, const char *func, const char *expr)
{
	extern void kputc(int c);
	const char *msg = "assertion failed: ";

	/* Print the assertion message character by character */
	{
		const char *p = msg;
		while (*p)
			kputc(*p++);
	}
	{
		const char *p = expr;
		while (*p)
			kputc(*p++);
	}
	kputc(' ');
	{
		const char *p = file;
		while (*p)
			kputc(*p++);
	}
	kputc(':');
	{
		/* Print line number as decimal */
		char ln[12];
		int i, val = line;
		for (i = 11; i >= 0; i--) {
			ln[i] = '0' + (val % 10);
			val /= 10;
			if (val == 0) {
				int j;
				for (j = i; j <= 11; j++)
					kputc(ln[j]);
				break;
			}
		}
	}
	kputc(' ');
	{
		const char *p = func;
		while (*p)
			kputc(*p++);
	}
	kputc('\n');

	/* Halt */
	for (;;)
		__asm__ volatile("cli; hlt");
}

/* =========================================================================
 * strlcat — Size-bounded string concatenation
 *
 * Appends src to dst, at most dstsize - strlen(dst) - 1 characters,
 * always NUL-terminating. Returns the total length that would be created.
 * ========================================================================= */

size_t strlcat(char *dst, const char *src, size_t dstsize)
{
	size_t dstlen = 0, srclen = 0;

	/* Find end of dst */
	while (dstlen < dstsize && dst[dstlen] != '\0')
		dstlen++;

	/* Count src length */
	while (src[srclen] != '\0')
		srclen++;

	if (dstlen == dstsize)
		return dstsize + srclen;

	/* Append */
	{
		size_t i;
		for (i = 0; i < srclen && dstlen + i < dstsize - 1; i++)
			dst[dstlen + i] = src[i];
		dst[dstlen + i] = '\0';
	}

	return dstlen + srclen;
}

/* =========================================================================
 * read_tsc_64 — Read 64-bit timestamp counter
 *
 * Used by arch_clock.c and proc.c for scheduling timing.
 * Reads the TSC via RDTSC and stores a 64-bit value.
 * ========================================================================= */

void read_tsc_64(u64_t *t)
{
	u32_t lo, hi;
	__asm__ volatile("rdtsc" : "=a"(lo), "=d"(hi));
	*t = ((u64_t)hi << 32) | lo;
}

/* =========================================================================
 * get_bp — Get current frame pointer
 *
 * Used by stacktrace.c for backtrace. On x86_64, returns RBP.
 * ========================================================================= */

unsigned long get_bp(void)
{
	unsigned long bp;
	__asm__ volatile("movq %%rbp, %0" : "=r"(bp));
	return bp;
}

/* =========================================================================
 * _minix_ipcvecs — Default IPC vector table
 *
 * Used by libexec (exec_general.c transitively through <minix/ipc.h>
 * inline functions). The kernel provides a null table because the
 * x86_64 user-space uses direct SYSCALL, not user-mapped vectors.
 * ========================================================================= */

struct minix_ipcvecs _minix_ipcvecs = { NULL };

/* =========================================================================
 * minix_mmap_for — Map memory for a given process
 *
 * Used by libexec (exec_general.c) for ELF segment loading.
 * On x86_64, this is a stub that returns MAP_FAILED — the actual
 * implementation lives in user-space libc. The kernel stub exists
 * only to satisfy the linker for libexec.
 * ========================================================================= */

#include <minix/vm.h>
#include <errno.h>
#include <libexec.h>

/* OK is not a POSIX errno — define it if not available. */
#ifndef OK
#define OK 0
#endif
/* minix_mmap_for stub — required by libexec */
void *minix_mmap_for(endpoint_t forwhom, void *addr, size_t len,
	int prot, int flags, int fd, off_t offset)
{
	return (void *)-1;  /* MAP_FAILED */
}

/* =========================================================================
 * util_stacktrace — Print kernel stack trace
 *
 * Walks the call stack using RBP chain and prints return addresses.
 * ========================================================================= */

void util_stacktrace(void)
{
#if USE_SYSDEBUG
	unsigned long bp, pc, hbp;
	extern void kputc(int c);

	bp = get_bp();
	while (bp) {
		pc = ((unsigned long *)bp)[1];
		hbp = ((unsigned long *)bp)[0];
		/* Print hex address using kputc */
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
				kputc(buf[i]);
		}
		if (hbp != 0 && hbp <= bp)
			break;
		bp = hbp;
	}
	kputc('\n');
#endif
}

/* =========================================================================
 * cpuavg_init — Initialize per-process CPU average
 *
 * Called when a process is forked. Zeros out the cpuavg structure.
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
 * Uses the TSC to add randomness to the kernel entropy pool.
 * ========================================================================= */

void get_randomness(struct k_randomness *rand, int source)
{
	u32_t tsc_high, tsc_low;

	source %= RANDOM_SOURCES;
	if (rand->bin[source].r_size >= RANDOM_ELEMENTS)
		return;

	read_tsc(&tsc_high, &tsc_low);
	rand->bin[source].r_buf[rand->bin[source].r_next] = (rand_t)tsc_low;
	if (rand->bin[source].r_size < RANDOM_ELEMENTS)
		rand->bin[source].r_size++;
	rand->bin[source].r_next = (rand->bin[source].r_next + 1) % RANDOM_ELEMENTS;
}

/* =========================================================================
 * libexec_copy_memcpy — Copy ELF segment data to memory
 *
 * Used by libexec to load ELF segments into process memory.
 * ========================================================================= */

int libexec_copy_memcpy(struct exec_info *execi,
	off_t off, vir_bytes vaddr, size_t len)
{
	if (off + len > execi->hdr_len)
		return EFAULT;
	memcpy((void *)vaddr, execi->hdr + off, len);
	return OK;
}

/* =========================================================================
 * libexec_clear_memset — Clear memory region for ELF loading
 *
 * Used by libexec to zero out BSS-like segments.
 * ========================================================================= */

int libexec_clear_memset(struct exec_info *execi,
	vir_bytes vaddr, size_t len)
{
	memset((void *)vaddr, 0, len);
	return OK;
}

/* =========================================================================
 * libexec_load_elf — Load ELF executable (stub)
 *
 * Stub implementation returning ENOEXEC. The full ELF loader
 * lives in libexec; this stub exists only to satisfy the linker.
 * ========================================================================= */

int libexec_load_elf(struct exec_info *execi)
{
	/* ELF loading not yet available in kernel - libexec not linked. */
	return -1;  /* ENOEXEC */
}
