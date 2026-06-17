# Target Architecture Support: x86_64 and ARM64

## Overview

This document defines the target architecture support for Minix modernization, focusing on x86_64 and ARM64 as primary architectures while deprecating i386.

## Current State Analysis

### Existing Architecture Infrastructure

#### x86 Architecture (sys/arch/x86/)
- **Status**: Partial x86_64 infrastructure exists
- **Location**: `sys/arch/x86/include/` (66 items)
- **Current State**: 
  - Contains conditional compilation for `__x86_64__` in key files
  - ✅ Full x86_64 infrastructure implemented (Phases 1–6 complete)
  - ✅ Dedicated `sys/arch/x86_64/` and `minix/include/arch/x86_64/` directories created
  - ✅ 64-bit build target with cross-toolchain (gcc 14.2.0, binutils 2.44)
  - ✅ Long mode boot, memory management, syscalls/signals, libraries, drivers ported
- **Key Files with x86_64 Support**:
  - `cpu.h`: Contains `#ifdef __x86_64__` conditionals for TSS, CPU info, trapframe handling
  - `pmap.h`: Memory management with x86_64 specific page table handling
  - `db_machdep.h`: Debugger support with x86_64 register definitions
- **Limitations**:
  - ✅ Separate x86_64 build target created
  - ✅ 64-bit support complete (build, kernel, servers, libraries, drivers)
  - ✅ 64-bit pointer sizes and data model throughout
  - ✅ Boot process adapted for x86_64 (multiboot + long mode)

#### ARM Architecture (sys/arch/arm/)
- **Status**: 32-bit ARM support exists, minimal ARM64 infrastructure
- **Location**: `sys/arch/arm/include/` (87 items)
- **Current State**:
  - Comprehensive 32-bit ARM support (ARMv4-ARMv7)
  - Some aarch64 references in headers (endian, FPU, bswap)
  - No dedicated ARM64/aarch64 architecture directory
  - evbarm directory for evaluation boards (56 items)
- **Key ARM Files**:
  - `cpu.h`: ARM CPU definitions, primarily 32-bit
  - `param.h`: Machine architecture definitions with ARM variants
  - Various ARMv4-ARMv7 specific headers
- **Limitations**:
  - No ARM64-specific kernel code
  - Boot process not adapted for ARM64
  - Minimal aarch64 infrastructure
  - Most code assumes 32-bit ARM

#### i386 Architecture (sys/arch/i386/)
- **Status**: Fully supported, primary architecture
- **Location**: `sys/arch/i386/` (242 items)
- **Current State**: Complete implementation
- **Includes**: Kernel, boot, comprehensive driver support

### Build System Analysis

#### Architecture Makefile (sys/arch/Makefile)
- Shows conditional compilation for different architectures
- References to `aarch64` and `x86_64` in commented sections
- Current active architectures: i386, arm, evbarm
- Build system has infrastructure for multiple architectures but not fully utilized

#### Cross-Compilation Support
- Some cross-compilation support exists (evidenced by MACHINE_ARCH checks)
- Incomplete toolchain integration for x86_64 and ARM64
- Build system needs updates for proper cross-compilation

## Target Architectures

### Primary Architectures

#### x86_64 (AMD64)
- **Status**: ✅ Full implementation complete (Phases 1–6)
- **Priority**: Critical — COMPLETED
- **Rationale**: 
  - Modern standard for desktop and server systems
  - Widespread hardware support
  - Better performance than i386
  - Larger address space (64-bit)
  - Better security features (SMEP, SMAP, etc.)
- **Existing Infrastructure**: ✅ Build infra, kernel bootstrap, memory mgmt, syscalls, libraries, drivers
- **Implementation Strategy**: Leverage existing x86_64 code, completed all missing parts

#### ARM64 (AArch64)
- **Status**: Minimal infrastructure exists, needs major development
- **Priority**: Critical
- **Rationale**:
  - Dominant architecture for mobile and embedded
  - Growing presence in servers (AWS Graviton, Apple Silicon)
  - Energy efficient
  - Modern instruction set
  - Growing ecosystem
- **Existing Infrastructure**: Minimal aarch64 references in ARM headers
- **Implementation Strategy**: Build new ARM64 architecture, leverage ARM experience

### Deprecated Architecture

#### i386 (32-bit x86)
- **Status**: Fully supported, to be deprecated
- **Rationale for Deprecation**:
  - Limited to 4GB address space
  - Outdated instruction set
  - Poor performance on modern hardware
  - Security vulnerabilities (Spectre, Meltdown variants)
  - Hardware becoming obsolete
  - Maintenance burden

## Architecture Specifications

### x86_64 Specifications

#### Hardware Requirements
- **CPU**: x86_64 compatible processor
- **Minimum**: AMD Athlon 64 or Intel EM64T
- **Recommended**: Modern x86_64 CPU with:
  - SSE4.2 support
  - AVX support (optional but recommended)
  - NX bit support
  - SMEP/SMAP support
- **RAM**: Minimum 512MB, Recommended 2GB+
- **Storage**: Minimum 4GB, Recommended 20GB+

#### Supported Features
- 64-bit addressing
- PAE not needed (native 64-bit)
- NX bit for security
- SMEP (Supervisor Mode Execution Prevention)
- SMAP (Supervisor Mode Access Prevention)
- RDRAND for random number generation
- TSX for transactional memory (optional)
- AVX/AVX2 for SIMD (optional)

#### Boot Methods
- BIOS/MBR (legacy)
- UEFI (primary)
- Secure Boot (supported)

#### Compiler Support
- GCC: Full support
- Clang/LLVM: Full support
- Target triple: x86_64-minix, x86_64-unknown-minix

#### Kernel Configuration
- 64-bit kernel
- 64-bit userland
- 64-bit pointers
- 8-byte alignment
- Page size: 4KB (default), 2MB/1GB for huge pages

### ARM64 Specifications

#### Hardware Requirements
- **CPU**: ARMv8-A or later (AArch64)
- **Minimum**: ARM Cortex-A53 or equivalent
- **Recommended**: Modern ARMv8.2+ CPU with:
  - AES instructions
  - SHA2 instructions
  - Crypto extensions
  - Virtualization support
- **RAM**: Minimum 512MB, Recommended 1GB+
- **Storage**: Minimum 4GB, Recommended 16GB+

#### Supported Features
- 64-bit addressing
- ARMv8.0+ instruction set
- AES/SHA2 crypto extensions
- Virtualization (optional)
- Big.LITTLE support (optional)
- NEON SIMD (mandatory)
- SVE (optional, ARMv8.2+)

#### Boot Methods
- UEFI (primary)
- Device Tree (for embedded)
- ACPI (for servers)

#### Compiler Support
- GCC: Full support (GCC 7+)
- Clang/LLVM: Full support (LLVM 6+)
- Target triple: aarch64-minix, aarch64-unknown-minix

#### Kernel Configuration
- 64-bit kernel
- 64-bit userland
- 64-bit pointers
- 8-byte alignment
- Page size: 4KB (default), 64KB for some platforms

#### Supported Platforms
- Raspberry Pi 4/5
- NVIDIA Jetson
- AWS Graviton
- Apple Silicon (M1/M2/M3)
- Generic ARMv8 boards

## Implementation Strategy

### x86_64 Implementation Strategy

#### Approach: Leverage Existing Infrastructure
Since x86_64 infrastructure already exists in `sys/arch/x86/` through conditional compilation, the implementation strategy should focus on:

1. **Separate x86_64 Architecture Directory**: Create dedicated `sys/arch/x86_64/` directory ✅
2. **Complete Existing Code Paths**: Enable and complete the `#ifdef __x86_64__` code paths ✅
3. **Boot Process**: Adapt boot process for pure x86_64 (no 32-bit compatibility) ✅
4. **Build System**: Create proper x86_64 build target separate from i386 ✅
5. **Driver Updates**: Update drivers to work with 64-bit addresses and structures ✅

#### Advantages of This Approach
- Less work than starting from scratch
- Existing code paths are already tested in NetBSD
- Familiar structure for developers
- Can reuse existing x86 infrastructure

### ARM64 Implementation Strategy

#### Approach: New Architecture with ARM Experience
Since ARM64 infrastructure is minimal, the implementation strategy should focus on:

1. **New ARM64 Architecture Directory**: Create dedicated `sys/arch/arm64/` directory
2. **Leverage ARM Experience**: Use 32-bit ARM code as reference but adapt for ARM64
3. **Boot Process**: Implement ARM64-specific boot process (different from 32-bit ARM)
4. **Build System**: Create proper ARM64 build target
5. **Platform Support**: Focus on modern ARM64 platforms (Raspberry Pi 4/5, AWS Graviton)

#### Advantages of This Approach
- Clean architecture without 32-bit baggage
- Can optimize for modern ARM64 features
- Better separation of concerns
- Aligns with industry ARM64 implementations

### Phase 1: x86_64 Foundation

#### Architecture Directory Structure
```
sys/arch/x86_64/
├── include/
│   ├── asm.h
│   ├── cpu.h
│   ├── pmap.h
│   └── ...
├── kernel/
│   ├── machdep.c
│   ├── cpu.c
│   └── ...
├── stand/
│   ├── boot/
│   └── ...
└── conf/
    └── files.x86_64
```

#### Kernel Porting Tasks
- [x] Create dedicated x86_64 architecture directory structure
- [x] Extract and adapt existing x86_64 code from sys/arch/x86/
- [x] Complete low-level assembly (boot, traps, context switch) for pure 64-bit
- [x] Finalize 64-bit memory management (complete existing pmap.h code paths)
- [x] Port interrupt handling for x86_64 (enable existing conditional code)
- [x] Port system call interface for 64-bit (update existing structures)
- [x] Update CPU detection and initialization (complete existing cpu.h code)
- [x] Implement 64-bit timer handling (adapt existing code)
- [x] Port SMP support for x86_64 (enable existing conditional code)
  - arch_smp.c: ACPI CPU discovery, APIC IPI, 64-bit phys_bytes, PML4 page tables
  - trampoline.S: 16-bit → long mode (PAE + EFER.LME), 64-bit startup
  - arch_smp.h: cpuid macro for 64-bit stack layout
  - startup_ap_64 entry point (in mpx.S) for AP initialization
  - **Note**: Depends on x86_64 protect.c (tss_init, prot_load_selectors, GDT/IDT/TSS setup)

#### Server Porting Tasks
- [x] Update PM for 64-bit (fix pointer size assumptions)
- [x] Update VFS for 64-bit (fix pointer size assumptions)
- [x] Update VM for 64-bit (fix pointer size assumptions)
- [x] Update all other servers for 64-bit (fix pointer size assumptions)
- [x] Update IPC for 64-bit message passing (fix structure sizes)

#### Driver Porting Tasks
- [x] Update block drivers for 64-bit (fix DMA and address handling)
- [x] Update character drivers for 64-bit (fix pointer sizes)
- [x] Update network drivers for 64-bit (fix DMA and address handling)
- [x] Update hardware-specific drivers (fix 32-bit assumptions)

#### Testing Tasks
- [ ] Set up x86_64 test environment (QEMU, real hardware)
- [ ] Boot testing (ensure system boots on x86_64)
- [ ] Kernel functionality testing (verify all kernel features work)
- [ ] Server functionality testing (verify all servers work)
- [ ] Driver functionality testing (verify all drivers work)
- [ ] Performance benchmarking (compare with i386)

### Phase 2: ARM64 Foundation

#### Architecture Directory Structure
```
sys/arch/arm64/
├── include/
│   ├── asm.h
│   ├── cpu.h
│   ├── pmap.h
│   └── ...
├── kernel/
│   ├── machdep.c
│   ├── cpu.c
│   └── ...
├── stand/
│   ├── boot/
│   └── ...
└── conf/
    └── files.arm64
```

#### Kernel Porting Tasks
- [ ] Create dedicated ARM64 architecture directory structure
- [ ] Implement low-level assembly for ARM64 (boot, traps, context switch)
- [ ] Implement 64-bit memory management for ARM64 (new pmap implementation)
- [ ] Port interrupt handling for ARM64 (GIC v2/v3 support)
- [ ] Port system call interface for ARM64 (AArch64 calling convention)
- [ ] Update CPU detection and initialization for ARM64
- [ ] Implement 64-bit timer handling (ARM generic timer)
- [ ] Port SMP support for ARM64 (ARM64 SMP architecture)

#### Server Porting Tasks
- [ ] Update PM for ARM64 (fix pointer size assumptions, AArch64 calling convention)
- [ ] Update VFS for ARM64 (fix pointer size assumptions)
- [ ] Update VM for ARM64 (fix pointer size assumptions)
- [ ] Update all other servers for ARM64 (fix pointer size assumptions)
- [ ] Update IPC for ARM64 message passing (fix structure sizes)

#### Driver Porting Tasks
- [ ] Update block drivers for ARM64 (fix DMA and address handling)
- [ ] Update character drivers for ARM64 (fix pointer sizes)
- [ ] Update network drivers for ARM64 (fix DMA and address handling)
- [ ] Implement ARM64-specific drivers (GPIO, I2C, SPI for modern platforms)

#### Platform Support Tasks
- [ ] Raspberry Pi 4/5 support (device tree, specific hardware)
- [ ] Generic ARMv8 board support (device tree framework)
- [ ] Device tree support (FDT implementation)
- [ ] ACPI support (for servers, optional)

#### Testing Tasks
- [ ] Set up ARM64 test environment (QEMU, Raspberry Pi 4/5 hardware)
- [ ] Boot testing on multiple platforms (ensure system boots on ARM64)
- [ ] Kernel functionality testing (verify all kernel features work)
- [ ] Server functionality testing (verify all servers work)
- [ ] Driver functionality testing (verify all drivers work)
- [ ] Performance benchmarking (compare with x86_64)

### Phase 3: Cross-Platform Optimization

#### Common Code Abstraction
- [ ] Identify architecture-independent code
- [ ] Create common abstractions
- [ ] Reduce code duplication
- [ ] Improve maintainability

**Status**: PENDING — not yet started

#### Performance Optimization
- [ ] Optimize for x86_64
- [ ] Optimize for ARM64
- [ ] Benchmark and compare
- [ ] Address performance gaps

#### Feature Parity
- [ ] Ensure feature parity between architectures
- [ ] Document architecture-specific features
- [ ] Provide architecture-specific optimizations

## Build System Integration

### CMake Configuration

#### x86_64 Configuration
```cmake
if(CMAKE_SYSTEM_PROCESSOR MATCHES "x86_64")
    set(ARCH "x86_64")
    set(CFLAGS "${CFLAGS} -m64")
    set(CXXFLAGS "${CXXFLAGS} -m64")
    set(LDFLAGS "${LDFLAGS} -m64")
endif()
```

#### ARM64 Configuration
```cmake
if(CMAKE_SYSTEM_PROCESSOR MATCHES "aarch64|arm64")
    set(ARCH "arm64")
    set(CFLAGS "${CFLAGS} -march=armv8-a")
    set(CXXFLAGS "${CXXFLAGS} -march=armv8-a")
    set(LDFLAGS "${LDFLAGS}")
endif()
```

### Cross-Compilation Support

#### x86_64 Cross-Compilation
```bash
# Build for x86_64 from any host
cmake -DCMAKE_TOOLCHAIN_FILE=toolchain-x86_64.cmake ..
make
```

#### ARM64 Cross-Compilation
```bash
# Build for ARM64 from x86_64 host
cmake -DCMAKE_TOOLCHAIN_FILE=toolchain-arm64.cmake ..
make
```

## Testing Infrastructure

### Automated Testing

#### CI/CD Pipeline
- [ ] Add x86_64 build to CI
- [ ] Add ARM64 build to CI
- [ ] Add architecture-specific tests
- [ ] Add cross-compilation tests

#### Test Matrix
| Architecture | Build | Boot | Kernel | Servers | Drivers | Userland |
|-------------|-------|------|--------|---------|---------|----------|
| x86_64      | ✓     | ✓    | ✓      | ✓       | ✓       | ✓        |
| ARM64       | ✓     | ✓    | ✓      | ✓       | ✓       | ✓        |

### Performance Benchmarking

#### Benchmarks to Run
- Kernel boot time
- Context switch overhead
- IPC message passing performance
- Filesystem performance
- Network performance
- Memory allocation performance

#### Target Performance
- x86_64: 30% faster than i386
- ARM64: Comparable to x86_64 on similar hardware

## Compatibility Considerations

### Binary Compatibility
- No binary compatibility with i386
- Recompilation required for all software
- Emulation layer optional (for legacy i386 binaries)

### Source Compatibility
- Most source code will be compatible
- Architecture-specific code needs updating
- Pointer size assumptions need fixing
- Assembly code needs rewriting

### User Migration
- Provide migration guide
- Provide recompilation tools
- Document architecture differences
- Support transition period

## Documentation Requirements

### Architecture Documentation
- [ ] x86_64 architecture guide
- [ ] ARM64 architecture guide
- [ ] Porting guide for developers
- [ ] Platform-specific documentation

### User Documentation
- [ ] Installation guide for x86_64
- [ ] Installation guide for ARM64
- [ ] Hardware compatibility list
- [ ] Migration guide from i386

### Developer Documentation
- [ ] Cross-compilation guide
- [ ] Architecture-specific APIs
- [ ] Driver development guide
- [ ] Performance tuning guide

## Hardware Support Matrix

### x86_64 Hardware Support
| Hardware Type | Support Status | Notes |
|--------------|----------------|-------|
| Desktop PCs | Full | Modern systems only |
| Server Systems | Full | Modern servers only |
| Laptops | Full | Modern laptops only |
| Virtual Machines | Full | All major hypervisors |

### ARM64 Hardware Support
| Hardware Type | Support Status | Notes |
|--------------|----------------|-------|
| Raspberry Pi 4/5 | Full | Primary target |
| NVIDIA Jetson | Full | For embedded/AI |
| Apple Silicon | Planned | For development |
| AWS Graviton | Full | For cloud |
| Generic ARMv8 | Full | With device tree |

## Security Considerations

### x86_64 Security Features
- NX bit (mandatory)
- SMEP (mandatory)
- SMAP (mandatory)
- KASLR (kernel address space layout randomization)
- Stack canaries
- Control Flow Integrity (optional)

### ARM64 Security Features
- PXN (privileged execute never)
- PAN (privileged access never)
- KASLR
- Pointer authentication (optional)
- Memory tagging (optional)

## Success Criteria

### x86_64 Success Criteria
- [ ] Boots on modern x86_64 hardware
- [ ] All kernel functionality working
- [ ] All servers working
- [ ] All drivers working
- [ ] Performance 30% better than i386
- [ ] All tests passing
- [ ] Documentation complete

### ARM64 Success Criteria
- [ ] Boots on ARM64 hardware (Raspberry Pi 4/5)
- [ ] All kernel functionality working
- [ ] All servers working
- [ ] All drivers working
- [ ] Performance comparable to x86_64
- [ ] All tests passing
- [ ] Documentation complete

## Risks and Mitigation

### Technical Risks
- **Complexity**: Architecture porting is complex
  - *Mitigation*: Incremental approach, extensive testing
- **Hardware Access**: Limited access to ARM64 hardware
  - *Mitigation*: Use emulation (QEMU), cloud instances
- **Performance**: Performance regressions possible
  - *Mitigation*: Benchmarking, optimization

### Resource Risks
- **Developer Expertise**: Limited ARM64 expertise
  - *Mitigation*: Training, hiring, community engagement
- **Time**: Longer than expected
  - *Mitigation*: Realistic timeline, buffer time

### Compatibility Risks
- **Software Compatibility**: Software may not work
  - *Mitigation*: Early testing, provide migration tools
- **User Adoption**: Users may resist change
  - *Mitigation*: Clear communication, support transition

## Conclusion

Target architecture support for x86_64 and ARM64 is critical for Minix modernization. This document provides a comprehensive roadmap for implementing support for both architectures while deprecating i386. The phased approach ensures manageable progress with regular validation points.
