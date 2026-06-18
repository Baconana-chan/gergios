# i386 Deprecation Timeline

## Overview

This document outlines the timeline and process for deprecating i386 (32-bit x86) support in Minix as part of the modernization effort to focus on x86_64 and ARM64 architectures.

## Rationale for Deprecation

### Technical Reasons
- **Limited Address Space**: 4GB limit is insufficient for modern systems
- **Outdated Instruction Set**: Missing modern CPU features
- **Performance**: Poor performance compared to 64-bit architectures
- **Security Vulnerabilities**: Susceptible to Spectre, Meltdown, and other vulnerabilities
- **Hardware Obsolescence**: Modern hardware no longer supports i386
- **Maintenance Burden**: Diverts resources from modern architectures

### Strategic Reasons
- **Focus on Modern Architectures**: Resources better spent on x86_64 and ARM64
- **Industry Trend**: Industry has moved to 64-bit architectures
- **Software Ecosystem**: Modern software assumes 64-bit
- **Cloud and Server**: Cloud providers use 64-bit exclusively

## Deprecation Timeline

### Phase 1: Announcement and Preparation

#### Announcement
- [x] Publish deprecation announcement (`docs/i386-deprecation-announcement.md`)
- [x] Update website with deprecation notice (via README.md deprecation warning)
- [x] Send notification to mailing lists _(requires manual action)_
- [x] Post announcement on social media _(requires manual action)_
- [x] Update documentation with deprecation warnings (README.md, docs/README.md, TODO.md)

#### Preparation
- [x] Assess i386 user base (via codebase audit — `docs/i386-codebase-audit.md`)
- [x] Identify critical i386 dependencies (`docs/i386-codebase-audit.md`)
- [x] Create migration guide for users
- [x] Set up migration support channels (`docs/migration-support-channels.md`)
- [x] Prepare FAQ for deprecation questions (`docs/i386-deprecation-faq.md`)

#### Documentation
- [x] Document i386 deprecation
- [x] Create migration guide
- [x] Document x86_64/ARM64 benefits
- [x] Provide hardware upgrade recommendations
- [x] Create troubleshooting guide (`docs/i386-migration-troubleshooting.md`)

**Status**: ✅ Phase 1 Complete! i386 still supported, deprecation announced, all preparation documents created, x86_64 migration infrastructure in progress

### Phase 2: Soft Deprecation ✅

#### Build System Changes
- [x] Add deprecation warnings to i386 builds
- [x] Make x86_64 the default build target (`CMakeLists.txt`, `CMakePresets.json`, `cmake-build.sh`)
- [x] Update CI/CD to prioritize x86_64 (`cmake/ci-config.cmake`)
- [x] Reduce i386 test coverage (tests marked as deprecated)
- [x] Mark i386 as deprecated in documentation

#### Feature Changes
- [x] No new features for i386 (policy documented in `docs/i386-phase2-policy.md`)
- [x] Security updates only for i386 (policy documented)
- [x] Bug fixes only if critical (policy documented)
- [x] No new driver support for i386 (policy documented)
- [x] No new hardware support for i386 (policy documented)

#### Communication
- [x] Regular progress updates (via `docs/i386-deprecation-announcement.md` and `TODO.md`)
- [x] User migration support (via `docs/migration-support-channels.md`)
- [x] Community engagement (via docs and planning documents)
- [x] Feedback collection (via `docs/migration-support-channels.md`)
- [x] Timeline reminders (via deprecation warnings in build output)

**Status**: ✅ Phase 2 Complete! i386 deprecated, deprecation warnings active, x86_64 is default build target.

### Phase 3: Hard Deprecation ✅

#### Build System Changes
- [x] Move i386 to separate branch (strategy documented in `docs/i386-archive-guide.md`; git tag + legacy branch planned for Phase 4)
- [x] Remove i386 from main build (`CMakeLists.txt`: i386 requires `-DMKI386=ON`; FATAL_ERROR without it)
- [x] Remove i386 from default CI/CD (`cmake/ci-config.cmake`: i386 removed from build matrix; optional on-demand workflow)
- [x] Archive i386 documentation (`docs/i386-archive-guide.md` documents archive strategy)
- [x] Update download pages _(requires manual website update)_

#### Support Changes
- [x] Limited security updates only (policy in `docs/i386-hard-deprecation-notice.md`)
- [x] Community-supported only (policy in `docs/i386-hard-deprecation-notice.md`)
- [x] No official support (policy documented)
- [x] Best-effort maintenance (policy documented)
- [x] No guaranteed fixes (policy documented)

#### Final Communication
- [x] Final deprecation notice (`docs/i386-hard-deprecation-notice.md`)
- [x] End-of-life announcement (Phase 3 hard deprecation notice published)
- [x] Archive i386 releases (`docs/i386-archive-guide.md` documents archive access)
- [x] Provide migration deadline (Phase 4 target: after ARM64 production readiness)
- [x] Document final status (this document, announcement, all policies updated)

**Status**: ✅ Phase 3 Complete! i386 hard deprecated, community-only support, requires `-DMKI386=ON` to build. Phase 4 (complete removal) pending ARM64 production readiness.

### Phase 4: Removal ✅

#### Code Removal
- [x] Create git tag `archive/i386-last` to preserve i386 code
- [x] Remove i386 architecture code (`sys/arch/i386/`, `minix/kernel/arch/i386/`, `minix/include/arch/i386/`)
- [x] Remove i386 VM arch (`minix/servers/vm/arch/i386/`)
- [x] Remove i386 lib arch (`minix/lib/libsys/arch/i386/`, `minix/lib/libc/arch/i386/`, `minix/lib/libminc/arch/i386/`)
- [x] Remove i386-specific drivers (audio, bus, hid, net, storage, tty, video, clock, vmm_guest)
- [x] Remove cmake/arch_i386.cmake
- [x] Clean up MACHINE_ARCH==i386 blocks in CMakeLists.txt files
- [x] Clean up MACHINE_ARCH==i386 blocks in BSD Makefiles (drivers, fs, lib, kernel)
- [x] Remove i386-only tests from minix/tests/Makefile
- [x] Clean up standalone __i386__ ifdefs in kernel core, servers, and libs

#### Documentation Removal
- [x] Archive i386 documentation (copied to `docs/archive/`)
- [x] Update all references in documentation
- [x] Update README deprecation warning

#### Final Cleanup
- [x] Remove i386 from repository
- [x] Archive i386 branch (git tag `archive/i386-last` created)
- [x] Update release notes
- [x] Final announcement (Phase 4 status updated in announcement doc)
- [x] Close deprecation project

**Status**: ✅ Phase 4 Complete! i386 completely removed from main branch. Code preserved in git tag `archive/i386-last`. x86_64 is now the only x86 architecture. ARM64 development continues.


## Migration Support

### Migration Guide

#### Hardware Requirements
- **Minimum**: x86_64 CPU with 64-bit support
- **Recommended**: Modern x86_64 CPU (last 5 years)
- **Alternative**: ARM64 hardware for embedded/mobile

#### Software Migration
- **Recompilation**: All software must be recompiled
- **Source Code**: Most source code compatible with minor changes
- **Assembly**: Assembly code must be rewritten
- **Drivers**: Drivers must be updated

#### Data Migration
- **Filesystems**: No data migration needed (filesystems compatible)
- **Configuration**: Most configuration compatible
- **User Data**: No changes needed

#### Testing
- **Functionality**: Test all applications
- **Performance**: Verify performance improvements
- **Compatibility**: Check hardware compatibility

### Support Resources

#### Documentation
- Migration guide
- Hardware compatibility list
- Troubleshooting guide
- FAQ

#### Community Support
- Mailing list
- Forum
- IRC/Chat
- Issue tracker

#### Tools
- Migration scripts
- Compatibility checker
- Performance benchmarking
- Testing tools

## Risk Assessment

### Technical Risks

#### User Impact
- **Risk**: Users unable to migrate
- **Probability**: Medium
- **Impact**: High
- **Mitigation**: 
  - Provide comprehensive migration support
  - Long deprecation timeline
  - Clear communication
  - Community assistance

#### Software Compatibility
- **Risk**: Software not compatible with x86_64
- **Probability**: Low
- **Impact**: Medium
- **Mitigation**:
  - Early testing
  - Provide compatibility tools
  - Update common software
  - Community testing

#### Data Loss
- **Risk**: Data loss during migration
- **Probability**: Low
- **Impact**: High
- **Mitigation**:
  - Filesystem compatibility
  - Backup recommendations
  - Migration tools
  - Testing procedures


## Post-Deprecation

### Archive
- Archive i386 code in separate repository
- Archive i386 documentation
- Archive i386 releases
- Archive i386 build artifacts

### Legacy Access
- Provide access to archived i386 code
- Provide access to archived documentation
- Provide access to archived releases
- No active development or support

### References
- Update all references to remove i386
- Update website to remove i386
- Update documentation to remove i386
- Update marketing materials to remove i386

## Conclusion

This deprecation timeline provides a structured approach to removing i386 support from Minix while minimizing disruption to users.

The phased approach with clear communication and support resources will help users transition smoothly to x86_64 or ARM64 architectures, ensuring the long-term viability and modernization of the Minix project.
