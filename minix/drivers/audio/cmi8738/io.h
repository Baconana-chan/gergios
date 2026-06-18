#ifndef _IO_H
#define _IO_H

#include <sys/types.h>
#include <minix/syslib.h>
#include "cmi8738.h"

/* I/O functions */
static u8_t my_inb(u32_t port) {
	u32_t value;
	int r;
#ifdef DMA_BASE_IOMAP
	value = *(volatile u8_t *)(port);
#else
	if ((r = sys_inb(port, &value)) != OK)
		printf("SDR: sys_inb failed: %d\n", r);
#endif
	return (u8_t)value;
}

static u16_t my_inw(u32_t port) {
	u32_t value;
	int r;
#ifdef DMA_BASE_IOMAP
	value = *(volatile u16_t *)(port);
#else
	if ((r = sys_inw(port, &value)) != OK)
		printf("SDR: sys_inw failed: %d\n", r);
#endif
	return (u16_t)value;
}

static u32_t my_inl(u32_t port) {
	u32_t value;
	int r;
#ifdef DMA_BASE_IOMAP
	value = *(volatile u32_t *)(port);
#else
	if ((r = sys_inl(port, &value)) != OK)
		printf("SDR: sys_inl failed: %d\n", r);
#endif
	return value;
}

static void my_outb(u32_t port, u32_t value) {
	int r;
#ifdef DMA_BASE_IOMAP
	*(volatile u8_t *)(port) = value;
#else
	if ((r = sys_outb(port, (u8_t)value)) != OK)
		printf("SDR: sys_outb failed: %d\n", r);
#endif
}

static void my_outw(u32_t port, u32_t value) {
	int r;
#ifdef DMA_BASE_IOMAP
	*(volatile u16_t *)(port) = value;
#else
	if ((r = sys_outw(port, (u16_t)value)) != OK)
		printf("SDR: sys_outw failed: %d\n", r);
#endif
}

static void my_outl(u32_t port, u32_t value) {
	int r;
#ifdef DMA_BASE_IOMAP
	*(volatile u32_t *)(port) = value;
#else
	if ((r = sys_outl(port, value)) != OK)
		printf("SDR: sys_outl failed: %d\n", r);
#endif
}

/*
 * C11 _Generic unified I/O accessors.
 * Usage:
 *   val = sdr_read(u8_t,  base, offset);
 *   sdr_write(u32_t, base, offset, value);
 *
 * The type parameter selects the correct width at compile time.
 */
#define sdr_read(type, port, offset) _Generic(((type){0}), \
    u8_t:  my_inb((port) + (offset)), \
    u16_t: my_inw((port) + (offset)), \
    u32_t: my_inl((port) + (offset))  \
)

#define sdr_write(type, port, offset, value) _Generic(((type){0}), \
    u8_t:  my_outb((port) + (offset), (u8_t)(value)), \
    u16_t: my_outw((port) + (offset), (u16_t)(value)), \
    u32_t: my_outl((port) + (offset), (u32_t)(value))  \
)

/* Backward-compatible aliases */
#define sdr_in8(port, offset)  sdr_read(u8_t,  (port), (offset))
#define sdr_in16(port, offset) sdr_read(u16_t, (port), (offset))
#define sdr_in32(port, offset) sdr_read(u32_t, (port), (offset))
#define sdr_out8(port, offset, value)  sdr_write(u8_t,  (port), (offset), (value))
#define sdr_out16(port, offset, value) sdr_write(u16_t, (port), (offset), (value))
#define sdr_out32(port, offset, value) sdr_write(u32_t, (port), (offset), (value))

#endif
