# Gergi TODO List

This document consolidates all TODO items across the Minix codebase, including existing tasks from various TODO files and new strategic directions.

## Table of Contents
- [Action Plan for 2026](#action-plan-for-2026)
- [Phase 1: Foundation](#phase-1-foundation)
- [Phase 2: Core Modernization](#phase-2-core-modernization)
- [Phase 3: Advanced Features](#phase-3-advanced-features)
- [Phase 4: Cleanup & Deprecation](#phase-4-cleanup--deprecation)
- [Detailed Component Analysis](#detailed-component-analysis)

---

## Action Plan for 2026

This document provides a concrete action plan for modernizing Minix in 2026, focusing on what TO DO rather than what NOT to do.

### Key Principles
- **Build modern alternatives first**, then deprecate legacy
- **Focus on x86_64 and ARM64**, deprecate i386
- **Adopt existing open-source solutions** instead of building from scratch
- **Incremental migration** with clear phases
- **Security and stability** as top priorities

---

## Phase 1: Foundation

### 1.1 Infrastructure Setup
- [x] Set up modern CI/CD pipeline (GitHub Actions or similar)
- [x] Create automated testing framework for existing codebase
- [x] Establish code coverage metrics and reporting
- [x] Set up static analysis tools (clang-tidy, cppcheck)
- [x] Create security scanning pipeline (Coverity, OSS-Fuzz)
Finished - 2026.06.12

### 1.2 Architecture Planning
- [x] Document current Minix microkernel architecture
- [x] Identify all legacy technology dependencies
- [x] Create migration roadmap for each obsolete component
- [x] Define target architecture support (x86_64, ARM64)
- [x] Plan deprecation timeline for i386 support
Finished - 2026.06.12

### 1.3 Rust Integration Foundation
- [ ] Evaluate Rust integration approach for microkernel architecture
- [ ] Set up Rust cross-compilation toolchain for Minix targets
- [ ] Create build system integration for Rust components
- [ ] Define Rust-C FFI interface standards
- [ ] Create prototype Rust component (simple userland utility)

### 1.4 Filesystem Strategy
- [ ] Evaluate modern filesystem options (ZFS, Btrfs, ext4)
- [ ] Choose primary filesystem for Minix (recommend ext4 for simplicity)
- [ ] Design filesystem integration architecture
- [ ] Create test environment for filesystem development
- [ ] Begin ext4 driver research and design

### 1.5 Authentication Modernization
- [ ] Research Zero Trust authentication models
- [ ] Design authentication system for container-based environments
- [ ] Evaluate OAuth2/OpenID Connect integration
- [ ] Plan migration from Kerberos to modern auth
- [ ] Create authentication service prototype

---

## Phase 2: Core Modernization

### 2.1 Kernel SMP Improvements
- [ ] Fix ARM SMP issues in arch/earm/arch_clock.c
- [ ] Implement CPU-local watchdog for all CPUs
- [ ] Add cacheline padding for CPU-local structures
- [ ] Fix timer synchronization across CPUs
- [ ] Test SMP functionality on multi-core ARM hardware

### 2.2 Modern Filesystem Implementation
- [ ] Begin ext4 filesystem driver implementation
- [ ] Implement basic ext4 features (read, write, directory operations)
- [ ] Add journaling support
- [ ] Implement ext4-specific features (extents, delayed allocation)
- [ ] Create ext4 test suite and validation

### 2.3 USB Stack Modernization
- [ ] Evaluate Linux USB stack for porting
- [ ] Design USB driver architecture for Minix
- [ ] Create USB core driver framework
- [ ] Implement USB host controller drivers (EHCI, xHCI)
- [ ] Port USB mass storage driver from Linux

### 2.4 VFS Performance Improvements
- [ ] Replace O(n) loop in smap.c with hash table
- [ ] Implement modern async I/O patterns
- [ ] Add lock-free data structures where appropriate
- [ ] Optimize path resolution in path.c
- [ ] Benchmark VFS performance improvements

### 2.5 Rust Component Development
- [ ] Rewrite string handling libraries in Rust
- [ ] Implement buffer parsing utilities in Rust
- [ ] Create network protocol parsers in Rust
- [ ] Develop Rust bindings for Minix system calls
- [ ] Port first userland utility to Rust

---

## Phase 3: Advanced Features

### 3.1 Package Manager Modernization
- [ ] Evaluate modern package managers (apk, dpkg, rpm)
- [ ] Choose package manager for Minix (recommend apk for embedded focus)
- [ ] Design package manager integration architecture
- [ ] Implement package manager integration
- [ ] Migrate existing packages to new system

### 3.2 Display and Graphics Infrastructure
- [ ] Enhance framebuffer driver support
- [ ] Add hardware acceleration (VESA, VBE)
- [ ] Design display server architecture (Wayland-based)
- [ ] Implement basic 2D graphics primitives
- [ ] Add font rendering engine

### 3.3 Input Device Support
- [ ] Implement keyboard driver with modern protocols
- [ ] Add mouse/touchpad driver support
- [ ] Create input device abstraction layer
- [ ] Implement hot-plug support for input devices
- [ ] Add multi-touch support

### 3.4 Window Management System
- [ ] Design window manager for microkernel architecture
- [ ] Implement window composition
- [ ] Add window decoration and theming
- [ ] Implement drag-and-drop functionality
- [ ] Add clipboard management

### 3.5 Advanced Rust Integration
- [ ] Implement Rust-based device drivers
- [ ] Create Rust-based service servers
- [ ] Develop memory-safe filesystem utilities
- [ ] Add Rust-based networking components
- [ ] Implement Rust security frameworks

---

## Phase 4: Cleanup & Deprecation

### 4.1 Legacy Code Removal
- [ ] Deprecate i386 architecture support
- [ ] Remove DDEKit-based USB drivers
- [ ] Remove legacy filesystems (LFS, UFS2, ext2, CHFS)
- [ ] Remove Kerberos authentication system
- [ ] Remove legacy package management tools

### 4.2 Documentation Updates
- [ ] Update all documentation to reflect modern architecture
- [ ] Create migration guides for users
- [ ] Document new APIs and interfaces
- [ ] Create developer onboarding guides
- [ ] Update system architecture documentation

### 4.3 Testing and Validation
- [ ] Comprehensive security audit of new components
- [ ] Performance benchmarking against legacy system
- [ ] Stress testing of new filesystem and USB stack
- [ ] Validation of Rust integration safety guarantees
- [ ] End-to-end testing of GUI components

---

## Detailed Component Analysis

### Components to Build (Modern Alternatives)

#### 1. Filesystem: ext4 Driver
**Why**: ext4 is the standard Linux filesystem, mature, well-documented, and widely supported
**Approach**: Port ext4 driver from Linux, adapt to Minix VFS layer
**Priority**: High (Q2 2026)
**Effort**: 3-4 months
**Dependencies**: VFS layer improvements

#### 2. USB Stack: Linux USB Stack Port
**Why**: Linux USB stack is mature, supports wide range of hardware, actively maintained
**Approach**: Port Linux USB core and host controller drivers to Minix
**Priority**: High (Q2 2026)
**Effort**: 4-5 months
**Dependencies**: Driver framework design

#### 3. Authentication: Zero Trust System
**Why**: Modern security model, suitable for container-based environments
**Approach**: Implement OAuth2/OpenID Connect service, integrate with system services
**Priority**: High (Q1 2026)
**Effort**: 2-3 months
**Dependencies**: None

#### 4. Package Manager: apk Integration
**Why**: Alpine's apk is lightweight, suitable for embedded systems, well-maintained
**Approach**: Integrate apk package manager, create Minix package repository
**Priority**: Medium (Q3 2026)
**Effort**: 2-3 months
**Dependencies**: None

#### 5. Display: Wayland Compositor
**Why**: Modern display server protocol, secure, compositing-friendly
**Approach**: Implement Wayland compositor for Minix, integrate with framebuffer
**Priority**: Medium (Q3 2026)
**Effort**: 4-6 months
**Dependencies**: Graphics infrastructure

#### 6. Rust Integration: Incremental Migration
**Why**: Memory safety, modern tooling, future-proof codebase
**Approach**: Start with userland utilities, move to drivers, then kernel components
**Priority**: High (Q1-Q4 2026)
**Effort**: Ongoing throughout 2026
**Dependencies**: Build system integration

### Components to Deprecate

#### 1. i386 Architecture Support
**Timeline**: Begin deprecation Q3 2026, complete removal 2027
**Reason**: Legacy architecture, modern systems use x86_64 and ARM64
**Migration Path**: Focus on x86_64 and ARM64 support

#### 2. DDEKit USB Framework
**Timeline**: Replace Q2 2026, remove Q4 2026
**Reason**: Unmaintained, API issues, limited hardware support
**Migration Path**: Port Linux USB stack

#### 3. Legacy Filesystems (LFS, UFS2, ext2, CHFS)
**Timeline**: Replace Q2-Q3 2026, remove Q4 2026
**Reason**: Obsolete, limited features, poor performance on modern hardware
**Migration Path**: Implement ext4 support

#### 4. Kerberos Authentication
**Timeline**: Replace Q1 2026, remove Q3 2026
**Reason**: Dying technology, unsuitable for modern container environments
**Migration Path**: Implement Zero Trust authentication

#### 5. Legacy Package Management (pkg_*, sysinst)
**Timeline**: Replace Q3 2026, remove Q4 2026
**Reason**: Custom system, limited features, not standard
**Migration Path**: Integrate apk package manager

---

## Success Metrics

### Phase 1
- CI/CD pipeline operational
- 80% code coverage on core components
- Rust toolchain integrated
- Migration roadmap complete

### Phase 2
- ARM SMP fully functional
- ext4 driver basic operations working
- USB core driver operational
- VFS performance improved by 30%
- First Rust component in production

### Phase 3
- Package manager integrated
- Basic GUI functional
- Input devices working
- Window manager operational
- 5+ Rust components in production

### Phase 4
- All legacy code removed
- Documentation complete
- Security audit passed
- Performance benchmarks met
- System fully modernized

---

## Notes

- This TODO focuses on concrete actions with clear deliverables
- Each phase builds on the previous one
- Regular reviews and adjustments recommended
- Community feedback should be incorporated
- Focus on incremental, testable changes
