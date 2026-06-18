# i386 Legacy Code Archive Guide

## Overview

This document describes how to access archived i386 code and documentation after Phase 3 Hard Deprecation.

**i386 code remains in the repository but requires explicit opt-in (`-DMKI386=ON`) to build.** It will be fully removed in Phase 4 (after ARM64 reaches production readiness).

---

## Accessing i386 Code

### Current Repository

i386 architecture code is still present in the repository:

| Location | Contents |
|----------|----------|
| `sys/arch/i386/` | 242 i386 architecture files |
| `minix/kernel/arch/i386/` | Kernel architecture assembly and C files |
| `minix/servers/vm/arch/i386/` | VM pagetable headers |
| `minix/lib/libsys/arch/i386/` | System library arch files |
| `minix/include/arch/i386/` | Architecture headers |
| `cmake/arch_i386.cmake` | CMake architecture definition |

### Building i386 Code

```bash
# Explicit opt-in required
cmake -DMACHINE_ARCH=i386 -DMKI386=ON ..
make
```

### Reading i386 Code Without Building

The code is fully readable and browsable via the normal repository interface. All i386 files are preserved for reference, porting, and education.

---

## Future Removal (Phase 4)

When Phase 4 begins (after ARM64 reaches production readiness):

### What Will Be Removed

- `sys/arch/i386/` — Full architecture directory
- `minix/kernel/arch/i386/` — Kernel arch files
- `minix/servers/vm/arch/i386/` — VM arch files
- `minix/lib/libsys/arch/i386/` — Library arch files
- `minix/include/arch/i386/` — Architecture headers
- `cmake/arch_i386.cmake` — CMake definition
- All i386-specific drivers and code
- All `__i386__` conditional compilation blocks

### What Will Be Archived

- A **git tag** (`archive/i386-last`) will mark the last commit with i386 support
- i386 documentation will be preserved in a `docs/archive/` directory
- A **separate branch** (`legacy/i386-support`) may be created for community maintenance

### How to Access Archived Code

```bash
# Via git tag
git checkout archive/i386-last

# Via legacy branch (if created)
git checkout legacy/i386-support
```

---

## Documentation Archive

After Phase 4, i386-specific documentation will be moved to:

| Document | Archived Location |
|----------|-------------------|
| i386 Deprecation Announcement | `docs/archive/i386-deprecation-announcement.md` |
| i386 Migration FAQ | `docs/archive/i386-deprecation-faq.md` |
| i386 Troubleshooting Guide | `docs/archive/i386-migration-troubleshooting.md` |
| i386 Codebase Audit | `docs/archive/i386-codebase-audit.md` |
| i386 Phase 2 Policy | `docs/archive/i386-phase2-policy.md` |
| i386 Hard Deprecation Notice | `docs/archive/i386-hard-deprecation-notice.md` |
| i386 Archive Guide | `docs/archive/i386-archive-guide.md` |

---

## References for Future Porting

Even after removal, i386 code serves as a reference for:

- **MINIX kernel architecture patterns** (how IPC, scheduling, and process management work)
- **MINIX driver model** (how block, char, and network drivers interface with servers)
- **MINIX build system** (how architecture-specific code is organized)
- **MINIX memory management** (how VM and pagetable code is structured)

These patterns are architecture-independent and relevant for x86_64, ARM64, and future ports.

---

*Last updated: June 18, 2026*
