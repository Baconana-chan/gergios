# Minix Microkernel Architecture Documentation

## Overview

Minix is a microkernel-based operating system that follows the principle of minimal kernel design, moving most operating system services to user-space servers.

## Core Components

### 1. Microkernel (minix/kernel/)

The microkernel provides only essential services:
- **Process Management**: Process creation, scheduling, and termination
- **Inter-Process Communication (IPC)**: Message passing between processes
- **Interrupt Handling**: Low-level interrupt processing
- **Memory Management**: Basic memory allocation and protection
- **Clock Management**: System timer and timekeeping
- **System Calls**: Interface for user-space to request kernel services

Key files:
- `main.c` - Kernel initialization and main loop
- `proc.c` - Process management and scheduling
- `ipc.h` - IPC definitions and message structures
- `system.c` - System call handling
- `interrupt.c` - Interrupt handling

### 2. System Servers (minix/servers/)

Services run as user-space processes:

#### Process Manager (PM)
- Location: `minix/servers/pm/`
- Responsibilities: Process creation, termination, signal handling
- Forks, execs, wait, signal management

#### Virtual Filesystem (VFS)
- Location: `minix/servers/vfs/`
- Responsibilities: Filesystem abstraction and management
- File operations, directory operations, pathname resolution

#### Memory Manager (VM)
- Location: `minix/servers/vm/`
- Responsibilities: Virtual memory management
- Page allocation, memory mapping, swap management

#### Information Service (IS)
- Location: `minix/servers/is/`
- Responsibilities: System information and statistics
- Process information, system statistics

#### Device Drivers
- Location: `minix/drivers/`
- Responsibilities: Hardware device management
- Block devices, character devices, network devices

### 3. Architecture Support (sys/arch/)

Current supported architectures:
- **i386** - 32-bit x86 (primary, fully supported)
  - Location: `sys/arch/i386/`
  - Status: Complete implementation with 242 items
  - Includes: kernel, boot, comprehensive driver support

- **x86** - Shared x86 architecture directory (partial x86_64 support)
  - Location: `sys/arch/x86/`
  - Status: Limited x86_64 support via conditional compilation
  - Details: Contains include files with `#ifdef __x86_64__` conditionals
  - Current state: Infrastructure exists but incomplete; no dedicated x86_64 architecture directory
  - Key files: cpu.h, pmap.h, db_machdep.h with x86_64 specific code paths
  - Limitations: Mixed with i386 code, no separate x86_64 build target

- **arm** - 32-bit ARM architecture (limited support)
  - Location: `sys/arch/arm/`
  - Status: 32-bit ARM support (ARMv4-ARMv7), 87 items
  - Details: Primarily 32-bit ARM with some aarch64 references in headers
  - Current state: Good 32-bit ARM infrastructure, minimal ARM64 support
  - Limitations: No dedicated ARM64/aarch64 architecture directory

- **evbarm** - ARM evaluation boards (limited support)
  - Location: `sys/arch/evbarm/`
  - Status: ARM evaluation board support, 56 items
  - Details: Platform-specific ARM board support
  - Current state: Board-specific configurations, relies on arm/ directory

**Architecture Analysis Summary**:
- The codebase has infrastructure for x86_64 and ARM64 but both are incomplete
- x86_64 support exists as conditional compilation within the x86 architecture directory
- ARM support is primarily 32-bit with minimal ARM64 infrastructure
- No dedicated architecture directories for x86_64 or ARM64 (aarch64)
- Current support is indeed "limited" as originally stated

### 4. Filesystems (minix/fs/)

Supported filesystems:
- **Minix filesystem** - Native Minix filesystem (versions 1, 2, 3)
- **ext2** - Linux ext2 filesystem
- **ISO9660** - CD-ROM filesystem
- **proc** - Virtual filesystem for process information
- **mfs** - Memory filesystem

### 5. Libraries (minix/lib/)

System libraries:
- **libminixfs** - Minix filesystem library
- **libsys** - System call library
- **libc** - Standard C library (NetBSD compatible)

## Communication Model

### Message Passing

Minix uses synchronous message passing for all IPC:
- Processes send messages to servers
- Servers process requests and send replies
- No shared memory between processes (except for special cases)

### System Call Flow

1. User process makes system call
2. Library traps to kernel
3. Kernel validates and forwards to appropriate server
4. Server processes request
5. Server replies to kernel
6. Kernel returns result to user process

## Memory Layout

### Kernel Space
- Reserved for microkernel
- Direct hardware access
- Protected from user processes

### User Space
- All servers and user processes
- Isolated from each other
- Communicate via IPC

## Boot Process

1. Bootloader loads kernel
2. Kernel initializes hardware
3. Kernel starts PM, VFS, VM servers
4. Servers initialize and register with kernel
5. Init process started
6. System services started

## Current Limitations

### Architecture Limitations
- Primary focus on i386 (32-bit)
- Limited x86_64 support
- ARM support is experimental
- No ARM64 (aarch64) support

### Technology Limitations
- Legacy C codebase (C89/C90)
- Limited use of modern C features
- No memory safety guarantees
- Manual memory management throughout

### Filesystem Limitations
- Minix filesystem is outdated
- Limited support for modern filesystems
- No support for ZFS, Btrfs, or modern features

### Driver Model
- Monolithic driver model within servers
- Limited driver support
- Outdated driver interfaces

## Security Model

### Process Isolation
- Each process runs in isolated address space
- Servers are isolated from each other
- Kernel provides protection mechanisms

### Privilege Levels
- User mode (Ring 3)
- Kernel mode (Ring 0)
- No intermediate privilege levels

### Access Control
- Unix-style permissions
- No capability-based security
- Limited mandatory access control

## Performance Characteristics

### Strengths
- Microkernel design provides modularity
- Server isolation improves reliability
- Message passing is predictable

### Weaknesses
- Message passing overhead
- Context switch overhead
- Limited optimization for modern hardware

## Dependencies

### External Dependencies
- NetBSD userland tools
- GNU toolchain (gcc, binutils)
- BSD make system

### Internal Dependencies
- Kernel depends on architecture-specific code
- Servers depend on kernel IPC
- Drivers depend on server interfaces

## Current Build System

- BSD make (bmake)
- Architecture-specific Makefiles
- Recursive make through directory tree
- Limited parallelization support

## Testing Infrastructure

- ATF (Automated Testing Framework)
- Kyua test runner
- Limited test coverage
- No integration tests

## Documentation Status

- Limited inline documentation
- Some man pages
- Outdated design documents
- No architecture documentation

## Key Design Decisions

### Microkernel Choice
- Minimizes trusted computing base
- Improves system reliability
- Enables modular development
- Trades performance for safety

### Message Passing
- Synchronous for simplicity
- No shared memory for safety
- Predictable behavior
- Higher overhead than shared memory

### Server Model
- Each service in separate process
- Clear separation of concerns
- Easier to debug and maintain
- More context switches

## Future Architecture Goals

1. **Modernize Architecture Support**
   - Full x86_64 support
   - ARM64 (aarch64) support
   - Deprecate i386

2. **Improve Performance**
   - Optimize message passing
   - Reduce context switch overhead
   - Add shared memory IPC where safe

3. **Enhance Security**
   - Capability-based security
   - Memory-safe language integration
   - Improved isolation

4. **Modernize Filesystem**
   - Support for modern filesystems
   - Better performance
   - More features

5. **Update Driver Model**
   - Modern driver interfaces
   - Better hardware support
   - Hot-plug support
