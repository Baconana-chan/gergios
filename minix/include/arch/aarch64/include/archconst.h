/* AArch64 architecture constants */
#ifndef _AARCH64_ARCHCONST_H
#define _AARCH64_ARCHCONST_H

/* Timer frequency: use 100 Hz as default (same as x86_64).
 * ARM Generic Timer on QEMU virt runs at ~62.5 MHz but the
 * kernel scales it to the configured HZ value.
 */
#define DEFAULT_HZ        100

/* Boot parameters buffer size (matches MULTIBOOT_PARAM_BUF_SIZE
 * on other architectures, but defined independently for AArch64). */
#define MULTIBOOT_PARAM_BUF_SIZE 1024

#endif /* _AARCH64_ARCHCONST_H */
