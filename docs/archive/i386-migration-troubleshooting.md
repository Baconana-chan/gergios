# i386 to x86_64 Migration Troubleshooting Guide

## Overview

This guide provides solutions for common issues encountered when migrating from i386 (32-bit) to x86_64 (64-bit) in MINIX. It covers build issues, runtime issues, and common migration pitfalls.

---

## Build System Issues

### Problem: Build fails with "unknown architecture" or "unsupported architecture"

**Symptoms:**
```
CMake Error: Unknown architecture
```

**Solution:**
The x86_64 architecture must be explicitly selected during configuration:

```bash
# Using CMake
cmake -DMACHINE_ARCH=x86_64 ..

# Using cmake-build.sh
./releasetools/cmake-build.sh configure x86_64

# Using build.sh (legacy)
./build.sh -m amd64 build
```

**Verification:**
After configuration, verify the architecture:
```bash
cmake -LA | grep MACHINE_ARCH
# Should output: MACHINE_ARCH=x86_64
```

### Problem: Linker errors with 32-bit object files

**Symptoms:**
```
ld: i386 architecture of input file `foo.o' is incompatible with i386:x86-64 output
```

**Solution:**
This occurs when mixing 32-bit and 64-bit object files. Perform a clean build:
```bash
# Clean and rebuild
rm -rf build/
mkdir build && cd build
cmake -DMACHINE_ARCH=x86_64 ..
make clean
make
```

### Problem: Missing x86_64 compiler

**Symptoms:**
```
cc: error: unrecognized command-line option '-m64'
```

**Solution:**
Ensure your compiler supports x86_64 targets:
```bash
# Check compiler support
gcc -dumpmachine | grep x86_64

# If not found, install a cross-compiler or native x86_64 toolchain
```

---

## Assembly Code Issues

### Problem: Inline assembly uses 32-bit registers

**Incorrect (i386):**
```c
__asm__ __volatile__("movl %0, %%eax" : : "r"(val));
```

**Correct (x86_64):**
```c
__asm__ __volatile__("movq %0, %%rax" : : "r"(val));
```

**Key differences:**
| i386 | x86_64 | Description |
|------|--------|-------------|
| `eax` | `rax` | Accumulator (32-bit → 64-bit) |
| `ebx` | `rbx` | Base register |
| `ecx` | `rcx` | Counter register |
| `edx` | `rdx` | Data register |
| `esi` | `rsi` | Source index |
| `edi` | `rdi` | Destination index |
| `ebp` | `rbp` | Base pointer |
| `esp` | `rsp` | Stack pointer |
| — | `r8`–`r15` | Additional registers (x86_64 only) |

### Problem: `int 0x80` system calls fail

**Incorrect (i386):**
```c
__asm__ __volatile__("int $0x80" : : "a"(syscall_num));
```

**Correct (x86_64):**
```c
__asm__ __volatile__("syscall" : : "a"(syscall_num));
```

**Note:** On x86_64, the `syscall` instruction is used instead of `int 0x80`. The `int 0x80` mechanism still works for compatibility but is slower and limited to 32-bit arguments.

### Problem: `iretd` instruction fails

**Incorrect (i386):**
```asm
iretd
```

**Correct (x86_64):**
```asm
iretq
```

The x86_64 return from interrupt instruction uses `iretq`, which pops 64-bit registers (RIP, CS, RFLAGS, RSP, SS) instead of 32-bit (EIP, CS, EFLAGS, ESP, SS).

---

## Pointer and Type Issues

### Problem: Pointer truncation

**Symptoms:**
```
warning: cast to pointer from integer of different size
```

**Incorrect:**
```c
void *ptr = (void *)0xfeedface;  // 32-bit value truncated to 64-bit pointer
uint32_t addr = (uint32_t)ptr;   // 64-bit pointer truncated to 32-bit
```

**Correct:**
```c
void *ptr = (void *)(uintptr_t)0xfeedface;
uint64_t addr = (uint64_t)(uintptr_t)ptr;
```

**Best practices:**
- Use `uintptr_t` for casting between integers and pointers
- Use `ptrdiff_t` for pointer differences
- Use `size_t` for sizes
- Avoid assuming `sizeof(void *) == 4`

### Problem: Structure layout changes due to alignment

**Symptoms:**
- Data corruption in IPC messages
- Incorrect structure sizes
- ABI mismatches between components

**Solution:**
Check your structure definitions for alignment-sensitive fields:

```c
// Problem: layout changes between 32-bit and 64-bit
struct my_message {
    uint32_t type;    // 4 bytes
    void *data;       // 4 bytes on i386, 8 bytes on x86_64!
    uint32_t length;  // 4 bytes
    // Padding: 0 bytes on i386, 4 bytes on x86_64!
};

// Fix: use explicit-sized fields
struct my_message {
    uint32_t type;
    uint64_t data_ptr;
    uint32_t length;
    uint32_t padding;  // Explicit padding
};
```

**Use `__attribute__((packed))`** only when necessary for wire protocols or hardware interfaces.

---

## Runtime Issues

### Problem: "Kernel too large" or boot failures

**Symptoms:**
- System doesn't boot
- "Cannot load kernel" error
- QEMU fails to start

**Solution:**
Ensure the bootloader is configured for x86_64. For QEMU testing:
```bash
# Use the correct machine type
qemu-system-x86_64 -machine q35 -m 1024 -kernel path/to/kernel

# For multiboot-compatible kernels
qemu-system-x86_64 -kernel kernel.bin
```

### Problem: Process crashes with "Segmentation fault" or "Bus error"

**Symptoms:**
- Processes crash on startup
- Signal 11 (SIGSEGV) errors
- Random crashes

**Common causes:**
1. **Stack alignment**: x86_64 requires 16-byte stack alignment before `call` instructions
2. **Pointer truncation**: Storing 64-bit pointers in 32-bit variables
3. **Incorrect signal handlers**: Signal handler frame layout differs
4. **Syscall ABI mismatch**: Using wrong argument registers

**Debugging:**
```bash
# Enable verbose kernel output
boot -v

# Check crash dumps
cat /var/log/messages
```

### Problem: IPC messages corrupted

**Symptoms:**
- Servers fail to communicate
- Garbled message contents
- "Invalid message" errors

**Solution:**
Check IPC message structure definitions. In x86_64:
- Message structures must be 64-bit aligned
- Pointer fields are 8 bytes instead of 4
- Use `vir_bytes` and `phys_bytes` types which adapt to architecture

```c
// Correct IPC message structure (architecture-independent)
typedef struct {
    int m_source;              // Sender (int, 4 bytes)
    int m_type;                // Message type (int, 4 bytes)
    union {
        message_data_t data;   // Message data
        void *m_ptr;           // Pointer data
        uint64_t m_long;       // Long data
    } m_data;
} message;
```

---

## Driver Issues

### Problem: I/O port access fails

**Symptoms:**
- `inb`/`outb` instructions cause faults
- Device initialization fails
- Permission denied errors

**Solution:**
I/O port access works the same on x86_64, but permissions must be granted:
```c
// Grant I/O permission to process (same as i386)
if (sys_privctl(SELF, SYS_PRIV_ALLOW, &priv) != OK) {
    // Handle error
}
```

**Note:** On x86_64, the `in` and `out` instructions work identically to i386. No changes needed for I/O port access code.

### Problem: MMIO address space too large

**Symptoms:**
- Device driver can't map MMIO regions
- Memory mapping fails for high addresses

**Solution:**
Use 64-bit physical addresses for MMIO:
```c
// Use phys_bytes (64-bit on x86_64) instead of phys_bytes (32-bit on i386)
phys_bytes mmio_base = pci_get_bar_addr(dev, 0);
vm_map_phys(SELF, &mmio_base, size);
```

### Problem: DMA with 32-bit addresses

**Symptoms:**
- DMA transfers fail or corrupt data
- Device reports "address error"

**Solution:**
Ensure DMA buffers use 64-bit addresses:
```c
// Allocate DMA buffer with 64-bit address
void *dma_buffer;
phys_bytes dma_phys;

if (sys_umap(SELF, VM_TYPE, dma_buffer, size, &dma_phys) != OK) {
    // Handle error
}

// Program device with 64-bit DMA address
write_device_register(device, DMA_ADDR_LOW, dma_phys & 0xFFFFFFFF);
write_device_register(device, DMA_ADDR_HIGH, (dma_phys >> 32) & 0xFFFFFFFF);
```

---

## Memory Management Issues

### Problem: vm_map or vm_alloc failures

**Symptoms:**
- Memory allocation returns ENOMEM
- vm_map fails with EFAULT or EINVAL

**Solutions:**
1. **Address space exhaustion**: Use larger virtual address space
2. **Page table mismatch**: Use 4-level paging functions for x86_64
3. **Alignment**: x86_64 requires 4KB alignment for page mapping

### Problem: Kernel memory allocation fails

**Symptoms:**
- `alloc_mem()` returns NULL
- Kernel panic during memory allocation

**Solution:**
The kernel's virtual address space is mapped differently on x86_64:
- Kernel base: `0xFFFF800000000000` (vs `0x0` on i386)
- Stack pointer: RSP (64-bit) vs ESP (32-bit)
- Heap: Must be in canonical address form

---

## Debugging

### Architecture detection

Use these macros to write architecture-independent code:
```c
#if defined(__i386__)
    // 32-bit specific code
#elif defined(__x86_64__)
    // 64-bit specific code
#else
    #error "Unsupported architecture"
#endif
```

### Common error messages and solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `relocation truncated to fit` | 32-bit relocation in 64-bit code | Use `-mcmodel=large` or fix pointer types |
| `undefined reference to _alloca` | Stack probing missing | Add `-mno-stack-arg-probe` |
| `PIE not supported` | Position-independent executable | Add `-no-pie` or use `-fPIE` |
| `TLS model conflict` | Thread-local storage mismatch | Use `-ftls-model=initial-exec` |
| `out of memory` in kernel | Heap space exhausted | Increase kernel heap in linker script |

### Verification checklist

After building for x86_64, verify:
- [ ] `file kernel` shows "ELF 64-bit LSB executable, x86-64"
- [ ] `readelf -h kernel` shows `Machine: Advanced Micro Devices X86-64`
- [ ] `objdump -d kernel` shows 64-bit instructions (rax, rbx, etc.)
- [ ] No 32-bit object files remain in the build
- [ ] All libraries are 64-bit: `file /usr/lib/libc.so`

---

## Getting Help

If you encounter issues not covered here:

1. **Check the FAQ**: [i386 Deprecation FAQ](i386-deprecation-faq.md)
2. **Read the migration plan**: [x86_64 Migration Plan](../planning/07_x86_64_migration_plan.md)
3. **Review the codebase audit**: [i386 Codebase Audit](i386-codebase-audit.md)
4. **Search for existing issues** in the issue tracker
5. **File a new issue** with:
   - Full error messages and logs
   - Architecture and hardware details
   - Steps to reproduce
   - Build configuration used

---

*Last updated: June 18, 2026*
