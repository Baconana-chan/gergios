# CMake Migration Guide for MINIX Developers

## Overview

This guide documents the transition from BSD Make (bmake) to CMake for the
MINIX build system. It covers best practices, common patterns, and how to
migrate existing Makefiles to CMakeLists.txt.

## Quick Reference: BSD Make → CMake Mapping

| BSD Make | CMake Equivalent |
|----------|-----------------|
| `PROG=foo` | `add_executable(foo foo.c)` |
| `LIB=foo` | `add_library(foo STATIC foo.c)` |
| `SRCS=...` | `add_library/executable( ... a.c b.c)` |
| `LDADD+= -lfoo` | `target_link_libraries(target PRIVATE foo)` |
| `DPADD+= ${LIBFOO}` | Handled automatically by CMake target dependencies |
| `CPPFLAGS+= -DFOO` | `target_compile_definitions(target PRIVATE FOO)` |
| `CFLAGS+= -O2` | `target_compile_options(target PRIVATE -O2)` |
| `BINDIR=/usr/sbin` | `install(TARGETS target RUNTIME DESTINATION /usr/sbin)` |
| `SUBDIR+= foo .WAIT bar` | `add_subdirectory(foo)` then `add_dependencies(bar foo)` |
| `MAN=` | `install(FILES foo.1 DESTINATION /usr/share/man/man1)` |
| `CLEANFILES+=` | Handled automatically by CMake |

## Migration Steps for a Single Component

### 1. Create `CMakeLists.txt`

Start with the CMake equivalent of the component's Makefile:

**Before (BSD Make):**
```makefile
PROG=    hello
SRCS=    hello.c
BINDIR=  /usr/bin
LDADD+=  -lm
MAN=
.include <bsd.prog.mk>
```

**After (CMake):**
```cmake
add_executable(hello hello.c)
target_link_libraries(hello PRIVATE m)
install(TARGETS hello RUNTIME DESTINATION /usr/bin)
```

### 2. Handle MINIX-specific Patterns

For MINIX services (servers/drivers), use the provided macros:

```cmake
include(macros)

add_minix_service(myservice
    SOURCES main.c helper.c
    LIBS sys timers
)
```

For kernel components with unpaged objects:

```cmake
include(macros)

add_unpaged_objects(TARGET kernel
    OBJECTS head.o pre_init.o
    FROM_DIR arch/${MACHINE_ARCH}
)
```

### 3. Handle Conditional Compilation

**BSD Make:**
```makefile
.if ${USE_ACPI} != "no"
SRCS+= acpi.c
CPPFLAGS+= -DUSE_ACPI
.endif
```

**CMake:**
```cmake
if(USE_ACPI)
    target_sources(my_prog PRIVATE acpi.c)
    target_compile_definitions(my_prog PRIVATE USE_ACPI)
endif()
```

### 4. Handle Architecture-Specific Code

**BSD Make:**
```makefile
.include "arch/${MACHINE_ARCH}/Makefile.inc"
```

**CMake:**
```cmake
if(MACHINE_ARCH STREQUAL "i386")
    target_sources(kernel PRIVATE arch/i386/foo.c)
elseif(MACHINE_ARCH STREQUAL "earm")
    target_sources(kernel PRIVATE arch/earm/foo_arm.c)
endif()
```

## CMake Best Practices for MINIX

### 1. Use Toolchain Files

Always configure cross-compilation via a CMake toolchain file:

```bash
cmake -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
      -DMACHINE_ARCH=i386 \
      ..
```

### 2. Out-of-Source Builds

Always build in a separate directory:

```bash
mkdir build && cd build
cmake .. -G Ninja
ninja
ctest
```

### 3. Use Ninja for Speed

Ninja is significantly faster than Make for incremental builds:

```bash
cmake .. -G Ninja
ninja -j$(nproc)
```

### 4. Define Targets, Not Commands

| Don't | Do |
|-------|-----|
| `add_custom_target(run ...)` | `add_test(NAME run ...)` |
| Shell scripts for everything | CMake `add_custom_command()` |
| Manual dependency tracking | CMake automatic `.d` file support |

### 5. Organize CMake Modules

Place reusable functions in `cmake/`:

```
cmake/
├── macros.cmake        # add_minix_executable, add_minix_service, etc.
├── options.cmake       # MK*/USE* option handling
├── arch/
│   ├── i386.cmake      # i386 arch flags
│   └── earm.cmake      # ARM arch flags
└── toolchain-minix.cmake  # Cross-compilation toolchain
```

### 6. Preserve Build Options

Convert `MK*` variables to CMake options:

```bash
# Old way (BSD Make):
make USE_ACPI=no MKSMALL=yes

# New way (CMake):
cmake -DUSE_ACPI=OFF -DMKSMALL=ON ..
```

### 7. Use Generator Expressions

For per-config or per-platform flags, use generator expressions:

```cmake
target_compile_definitions(kernel PRIVATE
    $<$<CONFIG:Debug>:_DEBUG>
    $<${USE_SMP}:CONFIG_SMP>
)
```

### 8. Handle Generated Files

Use `add_custom_command` for generated headers:

```cmake
add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/procoffsets.h
    DEPENDS procoffsets.cf kernel.h
    COMMAND genassym ... > procoffsets.h
)
target_sources(kernel PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/procoffsets.h)
```

## Common Pitfalls

### 1. Linker Scripts

CMake doesn't natively handle custom linker scripts. Use:

```cmake
target_link_options(kernel PRIVATE
    -T "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/kernel.lds"
)
```

### 2. Assembly Sources

CMake handles `.S` and `.s` files automatically, but MINIX uses `.S` for
preprocessed assembly. Ensure `.S` files are listed in sources.

### 3. Object File Symbol Prefixing

The `__k_unpaged_` prefix for kernel objects requires custom post-processing.
Use `add_unpaged_objects()` macro from `cmake/macros.cmake`.

### 4. Bitcode/MAGIC Passes

MINIX uses LLVM bitcode and custom MAGIC/ASR passes. These require
special CMake handling via custom commands. See `cmake/macros.cmake` and
`cmake/bitcode.cmake` (future work).

### 5. Cross-Compilation Include Paths

MINIX headers are organized by architecture:

```cmake
target_include_directories(kernel PRIVATE
    arch/${MACHINE_ARCH}
    arch/${MACHINE_ARCH}/include
    ${MINIX_SOURCE_DIR}/minix/include/arch/${MACHINE_ARCH}/include
)
```

## Testing the CMake Build

### Quick Smoke Test

```bash
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Debug -DMACHINE_ARCH=i386
cmake --build . --target wolfssl  # Build only wolfSSL
cmake --build . --target kernel    # Build only kernel
ctest -R wolfssl                   # Run wolfSSL tests only
```

### Full Build Test

```bash
cmake --build .
ctest --output-on-failure
```

## Migration Checklist

For each component:

- [ ] Create `CMakeLists.txt` in component directory
- [ ] Map `SRCS` to `target_sources` or `add_executable` sources
- [ ] Map `LDADD` to `target_link_libraries`
- [ ] Map `CPPFLAGS` to `target_compile_definitions`
- [ ] Map `BINDIR` to `install(TARGETS ... RUNTIME DESTINATION ...)`
- [ ] Handle `MAN` pages via `install(FILES)`
- [ ] Handle `CLEANFILES` (automatic in CMake)
- [ ] Convert conditional `.if` blocks to `if()` blocks
- [ ] Add to parent `CMakeLists.txt` via `add_subdirectory()`
- [ ] Test: `cmake --build . --target <component>`
- [ ] Add to CTest if applicable
