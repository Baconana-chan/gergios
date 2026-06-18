# i386 Phase 3 — Hard Deprecation Notice

**Effective**: June 18, 2026
**Status**: Phase 3 — Hard Deprecation (Active)

## Summary

i386 (32-bit x86) architecture has entered **Phase 3: Hard Deprecation**. This phase marks the transition from "deprecated but supported" to "community-supported only, best-effort basis."

**i386 no longer builds by default** — explicit opt-in (`-DMKI386=ON`) is required.

---

## Support Model

### What Remains

- **Source code**: i386 code remains in the repository for community contributors
- **Build capability**: i386 can still be built with `-DMKI386=ON`
- **Community contributions**: i386 bug fixes from the community will be reviewed and merged
- **Security patches**: Critical security vulnerabilities may be addressed on a best-effort basis

### What Is Removed

- **Default build**: i386 no longer builds without explicit opt-in
- **CI/CD testing**: i386 is removed from the default CI/CD pipeline
- **Official support**: i386 is not an officially supported architecture
- **New features**: No new features, drivers, or hardware support for i386
- **Guaranteed fixes**: No guaranteed timeline for bug fixes
- **Release binaries**: No i386 binaries in future releases

---

## Build Instructions (for legacy users)

```bash
# i386 requires explicit opt-in
cmake -DMACHINE_ARCH=i386 -DMKI386=ON ..

# Or with CMake preset
cmake --preset i386-debug
```

Building for i386 will display:
```
FATAL_ERROR: i386 is Phase 3 HARD DEPRECATED.
To build for i386, you MUST explicitly enable it:
  cmake -DMACHINE_ARCH=i386 -DMKI386=ON ..
```

---

## Community Contribution Guidelines

i386 contributions are accepted under these conditions:

1. **Priority**: x86_64 and ARM64 contributions take priority
2. **Scope**: Only critical bug fixes and security patches
3. **Review**: i386 changes require additional maintainer review
4. **Testing**: Contributors must verify builds themselves (no CI)
5. **No regressions**: Changes must not break x86_64 or ARM64 builds

---

## Migration Deadline

Users still on i386 should plan migration to x86_64 or ARM64.

| Milestone | Target | Status |
|-----------|--------|--------|
| Phase 1 — Announcement | Q2 2026 | ✅ Complete |
| Phase 2 — Soft Deprecation | Q2 2026 | ✅ Complete |
| Phase 3 — Hard Deprecation | **CURRENT** | ✅ **Active** |
| Phase 4 — Complete Removal | After ARM64 production readiness | ⏳ Pending |

**Recommended migration path**: x86_64 for desktop/server, ARM64 for embedded/mobile.

---

## References

- [Deprecation Announcement](i386-deprecation-announcement.md)
- [Deprecation Timeline](../planning/05_i386_deprecation_timeline.md)
- [Migration FAQ](i386-deprecation-faq.md)
- [Archive Guide](i386-archive-guide.md)
- [Codebase Audit](i386-codebase-audit.md)
- [Migration Support Channels](migration-support-channels.md)
