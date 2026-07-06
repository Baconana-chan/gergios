/* hellofs — Minimal FUSE example filesystem using librefuse
 *
 * Creates a virtual filesystem with a single "hello.txt" file.
 * Demonstrates the FUSE API working on MINIX via librefuse/libpuffs.
 *
 * Mount:
 *   mkdir /tmp/hello
 *   hellofs /tmp/hello
 *   cat /tmp/hello/hello.txt
 *   fusermount -u /tmp/hello
 */

#include <fuse.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <fcntl.h>

static const char *hello_str = "Hello from GergiOS FUSE!\n";
static const char *hello_name = "hello.txt";

/* ─── getattr — return file or directory attributes ──────────────── */

static int hellofs_getattr(const char *path, struct stat *stbuf)
{
	int res = 0;

	memset(stbuf, 0, sizeof(struct stat));

	if (strcmp(path, "/") == 0) {
		stbuf->st_mode = S_IFDIR | 0755;
		stbuf->st_nlink = 2;
	} else if (strcmp(path, "/" hello_name) == 0) {
		stbuf->st_mode = S_IFREG | 0444;
		stbuf->st_nlink = 1;
		stbuf->st_size = strlen(hello_str);
	} else {
		res = -ENOENT;
	}

	return res;
}

/* ─── readdir — list directory contents ──────────────────────────── */

static int hellofs_readdir(const char *path, void *buf, fuse_fill_dir_t filler,
			   off_t offset, struct fuse_file_info *fi)
{
	(void)offset;
	(void)fi;

	if (strcmp(path, "/") != 0)
		return -ENOENT;

	filler(buf, ".", NULL, 0);
	filler(buf, "..", NULL, 0);
	filler(buf, hello_name, NULL, 0);

	return 0;
}

/* ─── open — check file exists and is accessible ────────────────── */

static int hellofs_open(const char *path, struct fuse_file_info *fi)
{
	if (strcmp(path, "/" hello_name) != 0)
		return -ENOENT;

	if ((fi->flags & 3) != O_RDONLY)
		return -EACCES;

	return 0;
}

/* ─── read — read file contents ──────────────────────────────────── */

static int hellofs_read(const char *path, char *buf, size_t size, off_t offset,
			struct fuse_file_info *fi)
{
	size_t len;
	(void)fi;

	if (strcmp(path, "/" hello_name) != 0)
		return -ENOENT;

	len = strlen(hello_str);
	if (offset >= (off_t)len)
		return 0;

	if (offset + size > len)
		size = len - offset;

	memcpy(buf, hello_str + offset, size);
	return size;
}

/* ─── FUSE operations table ──────────────────────────────────────── */

static struct fuse_operations hellofs_ops = {
	.getattr	= hellofs_getattr,
	.readdir	= hellofs_readdir,
	.open		= hellofs_open,
	.read		= hellofs_read,
};

/* ─── main ───────────────────────────────────────────────────────── */

int main(int argc, char *argv[])
{
	return fuse_main(argc, argv, &hellofs_ops, NULL);
}
