# Security Model — GergiOS

> **Last updated**: July 2026
> **Status**: ✅ Complete (all 6 phases)
> **Related**: `planning/26_security_model_modernization.md`, `docs/network-security.md`

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Layer 0: Memory Safety](#2-layer-0-memory-safety)
3. [Layer 1: Kernel Privilege System](#3-layer-1-kernel-privilege-system)
4. [Layer 2: Capability Model](#4-layer-2-capability-model)
5. [Layer 3: MAC Framework](#5-layer-3-mac-framework)
6. [Layer 4: Audit & Monitoring](#6-layer-4-audit--monitoring)
7. [Integration Points](#7-integration-points)
8. [Configuration](#8-configuration)
9. [Tools Reference](#9-tools-reference)
10. [Example Configurations](#10-example-configurations)
11. [Troubleshooting](#11-troubleshooting)
12. [FAQ](#12-faq)

---

## 1. Architecture Overview

GergiOS implements a **layered security model** built on the MINIX 3 microkernel
architecture. Each layer provides defense-in-depth, with opt-in complexity:

```
Layer 4: Audit & Monitoring
  ┌──────────────────────────────────────────┐
  │  auditd — structured event log           │
  │  auditctl — runtime audit control        │
  │  audit2txt — log viewer                  │
  └──────────────────────────────────────────┘

Layer 3: Mandatory Access Control
  ┌──────────────────────────────────────────┐
  │  macd — policy daemon                    │
  │  macctl — runtime enforcement toggle     │
  │  mac-compile — policy compiler           │
  └──────────────────────────────────────────┘

Layer 2: Capability Model
  ┌──────────────────────────────────────────┐
  │  cap_get_proc() / cap_set_proc()         │
  │  system.conf "capabilities" directive    │
  │  SYS_CAPCTL kernel call                  │
  └──────────────────────────────────────────┘

Layer 1: Kernel Privilege System (Existing)
  ┌──────────────────────────────────────────┐
  │  IPC send masks                          │
  │  Kernel call masks                       │
  │  I/O / memory / IRQ ranges              │
  └──────────────────────────────────────────┘

Layer 0: Memory Safety
  ┌──────────────────────────────────────────┐
  │  W^X enforcement                         │
  │  KASLR (PIE kernel)                      │
  │  CFI / SafeStack (optional)              │
  │  Stack canaries                          │
  └──────────────────────────────────────────┘
```

### 1.1 Design Principles

| Principle | Rationale |
|-----------|-----------|
| **Least privilege** | Every service gets only the capabilities it needs |
| **Defense in depth** | Multiple layers: IPC masks + capabilities + MAC + audit |
| **Opt-in complexity** | MAC is optional; base model is capability refinement only |
| **No ABI breakage** | Old `system.conf` files without `capabilities` still work |
| **Audit by default** | Security events logged; policy determines verbosity |

### 1.2 Threat Model

GergiOS protects against:

| Threat | Layer | Mechanism |
|--------|-------|-----------|
| User process accessing kernel memory | 0 | MMU user/kernel split |
| User process sending IPC to any service | 1 | IPC send mask |
| Service impersonation | 1, 3 | IPC source validation + MAC |
| Driver accessing wrong I/O ports | 1, 2 | I/O range enforcement + capabilities |
| Privilege escalation via setuid binary | 2 | Capability bounding set |
| Defective driver corrupting kernel data | 0 | Microkernel isolation |
| Kernel exploit from compromised service | 0 | KASLR, W^X, CFI |
| Unauthorised file access | 2, 3 | Capabilities + MAC file hooks |

---

## 2. Layer 0: Memory Safety

### 2.1 W^X Enforcement

**Write-or-eXecute** prevents memory from being simultaneously writable and
executable, mitigating JIT spray and shellcode injection attacks.

**Triple enforcement:**

| Level | Location | Mechanism |
|-------|----------|-----------|
| Software | `VM server (mmap.c)` | `mmap(PROT_WRITE\|PROT_EXEC)` → `EPERM` |
| Hardware | `Pagetable (pt_writemap())` | NX bit (bit 63) set on all writable PTEs |
| API | `VM_MPROTECT` | `mprotect()` rejects W+X combinations |

### 2.2 KASLR (Kernel Address Space Layout Randomisation)

**Three-phase implementation:**

| Phase | What | Mechanism |
|-------|------|-----------|
| Infrastructure | RDRAND entropy seed | CPUID guard, `kaslr_seed` in `kinfo_t` |
| Physical | Random page allocation | `pg_alloc_page_random()` with xorshift64 |
| Virtual (PIE) | Random kernel VMA | 511 possible 2MB-aligned positions |

The PIE kernel uses ELF relocations (`R_X86_64_RELATIVE`) to adjust all absolute
addresses by a random delta at boot time. Boot flow:

```
head.S → apply_relocations(delta=0)     [verify infrastructure]
       → pre_init()                     [compute offset, create page tables]
       → apply_relocations(REAL_delta)  [patch all addresses]
       → kmain()                        [at randomised VMA]
       → pg_unmap_linked_vma()          [remove original VMA]
```

### 2.3 Optional Memory Safety

| Feature | CMake Flag | Toolchain | Effect |
|---------|-----------|-----------|--------|
| CFI | `-DSANITIZE_CFI=ON` | Clang+LTO | Prevents indirect call hijacking |
| SafeStack | `-DSANITIZE_SAFESTACK=ON` | Clang | Separate safe/unsafe stacks |
| ASan | `-DSANITIZE_ADDRESS=ON` | Clang/GCC | Buffer overflow detection |
| UBSan | `-DSANITIZE_UNDEFINED=ON` | Clang/GCC | Undefined behaviour detection |
| Stack protector | Always on | Any | `-fstack-protector-strong` |

---

## 3. Layer 1: Kernel Privilege System

### 3.1 Privilege Structure

Every process has a `struct priv` containing:

```c
struct priv {
    sys_id_t     s_id;           /* unique privilege ID */
    sys_flags_t  s_flags;        /* SYS_PROC, PREEMPTIBLE, etc. */
    trap_mask_t  s_trap_mask;    /* allowed kernel traps */
    sys_map_t    s_ipc_to;       /* allowed IPC targets (bitmask) */
    sys_mask_t   s_k_call_mask[3]; /* allowed kernel calls */
    int          s_sig_mgr;      /* signal manager endpoint */
    int          s_nr_io_range;  /* I/O port ranges */
    struct io_range s_io_tab[NR_IO_RANGE];
    int          s_nr_mem_range; /* physical memory ranges */
    struct minix_mem_range s_mem_tab[NR_MEM_RANGE];
    int          s_nr_irq;       /* allowed IRQs */
    irq_t        s_irq_tab[NR_IRQ];
};
```

### 3.2 Key Privilege Flags

| Flag | Meaning |
|------|---------|
| `SYS_PROC` | System process (server/driver) |
| `PREEMPTIBLE` | Can be preempted |
| `DYN_PRIV_ID` | Dynamic privilege ID |
| `ROOT_SYS_PROC` | Root system process (RS) |
| `VM_SYS_PROC` | VM server |
| `CHECK_IRQ` | Enforce IRQ access |
| `CHECK_IO_PORT` | Enforce I/O port access |
| `CHECK_MEM` | Enforce memory range access |

### 3.3 IPC Send Masks

The kernel enforces a per-process bitmask (`s_ipc_to`) that controls which
other processes a service can send IPC messages to. Configured via the `ipc`
directive in `system.conf`.

---

## 4. Layer 2: Capability Model

### 4.1 Named Capabilities

13 named capabilities, stored as a 64-bit bitmask:

| Capability | Bit | Description |
|------------|-----|-------------|
| `CAP_SYS_RAWIO` | 0 | Direct I/O port access |
| `CAP_NET_RAW` | 1 | Raw socket access (AF_PACKET) |
| `CAP_NET_BIND` | 2 | Bind to privileged ports (<1024) |
| `CAP_NET_ADMIN` | 3 | Network interface configuration |
| `CAP_SYS_ADMIN` | 4 | System administration (RS control) |
| `CAP_SYS_BOOT` | 5 | System reboot/shutdown |
| `CAP_IPC_OWNER` | 6 | Bypass IPC send masks |
| `CAP_FS_MOUNT` | 7 | Mount/umount filesystems |
| `CAP_FS_CHOWN` | 8 | Change file ownership |
| `CAP_FS_DAC_OVERRIDE` | 9 | Bypass file permission checks |
| `CAP_VM_MAP` | 10 | Map physical memory |
| `CAP_IRQ_ALLOC` | 11 | Allocate IRQ lines |
| `CAP_PCI_ACCESS` | 12 | PCI configuration space access |

### 4.2 Capability Sets

| Set | Capabilities | Use Case |
|-----|-------------|----------|
| `base` | None | Minimal user process |
| `system` | `CAP_NET_BIND`, `CAP_IPC_OWNER` | Standard server |
| `driver` | `CAP_SYS_RAWIO`, `CAP_IRQ_ALLOC`, `CAP_VM_MAP` | Device driver |
| `network` | `CAP_NET_RAW`, `CAP_NET_ADMIN`, `CAP_NET_BIND` | Network service |
| `admin` | `CAP_SYS_ADMIN`, `CAP_SYS_BOOT`, `CAP_FS_MOUNT` | Admin tool |
| `all` | All 13 | Core services (RS, VM, PM) |

### 4.3 Inheritance Rules

```
fork():
    child->cap_effective = parent->cap_effective
    child->cap_permitted = parent->cap_permitted
    child->cap_bound     = parent->cap_bound

exec():
    child->cap_effective = child->cap_bound
    child->cap_permitted = child->cap_bound
    child->cap_bound     = parent->cap_bound

setuid(0):
    NO automatic elevation. CAP_SYS_ADMIN required.
```

### 4.4 API

```c
#include <sys/capability.h>

// Query capabilities
int cap_get_proc(cap_t *caps);
int cap_get_bound(cap_t *caps);

// Set capabilities (can only drop — requires CAP_SYS_ADMIN to raise)
int cap_set_proc(const cap_t *caps);
```

### 4.5 system.conf Directive

```sh
service myapp
{
    capabilities NET_BIND IPC_OWNER;  # named capabilities
    # OR:
    capabilities ALL;                  # all 13 capabilities
    # OR:
    capabilities NONE;                 # no capabilities (default for user procs)
}
```

---

## 5. Layer 3: MAC Framework

### 5.1 Architecture

The MAC (Mandatory Access Control) framework uses a **policy daemon** (macd)
that is consulted by kernel and server hooks:

```
Application (open, sendto, bind, ioctl)
    │
    ▼
LSM Hook Layer (kernel + servers)
    │  mac_kernel_check(MAC_IPC_SEND, src, dst, ...)
    │  mac_request(MAC_FILE_ACCESS, proc, vnode, ...)
    ▼
macd daemon
    │  policy_check(ctx_from, ctx_to, operation)
    ▼
    ├── MAC_ALLOW  → operation proceeds
    └── MAC_DENY   → operation blocked (EACCES)
         │
         ▼
    audit_log(AUDIT_MAC_VIOLATION, ...)
```

### 5.2 Hook Types

| Hook | Location | Purpose |
|------|----------|---------|
| `MAC_IPC_SEND` | Kernel `proc.c` | IPC send between processes |
| `MAC_PRIVCTL_SET_SYS` | Kernel `do_privctl.c` | Privilege elevation |
| `MAC_FILE_ACCESS` | VFS `protect.c` | File open/read/write |
| `MAC_DEVICE_BIND` | Devman `bind.c` | Device binding |

### 5.3 Policy Language

Policy files use a simple allow/deny syntax:

```sh
# /etc/macd.conf

# Allow ALL (compat mode — MAC inactive)
allow ALL;

# OR: explicit per-service rules (STRICT mode)
allow IPC_SEND from pm to vfs;
allow IPC_SEND from pm to vm;
allow PRIVCTL_SET_SYS from rs to any;
allow FILE_ACCESS from vfs to mfs;
allow DEVICE_BIND from devman to ahci;

# Default deny (implicit — if no rule matches, deny)
```

### 5.4 Policy Compilation

For better performance, compile policy to binary format:

```sh
mac-compile -o /etc/macd.policy /etc/macd.conf
```

Binary format:
```
Header:  magic(0x4D414350) + version(1) + num_rules(4)
Rules:   action(4) + op_type(4) + from_label(32) + to_label(32)
         (72 bytes per rule)
```

macd automatically tries `.policy` first, then falls back to `.conf`.

### 5.5 Runtime Toggle

```sh
macctl status    # Show enforcement status + rule count
macctl on        # Enable MAC enforcement
macctl off       # Disable MAC enforcement (all operations ALLOW)
```

### 5.6 Reference Policy

A comprehensive reference policy is provided in `/etc/macd.conf`, covering
all services from `system.conf` in two modes:

- **COMPAT**: `allow ALL` — MAC disabled, full compatibility
- **STRICT**: Explicit per-service rules (commented out; activate by
  commenting `allow ALL` and uncommenting the strict rules)

Groups:
- **Core** (8 services): rs, pm, vm, ds, sched, vfs, macd, auditd
- **Filesystems** (11): mfs, ext4, ext2, pfs, procfs, isofs, hgfs, etc.
- **Drivers** (19): tty, memory, pci, acpi, ahci, virtio_blk, etc.
- **User** (9): init, log, mib, is, input, fbd, pty, klkm, edfictl

---

## 6. Layer 4: Audit & Monitoring

### 6.1 Architecture

```
Event occurs → audit_log(type, result, subject, object, extra)
    │
    ▼
kernel ring buffer (1024 entries, lock-free)
    │
    │ (polled periodically by auditd)
    ▼
auditd daemon
    ├── Write to /var/log/audit/audit.log
    └── Log rotation (size + time triggers)
```

### 6.2 Audit Event Types

| Type | Value | Description |
|------|-------|-------------|
| `AUDIT_AUTH_SUCCESS` | 0 | Successful authentication |
| `AUDIT_AUTH_FAILURE` | 1 | Failed authentication |
| `AUDIT_PRIV_CHANGE` | 2 | Capability or privilege change |
| `AUDIT_IPC_DENIED` | 3 | IPC send denied |
| `AUDIT_FILE_DENIED` | 4 | File access denied |
| `AUDIT_DEVICE_BIND` | 5 | Device bound/unbound |
| `AUDIT_SYSCALL_AUTH` | 6 | Kernel call authorised |
| `AUDIT_MAC_VIOLATION` | 7 | MAC policy violation |
| `AUDIT_SERVICE_START` | 8 | Service started/stopped |
| `AUDIT_SERVICE_CRASH` | 9 | Service crash |

### 6.3 Integration Points

| Subsystem | Events | File |
|-----------|--------|------|
| Kernel `sys_privctl` | `AUDIT_IPC_DENIED`, `AUDIT_PRIV_CHANGE`, `AUDIT_SYSCALL_AUTH`, `AUDIT_MAC_VIOLATION` | `do_privctl.c` |
| VFS `forbidden()` | `AUDIT_FILE_DENIED` | `protect.c` |
| RS `check_call_permission()` | `AUDIT_IPC_DENIED` | `manager.c` |
| Devman bind/unbind | `AUDIT_DEVICE_BIND` | `bind.c` |

### 6.4 Log Rotation

| Parameter | Default | Description |
|-----------|---------|-------------|
| `rotate_size_mb` | 10 | Rotate when log exceeds this size |
| `rotate_interval_s` | 3600 | Rotate every N seconds |
| `max_days` | 30 | Delete logs older than N days |

Rotation produces `audit.log.YYYYMMDD_HHMMSS` files.

### 6.5 Log Format

```
serial|ticks|type|result|subject|object|extra
```

Example:
```
1|213|2|0|2|23|OK
2|214|3|1|14|5|EPERM
```

View with `audit2txt`:

```
[+00:00:02.130]  IPC_DENIED      subj=14     obj=5       EPERM
[+00:00:02.140]  PRIV_CHANGE     subj=2      obj=23      OK
```

---

## 7. Integration Points

### 7.1 system.conf → Capability Mapping

| system.conf Directive | Equivalent Capability |
|----------------------|----------------------|
| `io ALL` | `CAP_SYS_RAWIO` |
| `irq <n>` | `CAP_IRQ_ALLOC` |
| `pci device` | `CAP_PCI_ACCESS` |
| (new) `capabilities NET_RAW` | `CAP_NET_RAW` |
| (new) `capabilities NET_BIND` | `CAP_NET_BIND` |
| (new) `capabilities FS_MOUNT` | `CAP_FS_MOUNT` |

### 7.2 Default Service Capabilities

| Service | Capabilities | Reasoning |
|---------|-------------|-----------|
| RS | `all` | System management |
| VM | `all` | Memory management |
| PM | `all` | Process management |
| VFS | `system + FS_MOUNT + CHOWN + DAC_OVERRIDE` | File system access |
| MFS | `base` | Just serves VFS requests |
| TTY | `driver` | Terminal I/O |
| MEMORY | `driver + VM_MAP` | Physical memory access |
| PCI | `driver + PCI_ACCESS` | PCI configuration |
| AHCI | `driver + VM_MAP` | Disk I/O + DMA |
| INIT | `base` | Minimal process |
| USER | `base` | Unprivileged |

---

## 8. Configuration

### 8.1 Files

| File | Purpose | Man Page |
|------|---------|----------|
| `/etc/system.conf` | Service privilege configuration | `system.conf(5)` |
| `/etc/macd.conf` | MAC policy rules | `mac-policy(5)` |
| `/etc/macd.policy` | Compiled MAC policy (optional) | `mac-policy(5)` |
| `/etc/auditd.conf` | Audit daemon configuration | `auditd(8)` |
| `/var/log/audit/audit.log` | Audit log file | — |

### 8.2 Boot Sequence

```
1. Kernel boots → privilege system active (IPC masks, kcall masks)
2. RS starts → reads system.conf → applies capabilities
3. macd starts → loads MAC policy → enforcement enabled
4. auditd starts → begins polling kernel buffer
5. Services start → all 4 layers active
```

### 8.3 Example system.conf

```sh
service my-httpd
{
    capabilities NET_BIND;       # can bind to port 80
    ipc     vfs;                 # can talk to VFS only
    system  BASIC;
    vm      BASIC;
    uid     0;
};

service my-driver
{
    capabilities SYS_RAWIO IRQ_ALLOC;  # hardware access
    capability_io  0x1000:0x100;       # device MMIO
    irq    16;
    system  UMAP VUMAP IRQCTL DEVIO;
    pci device  1234:5678;
};

service my-app
{
    capabilities NONE;            # no capabilities
    ipc     vfs ds;               # only VFS and DS
    system  NONE;
    vm      BASIC;
};
```

---

## 9. Tools Reference

### 9.1 Capability Tools

| Tool | Purpose | Man Page |
|------|---------|----------|
| `cap_get_proc()` | Query process capabilities | `cap_get_proc(2)` |
| `cap_set_proc()` | Modify process capabilities | `cap_set_proc(2)` |

### 9.2 MAC Tools

| Tool | Purpose | Man Page |
|------|---------|----------|
| `macctl status` | Show enforcement status | `macctl(8)` |
| `macctl on` | Enable MAC enforcement | `macctl(8)` |
| `macctl off` | Disable MAC enforcement | `macctl(8)` |
| `mac-compile` | Compile MAC policy to binary | `mac-compile(8)` |

### 9.3 Audit Tools

| Tool | Purpose | Man Page |
|------|---------|----------|
| `auditctl -s` | Show audit status | `auditctl(8)` |
| `auditctl -e enable` | Enable audit logging | `auditctl(8)` |
| `auditctl -e disable` | Disable audit logging | `auditctl(8)` |
| `auditctl -f` | Force kernel buffer poll | `auditctl(8)` |
| `auditctl -r` | Reopen log file | `auditctl(8)` |
| `auditctl -R` | Force log rotation | `auditctl(8)` |
| `audit2txt -f` | Follow log in real-time | `audit2txt(8)` |
| `audit2txt -t TYPE` | Filter by event type | `audit2txt(8)` |
| `audit2txt -p EP` | Filter by process endpoint | `audit2txt(8)` |

---

## 10. Example Configurations

### 10.1 Minimal HTTP Service

```sh
service my-httpd
{
    capabilities NET_BIND;       # can bind to port 80
    ipc     vfs;                 # can talk to VFS only
    system  BASIC;
    vm      BASIC;
    uid     0;
};
```

### 10.2 Network Driver

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

### 10.3 Unprivileged User Process

```sh
service my-app
{
    capabilities NONE;           # no special capabilities
    ipc     vfs ds;              # only VFS and DS
    system  NONE;
    vm      BASIC;
};
```

### 10.4 Enabling STRICT MAC

```sh
# 1. Edit /etc/macd.conf — comment out "allow ALL"
#    and uncomment the STRICT POLICY section

# 2. Compile to binary (optional, for performance)
mac-compile -o /etc/macd.policy /etc/macd.conf

# 3. Reload policy
kill -HUP $(cat /var/run/macd.pid)

# 4. Verify
macctl status
audit2txt -f /var/log/audit/audit.log
```

### 10.5 Viewing Audit Log

```sh
# Real-time monitoring
audit2txt -f /var/log/audit/audit.log

# Filter by event type
audit2txt -t FILE_DENIED /var/log/audit/audit.log

# Filter by process
audit2txt -p 14 /var/log/audit/audit.log
```

---

## 11. Troubleshooting

### 11.1 MAC Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| Service cannot start | MAC denying IPC to RS | Check `macctl status`, switch to COMPAT |
| File access denied | MAC file hook blocking | Check audit log: `audit2txt -t FILE_DENIED` |
| MAC status shows 0 rules | Policy not loaded | Check `/etc/macd.conf` syntax, run `mac-compile -v` |

### 11.2 Capability Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| Service cannot bind port 80 | Missing `CAP_NET_BIND` | Add `capabilities NET_BIND;` to system.conf |
| Service cannot access I/O ports | Missing `CAP_SYS_RAWIO` | Add `capabilities SYS_RAWIO;` to system.conf |
| fork/exec loses capabilities | Design: exec resets to bounding set | Set bounding set via `CAP_SYS_ADMIN` |

### 11.3 Audit Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| No audit events | auditd not running | Check `auditctl -s`, start auditd |
| Log file missing | auditd not configured | Check `/etc/auditd.conf`, verify log path |
| audit2txt shows no output | Wrong file or format | Verify log format: `head /var/log/audit/audit.log` |

### 11.4 W^X Issues

| Problem | Likely Cause | Solution |
|---------|-------------|----------|
| JIT compiler fails | W^X rejecting PROT_WRITE\|PROT_EXEC | Use `mprotect()` to toggle between R/W and R/X |
| mmap MAP_SHARED fails | W^X check in do_mmap() | Check memory protections are valid |

---

## 12. FAQ

### Q: Does enabling MAC affect performance?

On the COMPAT policy (`allow ALL`), there is minimal overhead — macd returns
`MAC_ALLOW` immediately. On STRICT policy with many rules, expect ~1 µs per
checked operation. The policy compiler (binary format) reduces this further.

### Q: Are old system.conf files compatible?

**Yes.** The `capabilities` directive is optional. If absent, services default
to a sensible set based on their `io`, `irq`, and `system` directives. Old
configs without `capabilities` work unchanged.

### Q: How does MAC interact with Unix permissions?

MAC is an additional layer. Both DAC (Unix permissions) and MAC are checked:
- If DAC denies → `EACCES` immediately
- If DAC allows → MAC policy is consulted
- If MAC denies → `EACCES` (audit event logged)
- Both must allow for access to proceed

### Q: Can I use MAC without capabilities?

**Yes.** MAC and capabilities are independent layers. You can enable MAC
`system.conf` directives without adding `capabilities` to any service.

### Q: How does KASLR affect debugging?

KASLR randomises the kernel's virtual address, making kernel addresses
non-deterministic across boots. For debugging:
- Disable KASLR with `kaslr=no` in `limine.conf`
- Or use `kinfo.kaslr_virt_offset` to compute actual addresses

### Q: What happens when the audit log fills the disk?

auditd rotates logs at configurable size (default 10 MB) and cleans up
logs older than `max_days` (default 30). The kernel ring buffer (1024 entries)
never blocks — it discards oldest entries on overflow.

---

> **See also**: `planning/26_security_model_modernization.md` for the full
> implementation plan, `docs/network-security.md` for network-level security
> features (SYN cookies, IPsec, WireGuard, DTLS).
