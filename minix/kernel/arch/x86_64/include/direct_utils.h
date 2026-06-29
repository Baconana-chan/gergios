/* ============================================================
 * direct_utils.h — x86_64 direct VGA text mode I/O
 *
 * Declares the direct (pre-console) output functions used for
 * emergency/shutdown messages. On x86_64, these write directly
 * to the VGA text framebuffer at 0xB8000, bypassing the
 * console subsystem.
 *
 * Implemented in arch/x86_64/direct_tty_utils.c.
 * ============================================================ */

#ifndef _X86_64_DIRECT_UTILS_H_
#define _X86_64_DIRECT_UTILS_H_

/* Print a string directly to the VGA text framebuffer. */
void direct_print(const char *str);

/* Clear the VGA text framebuffer (white-on-black spaces). */
void direct_cls(void);

/* Read a character from the PS/2 keyboard (non-blocking).
 * Returns 1 if a character was read, 0 if no key pressed.
 * The character value is stored in *ch.
 */
int direct_read_char(unsigned char *ch);

#endif /* _X86_64_DIRECT_UTILS_H_ */
