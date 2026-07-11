//! BPF Virtual Machine — shared header for Phase 9.4 BPF tests
//!
//! Implements a simplified BPF (Berkeley Packet Filter) interpreter
//! for validating filter programs on the host without a running
//! MINIX kernel.
//!
//! Supported: LD, LDX, ALU, JMP, RET, MISC (TAX, TXA)
//! Not supported: ST, STX (scratch memory store) — see note in code
//!
//! This header is included by both bpf_filter.cpp and bpf_attach.cpp.

#ifndef BPF_VM_H
#define BPF_VM_H

#include <cstdint>
#include <cstring>
#include <arpa/inet.h>

// ============================================================================
// BPF instruction set (based on BSD bpf.h / net/bpf.h)
// ============================================================================

// BPF instruction classes
static constexpr uint16_t BPF_LD    = 0x00;
static constexpr uint16_t BPF_LDX   = 0x01;
static constexpr uint16_t BPF_ALU   = 0x04;
static constexpr uint16_t BPF_JMP   = 0x05;
static constexpr uint16_t BPF_RET   = 0x06;
static constexpr uint16_t BPF_MISC  = 0x07;

// BPF LD/LDX size
static constexpr uint16_t BPF_W     = 0x00;
static constexpr uint16_t BPF_H     = 0x08;
static constexpr uint16_t BPF_B     = 0x10;

// BPF LD/LDX modes
static constexpr uint16_t BPF_IMM   = 0x00;
static constexpr uint16_t BPF_ABS   = 0x20;
static constexpr uint16_t BPF_IND   = 0x40;
static constexpr uint16_t BPF_MEM   = 0x60;
static constexpr uint16_t BPF_LEN   = 0x80;
static constexpr uint16_t BPF_MSH   = 0xa0;

// BPF ALU operations
static constexpr uint16_t BPF_ADD   = 0x00;
static constexpr uint16_t BPF_SUB   = 0x10;
static constexpr uint16_t BPF_MUL   = 0x20;
static constexpr uint16_t BPF_DIV   = 0x30;
static constexpr uint16_t BPF_OR    = 0x40;
static constexpr uint16_t BPF_AND   = 0x50;
static constexpr uint16_t BPF_LSH   = 0x60;
static constexpr uint16_t BPF_RSH   = 0x70;
static constexpr uint16_t BPF_NEG   = 0x80;
static constexpr uint16_t BPF_MOD   = 0x90;
static constexpr uint16_t BPF_XOR   = 0xa0;

// BPF JMP conditions
static constexpr uint16_t BPF_JA    = 0x00;
static constexpr uint16_t BPF_JEQ   = 0x10;
static constexpr uint16_t BPF_JGT   = 0x20;
static constexpr uint16_t BPF_JGE   = 0x30;
static constexpr uint16_t BPF_JSET  = 0x40;

// BPF RET values
static constexpr uint32_t BPF_K_ACCEPT = 0xFFFFFFFFu;  // accept packet
static constexpr uint32_t BPF_K_REJECT = 0x00000000u;  // reject packet

// BPF MISC
static constexpr uint16_t BPF_TAX    = 0x00;  // A -> X
static constexpr uint16_t BPF_TXA    = 0x80;  // X -> A

// BPF scratch memory
static constexpr int BPF_MEMWORDS = 16;

// BPF instruction (8 bytes, packed)
struct [[gnu::packed]] BpfInsn {
    uint16_t code;   // opcode
    uint8_t  jt;     // jump if true
    uint8_t  jf;     // jump if false
    uint32_t k;      // constant / jump offset
};

// ============================================================================
// BPF virtual machine state
// ============================================================================

struct BpfVm {
    uint32_t A = 0;                         // accumulator
    uint32_t X = 0;                         // index register
    uint32_t M[BPF_MEMWORDS] = {0};         // scratch memory
    const uint8_t* pkt = nullptr;           // packet data
    uint32_t pktlen = 0;                    // packet length

    BpfVm(const uint8_t* _pkt, uint32_t _pktlen)
        : pkt(_pkt), pktlen(_pktlen) {}
};

// ============================================================================
// Execute a single BPF instruction
// Returns true to continue, false to halt.
// ============================================================================

static bool bpf_exec_insn(const BpfInsn* prog, uint32_t& pc,
                          BpfVm& vm, uint32_t& retval) {
    const BpfInsn& insn = prog[pc];
    uint16_t opclass = insn.code & 0x07;
    uint16_t opmode  = insn.code & 0xe0;
    uint16_t opsize  = insn.code & 0x18;

    switch (opclass) {
    case BPF_LD:
        if (opmode == BPF_IMM) {
            vm.A = insn.k;
        } else if (opmode == BPF_ABS) {
            // Load from packet at absolute offset k
            if (opsize == BPF_W) {
                if (insn.k + 4 > vm.pktlen) return false;
                vm.A = ntohl(*reinterpret_cast<const uint32_t*>(
                    vm.pkt + insn.k));
            } else if (opsize == BPF_H) {
                if (insn.k + 2 > vm.pktlen) return false;
                uint16_t val;
                memcpy(&val, vm.pkt + insn.k, 2);
                vm.A = ntohs(val);
            } else if (opsize == BPF_B) {
                if (insn.k + 1 > vm.pktlen) return false;
                vm.A = vm.pkt[insn.k];
            }
        } else if (opmode == BPF_IND) {
            // Load from packet at offset k + X
            uint32_t offset = insn.k + vm.X;
            if (opsize == BPF_W) {
                if (offset + 4 > vm.pktlen) return false;
                vm.A = ntohl(*reinterpret_cast<const uint32_t*>(
                    vm.pkt + offset));
            } else if (opsize == BPF_H) {
                if (offset + 2 > vm.pktlen) return false;
                uint16_t val;
                memcpy(&val, vm.pkt + offset, 2);
                vm.A = ntohs(val);
            } else if (opsize == BPF_B) {
                if (offset + 1 > vm.pktlen) return false;
                vm.A = vm.pkt[offset];
            }
        } else if (opmode == BPF_LEN) {
            vm.A = vm.pktlen;
        } else if (opmode == BPF_MSH) {
            if (insn.k + 1 > vm.pktlen) return false;
            vm.X = (vm.pkt[insn.k] & 0x0f) * 4;
        }
        pc++;
        return true;

    case BPF_LDX:
        if (opmode == BPF_IMM) {
            vm.X = insn.k;
        } else if (opmode == BPF_MEM) {
            if (insn.k >= BPF_MEMWORDS) return false;
            vm.X = vm.M[insn.k];
        } else if (opmode == BPF_LEN) {
            vm.X = vm.pktlen;
        } else if (opmode == BPF_MSH) {
            if (insn.k + 1 > vm.pktlen) return false;
            vm.X = (vm.pkt[insn.k] & 0x0f) * 4;
        } else if (opmode == BPF_ABS) {
            if (opsize == BPF_B) {
                if (insn.k + 1 > vm.pktlen) return false;
                vm.X = vm.pkt[insn.k];
            }
        }
        pc++;
        return true;

    case BPF_ALU: {
        uint32_t op = insn.code & 0xf0;
        uint32_t rhs = (insn.code & 0x08) ? vm.X : insn.k;
        switch (op) {
        case BPF_ADD: vm.A += rhs; break;
        case BPF_SUB: vm.A -= rhs; break;
        case BPF_MUL: vm.A *= rhs; break;
        case BPF_DIV: vm.A = rhs ? vm.A / rhs : 0; break;
        case BPF_OR:  vm.A |= rhs; break;
        case BPF_AND: vm.A &= rhs; break;
        case BPF_LSH: vm.A <<= rhs; break;
        case BPF_RSH: vm.A >>= rhs; break;
        case BPF_NEG: vm.A = ~vm.A + 1; break;
        case BPF_MOD: vm.A = rhs ? vm.A % rhs : 0; break;
        case BPF_XOR: vm.A ^= rhs; break;
        default: return false;
        }
        pc++;
        return true;
    }

    case BPF_JMP: {
        uint16_t jmp_op = insn.code & 0xf0;
        bool condition = false;
        if (jmp_op == BPF_JA) {
            pc += insn.k;
            return true;
        }
        uint32_t rhs = (insn.code & 0x08) ? vm.X : insn.k;
        switch (jmp_op) {
        case BPF_JEQ:  condition = (vm.A == rhs); break;
        case BPF_JGT:  condition = (vm.A > rhs);  break;
        case BPF_JGE:  condition = (vm.A >= rhs); break;
        case BPF_JSET: condition = (vm.A & rhs) != 0; break;
        default: return false;
        }
        if (condition) {
            pc += insn.jt;
        } else {
            pc += insn.jf;
        }
        return true;
    }

    case BPF_RET:
        retval = insn.k;
        return false;  // halt

    case BPF_MISC:
        if ((insn.code & 0xf0) == BPF_TAX) {
            vm.X = vm.A;
        } else if ((insn.code & 0xf0) == BPF_TXA) {
            vm.A = vm.X;
        }
        pc++;
        return true;

    default:
        return false;  // unknown instruction
    }
}

// ============================================================================
// Run a BPF program on a packet
// Returns the return value (0 = reject, non-zero = accept with capture len)
// ============================================================================

static inline uint32_t bpf_run(const BpfInsn* prog, uint32_t prog_len,
                               const uint8_t* pkt, uint32_t pkt_len) {
    BpfVm vm(pkt, pkt_len);
    uint32_t pc = 0;
    uint32_t retval = BPF_K_REJECT;

    while (pc < prog_len) {
        if (!bpf_exec_insn(prog, pc, vm, retval)) {
            return retval;
        }
    }
    return BPF_K_REJECT;  // fall through → reject
}

#endif // BPF_VM_H
