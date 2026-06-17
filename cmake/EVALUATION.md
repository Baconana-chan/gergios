# CMake Build System Evaluation for MINIX

## Current State: BSD Make (bmake)

MINIX currently uses a recursive BSD Make (bmake) build system inherited from
NetBSD. Key characteristics:

- **241+ Makefiles** across `minix/` tree
- **Recursive builds** via `SUBDIR` lists and `bsd.subdir.mk`
- **Centralized rules** in `share/mk/` (`bsd.prog.mk`, `bsd.lib.mk`, `bsd.own.mk`)
- **Complex architecture handling** with `.if ${MACHINE_ARCH} == "i386"` conditionals
- **Custom toolchain wrappers** via `build.sh` and `${toolprefix}make`
- **Unpaged object handling** via `OBJCOPY --prefix-symbols=__k_unpaged_`
- **Bitcode/LLVM magic passes** for MINIX-specific transformations

### Current Pain Points

1. **No parallel build safety**: Recursive make is prone to races
2. **No IDE integration**: No project files for CLion, VS Code, etc.
3. **Complex dependency tracking**: Manual `.depend` files, not incremental
4. **Cross-compilation complexity**: `build.sh` is 1000+ lines of shell
5. **Hard to onboard developers**: BSD Make syntax is obscure

## Target State: CMake + Ninja

### Why CMake?

| Feature | BSD Make | CMake |
|---------|----------|-------|
| Generator support | None | Ninja, Make, Xcode, VS |
| IDE integration | None | CLion, VS Code, Qt Creator |
| Cross-compilation | Custom shell scripts | Toolchain files |
| Dependency tracking | Manual `.depend` | Automatic |
| CTest integration | No | Built-in |
| Package discovery | Manual | `find_package()` |
| Targets export | Manual | `install(EXPORT)` |
| Presets support | No | `CMakePresets.json` |

### Why not alternatives?

| Alternative | Issue |
|-------------|-------|
| **Autotools** | Worse than BSD Make for cross-compilation |
| **Meson** | Less ecosystem support, no `.d` dependency auto-tracking |
| **Bazel** | Requires Java, overkill for OS kernel |
| **Plain Make** | Even more manual work than current system |

## Proposed CMake Structure

```
cmake/
├── toolchain-minix.cmake       # MINIX cross-compilation toolchain
├── toolchain-minix-i386.cmake  # i386-specific toolchain
├── toolchain-minix-earm.cmake  # ARM-specific toolchain
├── toolchain-minix-x86_64.cmake # x86_64-specific toolchain (future)
├── macros.cmake                # Reusable MINIX CMake macros
├── arch/
│   ├── i386.cmake             # i386 architecture definitions
│   └── earm.cmake             # ARM architecture definitions
├── options.cmake              # MINIX option handling (MK*, USE_*)
├── minix.service.cmake        # Service/daemon build macros
├── unpaged-objects.cmake       # Unpaged kernel object handling
├── bitcode.cmake              # LLVM bitcode support
└── asr.cmake                  # ASR (Address Space Randomization) support

src/ (or kept at root)
├── CMakeLists.txt             # Root: project, subdirectories, global flags
├── minix/
│   ├── CMakeLists.txt         # MINIX subtree entry
│   ├── kernel/
│   │   ├── CMakeLists.txt     # Kernel build
│   │   └── arch/
│   │       ├── i386/
│   │       │   └── CMakeLists.txt  # i386 kernel arch code
│   │       └── earm/
│   │           └── CMakeLists.txt  # ARM kernel arch code
│   ├── servers/
│   │   ├── CMakeLists.txt
│   │   └── */CMakeLists.txt
│   ├── drivers/
│   │   ├── CMakeLists.txt
│   │   └── */CMakeLists.txt
│   ├── lib/
│   │   ├── CMakeLists.txt
│   │   └── */CMakeLists.txt
│   └── commands/
│       ├── CMakeLists.txt
│       └── */CMakeLists.txt
├── crypto/
│   └── external/
│       └── gpl2/
│           └── wolfssl/
│               └── CMakeLists.txt  # wolfSSL CMake build
└── tests/
    └── CMakeLists.txt         # CTest configuration
```

## Migration Strategy

### Phase 1: Preparation (THIS DOCUMENT)
- Evaluate CMake structure
- Create CMake prototypes for kernel and key libraries
- Set up CTest infrastructure
- Document best practices

### Phase 2: Core Migration
- Migrate kernel build to CMake
- Migrate servers and drivers
- Migrate libraries

### Phase 3: Userland Migration
- Migrate userland tools
- Migrate tests
- Update CI/CD

### Phase 4: Cleanup
- Remove BSD Makefiles (keep for reference during transition)
- Update build scripts
- Final testing

## Key Migration Decisions

### Decision 1: In-source vs Out-of-source builds
**Choice**: Out-of-source with `CMAKE_BINARY_DIR` separate from source tree.
This is CMake's default and cleanest approach.

### Decision 2: Single vs Multiple CMake projects
**Choice**: Single project with multiple subdirectories. Simpler dependency
management and allows unified toolchain configuration.

### Decision 3: Preserve `MK*` variables or convert
**Choice**: Convert `MK*`/`USE_*` to CMake options with `option()` and cache
variables. This enables `cmake -DUSE_WATCHDOG=ON` style configuration.

### Decision 4: Handle `bsd.prog.mk` patterns
**Choice**: Create `add_minix_executable()`, `add_minix_service()`, and
`add_minix_library()` CMake macros that replicate the most common patterns
from `bsd.prog.mk` and `minix.service.mk`.

### Decision 5: Unpaged object handling
**Choice**: Create a custom CMake function `add_unpaged_objects()` that wraps
`add_library(OBJECT)` with post-processing via `OBJCOPY --prefix-symbols=` to
replicate the `__k_unpaged_` namespace prefixing.

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Bitcode/MAGIC pass incompatibility | High | Keep dual build system during transition |
| Unpaged object symbol ordering | Medium | Test kernel boots with CMake build |
| Linker script integration | Low | CMake can pass arbitrary linker flags |
| Cross-compilation toolchain | Medium | Use CMake toolchain files |
| Developer learning curve | Low | Provide migration guide and training |
