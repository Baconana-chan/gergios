/* main.c — mkfs.ext4: create an ext4 filesystem on a block device.
 *
 * This is a thin C wrapper around the Rust ext4_mkfs() FFI function
 * in ext4-core. Usage:
 *
 *   mkfs.ext4 [-b block_size] device
 *
 * Default block size is 4096 bytes. Supported: 1024, 2048, 4096.
 *
 * The device must be writable. The filesystem created is minimal:
 * single block group, root + lost+found, no journal.
 */

#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <stdint.h>

/* ─── FFI declaration (from ext4-core Rust crate) ─────────────────── */

/* Create a minimal ext4 filesystem on the given fd.
 * Returns 0 on success, or a positive errno value on failure. */
int ext4_mkfs(int fd, uint32_t block_size, uint64_t blocks_count);


/* ─── Helpers ──────────────────────────────────────────────────────── */

static void
usage(const char *prog)
{
	fprintf(stderr,
	    "Usage: %s [-b block_size] device\n"
	    "  -b block_size   Block size in bytes (1024, 2048, 4096; default 4096)\n"
	    "  device          Block device path (e.g., /dev/c0d0p0s0)\n"
	    "\n"
	    "Creates a minimal ext4 filesystem on the device.\n"
	    "No journal is created. Only 1 block group is supported.\n",
	    prog);
	exit(1);
}

/* Get the size of a block device in bytes. */
static int64_t
get_device_size(int fd)
{
	struct stat st;
	if (fstat(fd, &st) != 0)
		return -1;

	if (S_ISBLK(st.st_mode)) {
		/* Block device: use BLKGETSIZE64 ioctl or lseek */
		off_t end = lseek(fd, 0, SEEK_END);
		if (end < 0)
			return -1;
		lseek(fd, 0, SEEK_SET);
		return (int64_t)end;
	}

	if (S_ISREG(st.st_mode))
		return (int64_t)st.st_size;

	return (int64_t)st.st_size;
}


/* ─── Main ─────────────────────────────────────────────────────────── */

int
main(int argc, char *argv[])
{
	const char *prog = argv[0];
	const char *device = NULL;
	uint32_t block_size = 4096;
	int opt;
	int fd;
	int r;

	/* Parse options */
	while ((opt = getopt(argc, argv, "b:")) != -1) {
		switch (opt) {
		case 'b': {
			long val = strtol(optarg, NULL, 0);
			if (val != 1024 && val != 2048 && val != 4096) {
				fprintf(stderr,
				    "%s: invalid block size %ld "
				    "(must be 1024, 2048, or 4096)\n",
				    prog, val);
				return 1;
			}
			block_size = (uint32_t)val;
			break;
		}
		default:
			usage(prog);
		}
	}

	if (optind >= argc)
		usage(prog);

	device = argv[optind];

	/* Open the device */
	fd = open(device, O_RDWR);
	if (fd < 0) {
		fprintf(stderr, "%s: cannot open %s: %s\n",
		    prog, device, strerror(errno));
		return 1;
	}

	/* Get device size */
	int64_t dev_bytes = get_device_size(fd);
	if (dev_bytes < 0) {
		fprintf(stderr, "%s: cannot get size of %s: %s\n",
		    prog, device, strerror(errno));
		close(fd);
		return 1;
	}

	uint64_t blocks_count = (uint64_t)dev_bytes / block_size;

	if (blocks_count < 8) {
		fprintf(stderr, "%s: device too small (%lld bytes, "
		    "need at least %d bytes)\n",
		    prog, (long long)dev_bytes, 8 * (int)block_size);
		close(fd);
		return 1;
	}

	/* Warn about oversized device */
	if (blocks_count > 32768) {
		fprintf(stderr, "%s: warning: only first 128MB will be "
		    "formatted (device is %lld MB)\n",
		    prog, (long long)(dev_bytes / (1024 * 1024)));
	}

	printf("%s: creating ext4 filesystem on %s\n"
	    "  block_size=%u, blocks=%llu, size=%lld MB\n",
	    prog, device, block_size,
	    (unsigned long long)blocks_count,
	    (long long)(dev_bytes / (1024 * 1024)));

	/* Call Rust ext4_mkfs */
	r = ext4_mkfs(fd, block_size, blocks_count);
	if (r != 0) {
		fprintf(stderr, "%s: ext4_mkfs failed: %s\n",
		    prog, strerror(r));
		close(fd);
		return 1;
	}

	printf("%s: done\n", prog);

	close(fd);
	return 0;
}
