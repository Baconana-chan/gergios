#include "kernel/kernel.h"

/* This is the user-visible struct that has pointers to other bits of data. */
struct minix_kerninfo minix_kerninfo __section(".usermapped");

/* Kernel information structures. */
struct kinfo kinfo __section(".usermapped");		/* kernel information for services */
struct machine machine __section(".usermapped");	/* machine information for services */
struct kmessages kmessages __section(".usermapped");	/* diagnostic messages in kernel */
struct loadinfo loadinfo __section(".usermapped");	/* status of load average */
struct kuserinfo kuserinfo __section(".usermapped");
	/* kernel information for users */
struct arm_frclock arm_frclock __section(".usermapped");
	/* ARM free running timer information */
struct kclockinfo kclockinfo __section(".usermapped");	/* clock information */

/* KASLR: virtual offset applied to the kernel at boot (0 when disabled).
 * Declared extern in limine.c / pre_init.c; lives in .usermapped so
 * services can read it. */
u64_t kaslr_virt_offset_slot __section(".usermapped");
