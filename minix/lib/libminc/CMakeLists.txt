# minix/lib/libminc/CMakeLists.txt
# Minimal C library for servers and drivers (libminc)
#
# Replicates minix/lib/libminc/Makefile
#
# NOTE: libminc has an unusually complex build that imports individual
# .o files from multiple source trees (libc, libsa, libsys, compiler_rt).
# This CMakeLists.txt provides the structure; full source paths need
# to be verified against the actual Makefile.

# Source directory paths
set(LIBSADIR "${MINIX_SOURCE_DIR}/sys/lib/libsa")
set(LIBSYSDIR "${MINIX_SOURCE_DIR}/minix/lib/libsys")
set(LIBMINIXCDIR "${MINIX_SOURCE_DIR}/minix/lib/libc")
set(LIBCDIR "${MINIX_SOURCE_DIR}/lib/libc")
set(LIBCOMMONCDIR "${MINIX_SOURCE_DIR}/common/lib/libc")
set(COMPILER_RTDIR "${MINIX_SOURCE_DIR}/sys/external/bsd/compiler_rt/dist/lib/builtins")

# Own sources
set(LIBMINC_OWN_SOURCES
    atoi.c
    fputs.c
    _snprintf.c
    strtol.c
)

# Sources from libsa (StandAlone library)
set(LIBMINC_LIBSA_SOURCES
    ${LIBSADIR}/errno.c
    ${LIBSADIR}/printf.c
    ${LIBSADIR}/strerror.c
    ${LIBSADIR}/subr_prf.c
)

# Sources from libsys
set(LIBMINC_LIBSYS_SOURCES
    ${LIBSYSDIR}/kputc.c
    ${LIBSYSDIR}/sys_diagctl.c
)

# Sources from common/lib/libc
set(LIBMINC_COMMON_SOURCES
    ${LIBCOMMONCDIR}/gen/bswap64.c
    ${LIBCOMMONCDIR}/gen/rb.c
    ${LIBCOMMONCDIR}/hash/sha2/sha2.c
    ${LIBCOMMONCDIR}/stdlib/random.c
    ${LIBCOMMONCDIR}/stdlib/strtoul.c
    ${LIBCOMMONCDIR}/string/strcasecmp.c
    ${LIBCOMMONCDIR}/string/strcspn.c
    ${LIBCOMMONCDIR}/string/strncasecmp.c
    ${LIBCOMMONCDIR}/string/strnlen.c
    ${LIBCOMMONCDIR}/string/strlcat.c
)

# Sources from compiler-rt
set(LIBMINC_COMPILERT_SOURCES
    ${COMPILER_RTDIR}/ashrdi3.c
    ${COMPILER_RTDIR}/divdi3.c
    ${COMPILER_RTDIR}/lshrdi3.c
    ${COMPILER_RTDIR}/moddi3.c
    ${COMPILER_RTDIR}/udivdi3.c
    ${COMPILER_RTDIR}/umoddi3.c
    ${COMPILER_RTDIR}/umodsi3.c
    ${COMPILER_RTDIR}/udivsi3.c
    ${COMPILER_RTDIR}/udivmoddi4.c
    ${COMPILER_RTDIR}/divsi3.c
    ${COMPILER_RTDIR}/modsi3.c
    ${COMPILER_RTDIR}/divmoddi4.c
    ${COMPILER_RTDIR}/divmodsi4.c
    ${COMPILER_RTDIR}/udivmodsi4.c
)

# Sources from libc/gen
set(LIBMINC_LIBCGEN_SOURCES
    ${LIBCDIR}/gen/_errno.c
    ${LIBCDIR}/gen/getprogname.c
    ${LIBCDIR}/gen/setprogname.c
    ${LIBCDIR}/gen/execle.c
    ${LIBCDIR}/gen/sleep.c
    ${LIBCDIR}/gen/time.c
    ${LIBCDIR}/gen/ctype_.c
    ${LIBCDIR}/gen/tolower_.c
    ${LIBCDIR}/gen/toupper_.c
    ${LIBCDIR}/gen/usleep.c
    ${LIBCDIR}/gen/waitpid.c
    ${LIBCDIR}/gen/sigsetops.c
)

# Sources from libc/stdlib
set(LIBMINC_LIBCSTDLIB_SOURCES
    ${LIBCDIR}/stdlib/abort.c
    ${LIBCDIR}/stdlib/atexit.c
    ${LIBCDIR}/stdlib/_env.c
    ${LIBCDIR}/stdlib/exit.c
    ${LIBCDIR}/stdlib/getenv.c
    ${LIBCDIR}/stdlib/ldiv.c
    ${LIBCDIR}/stdlib/malloc.c
    ${LIBCDIR}/stdlib/setenv.c
    ${LIBCDIR}/stdlib/reallocarr.c
    ${LIBCDIR}/stdlib/_rand48.c
    ${LIBCDIR}/stdlib/lrand48.c
    ${LIBCDIR}/stdlib/srand48.c
)

# Sources from libc/string
set(LIBMINC_LIBCSTR_SOURCES
    ${LIBCDIR}/string/strdup.c
)

# Sources from libc/regex
set(LIBMINC_LIBCREGEX_SOURCES
    ${LIBCDIR}/regex/regcomp.c
    ${LIBCDIR}/regex/regerror.c
    ${LIBCDIR}/regex/regexec.c
    ${LIBCDIR}/regex/regfree.c
)

# MINIX-specific libc sources
set(LIBMINC_MINIX_GEN_SOURCES
    ${LIBMINIXCDIR}/gen/read_tsc_64.c
    ${LIBMINIXCDIR}/gen/fslib.c
    ${LIBMINIXCDIR}/gen/itoa.c
)

# libc/misc sources
set(LIBMINC_LIBCMISC_SOURCES
    ${LIBCDIR}/misc/initfini.c
    ${LIBCDIR}/misc/stack_protector.c
)

# libc/time sources
set(LIBMINC_LIBC_TIME_SOURCES
    ${LIBCDIR}/time/localtime.c
)

# Architecture-specific sources (from minix/lib/libc/arch/${MACHINE_CPU})
# NOTE: i386-specific arch sources removed in Phase 4 i386 deprecation.
set(LIBMINC_ARCH_SOURCES
    ${LIBMINIXCDIR}/arch/${MACHINE_CPU}/sys/brksize.S
    ${LIBMINIXCDIR}/arch/${MACHINE_CPU}/sys/_do_kernel_call_intr.S
    ${LIBMINIXCDIR}/arch/${MACHINE_CPU}/sys/ipc_minix_kerninfo.S
    ${LIBMINIXCDIR}/arch/${MACHINE_CPU}/sys/_ipc.S
    ${LIBMINIXCDIR}/arch/${MACHINE_CPU}/sys/ucontext.S
)

# MINIX syscall wrappers (from minix/lib/libc/sys)
set(LIBMINC_MINIX_SYS_SOURCES
    ${LIBMINIXCDIR}/sys/access.c
    ${LIBMINIXCDIR}/sys/brk.c
    ${LIBMINIXCDIR}/sys/close.c
    ${LIBMINIXCDIR}/sys/environ.c
    ${LIBMINIXCDIR}/sys/execve.c
    ${LIBMINIXCDIR}/sys/fork.c
    ${LIBMINIXCDIR}/sys/fsync.c
    ${LIBMINIXCDIR}/sys/getgid.c
    ${LIBMINIXCDIR}/sys/getpid.c
    ${LIBMINIXCDIR}/sys/geteuid.c
    ${LIBMINIXCDIR}/sys/getuid.c
    ${LIBMINIXCDIR}/sys/gettimeofday.c
    ${LIBMINIXCDIR}/sys/getvfsstat.c
    ${LIBMINIXCDIR}/sys/init.c
    ${LIBMINIXCDIR}/sys/kernel_utils.c
    ${LIBMINIXCDIR}/sys/kill.c
    ${LIBMINIXCDIR}/sys/link.c
    ${LIBMINIXCDIR}/sys/loadname.c
    ${LIBMINIXCDIR}/sys/lseek.c
    ${LIBMINIXCDIR}/sys/_mcontext.c
    ${LIBMINIXCDIR}/sys/minix_rs.c
    ${LIBMINIXCDIR}/sys/mknod.c
    ${LIBMINIXCDIR}/sys/mmap.c
    ${LIBMINIXCDIR}/sys/nanosleep.c
    ${LIBMINIXCDIR}/sys/open.c
    ${LIBMINIXCDIR}/sys/pread.c
    ${LIBMINIXCDIR}/sys/pwrite.c
    ${LIBMINIXCDIR}/sys/read.c
    ${LIBMINIXCDIR}/sys/sbrk.c
    ${LIBMINIXCDIR}/sys/select.c
    ${LIBMINIXCDIR}/sys/setuid.c
    ${LIBMINIXCDIR}/sys/sigprocmask.c
    ${LIBMINIXCDIR}/sys/stack_utils.c
    ${LIBMINIXCDIR}/sys/stat.c
    ${LIBMINIXCDIR}/sys/stime.c
    ${LIBMINIXCDIR}/sys/svrctl.c
    ${LIBMINIXCDIR}/sys/syscall.c
    ${LIBMINIXCDIR}/sys/__sysctl.c
    ${LIBMINIXCDIR}/sys/_ucontext.c
    ${LIBMINIXCDIR}/sys/umask.c
    ${LIBMINIXCDIR}/sys/unlink.c
    ${LIBMINIXCDIR}/sys/wait4.c
    ${LIBMINIXCDIR}/sys/write.c
)

# Generated errlist.c
set(LIBMINC_ERRNO_SOURCE "${CMAKE_CURRENT_BINARY_DIR}/errlist.c")
add_custom_command(
    OUTPUT "${LIBMINC_ERRNO_SOURCE}"
    DEPENDS
        ${LIBCDIR}/gen/errlist.awk
        ${MINIX_SOURCE_DIR}/sys/sys/errno.h
    COMMAND
        ${CMAKE_COMMAND} -E env
            SHELL="${CMAKE_COMMAND} -E"
        ${CMAKE_COMMAND} -E env sh -c
            "cat '${MINIX_SOURCE_DIR}/sys/sys/errno.h'
             | sed 's/(_SIGN//'
             | awk -f '${LIBCDIR}/gen/errlist.awk'
             > '${LIBMINC_ERRNO_SOURCE}'"
    COMMENT "Generating errlist.c"
)

add_minix_library(minc
    SOURCES
        ${LIBMINC_OWN_SOURCES}
        ${LIBMINC_LIBSA_SOURCES}
        ${LIBMINC_LIBSYS_SOURCES}
        ${LIBMINC_COMMON_SOURCES}
        ${LIBMINC_COMPILERT_SOURCES}
        ${LIBMINC_LIBCGEN_SOURCES}
        ${LIBMINC_LIBCSTDLIB_SOURCES}
        ${LIBMINC_LIBCSTR_SOURCES}
        ${LIBMINC_LIBCREGEX_SOURCES}
        ${LIBMINC_MINIX_GEN_SOURCES}
        ${LIBMINC_LIBCMISC_SOURCES}
        ${LIBMINC_LIBC_TIME_SOURCES}
        ${LIBMINC_ARCH_SOURCES}
        ${LIBMINC_MINIX_SYS_SOURCES}
        "${LIBMINC_ERRNO_SOURCE}"
    INSTALL_DIR /usr/lib
)

# Compile definitions
target_compile_definitions(minc PRIVATE
    _LIBMINC
)

target_compile_options(minc PRIVATE
    -fno-builtin
    -Wno-missing-prototypes
    -Wno-old-style-definition
    -Wno-strict-prototypes
    -Wno-uninitialized
    -Wno-cast-qual
)

# Per-file compile definitions
set_source_files_properties(strtol.c PROPERTIES
    COMPILE_DEFINITIONS "_STANDALONE"
    INCLUDE_DIRECTORIES "${LIBCOMMONCDIR}/stdlib;${MINIX_SOURCE_DIR}/sys"
)
set_source_files_properties(strtoul.c PROPERTIES
    COMPILE_DEFINITIONS "_STANDALONE"
    INCLUDE_DIRECTORIES "${MINIX_SOURCE_DIR}/sys"
)
set_source_files_properties(subr_prf.c PROPERTIES
    COMPILE_DEFINITIONS "putchar=kputc;LIBSA_PRINTF_LONGLONG_SUPPORT;LIBSA_PRINTF_WIDTH_SUPPORT"
)
set_source_files_properties(strerror.c PROPERTIES
    INCLUDE_DIRECTORIES "${LIBSADIR}"
)
set_source_files_properties(rb.c PROPERTIES
    COMPILE_DEFINITIONS "_LIBC"
)
set_source_files_properties(sha2.c PROPERTIES
    INCLUDE_DIRECTORIES "${MINIX_SOURCE_DIR}/sys"
)
set_source_files_properties(stack_protector.c PROPERTIES
    COMPILE_DEFINITIONS "xprintf=printf"
)
set_source_files_properties(malloc.c PROPERTIES
    COMPILE_DEFINITIONS "_LIBSYS"
)
set_source_files_properties(fslib.c PROPERTIES
    INCLUDE_DIRECTORIES "${MINIX_SOURCE_DIR}/minix/fs"
)

# Include paths
target_include_directories(minc PRIVATE
    ${LIBCDIR}/include
    ${LIBCDIR}
)
