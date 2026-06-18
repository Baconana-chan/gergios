# i386 Architecture Deprecation Announcement

**Date**: June 18, 2026
**Status**: Phase 4 — Complete Removal (Done)

## Summary

The MINIX project is officially announcing the deprecation of the **i386 (32-bit x86)** architecture support. This decision follows the strategic direction outlined in the [i386 Deprecation Timeline](../planning/05_i386_deprecation_timeline.md) and aligns with our modernization goals to focus on **x86_64 (64-bit)** and **ARM64 (AArch64)** architectures.

## Timeline

| Phase | Description | Target Date | Status |
|-------|-------------|-------------|--------|
| **Phase 1** | Announcement and Preparation | Q2 2026 | ✅ Complete |
| **Phase 2** | Soft Deprecation | Q2 2026 | ✅ Complete |
| **Phase 3** | Hard Deprecation | Q2 2026 | ✅ Complete |
| **Phase 4** | Complete Removal | Q2 2026 | ✅ **CURRENT** |

## What This Means

### For Users on i386 Hardware
- **i386 is Phase 4 — completely removed** from the main branch
- **i386 code preserved** in git tag `archive/i386-last`
- **All i386 arch directories** (sys/arch/i386, kernel, drivers, libs) removed
- **All i386 build system references** cleaned up (CMakeLists.txt, BSD Makefiles)
- **All standalone __i386__ ifdefs** cleaned up in MINIX core code
- **Migration complete**: x86_64 is the only x86 architecture
- Migration guides and support resources are available (see below)

### For Developers
- **i386 code removed** from the main development tree
- **Archived** in git tag `archive/i386-last` for legacy access
- **All development** targets x86_64 and ARM64
- **x86_64** is the sole x86 architecture going forward

### For the Project
- Focusing resources on modern architectures will accelerate development
- x86_64 implementation is complete across all phases (build infra, kernel bootstrap, memory management, system calls/signals, libraries, drivers)
- ARM64 implementation is the next major milestone

## Why Deprecate i386?

### Technical Reasons
- **Limited Address Space**: 4GB limit is insufficient for modern systems
- **Outdated Instruction Set**: Missing modern CPU features (AES-NI, AVX, SMEP/SMAP)
- **Performance**: Modern hardware runs much faster in 64-bit mode
- **Security Vulnerabilities**: Susceptible to Spectre, Meltdown, and other vulnerabilities
- **Hardware Obsolescence**: Modern hardware no longer supports i386-only mode

### Strategic Reasons
- **Resource Focus**: Resources better spent on x86_64 and ARM64
- **Industry Trend**: The entire industry has moved to 64-bit architectures
- **Software Ecosystem**: Modern software assumes 64-bit
- **Cloud and Server**: Cloud providers use 64-bit exclusively

## Migration Path

### Hardware Options
| Architecture | Recommended Hardware | Performance | Availability |
|-------------|---------------------|-------------|-------------|
| **x86_64** | Any modern AMD64/Intel 64 CPU | ✅ Excellent | ✅ Widely available |
| **ARM64** | Raspberry Pi 4/5, AWS Graviton | ✅ Good | ✅ Widely available |

### Migration Resources
- [Migration Guide](../planning/03_migration_roadmap.md)
- [x86_64 Migration Plan](../planning/07_x86_64_migration_plan.md)
- [Target Architecture Support](../planning/04_target_architecture_support.md)
- [i386 Codebase Audit](i386-codebase-audit.md)
- [Migration FAQ](i386-deprecation-faq.md)
- [Troubleshooting Guide](i386-migration-troubleshooting.md)

## Support During Deprecation

### Phase 1 (Complete)
- Deprecation announcement and documentation published
- Migration guides, FAQ, troubleshooting, codebase audit created
- i386 fully supported with deprecation information available

### Phase 2 (Complete)
- ✅ Deprecation warnings in build scripts
- ✅ x86_64 is now the default build target
- ✅ CI/CD prioritizes x86_64
- ✅ i386 tests marked as deprecated
- ✅ Phase 2 feature restriction policy published

### Phase 3 — Hard Deprecation ✅
- ✅ i386 requires `-DMKI386=ON` to build
- ✅ i386 removed from default CI/CD
- ✅ Community-supported only — best-effort maintenance
- ✅ No official support, no guaranteed fixes
- ✅ Archive strategy documented
- ✅ Migration deadline set

### Phase 4 — Removal ✅
- ✅ All i386 architecture code removed
- ✅ i386-specific drivers removed
- ✅ Build system references (CMake, BSD Makefiles) cleaned up
- ✅ Standalone __i386__ ifdefs cleaned up
- ✅ i386-only tests removed
- ✅ Git tag `archive/i386-last` created for legacy code preservation
- ✅ Documentation archived to `docs/archive/`

## Deprecation Warnings

When building for i386, you will now see deprecation notices in the build output. These warnings are informational and do not affect the build process.

Example warning that will appear in Phase 2:
```
WARNING: i386 architecture is deprecated
WARNING: See docs/i386-deprecation-announcement.md for details
WARNING: Recommended migration: x86_64 or ARM64
```

## Contact and Feedback

For questions, concerns, or feedback about this deprecation:

- **Documentation**: See the `docs/` directory and `planning/` directory
- **Issue Tracker**: Report issues related to i386 deprecation
- **Migration Support**: See the [FAQ](i386-deprecation-faq.md) and [Troubleshooting Guide](i386-migration-troubleshooting.md)

---

*This announcement is part of the MINIX modernization project. For the full context, see [i386 Deprecation Timeline](../planning/05_i386_deprecation_timeline.md).*
