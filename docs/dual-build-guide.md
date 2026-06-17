# Dual Build System Guide

## Overview

During the transition from BSD Make (bmake) to CMake, MINIX supports both
build systems. This guide explains when to use each system and how they
interact.

## Build System Status

| Build System | Status | Recommendation |
|-------------|--------|----------------|
| BSD Make (bmake) | **DEPRECATED** | Use for production builds during transition |
| CMake + Ninja | **PROTOTYPE** | Use for development and testing |

## When to Use Each System

### Use BSD Make (legacy) when:
- Building a production MINIX image
- Running the release pipeline (`build.sh release`)
- Building kernel modules or boot images
- Building for unsupported architectures

```bash
./build.sh -m i386 build
```

### Use CMake when:
- Developing new components
- Rapid prototyping with fast incremental builds
- Using IDE integration (CLion, VS Code)
- Cross-compilation from non-MINIX hosts
- Running CTest for quick validation

```bash
# Using wrapper script
./releasetools/cmake-build.sh configure i386
./releasetools/cmake-build.sh build

# Or directly with cmake presets
cmake --preset default
cmake --build --preset default
ctest --preset default
```

## File Organization

```
project root/
├── CMakeLists.txt           # CMake build (PROTOTYPE)
├── CMakePresets.json        # CMake presets for common configs
├── build.sh                 # BSD Make build (LEGACY, DEPRECATED)
├── share/mk/                # BSD Make rules (DEPRECATED)
└── minix/
    ├── Makefile             # DEPRECATED — see CMakeLists.txt
    ├── kernel/Makefile      # DEPRECATED — see kernel/CMakeLists.txt
    ├── servers/Makefile     # DEPRECATED — see servers/CMakeLists.txt
    ├── drivers/Makefile     # DEPRECATED — see drivers/CMakeLists.txt
    ├── lib/Makefile         # DEPRECATED — see lib/CMakeLists.txt
    └── .../CMakeLists.txt   # NEW CMake build files
```

## Migrating New Components

When adding a new component:

1. **Create CMakeLists.txt** following `docs/cmake-migration-guide.md`
2. **Keep the Makefile** for backward compatibility during transition
3. **Add deprecation header** to the Makefile
4. **Test both** build systems before removing the Makefile

## Known Limitations

### CMake Prototype Limitations
- Magic/Bitcode passes not yet implemented
- Unpaged kernel objects from external libraries need manual setup
- Some library placeholders need real sources
- ASR (Address Space Randomization) not yet supported
- Release image building not yet implemented

### BSD Make Deprecation Timeline

| Milestone | Status | Details |
|-----------|--------|---------|
| **Phase 4** — Makefiles marked DEPRECATED | ✅ Current | All core Makefiles have deprecation headers; CMake is primary build system |
| **CMake production parity** — All builds work with CMake | 🔄 Next | Kernel, servers, drivers, libs all build correctly; release images can be built |
| **Makefile removal** — Core component Makefiles deleted | 📅 Planned | All `minix/{kernel,servers,drivers,lib,fs,net}/Makefile` removed |
| **build.sh removal** — Legacy build script deleted | 📅 Planned | `build.sh` removed; `releasetools/cmake-build.sh` is the only entry point |
| **share/mk removal** — BSD Make rules deleted | 📅 Planned | Entire `share/mk/` directory removed; no bmake dependency |
| **Final** — BSD Make completely removed from tree | 🎯 Target | NEXT major MINIX release

## Comparison

| Feature | BSD Make | CMake |
|---------|----------|-------|
| Build time (full) | ~30 min | ~15 min (estimated) |
| Incremental build | Slow | Fast (Ninja) |
| IDE integration | None | CLion, VS Code, Qt Creator |
| Cross-compilation | Complex (build.sh) | Toolchain files |
| Test integration | ATF/KYUA | CTest + ATF |
| Learning curve | Steep | Moderate |
| Maturity | Production | Prototype |
