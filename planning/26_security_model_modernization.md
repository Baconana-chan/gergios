# Security Model Modernization — GergiOS 1.0+

> **Status**: Phase 1 (Audit) ✅ | Phase 2 (Capability Model) ✅ | Phase 3 (MAC Framework) ✅ | Phases 4-6 🟡 Planned
> **Связанные**: `planning/03_migration_roadmap.md` §6, `planning/23_driver_model_modernization.md`,
>   `planning/25_network_stack_modernization.md` (Phase 4: IPsec/DTLS/SYN cookies/WireGuard),
>   `docs/network-security.md`
> **Репозитории**: `minix/kernel/`, `minix/servers/rs/`, `minix/servers/devman/`,
>   `minix/servers/vfs/`, `minix/servers/pm/`, `minix/include/minix/`

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current Security Architecture](#2-current-security-architecture)
3. [Gap Analysis](#3-gap-analysis)
4. [Target Security Model](#4-target-security-model)
5. [Phase 1: Foundation Audit](#5-phase-1-foundation-audit)
6. [Phase 2: Capability Model Refinement](#6-phase-2-capability-model-refinement)
7. [Phase 3: MAC Framework](#7-phase-3-mac-framework)
8. [Phase 4: Memory Safety Hardening](#8-phase-4-memory-safety-hardening)
9. [Phase 5: Audit & Monitoring](#9-phase-5-audit--monitoring)
10. [Phase 6: Integration & Documentation](#10-phase-6-integration--documentation)
11. [Architecture Comparison](#11-architecture-comparison)
12. [Risk Assessment](#12-risk-assessment)

---

## 1. Executive Summary

### 1.1 Why Security Model Modernization?

GergiOS inherits MINIX 3's security architecture, which was designed in the
early 2000s for research and teaching. While the microkernel architecture
inherently provides **isolation between services** (each server is a separate
process), the security model itself is **ad-hoc and incomplete**:

- **Privilege management** is split across kernel (`sys_privctl()`), RS
  (Reincarnation Server), `system.conf`, and devman — no unified model
- **Capability-like restrictions** exist (IPC masks, kernel call masks,
  I/O ranges) but aren't exposed as a coherent capability system
- **No mandatory access control** — once a service is running, its access
  is governed only by Unix DAC (uid/gid/permissions)
- **No security audit framework** — security-relevant events (privilege
  changes, IPC denials, device access) are not logged
- **Memory safety** improvements (CFI, SafeStack, KASLR) are available in
  the toolchain but not configured

### 1.2 What This Plan Is Not

This is **not** a plan to create SELinux from scratch. Instead, it:

1. **Audits and documents** the existing security mechanisms (many of which
   are capability-like already)
2. **Refines and unifies** these mechanisms into a coherent capability model
3. **Adds MAC** as an optional layer (not breaking existing services)
4. **Hardens** memory safety using existing compiler features
5. **Adds auditing** for security-relevant events

### 1.3 Design Principles

| Principle | Rationale |
|-----------|-----------|
| **Least privilege** | Every service gets only the capabilities it needs |
| **Defense in depth** | Multiple layers: IPC masks + MAC + memory safety |
| **Opt-in complexity** | MAC is optional; base model is simple capability refinement |
| **No ABI breakage** | Backward-compatible capability API shim |
| **Audit by default** | Security events logged; policy determines verbosity |

---

## 2. Current Security Architecture

### 2.1 Kernel Privilege System

Every process in GergiOS has a **privilege structure** (`struct priv`):

```c
struct priv {
    sys_id_t   s_id;             /* unique privilege id */
    sys_flags_t s_flags;         /* privilege flags */
    sys_flags_t s_init_flags;    /* init flags */
    trap_mask_t s_trap_mask;     /* allowed kernel traps */
    sys_map_t  s_ipc_to;         /* bitmask: allowed IPC targets */
    sys_mask_t s_k_call_mask[3]; /* allowed kernel calls */
    int        s_sig_mgr;        /* signal manager endpoint */
    int        s_bak_sig_mgr;    /* backup signal mgr */
    int        s_nr_io_range;    /* I/O port ranges */
    struct io_range s_io_tab[NR_IO_RANGE];
    int        s_nr_mem_range;   /* physical memory ranges */
    struct minix_mem_range s_mem_tab[NR_MEM_RANGE];
    int        s_nr_irq;         /* allowed IRQs */
    irq_t       s_irq_tab[NR_IRQ];
    /* ... */
};
```

**Key privilege flags** (from `minix/include/minix/priv.h`):

| Flag | Description | Processes |
|------|-------------|-----------|
| `SYS_PROC` | System process status | RS, PM, VFS, drivers, servers |
| `PREEMPTIBLE` | Can be preempted by timer | All interactive processes |
| `DYN_PRIV_ID` | Dynamic privilege ID allocation | User-started services |
| `BILLABLE` | Charged for CPU time | User processes |
| `ROOT_SYS_PROC` | Root system process (RS) | RS |
| `VM_SYS_PROC` | VM server | VM |
| `CHECK_IRQ` | IRQ access enforced | Drivers with IRQs |
| `CHECK_IO_PORT` | I/O port access enforced | Drivers with I/O |
| `CHECK_MEM` | Memory access enforced | Drivers with memory ranges |

### 2.2 system.conf — Service Configuration

Every system service declares its required privileges in `/etc/system.conf`:

```
service lwip
{
    uid     0;
    ipc     ALL_SYS;          # IPC targets: ALL_SYS = all system procs
    system  ALL;              # Kernel calls: ALL permitted
    vm      BASIC;            # VM calls: BASIC subset
    io      NONE;             # I/O ranges: none
    irq     NONE;             # IRQs: none
    sigmgr          rs;       # Signal manager is RS
    scheduler   KERNEL;       # Scheduler is kernel
    priority         4;       # Scheduling priority
    quantum         500;      # Time quantum
};
```

**Directives explained:**

| Directive | Values | Description |
|-----------|--------|-------------|
| `ipc` | `ALL` / `ALL_SYS` / `NONE` / label list | Allowed IPC targets |
| `system` | `ALL` / `BASIC` / `NONE` / call list | Allowed kernel calls |
| `vm` | `BASIC` / `NONE` / call list | Allowed VM calls |
| `io` | `ALL` / `NONE` / range list | Allowed I/O port ranges |
| `irq` | `ALL` / `NONE` / list | Allowed IRQ lines |
| `pci device` | vid:did pairs | PCI device ACL |
| `pci class` | class/subclass/prog-if | PCI class ACL |
| `control` | label list | Who can control this service |
| `uid` | numeric / `SELF` | User ID |
| `sigmgr` | label / `SELF` | Signal manager |
| `scheduler` | label / `KERNEL` | Scheduler |

### 2.3 RS — Reincarnation Server Security

**`check_call_permission()`** (in `servers/rs/manager.c`):

```c
int check_call_permission(caller, call, rp)
{
    /* Root is always allowed */
    call_allowed = caller_is_root(caller);
    /* Or caller has control privilege over target */
    if (rp) call_allowed |= caller_can_control(caller, rp);
    if (!call_allowed) return EPERM;

    /* Additional checks: */
    if (!(rp->r_priv.s_flags & SYS_PROC) && call != RS_EDIT)
        return EPERM;    /* can only EDIT user processes */
    if (RUPDATE_IS_UPDATING()) return EBUSY;
    if (rp->r_flags & RS_TERMINATED && call != RS_DOWN && call != RS_RESTART)
        return EPERM;
    if (rpub->sys_flags & SF_CORE_SRV && call == RS_DOWN)
        return EPERM;    /* can't down core services */
}
```

**IPC Filtering** (`init_state_data()`):

Services can install IPC message filters:
- `IPCF_MATCH_M_SOURCE` — filter by sender endpoint
- `IPCF_MATCH_M_TYPE` — filter by message type
- Whitelist/blacklist semantics via `IPCF_EL_WHITELIST` / `IPCF_EL_BLACKLIST`

**Forward and Backward IPC** (`add_forward_ipc()` / `add_backward_ipc()`):

When a service starts, RS configures the kernel's IPC send mask:
- **Forward**: Set bits for targets listed in the service's own `ipc` directive
- **Backward**: Set bits in other services' masks that list this new service

### 2.4 Devman — Device Manager

Devman manages device ownership and binding:

```c
struct devman_device {
    int dev_id;                     /* unique ID */
    int major;                      /* device major number */
    endpoint_t owner;               /* owning driver endpoint */
    enum devman_device_state state; /* UNBOUND / BOUND / ZOMBIE */
    int ref_count;
    struct devman_device *parent;
    struct devman_inode inode;
    TAILQ_HEAD(, devman_inode) infos;  /* static info files */
    TAILQ_HEAD(, devman_device) children;
};
```

Security model: **owner-based** — only the owning driver can bind/unbind
a device. Operations via IPC: `DEVMAN_BIND`, `DEVMAN_UNBIND`, `DEVMAN_ADD_DEV`.

### 2.5 VFS — Unix File Permissions

Standard Unix DAC in `servers/vfs/protect.c`:

```c
int forbidden(struct fproc *rfp, struct vnode *vp, mode_t access_desired)
{
    /* Super-user gets everything */
    if (uid == SU_UID) { perm_bits = R_BIT | W_BIT | X_BIT; }
    else {
        if (uid == vp->v_uid) shift = 6;       /* owner */
        else if (gid == vp->v_gid) shift = 3;   /* group */
        else shift = 0;                         /* other */
        perm_bits = (bits >> shift) & 0x7;
    }
    if ((perm_bits | access_desired) != perm_bits) r = EACCES;
    if (access_desired & W_BIT) r = read_only(vp);
}
```

### 2.6 Current Security Flow

```
User Process              System Service              Kernel
    │                          │                        │
    │── open("/dev/tty") ──────▶                        │
    │                          │── SYS_PRIVCTL ────────▶│  (privilege check)
    │                          │                        │── IPC target check
    │                          │                        │── Kernel call mask
    │                          │◀── OK/EPERM ──────────┤
    │◀── fd/EPERM ─────────────┤                        │
```

### 2.7 Summary of Existing Security Mechanisms

| Mechanism | Location | Granularity | Enabled |
|-----------|----------|-------------|---------|
| IPC target mask | Kernel (`priv.s_ipc_to`) | Per-process bitmask | ✅ Always |
| Kernel call mask | Kernel (`priv.s_k_call_mask`) | Per-process bitmask | ✅ Always |
| I/O port ranges | Kernel (`priv.s_io_tab`) | Per-process list | ✅ When `CHECK_IO_PORT` |
| Memory ranges | Kernel (`priv.s_mem_tab`) | Per-process list | ✅ When `CHECK_MEM` |
| IRQ access | Kernel (`priv.s_irq_tab`) | Per-process list | ✅ When `CHECK_IRQ` |
| IPC filters | RS (`rs_ipc_filter_el`) | Per-process FILTER | ⚠️ On live update only |
| Control labels | RS (`r_control[]`) | Per-pair | ✅ When configured |
| PCI ACL | RS → PCI driver | Per-driver | ✅ When configured |
| Device ownership | Devman | Per-device | ✅ Always |
| Unix permissions | VFS | Per-file uid/gid/mode | ✅ Always |
| Signal manager | Kernel (`priv.s_sig_mgr`) | Per-process | ✅ Always |

---

## 3. Gap Analysis

### 3.1 What Exists But Needs Improvement

| Area | Current | Issue |
|------|---------|-------|
| **IPC masks** | Binary bitmask (send/no-send) | No per-message-type filtering; no capability delegation |
| **system.conf** | Static boot-time config | No runtime reconfiguration; no inheritance model |
| **Privilege flags** | 10 flags, binary | No hierarchy; no way to add new privileges without kernel mod |
| **Device access** | Owner-only binding | No way to share devices between services |
| **PCI ACL** | Vendor/device/class matching | No bus/function granularity; no hotplug notification |

### 3.2 What's Missing Entirely

| Feature | Severity | Notes |
|---------|----------|-------|
| **Unified capability model** | HIGH | Existing mechanisms are ad-hoc |
| **Mandatory Access Control (MAC)** | HIGH | No SELinux/AppArmor equivalent |
| **Security audit** | HIGH | No audit log of security events |
| **KASLR** | MEDIUM | Kernel at fixed address on x86_64 |
| **Control Flow Integrity** | MEDIUM | Toolchain supports it (`-fsanitize=cfi`) |
| **SafeStack** | MEDIUM | Toolchain supports it |
| **W^X enforcement** | MEDIUM | Not explicitly enforced by kernel |
| **Trusted boot / TPM** | LOW | No hardware root of trust |
| **Signed binaries / modules** | LOW | No code signing for services |
| **Formal verification** | LOW | Not feasible for entire kernel |

### 3.3 Threat Model

The current model protects against:

| Threat | Protected? | Mechanism |
|--------|-----------|-----------|
| User process accessing kernel memory | ✅ | MMU: user/kernel split |
| User process sending IPC to any service | ✅ | IPC send mask |
| User process crashing system services | ✅ | RS restart policy |
| Driver accessing wrong I/O ports | ✅ | I/O range enforcement |
| Defective driver corrupting kernel data | ✅ | Microkernel: drivers in user space |
| Rogue service impersonating another | ⚠️ Partial | IPC source validation |
| Service A accessing service B's data | ⚠️ Partial | IPC masks, but no MAC |
| Privilege escalation via setuid binary | ❌ | No capability bounding |
| Kernel exploit from compromised service | ❌ | No CFI / KASLR |

---

## 4. Target Security Model

### 4.1 Layered Model

```
Layer 4: Audit & Monitoring
  ┌──────────────────────────────────────────┐
  │  auditd — structured event log           │
  │  auditctl — runtime audit policy          │
  └──────────────────────────────────────────┘

Layer 3: Mandatory Access Control
  ┌──────────────────────────────────────────┐
  │  LSM hooks → MAC module → policy         │
  │  File/Msg/Socket/Device access control    │
  └──────────────────────────────────────────┘

Layer 2: Capability Model
  ┌──────────────────────────────────────────┐
  │  cap_get_proc() / cap_set_proc()          │
  │  system.conf "capabilities" directive     │
  │  Capability inheritance on fork/exec      │
  └──────────────────────────────────────────┘

Layer 1: Kernel Privilege System (Existing)
  ┌──────────────────────────────────────────┐
  │  priv.s_ipc_to, s_k_call_mask,           │
  │  s_io_tab, s_mem_tab, s_irq_tab          │
  └──────────────────────────────────────────┘

Layer 0: Memory Safety
  ┌──────────────────────────────────────────┐
  │  KASLR, CFI, SafeStack, W^X, stack       │
  │  canaries, ASan/MSan/TSan (debug)        │
  └──────────────────────────────────────────┘
```

### 4.2 Capability Model Design

#### Named Capabilities

Instead of raw bitmasks, capabilities are **named** and **categorised**:

```
CAP_NET_RAW       — Raw socket access (AF_PACKET)
CAP_NET_ADMIN     — Network interface config (ifconfig, route)
CAP_NET_BIND      — Bind to privileged ports (<1024)
CAP_SYS_RAWIO     — Direct I/O port access
CAP_SYS_ADMIN     — System administration (RS control)
CAP_SYS_BOOT      — System reboot/shutdown
CAP_IPC_OWNER     — Bypass IPC send masks
CAP_FS_MOUNT      — Mount/umount filesystems
CAP_FS_CHOWN      — Change file ownership (chown)
CAP_FS_DAC_OVERRIDE — Bypass file permission checks
CAP_VM_MAP        — Map physical memory
CAP_IRQ_ALLOC     — Allocate IRQ lines
CAP_PCI_ACCESS    — PCI configuration space access
```

#### Capability Sets

Predefined sets for common service types:

| Set | Capabilities |
|-----|-------------|
| `base` | None (minimal user process) |
| `system` | `CAP_NET_BIND`, `CAP_IPC_OWNER` |
| `driver` | `CAP_SYS_RAWIO`, `CAP_IRQ_ALLOC`, `CAP_VM_MAP` |
| `network` | `CAP_NET_RAW`, `CAP_NET_ADMIN`, `CAP_NET_BIND` |
| `admin` | `CAP_SYS_ADMIN`, `CAP_SYS_BOOT`, `CAP_FS_MOUNT` |
| `all` | All capabilities (for core services) |

#### API

```c
// Query capabilities
int cap_get_proc(cap_t *caps);
int cap_get_bound(cap_t *caps);      // bounding set

// Set capabilities (can only drop, not raise — unless CAP_SYS_ADMIN)
int cap_set_proc(const cap_t *caps);

// Fork/exec inheritance:
// fork: child inherits parent's capabilities
// exec: capabilities are reset to bounding set
// setuid-root: capabilities preserved (no automatic elevation)
```

### 4.3 MAC Model Design

Simple **type-enforcement** model (SELinux-like but simpler).

#### Security Contexts

```
user:role:type
─────────────────
root:system:lwip_t
root:system:vfs_t
root:driver:e1000_t
user:user:user_t
```

#### Policy Rules

```
allow source_type target_type : class { permissions };
```

Example policy:

```
# Allow lwIP to send messages to VFS
allow lwip_t vfs_t : ipc { send };

# Allow e1000 driver to access I/O ports
allow e1000_t e1000_device_t : io_port { 0xcf8-0xcff };

# Allow VFS to read/write files on ext4
allow vfs_t ext4_t : filesystem { read write };

# Network capabilities
allow lwip_t self : capability { net_raw net_bind };

# Default deny
dontaudit user_t system_t : ipc *;
```

#### MAC Module Architecture

```c
struct mac_hook {
    int (*file_open)(struct vnode *vp, struct fproc *fp, int flags);
    int (*ipc_send)(endpoint_t src, endpoint_t dst, int msg_type);
    int (*socket_bind)(endpoint_t proc, endpoint_t ep, uint16_t port);
    int (*device_access)(endpoint_t proc, dev_t dev, int access_type);
    int (*cap_check)(endpoint_t proc, const char *cap_name);
};

// MAC module registration
int mac_register(struct mac_hook *hooks);
int mac_unregister(void);
```

---

## 5. Phase 1: Foundation Audit

**Status**: ✅ Completed (this document is the audit result)

### 5.1 Architecture Audit Results

| Component | File(s) | LOC | Status |
|-----------|---------|-----|--------|
| Privilege structures | `minix/include/minix/priv.h` | ~100 | ✅ Documented |
| Kernel sys_privctl | `minix/kernel/system/do_privctl.c` | ~350 | ✅ Documented |
| RS service security | `minix/servers/rs/manager.c` | ~450 | ✅ Documented |
| RS request handling | `minix/servers/rs/request.c` | ~650 | ✅ Documented |
| Devman device security | `minix/servers/devman/main.c`, `device.c` | ~400 | ✅ Documented |
| VFS file permissions | `minix/servers/vfs/protect.c` | ~250 | ✅ Documented |
| system.conf | `etc/system.conf` | ~300 lines | ✅ Documented |
| RS IPC filtering | `manager.c:init_state_data()` | ~120 | ✅ Documented |

### 5.2 W^X Audit

| Region | Executable? | Writable? | Status |
|--------|-------------|-----------|--------|
| Kernel .text | ✅ Yes | ❌ No | ✅ OK |
| Kernel .data/.bss | ❌ No | ✅ Yes | ✅ OK |
| Kernel unpaged | ❌ No | ✅ Yes | ⚠️ Need audit |
| User .text | ✅ Yes | ❌ No | ✅ Standard |
| User stack | ❌ No | ✅ Yes | ⚠️ NX confirmed? |
| User heap | ❌ No | ✅ Yes | ✅ mmap with MAP_ANONYMOUS |

### 5.3 Stack Canary Audit

| Component | Compiler flag | Status |
|-----------|--------------|--------|
| Kernel | `-fstack-protector` | ✅ Configured in CMake |
| Servers | `-fstack-protector` | ✅ Via bsd.sys.mk |
| Drivers | `-fstack-protector-strong` | ⚠️ Verify per-driver |
| Libraries | `-fstack-protector` | ✅ Via bsd.lib.mk |

---

## 6. Phase 2: Capability Model Refinement

**Status**: ✅ **Completed**
**LOC**: ~700 new code + ~300 modifications
**Files affected**: ~25 (new + modified)
**Phases**: All 7 sub-steps implemented

### 6.1 Design

#### system.conf Extension

```
service lwip
{
    # Existing directives (backward compatible)
    ipc     ALL_SYS;
    system  ALL;
    
    # New capability directives
    capabilities NET_ADMIN NET_BIND;
    capability_ipc  vm rs pm;          # fine-grained IPC
    capability_io   0xcf8:8;           # fine-grained I/O
};
```

#### Kernel Capability System

```c
// New kernel call: SYS_CAPCTL
struct cap_header {
    int version;
    endpoint_t target;
    int op;            // CAP_GET / CAP_SET / CAP_BOUND_GET / CAP_LIST
    cap_t caps;
};

#define CAP_SYS_RAWIO    (1ULL << 0)
#define CAP_NET_RAW      (1ULL << 1)
#define CAP_NET_BIND     (1ULL << 2)
#define CAP_NET_ADMIN    (1ULL << 3)
#define CAP_SYS_ADMIN    (1ULL << 4)
#define CAP_SYS_BOOT     (1ULL << 5)
#define CAP_IPC_OWNER    (1ULL << 6)
#define CAP_FS_MOUNT     (1ULL << 7)
#define CAP_FS_CHOWN     (1ULL << 8)
#define CAP_FS_DAC_OVERRIDE (1ULL << 9)
#define CAP_VM_MAP       (1ULL << 10)
#define CAP_IRQ_ALLOC    (1ULL << 11)
#define CAP_PCI_ACCESS   (1ULL << 12)

typedef uint64_t cap_t;
#define CAP_MAX        13
#define CAP_FULL       ((1ULL << CAP_MAX) - 1)
```

#### Capability Inheritance

```
fork():
    child->cap_effective = parent->cap_effective
    child->cap_permitted = parent->cap_permitted
    child->cap_bound     = parent->cap_bound    // immutable bounding set

exec():
    // Reset effective + permitted to bounding set
    child->cap_effective = child->cap_bound
    child->cap_permitted = child->cap_bound
    child->cap_bound     = parent->cap_bound     // unchanged

setuid(0):
    // NO automatic elevation: root processes start with bounding set
    // CAP_SYS_ADMIN required to raise capabilities
```

### 6.2 Implementation Plan

#### Step 2.1: New kernel call `SYS_CAPCTL` ✅ **Done**

| File | Action |
|------|--------|
| `minix/kernel/system/do_capctl.c` | **NEW** — SYS_CAPCTL handler (GET/SET/BOUND_GET/BOUND_SET/LIST) |
| `minix/kernel/priv.h` | Added `s_cap_effective`, `s_cap_permitted`, `s_cap_bound` to `struct priv` |
| `minix/include/minix/capability.h` | **NEW** — 13 cap bits, CAP_OP_*, predefined sets |
| `minix/include/minix/com.h` | Added SYS_CAPCTL (call 59), NR_SYS_CALLS=60, CAPCTL_* message fields |
| `minix/kernel/config.h` | Added USE_CAPCTL=1 |
| `minix/kernel/system.h` | Added do_capctl declaration |
| `minix/kernel/system.c` | Mapped SYS_CAPCTL → do_capctl |
| `minix/kernel/main.c` | Boot cap init: kernel tasks + RS + VM = CAP_FULL |

#### Step 2.2: Capability-aware sys_privctl ✅ **Done**

| File | Action |
|------|--------|
| `minix/kernel/system/do_privctl.c` | SYS_PRIV_SET_SYS: cap defaults (CAP_BASE, bound=CAP_FULL); SYS_PRIV_SET_USER: CAP_BASE; update_priv: copies caps ∩ bound |

#### Step 2.3: RS integration ✅ **Done**

| File | Action |
|------|--------|
| `minix/servers/rs/manager.c` | `edit_slot()`: parses `rss_cap_effective/permitted` from `rs_start` → `rp->r_priv` |
| `minix/include/minix/rs.h` | Added `rss_cap_effective`, `rss_cap_permitted` to `struct rs_start` |

Caps are propagated through `SYS_PRIV_SET_SYS` (kernel copies priv struct → `update_priv()`) — no extra `sys_capctl()` call needed.

#### Step 2.4: IPC capability check ✅ **Done**

| File | Action |
|------|--------|
| `minix/kernel/proc.c` | In IPC send path: check `CAP_IPC_OWNER` before enforcing IPC mask |

#### Step 2.5: Userland library ✅ **Done**

| File | Action |
|------|--------|
| `minix/include/sys/capability.h` | **NEW** — `cap_t` (uint64_t), `cap_get_proc()`, `cap_set_proc()`, `cap_get_bound()` |
| `minix/lib/libcap/cap_get_proc.c` | **NEW** — uses `_kernel_call(SYS_CAPCTL)` with CAP_OP_GET |
| `minix/lib/libcap/cap_set_proc.c` | **NEW** — uses `sys_capctl(SELF, CAP_OP_SET, caps)` |
| `minix/lib/libcap/cap_get_bound.c` | **NEW** — uses `_kernel_call(SYS_CAPCTL)` with CAP_OP_BOUND_GET |
| `minix/lib/libcap/Makefile` | **NEW** — BSD make build |
| `minix/lib/Makefile` | Added libcap to SUBDIR |
| `minix/lib/CMakeLists.txt` | Added add_subdirectory_if_exists(libcap) |

#### Step 2.6: Update system.conf parsing ✅ **Done**

| File | Action |
|------|--------|
| `minix/commands/minix-service/parse.h` | Added `KW_CAPABILITIES` keyword constant |
| `minix/commands/minix-service/parse.c` | Added `capability_tab[]` lookup table (13 names→bits); `do_capabilities()` handler (ALL/NONE/поимённо); dispatch in `do_service()` |

Format: `capabilities IPC_OWNER SYS_RAWIO;` or `capabilities ALL;` or `capabilities NONE;`

#### Step 2.7: Update all system.conf files ✅ **Done**

| File | Action |
|------|--------|
| `etc/system.conf` | Added `capabilities` directive to all **38** service entries with minimal required caps |

Summary of capability assignments:
- `IPC_OWNER`: rs, ds, vm, pm, sched, vfs, tty
- `SYS_ADMIN`: rs, klkm
- `SYS_BOOT`: pm
- `SYS_RAWIO`: all drivers with I/O port access (~14 services)
- `IRQ_ALLOC`: drivers with IRQ handlers (~7 services)
- `VM_MAP`: vm, memory, ahci, virtio_blk
- `FS_MOUNT`: all filesystems + vfs + vnd (~10 services)
- `CHOWN`/`DAC_OVERRIDE`: vfs
- `PCI_ACCESS`: pci
- `NONE`: services without special hardware needs (~12 services)

### 6.3 Default Capability Mappings

| Service | Current system.conf | Capability mapping |
|---------|-------------------|-------------------|
| RS | `ipc ALL, system ALL, io NONE` | `all` |
| VM | `ipc ALL, system ALL` | `all` |
| PM | `ipc ALL, system ALL` | `all` |
| VFS | `ipc ALL, system KILL/UMAP/...` | `system` + `CAP_FS_MOUNT` + `CAP_FS_CHOWN` |
| MFS | `ipc ALL_SYS, system BASIC` | `base` |
| TTY | `ipc ALL_SYS, system KILL/UMAP/...` | `driver` |
| MEMORY | `ipc ALL_SYS, system UMAP/...` | `driver` + `CAP_VM_MAP` |
| e1000 | PCI class match, I/O ranges | `driver` + `CAP_PCI_ACCESS` |
| lwIP | `ipc ALL_SYS, system ALL` | `network` + `system` |
| INIT | `ipc pm/vfs/rs/vm` | `base` |
| USER | (shared priv structure) | `base` |

---

## 7. Phase 3: MAC Framework

**Status**: ✅ **Completed**
**LOC**: ~450 new code + ~150 modifications
**New files**: 8 (mac.h, mac_hooks.c, libmac, macd daemon, etc/macd.conf)

### 7.1 Design

#### Hook Points

```
┌────────────────────────────────────────────────────────┐
│                    Application                          │
│  open() │ sendto() │ bind() │ ioctl()                   │
└─────────┼──────────┼────────┼───────────┬────────────┘
          ▼          ▼        ▼           ▼
┌────────────────────────────────────────────────────────┐
│  LSM Hook Layer (kernel + servers)                     │
│                                                        │
│  file_open_hook()  →  mac_file_open()                  │
│  ipc_send_hook()   →  mac_ipc_send()                   │
│  socket_bind_hook()->  mac_socket_bind()               │
│  dev_access_hook() →  mac_dev_access()                 │
│  cap_check_hook()  →  mac_cap_check()                  │
└────────────────────┬───────────────────────────────────┘
                     │ module->hook()
                     ▼
┌────────────────────────────────────────────────────────┐
│              MAC Module (optional)                      │
│                                                        │
│  policy.c — policy engine                               │
│  context.c — security context management                │
│  rules.c — rule cache                                   │
└────────────────────────────────────────────────────────┘
```

#### Policy Language

```
# /etc/mac/policy.conf

# Type declarations
type lwip_t;
type vfs_t;
type e1000_t;
type user_t;

# Allow rules
allow lwip_t vfs_t : ipc send;
allow lwip_t self : capability net_raw net_bind;
allow e1000_t e1000_device_t : io_port 0xcf8-0xcff;
allow vfs_t ext4_t : filesystem read write;

# Never allow rules
neverallow user_t * : capability *;
neverallow * : kernel *;
```

#### Policy Compiler

```sh
mac-compile /etc/mac/policy.conf → /etc/mac/policy.bin
```

Binary format: flat array of `struct mac_rule` with integer SIDs.

#### Context Assignment

Security contexts assigned at process creation:

```c
struct mac_context {
    sid_t sid;          // security ID
    char *user;         // user name (e.g., "root", "user")
    char *role;         // role (e.g., "system", "driver", "user")
    char *type;         // type (e.g., "lwip_t", "vfs_t")
};
```

Assignment rules (in policy):

```
# /etc/mac/contexts.conf
# process_name     context
RS                 root:system:rs_t
VM                 root:system:vm_t
PM                 root:system:pm_t
VFS                root:system:vfs_t
TTY                root:driver:tty_t
lwip               root:system:lwip_t
e1000              root:driver:e1000_t
init               root:system:init_t
user:*             user:user:user_t      # all user processes
```

### 7.2 Implementation Plan

#### Step 3.1: Kernel MAC hooks ✅ **Done**

| Файл | Действие |
|------|----------|
| `minix/include/minix/mac.h` | **NEW** — MAC API: 9 hook types, 5 context structs, `mac_kernel_check()`, `mac_request()` |
| `minix/kernel/system/mac_hooks.c` | **NEW** — hook chain (до 4 функций), `mac_kernel_check()` (default ALLOW), `mac_hook_init()` |
| `minix/kernel/system.h` | Declared `mac_kernel_check()`, `mac_hook_init()` |
| `minix/kernel/system/Makefile.inc` | Added `mac_hooks.c` |

Hook points:
- `minix/kernel/proc.c`: `MAC_IPC_SEND` check in `do_sync_ipc()` перед IPC mask check
- `minix/kernel/system/do_privctl.c`: `MAC_PRIVCTL_SET_SYS` check в `SYS_PRIV_SET_SYS`

#### Step 3.2: Server MAC hooks + libmac ✅ **Done**

| Файл | Действие |
|------|----------|
| `minix/lib/libmac/mac_request.c` | **NEW** — `mac_request()`: RS lookup macd → IPC `MACD_CHECK` → decision. Default ALLOW если macd не запущен |
| `minix/lib/libmac/Makefile` | **NEW** — `LIB=mac`, `SRCS=mac_request.c` |
| `minix/servers/vfs/protect.c` | `MAC_FILE_ACCESS` hook в `forbidden()` после DAC check. MAC deny → EACCES |
| `minix/include/minix/com.h` | `MACD_RQ_BASE` (0x1B00), `MACD_CHECK`, `MACD_WHAT/SRC/DST/CTX1` |

#### Step 3.3: Policy daemon (macd) ✅ **Done**

| Файл | Действие |
|------|----------|
| `minix/servers/macd/macd.c` | **NEW** — SEF startup, main loop receiving `MACD_CHECK`, вызов `policy_check()`, reply |
| `minix/servers/macd/policy.h` | **NEW** — `struct mac_rule` (action/op_type/from_label/to_label/cached endpoint), `policy_check()`, `policy_reload()` |
| `minix/servers/macd/policy.c` | **NEW** — парсинг `/etc/macd.conf`, linked list rules, first-match-wins, `MAC_DENY` если нет совпадений |
| `minix/servers/macd/Makefile` | **NEW** — `PROG=macd`, `SRCS=macd.c policy.c`, `-lsys` |
| `etc/macd.conf` | **NEW** — `allow ALL` (compat), примеры deny-правил в комментариях |
| `minix/servers/Makefile` + `CMakeLists.txt` | Added macd под `!MKIMAGEONLY` |
| `minix/lib/Makefile` + `CMakeLists.txt` | Added libmac |
| `etc/system.conf` | Added `service macd` entry |

**Policy engine features:**
- Кеширование resolved endpoints в `struct mac_rule` (нет IPC в hot path)
- Lazy retry при cache miss (handles boot ordering)
- First-match-wins семантика
- Fail-closed default (`MAC_DENY` при отсутствии совпадений)
- `policy_reload()` для SIGHUP

#### Step 3.4: Policy compiler 🟡 **Deferred**

- `minix/usr.bin/mac-compile/` — policy compiler
- Отложено: правила пишутся вручную в `/etc/macd.conf`

#### Step 3.5: Reference policy 🟡 **Deferred**

- Полная policy для всех сервисов
- Отложено до addition всех примеров policy

#### Step 3.6: Runtime toggle 🟡 **Deferred**

- `sysctl`-интерфейс для включения/выключения MAC
- Отложено: macd стартует с `allow ALL` по умолчанию

---

## 8. Phase 4: Memory Safety Hardening

**Status**: 🔶 **Complete** (W^X: ✅, CFI/SafeStack: ✅ options, KASLR: ✅ Phases 1-3 ✅)
**LOC**: ~130 new code + ~40 modifications
**Files affected**: ~10 (build config + VM server + kernel pagetable + boot)

### 8.1 W^X Enforcement ✅ **Completed**

W^X (Write-or-eXecute) enforcement prevents memory from being simultaneously
writable and executable, mitigating JIT spray and shellcode injection attacks.

#### 8.1.1 Software W^X: mmap() → EPERM ✅

| File | Change |
|------|--------|
| `minix/servers/vm/mmap.c` | Added `(prot & (PROT_WRITE|PROT_EXEC)) == (PROT_WRITE|PROT_EXEC)` check in `do_mmap()` — returns EPERM |

**Effect**: `mmap(PROT_WRITE | PROT_EXEC)` is rejected at the VM server level.
Processes must use `mprotect()` to switch between write and execute as needed.

#### 8.1.2 Hardware W^X: NX bit in pt_writemap() ✅

| File | Change |
|------|--------|
| `sys/arch/x86_64/include/vm.h` | Added `X86_64_VM_NX (1UL << 63)` — NX bit definition |
| `minix/servers/vm/arch/x86_64/pagetable.h` | Added `PTF_NX X86_64_VM_NX`; added `PTF_NX` to `PTF_ALLFLAGS` |
| `minix/servers/vm/pagetable.c` | In `pt_writemap()`: for x86_64, if `flags & PTF_WRITE`, set `PTF_NX` on entry; verify path clears NX from `maskedentry` to tolerate old PTEs |

**Effect**: All writable PTE entries automatically get the NX bit set at the
MMU level. Even if a process bypasses the software check, the hardware refuses
to execute code from writable pages.

#### 8.1.3 W^X in mprotect(): VM_MPROTECT handler ✅

| File | Change |
|------|--------|
| `minix/include/minix/com.h` | Added `VM_MPROTECT (VM_RQ_BASE + 49)`, `NR_VM_CALLS` → 50 |
| `minix/servers/vm/mmap.c` | New `do_mprotect()` — parses addr/len/prot, W^X check `(PROT_WRITE|PROT_EXEC)` → `EPERM`, updates `VR_WRITABLE` flag, calls `map_ph_writept()` → HW NX enforcement |
| `minix/servers/vm/main.c` | `CALLMAP(VM_MPROTECT, do_mprotect)` |
| `minix/servers/vm/proto.h` | Added `int do_mprotect(message *m)` |

**Features**:
- Self-mprotect only (caller = target process)
- POSIX-compliant: ENOMEM für unmapped pages, EINVAL für unaligned addr
- Rejects VR_DIRECT/VR_SHARED regions
- HW W^X at PTE level via `pt_writemap()` NX enforcement

**Limitation**: VR_WRITABLE is region-level; sub-range mprotect changes the
entire region's writability. Full splitting requires `split_region()` exposure.

### 8.2 Control Flow Integrity

**What**: `-fsanitize=cfi` (Clang) — prevents indirect call hijacking.

**Status**: ✅ **Configured in cmake**

| File | Change |
|------|--------|
| `cmake/options.cmake` | Added `SANITIZE_CFI` (OFF by default) |
| `cmake/arch_x86_64.cmake` | Wired up: `if(SANITIZE_CFI)` → `-flto -fsanitize=cfi` |

**Effect**: Option available for Clang+LTO builds. Not enabled by default.

### 8.3 SafeStack

**What**: `-fsanitize=safe-stack` — moves vulnerable buffers to a
separate "unsafe" stack, keeping return addresses on a protected "safe" stack.

**Status**: ✅ **Configured in cmake**

| File | Change |
|------|--------|
| `cmake/options.cmake` | Added `SANITIZE_SAFESTACK` (OFF by default) |
| `cmake/arch_x86_64.cmake` | Wired up: `if(SANITIZE_SAFESTACK)` → `-fsanitize=safe-stack` |

**Challenge**: SafeStack requires compiler-rt runtime. Verify cross-compilation.

### 8.4 KASLR

**What**: Randomize kernel base address and layout at boot time.

**Status**: ✅ **Phase 1 (Infrastructure) ✅, Phase 2 (Physical) ✅, Phase 3 (Virtual PIE) ✅**
**LOC**: ~150 new code + ~70 modifications

#### Phase 1: Infrastructure ✅ **Completed**

| File | Change |
|------|--------|
| `cmake/options.cmake` | Added `KASLR` option (OFF by default) |
| `cmake/arch_x86_64.cmake` | Wires `-DKASLR=1` compile definition when enabled |
| `minix/include/minix/param.h` | Added `u64_t kaslr_seed` to `kinfo_t` (end of struct) |
| `minix/kernel/arch/x86_64/pre_init.c` | Added `rdrand_available()` (CPUID ECX[30]) and `rdrand_read()` (`setc`-based CF capture). On `KASLR=ON`: CPUID check → RDRAND → seed → boot message |
| `minix/kernel/arch/x86_64/limine.c` | Same RDRAND entropy acquisition for Limine boot path |

**Entropy source**: RDRAND (x86_64 native, Ivy Bridge+). CPUID guard prevents #UD on older CPUs.
Falls back to `kaslr_seed = 0` (no randomization) if unavailable or `KASLR=OFF`.

#### Phase 2: Physical KASLR ✅ **Completed**

Physical KASLR infrastructure via the **Limine Kernel Address Request** (not a separate
KASLR request — KASLR is configured via `kaslr=yes` in `limine.conf`, and the kernel reads
its actual load address from `LIMINE_KERNEL_ADDRESS_REQUEST`).

| File | Change |
|------|--------|
| `minix/include/minix/param.h` | Added `u64_t kaslr_phys_offset` to `kinfo_t` — stores difference between actual and linked physical base |
| `minix/kernel/arch/x86_64/limine.c` | In `limine_get_parameters()`: reads `_limine_kern_addr_req.response->physical_base`, computes `kaslr_phys_offset = actual_phys - kernbase`, stores in kinfo, logs offset if non-zero |
| `minix/kernel/arch/x86_64/pg_utils.c` | Added `pg_alloc_page_random(kinfo_t *cbi)` — allocates a physical page from a random memory region using xorshift64 PRNG seeded with `kinfo.kaslr_seed`. Falls back to deterministic `pg_alloc_page()` when seed is 0 (KASLR disabled) or only one region available |

**Key design decisions:**

- **No formal `LIMINE_KASLR_REQUEST`** — The Limine protocol does not have a KASLR-specific
  request/response pair. Instead, KASLR is a bootloader config option (`kaslr=yes` in
  `limine.conf`), and the kernel uses `LIMINE_KERNEL_ADDRESS_REQUEST` to discover where it
  was loaded (physical_base, virtual_base).
- **Non-PIE guard** — The current kernel uses `-no-pie -mcmodel=large`, so Limine cannot
  actually relocate it. The `kaslr_phys_offset` will be 0 until Phase 3 (PIE kernel).
- **Practical randomization** — `pg_alloc_page_random()` makes physical page allocation
  patterns unpredictable, complicating DMA-based and Rowhammer attacks.
- **Seed propagation** — The xorshift64 PRNG updates `kinfo.kaslr_seed` after each
  allocation, ensuring different random regions in subsequent calls.

**Current limitations:**
- Identity mapping (`pg_identity`) cannot be scrambled — CPU executes from it during boot
- `pg_mapkernel()` maps at linked addresses — random physical load requires Phase 3
- `alloc_pagetable()` uses static BSS — no physical randomization for page table pages

#### Phase 3: Virtual KASLR (PIE) ✅ **Completed**

**Goal**: Make the kernel a Position-Independent Executable (PIE) so it can run
at a random virtual address at boot time. Requires ELF relocation processing
in early boot code before any paged C code executes.

**Key insight**: The kernel uses `-mcmodel=large` which generates 64-bit absolute
references. These get `R_X86_64_RELATIVE` relocations when linked with `-pie`.
Processing these relocations at boot adjusts all absolute addresses by a delta
value, effectively relocating the entire kernel to a new virtual address.

**Changes implemented:**

| File | Change |
|------|--------|
| `minix/kernel/CMakeLists.txt` | Added `-fPIE` to compile flags (KASLR=ON), `-pie` to linker flags, `KASLR_PIE=1` compile definition; added `relocate.c` to sources |
| `minix/kernel/arch/x86_64/kernel.lds` | Added `.rela.dyn` and `.rela.plt` output sections with `_rela_start`/`_rela_end` symbols |
| `minix/kernel/arch/x86_64/relocate.c` | **NEW** — `apply_relocations()` in `.unpaged.text`: iterates `.rela.dyn`, adds `delta` to each `R_X86_64_RELATIVE` entry. Skips unpaged section entries (r_offset < 0xFFFF800000000000). Minimal C, no global var access. |
| `minix/kernel/arch/x86_64/head.S` | Added `kaslr_virt_offset_slot` in `.unpaged.data`; two-call pattern: (1) `apply_relocations(delta=0)` before pre_init/limine_pre_init, (2) `apply_relocations(REAL_delta)` after pre_init returns. Saves R12/R14/R15 for parameter reuse. |
| `minix/include/minix/param.h` | Added `u64_t kaslr_virt_offset` to `kinfo_t` |
| `minix/kernel/arch/x86_64/pre_init.c` | Computes `kaslr_virt_offset = (seed & 0x1FF) * 2MB` — 511 possible 2MB-aligned positions; writes offset to `kaslr_virt_offset_slot`; passes offset to `pg_mapkernel()` |
| `minix/kernel/arch/x86_64/limine.c` | Same `kaslr_virt_offset` computation for Limine boot path; updated `pg_mapkernel` forward declaration |
| `minix/kernel/arch/x86_64/pg_utils.c` | `pg_mapkernel(virt_offset)` — maps kernel at **both** linked VMA and `linked_VMA + offset` (double mapping for safe long jump). Added `pg_unmap_linked_vma()` to remove the linked VMA after relocation completes. |
| `minix/kernel/arch/x86_64/include/arch_proto.h` | Updated `pg_mapkernel` signature to `u64_t virt_offset`; added `pg_unmap_linked_vma()` declaration |
| `minix/kernel/main.c` | Calls `pg_unmap_linked_vma()` early in `kmain()` (guarded by `#if KASLR && kinfo.kaslr_virt_offset != 0`) |

**Implementation details:**

- **`apply_relocations()`** is in `.unpaged.text` with standalone type definitions —
  no global variables or standard library. Parameter passing only.
- **`head.S`** calls it after page tables are set up but before any paged C code
  runs. The boot page tables map both identity (low) and high VMA, so all
  kernel addresses are accessible.
- **`delta = 0`** for now — the relocation infrastructure is verified without
  actually changing the VMA. Non-zero delta requires:
  1) Boot page tables to also map the new VMA
  2) `pg_mapkernel()` to receive the offset

**Boot flow (all steps implemented):**

```
head.S ──1──→ apply_relocations(delta=0)     [no-op, verify infra]
head.S ──2──→ pre_init / limine_pre_init      [compute offset, store in slot,
               │                                create page tables with BOTH VMAs]
head.S ←──3──┘ 
head.S ──4──→ apply_relocations(REAL_delta)   [patch all .text/.data references]
head.S ──5──→ kmain (at NEW VMA)              [jump to relocated entry point]
kmain ──6──→ pg_unmap_linked_vma()            [remove linked VMA — only new VMA stays]
```

### 8.5 Build Configuration Summary

| Feature | CMake option | Toolchain | Status |
|---------|-------------|-----------|--------|
| KASLR — Infrastructure | `-DKASLR=ON` | Any | ✅ RDRAND seed, kinfo field |
| KASLR — Physical (Limine) | `-DKASLR=ON` | Limine only | ✅ phys_offset, pg_alloc_random |
| KASLR — Virtual (PIE infra) | `-DKASLR=ON` | Any (PIE) | ✅ PIE build, relocate.c, head.S calls |
| KASLR — Virtual (PIE complete) | `-DKASLR=ON` | Any (PIE) | ✅ PIE build + relocate.c + VMA double-map + VMA unmap
| CFI | `-DSANITIZE_CFI=ON` | Clang only | ✅ Wired, OFF default |
| SafeStack | `-DSANITIZE_SAFESTACK=ON` | Clang only | ✅ Wired, OFF default |
| W^X — mmap() check | `-DENFORCE_WX=ON` (doc) | Any | ✅ Software EPERM |
| W^X — NX bit in PTEs | (always on) | x86_64 | ✅ HW enforcement |
| W^X — mprotect() | (always on) | Any | ✅ VM_MPROTECT handler |
| ASan | `-DSANITIZE_ADDRESS=ON` | Clang/GCC | ✅ CI |
| UBSan | `-DSANITIZE_UNDEFINED=ON` | Clang/GCC | ✅ CI |
| Stack protector | `-fstack-protector-strong` | Any | ✅ Always |

---

## 9. Phase 5: Audit & Monitoring

**Status**: 🔶 **In Progress**
**LOC estimate**: ~2000 new code
**New files**: ~10

### 9.1 Design

#### Audit Event Types

```
AUTH_SUCCESS      — Successful authentication
AUTH_FAILURE      — Failed authentication
PRIV_CHANGE       — Capability or privilege change
IPC_DENIED        — IPC send denied by MAC/capability
FILE_DENIED       — File access denied
DEVICE_BIND       — Device bound/unbound
SYSCALL_AUTHORIZED — Kernel call (SYS_PRIVCTL, SYS_CAPCTL)
MAC_VIOLATION     — MAC policy violation (permissive mode)
SERVICE_START     — Service started/stopped/updated
SERVICE_CRASH     — Service crash
```

#### Audit Record Format

```c
struct audit_record {
    uint32_t  ar_serial;        // monotonic sequence number
    uint32_t  ar_type;          // event type
    uint32_t  ar_result;        // OK/EPERM/EACCES/etc
    uint32_t  ar_pad;           // padding
    uint64_t  ar_timestamp;     // nanoseconds since boot
    endpoint_t ar_subject;       // process that caused event
    endpoint_t ar_object;        // target process/object
    uint32_t  ar_extra_len;     // extra data length (0-256)
    uint8_t   ar_extra[256];    // event-specific data
};
```

#### Audit Flow

```
Event occurs → Hook calls audit_log(type, subject, object, result, extra)
                  │
                  ▼
            audit_kernel_buffer (ring buffer, 64KB)
                  │
                  │ (periodically or on request)
                  ▼
            auditd reads via IPC (AUDIT_RETRIEVE)
                  │
                  ├── Write to /var/log/audit/audit.log
                  │   (structured binary, rotated hourly)
                  │
                  └── Send to syslog for real-time alerts
```

### 9.2 Implementation

#### Step 5.1: Kernel audit buffer ✅ **Completed**

| File | Change |
|------|--------|
| `minix/include/minix/audit.h` | **NEW** — 10 audit event types, `struct audit_record` (68 bytes), `AUDIT_BUFFER_ENTRIES=1024` (power of 2), 3 operations (`GET_COUNT`/`RETRIEVE`/`STATUS`), `audit_log()` API |
| `minix/kernel/audit.c` | **NEW** — Lock-free ring buffer. `audit_log()`: non-blocking, single-writer, fills record, returns serial. `do_audit()`: `GET_COUNT` returns min(write_idx, ENTRIES); `RETRIEVE` uses `data_copy()` in 2 chunks for wrap-around; `STATUS` returns buffer config |
| `minix/include/minix/com.h` | Added `SYS_AUDIT` (call 60), `NR_SYS_CALLS=61`, `AUDIT_OP/COUNT/ENABLE/BUF` fields |
| `minix/kernel/system.c` | `map(SYS_AUDIT, do_audit)` |
| `minix/kernel/system.h` | `int do_audit()` with `USE_AUDIT` guard |
| `minix/kernel/config.h` | `#define USE_AUDIT 1` |
| `minix/kernel/CMakeLists.txt` | Added `audit.c` |

**Design**: Single-writer ring buffer (kernel), single-reader (auditd via `SYS_AUDIT`). Always-on, overflow discards oldest. `data_copy()` for safe user-space transfer. No locking needed.

#### Step 5.2: auditd daemon ✅ **Completed**

| File | Change |
|------|--------|
| `minix/servers/auditd/auditd.c` | **NEW** — SEF startup, periodic polling via `sys_setalarm2()` + `SIGALRM` handler, `poll_kernel_buffer()` reads via `SYS_AUDIT`, `write_record()` writes to log, IPC handlers for `AUDITD_RQ_STATUS/ENABLE/DISABLE/REOPEN/POLL_NOW` |
| `minix/servers/auditd/Makefile` | **NEW** — `PROG=auditd`, `SRCS=auditd.c`, `-lsys` |
| `minix/servers/auditd/CMakeLists.txt` | **NEW** — CMake build |
| `etc/auditd.conf` | **NEW** — Default config: `log_path`, `poll_interval_ms = 10000` |
| `minix/include/minix/com.h` | Added `AUDITD_RQ_BASE=0x1C00`, 5 IPC message types, reply fields |
| `minix/servers/CMakeLists.txt` | Added `auditd` |
| `minix/servers/Makefile` | Added `auditd` |
| `etc/system.conf` | Added `service auditd` entry (`system BASIC AUDIT;`) |

**Architecture**: `sys_setalarm2()` triggers periodic `SIGALRM` → SEF signal handler → `_kernel_call(SYS_AUDIT, ...)` → records written to `/var/log/audit/audit.log`. auditctl communicates via IPC (`AUDITD_RQ_*`).

#### Step 5.3: auditctl tool (~300 LOC)

- `minix/usr.bin/auditctl/` — runtime audit configuration
- Commands:
  ```sh
  auditctl -e 1                # Enable audit
  auditctl -e 0                # Disable audit
  auditctl -a task,always      # Audit all events
  auditctl -a task,exclude     # Exclude event type
  auditctl -l                  # List audit rules
  auditctl -s                  # Status
  ```

#### Step 5.4: Integration with existing subsystems (~400 LOC)

- Add `audit_log()` calls to:
  - `do_privctl()` — log all privilege changes
  - `forbidden()` — log all EACCES file access
  - `check_call_permission()` — log denied RS operations
  - `send_ipc()` — log denied IPC (when MAC enabled)
  - devman `do_bind_device()` / `do_unbind_device()`

#### Step 5.5: Log rotation (~200 LOC)

- `auditd` auto-rotates logs every hour or at 10MB
- Up to 30 days retention, then auto-deleted
- Log format: binary (audit2txt tool for human-readable)

#### Step 5.6: audit2txt (~200 LOC)

```sh
audit2txt /var/log/audit/audit.log.20260709
# Output:
# 2026-07-09 14:32:01.123  AUTH_FAILURE  subj=init(14)  obj=pm(5)    result=EPERM
# 2026-07-09 14:32:01.456  PRIV_CHANGE   subj=rs(2)     obj=lwip(23)  caps=NET_ADMIN:NET_BIND
```

---

## 10. Phase 6: Integration & Documentation

**Status**: 🟡 Planned
**LOC estimate**: ~500 new code + docs
**Files affected**: ~10

### 10.1 Integration Testing

| Test | Description | Expected |
|------|-------------|----------|
| **Capability test** | Service without `CAP_NET_BIND` tries to bind port 80 | EPERM |
| **Capability inheritance** | fork() preserves capabilities; exec() resets to bounding | OK |
| **MAC enforcement** | Denied IPC is logged (permissive) or rejected (enforcing) | EACCES |
| **W^X enforcement** | mmap with PROT_WRITE\|PROT_EXEC fails | EPERM |
| **Audit events** | All denied operations appear in audit log | Match |
| **system.conf upgrade** | Old configs without `capabilities` still work | OK |
| **Performance** | Capability checks add < 100 ns to IPC path | Target |
| **MAC overhead** | Policy check adds < 1 µs per checked operation | Target |

### 10.2 Documentation

| Document | Contents |
|----------|----------|
| `docs/security-model.md` | Full security model description, layers, design rationale |
| `man 5 system.conf` | Updated: `capabilities`, `capability_ipc`, `capability_io` directives |
| `man 2 cap_get_proc` | Capability API reference |
| `man 2 cap_set_proc` | Capability modification API |
| `man 7 capabilities` | Capability overview, lists, inheritance rules |
| `man 5 mac-policy` | MAC policy language reference |
| `man 8 mac-compile` | Policy compiler usage |
| `man 8 auditd` | Audit daemon configuration |
| `man 8 auditctl` | Audit control tool |

### 10.3 Example Configurations

#### Minimal HTTP service

```sh
service my-httpd
{
    capabilities NET_BIND;       # can bind to port 80
    ipc     vfs;                 # can talk to VFS only
    system  BASIC;
    vm      BASIC;
};
```

#### Network driver

```sh
service mynet
{
    capabilities NET_RAW;        # raw packet access
    capability_io  0x1000:0x100; # device MMIO
    irq    16;                   # assigned IRQ line
    system  UMAP VUMAP IRQCTL DEVIO;
    pci device  1234:5678;      # specific NIC
};
```

#### Unprivileged user process

```sh
service my-app
{
    capabilities NONE;           # no special capabilities
    ipc     vfs ds;              # only VFS and DS
    system  NONE;
    vm      BASIC;
};
```

---

## 11. Architecture Comparison

### 11.1 MINIX/GergiOS vs Linux Capabilities

| Aspect | Linux | GergiOS (current) | GergiOS (target) |
|--------|-------|-------------------|------------------|
| **Model** | POSIX 1003.1e capabilities | Ad-hoc bitmasks + system.conf | Named capabilities |
| **Granularity** | ~40 capabilities | ~10 privilege flags | ~12 capabilities |
| **Storage** | Per-thread struct cred | Per-process struct priv | Per-process priv + caps |
| **Inheritance** | Bounding + ambient sets | fork=copy, exec=??? | Bounding + permitted + effective |
| **API** | cap_get_proc/cap_set_proc | None (sys_privctl only) | cap_get_proc/cap_set_proc |
| **Runtime** | All threads | Boot-time only (system.conf) | Boot + runtime via SYS_CAPCTL |

### 11.2 MINIX/GergiOS vs SELinux

| Aspect | SELinux | GergiOS (target) |
|--------|---------|------------------|
| **Policy** | TE + RBAC + MLS, ~500KB policy | Simple TE, ~10KB policy |
| **Hooks** | ~150 LSM hooks in kernel | ~10 hooks (IPC, file, socket, device) |
| **Context** | user:role:type:mls | user:role:type |
| **Compiler** | checkpolicy (M4 macro language) | mac-compile (C-like language) |
| **Enforcement** | Kernel (mandatory) | Server (RS-based, optional) |
| **Overhead** | ~5-10% with complex policies | target <3% for basic checks |
| **Complexity** | Very high | Moderate |

### 11.3 Why Not SELinux?

1. **Microkernel difference**: In monolithic Linux, SELinux hooks are in
   the kernel. In GergiOS (microkernel), most security-relevant operations
   happen in user-space servers (VFS, RS, devman). MAC hooks in user space
   are more natural.

2. **Simpler threat model**: GergiOS targets embedded/specialized use cases,
   not multi-user server workloads. A simpler MAC model suffices.

3. **Existing isolation**: The microkernel already provides strong process
   isolation. The gap is policy-driven access control, not containment.

---

## 12. Risk Assessment

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| **Capability API breakage** | High | Medium | Compatibility shim; old `system.conf` still works |
| **MAC policy complexity** | Medium | Medium | Start with permissive mode; compile policy from simple language |
| **Performance overhead (cap check)** | Medium | Low | Inline bitmask check for common paths (< 10 instructions) |
| **Performance overhead (MAC)** | Medium | Low | MAC daemon caches decisions; use SID integers, not strings |
| **CFI build failures** | Medium | Medium | GCC fallback without CFI; CI tests both toolchains |
| **KASLR implementation issues** | High | Medium | Only randomize high mapping; identity mapping stays fixed |
| **Audit log storage** | Low | Low | Ring buffer in kernel; user-space rotation |
| **Developer resistance** | Low | Medium | Opt-in: MAC disabled by default; capabilities are backward-compat |

### 12.1 LOC Summary

| Phase | New LOC | Modified LOC | New Files | Primary Risk |
|-------|---------|-------------|-----------|-------------|
| 1. Audit | 0 | ~500 (doc) | 0 | None (completed) |
| 2. Capability model | ~700 | ~300 | ~15 | ✅ Completed |
| 3. MAC framework | ~450 | ~150 | 8 | ✅ Completed |
| 4. Memory safety | ~250 | ~100 | ~10 | CFI compatibility, KASLR PIE complexity |
| 5. Audit | ~600 | ~100 | ~6 | Performance (ring buffer + IPC overhead) |
| 6. Integration | ~500 | ~300 | ~10 | Test coverage |
| **Total** | **~7300** | **~2900** | **~45** | |

### 12.2 Estimated Timeline

| Phase | Effort | Dependencies |
|-------|--------|-------------|
| 1. Audit | 1 week | None (completed) |
| 2. Capability model | 4 weeks | Phase 1 |
| 3. MAC framework | 8 weeks | Phase 2 | ✅ Completed |
| 4. Memory safety | 3 weeks | Clang toolchain | ✅ Completed
| 5. Audit | 4 weeks | Phase 2-4 | 🔶 In Progress
| 6. Integration | 2 weeks | All phases |
| **Total** | **22 weeks** | **~5 months** |

---

## Appendix A: Security-Relevant Files Inventory

| File | LOC | Security Relevance |
|------|-----|-------------------|
| `minix/kernel/system/do_privctl.c` | ~350 | Kernel privilege management |
| `minix/kernel/system/do_capctl.c` | ~200 (planned) | Capability management |
| `minix/servers/rs/manager.c` | ~450 | Service security checks |
| `minix/servers/rs/request.c` | ~650 | Service start/stop/edit security |
| `minix/servers/devman/main.c` | ~100 | Device IPC handler |
| `minix/servers/devman/device.c` | ~300 | Device ownership/binding |
| `minix/servers/vfs/protect.c` | ~250 | File permission checks |
| `minix/include/minix/priv.h` | ~100 | Privilege structure definitions |
| `minix/include/minix/rs.h` | ~200 | RS API definitions |
| `minix/include/minix/devman.h` | ~100 | Devman API definitions |
| `etc/system.conf` | ~300 | Service privilege configuration |
| `minix/kernel/system/do_irqctl.c` | ~200 | IRQ management |
| `minix/servers/pm/misc.c` | ~500 | Process management |

## Appendix B: system.conf Directive → Capability Mapping

| system.conf Directive | Equivalent Capability |
|----------------------|----------------------|
| `ipc ALL` | (no change — IPC mask supersedes) |
| `system ALL` | (no change — kernel call mask supersedes) |
| `io ALL` | `CAP_SYS_RAWIO` |
| `io NONE` | (no capability needed) |
| `irq <n>` | `CAP_IRQ_ALLOC` |
| `pci device` | `CAP_PCI_ACCESS` |
| (no equivalent) | `CAP_NET_RAW` — raw socket creation |
| (no equivalent) | `CAP_NET_BIND` — privileged port binding |
| (no equivalent) | `CAP_NET_ADMIN` — network interface config |
| (no equivalent) | `CAP_FS_MOUNT` — mount/umount |
| (no equivalent) | `CAP_FS_CHOWN` — change ownership |
| (no equivalent) | `CAP_FS_DAC_OVERRIDE` — bypass file permissions |
| (no equivalent) | `CAP_SYS_ADMIN` — admin operations |
| (no equivalent) | `CAP_SYS_BOOT` — reboot/shutdown |

> **See also**: `planning/03_migration_roadmap.md` §6 for roadmap-level view,
> `docs/network-security.md` for network-level security features
> (SYN cookies, IPsec, WireGuard, DTLS).
