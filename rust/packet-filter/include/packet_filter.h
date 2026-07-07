/* SPDX-License-Identifier: BSD-3-Clause */
/*
 * packet_filter.h — Safe Rust BPF verifier FFI header
 *
 * This header declares the C-callable functions exported by the
 * packet-filter Rust crate.  The verifier performs static analysis
 * on BPF filter programs, rejecting programs that:
 *
 *   - Divide or modulo by zero
 *   - Shift by 32 or more bits (undefined behaviour)
 *   - Read memory words that have not been written on all paths
 *   - Jump to targets outside the program
 *   - Do not terminate (missing BPF_RET)
 *   - Contain invalid or reserved opcodes
 *
 * The C structure `packet_filter_insn` is layout-compatible with
 * `struct bpf_insn` from <net/bpf.h>.
 */

#ifndef PACKET_FILTER_H
#define PACKET_FILTER_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * A single BPF instruction, matching the layout of struct bpf_insn.
 */
struct packet_filter_insn {
	uint16_t code;
	uint8_t  jt;
	uint8_t  jf;
	uint32_t k;
};

/*
 * Validate a BPF filter program.
 *
 * @param insns  Pointer to an array of BPF instructions, or NULL.
 * @param count  Number of instructions in the array.
 * @return 1 if the program is safe to execute, 0 otherwise.
 */
int packet_filter_validate(const struct packet_filter_insn *insns, int count);

#ifdef __cplusplus
}
#endif

#endif /* !PACKET_FILTER_H */
