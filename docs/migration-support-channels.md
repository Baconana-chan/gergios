# Migration Support Channels

This document outlines the support channels and resources available for users migrating from i386 to x86_64 or ARM64 architectures in MINIX.

---

## Documentation Resources

### Core Migration Documents

| Document | Location | Description |
|----------|----------|-------------|
| Deprecation Announcement | [i386-deprecation-announcement.md](i386-deprecation-announcement.md) | Official deprecation announcement |
| Migration FAQ | [i386-deprecation-faq.md](i386-deprecation-faq.md) | Frequently asked questions |
| Troubleshooting Guide | [i386-migration-troubleshooting.md](i386-migration-troubleshooting.md) | Common issues and solutions |
| Codebase Audit | [i386-codebase-audit.md](i386-codebase-audit.md) | Assessment of i386 dependencies |

### Planning Documents

| Document | Location | Description |
|----------|----------|-------------|
| i386 Deprecation Timeline | [planning/05_i386_deprecation_timeline.md](../planning/05_i386_deprecation_timeline.md) | Full deprecation timeline |
| x86_64 Migration Plan | [planning/07_x86_64_migration_plan.md](../planning/07_x86_64_migration_plan.md) | Technical migration plan |
| Target Architecture Support | [planning/04_target_architecture_support.md](../planning/04_target_architecture_support.md) | Architecture specifications |
| Migration Roadmap | [planning/03_migration_roadmap.md](../planning/03_migration_roadmap.md) | Overall migration strategy |

### Technical Documentation

| Document | Location | Description |
|----------|----------|-------------|
| Microkernel Architecture | [planning/01_microkernel_architecture.md](../planning/01_microkernel_architecture.md) | System architecture |
| Build Instructions | [docs/BUILDING.md](BUILDING.md) | Build system guide |
| Dual Build Guide | [docs/dual-build-guide.md](dual-build-guide.md) | CMake vs BSD Make |

---

## Community Support

### Issue Tracker

Report migration-related issues with the following labels:
- `architecture/i386` — i386-specific issues
- `architecture/x86_64` — x86_64-specific issues
- `architecture/arm64` — ARM64-specific issues
- `migration` — General migration issues
- `help-wanted` — Tasks needing community assistance

When filing an issue, please include:
1. Architecture (`uname -m` output)
2. Build configuration
3. Full error messages and logs
4. Steps to reproduce

---

## Migration Checklist

### For Users

- [ ] Read the [Deprecation Announcement](i386-deprecation-announcement.md)
- [ ] Check [FAQ](i386-deprecation-faq.md) for common questions
- [ ] Verify hardware supports x86_64 (AMD64/Intel 64 CPU)
- [ ] Back up all data (filesystems are compatible)
- [ ] Review the [x86_64 Migration Plan](../planning/07_x86_64_migration_plan.md)
- [ ] Install MINIX for x86_64
- [ ] Recompile software for 64-bit
- [ ] Test all applications
- [ ] Report any issues encountered

### For Developers

- [ ] Review [i386 Codebase Audit](i386-codebase-audit.md) for affected components
- [ ] Update `__i386__` conditionals to include `__x86_64__` where applicable
- [ ] Test on x86_64 hardware or QEMU
- [ ] Fix pointer-size assumptions (use `uintptr_t`, `size_t`, etc.)
- [ ] Update inline assembly for x86_64 registers and instructions
- [ ] Ensure structure alignment is correct
- [ ] Update Makefiles for architecture-conditional builds

---

## Communication Channels

### Official Channels
- **Documentation**: `docs/` directory
- **Planning**: `planning/` directory
- **Status Updates**: `TODO.md`

### Development
- **Code Reviews**: Via pull requests
- **Design Discussions**: Via planning documents
- **Bug Reports**: Via issue tracker

---

*Last updated: June 18, 2026*
