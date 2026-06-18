# Phase 2: i386 Feature Restriction Policy

**Effective**: June 18, 2026
**Phase**: Soft Deprecation (Phase 2)

## Overview

With the transition to Phase 2 (Soft Deprecation) of the i386 architecture deprecation, the following policy restrictions apply to i386-specific development.

---

## Policy Rules

### 1. No New Features for i386

Starting in Phase 2, **no new features** will be implemented specifically for i386. All new feature development must target **x86_64** or **ARM64** architectures.

**What this means:**
- New kernel features will only be implemented for x86_64 and ARM64
- New server functionality will target modern architectures
- New system calls and APIs will be designed for 64-bit first
- If an i386 implementation is trivial (e.g., adding `__i386__` to an existing `__x86_64__` branch), it may be accepted, but should be avoided

### 2. Security Updates Only for i386

i386 will receive **security updates only**. Non-security bug fixes will be accepted only if they are critical.

**What this means:**
- Security vulnerabilities in i386 code will be fixed
- Memory safety issues in i386 code will be prioritized
- Use-after-free, buffer overflow, and similar security bugs will be addressed
- Non-security bug fixes should target x86_64/ARM64 first

### 3. Bug Fixes Only if Critical

Bug fixes for i386 will be accepted **only if they are critical** to system stability or data integrity.

**Criteria for critical bugs:**
- System crash or kernel panic specific to i386
- Data corruption that affects i386 users
- Complete loss of functionality for i386
- Build failures preventing i386 from compiling

**Non-critical bugs:**
- Minor performance issues
- Cosmetic problems (e.g., display alignment)
- Non-standard hardware support
- Features that have equivalent x86_64 functionality

### 4. No New Driver Support for i386

**No new device drivers** will be written for i386. All new driver development must target x86_64 architectures.

**What this means:**
- New hardware support will be implemented for x86_64 only
- Existing i386 drivers will continue to work as-is
- Critical bug fixes for existing i386 drivers may be accepted
- Driver updates that affect both architectures should be tested on x86_64 first

### 5. No New Hardware Support for i386

**No new hardware support** will be added for i386-specific hardware. All new hardware enablement must target x86_64 or ARM64.

**What this means:**
- New CPU features will be enabled for x86_64 only
- New motherboard/chipset support will target modern platforms
- Legacy hardware support for i386-only devices will not be added
- Existing hardware support will be maintained as-is

---

## Exceptions

Exceptions to these policies require approval from the project maintainers and must be accompanied by a clear justification. Valid exceptions include:

1. **Migration compatibility**: Changes needed to support i386→x86_64 migration
2. **Build system**: Changes to the build system that affect i386 (like deprecation warnings)
3. **Documentation**: Documentation updates related to i386 deprecation
4. **Critical security**: Security vulnerabilities that also affect x86_64 via shared code

---

## Enforcement

- All pull requests adding new i386-specific code will be flagged during review
- Existing `#ifdef __i386__` blocks should not be expanded with new functionality
- New code should be written architecture-independently or for x86_64/ARM64
- Violations should be documented and escalated to project maintainers

---

## Related Documents

- [Deprecation Announcement](i386-deprecation-announcement.md)
- [Deprecation Timeline](../planning/05_i386_deprecation_timeline.md)
- [Migration FAQ](i386-deprecation-faq.md)
- [Codebase Audit](i386-codebase-audit.md)
- [Migration Support Channels](migration-support-channels.md)

---

*This policy applies from Phase 2 of the i386 deprecation until the removal of i386 architecture support in Phase 4.*
