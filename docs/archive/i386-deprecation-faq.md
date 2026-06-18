# i386 Deprecation FAQ

## General Questions

### Q1: What is happening to i386 support in MINIX?
i386 (32-bit x86) architecture support is being deprecated. The project is focusing development resources on x86_64 and ARM64 architectures. i386 will continue to work through the deprecation phases, but will eventually be removed.

### Q2: Is i386 being removed immediately?
No. The deprecation follows a phased timeline:
- **Phase 1 (Q2 2026)**: Announcement and preparation — i386 fully supported
- **Phase 2 (Q3 2026)**: Soft deprecation — security updates only, no new features
- **Phase 3 (Q4 2026)**: Hard deprecation — community-supported only
- **Phase 4 (2027)**: Complete removal

### Q3: Can I still use MINIX on my i386 machine?
Yes. i386 support will continue to work throughout Phases 1 and 2. You can continue using MINIX on i386 hardware while planning your migration to a 64-bit architecture.

### Q4: Why is MINIX dropping i386 support?
Multiple reasons:
- **Technical limitations**: 4GB address space is insufficient for modern workloads
- **Security**: i386 lacks modern security features (SMEP, SMAP, NX bit enforcement)
- **Performance**: 64-bit mode is significantly faster on modern hardware
- **Resource allocation**: Maintaining i386 diverts resources from x86_64 and ARM64 development
- **Industry direction**: The entire computing industry has moved to 64-bit

### Q5: What architectures will be supported instead?
- **x86_64 (AMD64/Intel 64)** — Primary desktop/server target, implementation complete
- **ARM64 (AArch64)** — Primary embedded/mobile target, implementation in progress

## Migration Questions

### Q6: How do I migrate from i386 to x86_64?
Full migration guides are available:
- [x86_64 Migration Plan](../planning/07_x86_64_migration_plan.md)
- [General Migration Roadmap](../planning/03_migration_roadmap.md)
- [Target Architecture Support](../planning/04_target_architecture_support.md)

The key steps are:
1. Verify your hardware supports x86_64 (any AMD64 or Intel 64 CPU)
2. Back up your data (filesystems are compatible)
3. Install MINIX for x86_64
4. Recompile your software for 64-bit

### Q7: What if my hardware doesn't support x86_64?
If your hardware is truly i386-only (pre-2003), you have several options:
- **Upgrade hardware**: Even budget modern hardware supports x86_64
- **Use an alternative OS**: Other operating systems may continue i386 support
- **Stay on the last i386 release**: The final i386 release will be archived

### Q8: Will my data be safe during migration?
Yes. MINIX filesystems are compatible between i386 and x86_64. You can migrate your data without conversion.

### Q9: Do I need to recompile my software for x86_64?
Yes. x86_64 binaries are not compatible with i386 and vice versa. All software must be recompiled for the target architecture. Most source code will compile with minor or no changes.

### Q10: What about assembly code?
Any assembly code in your applications will need to be updated for x86_64. See the [x86_64 Migration Plan](../planning/07_x86_64_migration_plan.md) for details on assembly differences.

## Technical Questions

### Q11: What are the main differences between i386 and x86_64?
- **Registers**: 16 general-purpose registers (vs 8 on i386), all 64-bit
- **Address space**: Up to 256TB (48-bit) vs 4GB (32-bit)
- **Page tables**: 4-level paging (PML4) vs 2-level (legacy) or 3-level (PAE)
- **System calls**: SYSCALL/SYSRET instruction vs `int 0x80`
- **Calling convention**: 6 register arguments (RDI, RSI, RDX, RCX, R8, R9)
- **Security**: SMEP, SMAP, NX bit, KASLR

### Q12: How extensive are the i386 dependencies in the codebase?
An audit of `__i386__` conditional compilation found **226+ occurrences** across the codebase in files spanning:
- Kernel system calls (do_sigsend.c, do_sigreturn.c, do_fork.c, etc.)
- VM pagetable management (pagetable.c — 26 ifdefs)
- Library code (libc, libm, libpthread, etc.)
- Device drivers (RS232, audio, storage, etc.)
- Filesystem code (procfs, isofs)
- External dependencies (OpenSSL, GCC, LLVM, Xorg, wolfSSL)

A full breakdown is available in the [i386 Codebase Audit](i386-codebase-audit.md).

### Q13: What is the performance benefit of migrating to x86_64?
- **Memory access**: 64-bit mode provides faster memory access with more registers
- **Address space**: No more 4GB limitations for memory-intensive applications
- **CPU features**: Access to modern instructions (AES-NI, AVX, CLMUL)
- **Security**: Hardware-enforced security features
- **Expected**: 30%+ performance improvement over i386

### Q14: Will there be a transition period where both i386 and x86_64 are supported?
Yes. During Phases 1 and 2, both architectures will be fully supported. During Phase 3, i386 will receive limited community support. The build system currently supports both architectures.

## Community Questions

### Q15: Can I help with the migration?
Yes! We welcome contributions to:
- Testing x86_64 on various hardware
- Developing ARM64 support
- Porting drivers and software to 64-bit
- Improving documentation and migration guides
- Testing and reporting issues

### Q16: What if I have critical i386-dependent workflows?
We encourage you to:
1. Start planning migration to x86_64 or ARM64
2. Report any issues you encounter during migration
3. Share your use case with the community
4. Contribute to making the migration smoother for others

### Q17: How can I stay updated on the deprecation progress?
- Watch the `planning/05_i386_deprecation_timeline.md` document
- Follow updates in the `docs/` directory
- Monitor the `TODO.md` for overall project status
- Check release notes for each MINIX release

---

*Last updated: June 18, 2026*
