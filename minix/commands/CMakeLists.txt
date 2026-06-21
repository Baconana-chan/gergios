# minix/commands/CMakeLists.txt
# Userland command build
#
# Replicates minix/commands/Makefile and minix/commands/Makefile.inc
#
# All commands get -lasyn -lterminfo by default (from Makefile.inc)
# and install to /usr/bin unless overridden.

# ============================================================================
# Helper: simple command (just PROG=name with no extra flags)
# ============================================================================

function(_add_simple_command NAME)
    # Each command is in its own subdirectory: commands/{NAME}/{NAME}.c
    set(_SRC_DIR "${CMAKE_CURRENT_SOURCE_DIR}/${NAME}")
    add_minix_executable(${NAME}
        SOURCES "${_SRC_DIR}/${NAME}.c"
        LIBS asyn terminfo
        BINDIR /usr/bin
    )
    # Install man page if it exists (in the command's subdirectory)
    if(EXISTS "${_SRC_DIR}/${NAME}.1")
        install(FILES "${_SRC_DIR}/${NAME}.1" DESTINATION /usr/share/man/man1)
    elseif(EXISTS "${_SRC_DIR}/${NAME}.8")
        install(FILES "${_SRC_DIR}/${NAME}.8" DESTINATION /usr/share/man/man8)
    endif()
endfunction()

# ============================================================================
# Helper: command with explicit sources
# ============================================================================

function(_add_command NAME)
    cmake_parse_arguments(PARSE_ARGV 1 ARG
        ""
        "BINDIR"
        "SOURCES;LIBS;MAN"
    )
    # Each command is in its own subdirectory: commands/{NAME}/
    set(_SRC_DIR "${CMAKE_CURRENT_SOURCE_DIR}/${NAME}")
    if(NOT ARG_SOURCES)
        set(ARG_SOURCES "${NAME}.c")
    endif()
    # Prepend subdirectory path to each source
    set(_FULL_SOURCES)
    foreach(_SRC ${ARG_SOURCES})
        if(IS_ABSOLUTE "${_SRC}")
            list(APPEND _FULL_SOURCES "${_SRC}")
        else()
            list(APPEND _FULL_SOURCES "${_SRC_DIR}/${_SRC}")
        endif()
    endforeach()
    if(NOT ARG_BINDIR)
        set(ARG_BINDIR "/usr/bin")
    endif()
    set(_LIBS ${ARG_LIBS} asyn terminfo)
    add_minix_executable(${NAME}
        SOURCES ${_FULL_SOURCES}
        LIBS ${_LIBS}
        BINDIR ${ARG_BINDIR}
    )
    if(ARG_MAN)
        install(FILES "${_SRC_DIR}/${ARG_MAN}" DESTINATION /usr/share/man/man1)
    endif()
endfunction()

# ============================================================================
# Helper: script-only command (installed as-is, no compilation)
# ============================================================================

function(_add_script NAME)
    cmake_parse_arguments(PARSE_ARGV 1 ARG
        ""
        "BINDIR;SOURCE"
        ""
    )
    set(_SRC_DIR "${CMAKE_CURRENT_SOURCE_DIR}/${NAME}")
    if(NOT ARG_SOURCE)
        set(ARG_SOURCE "${NAME}.sh")
    endif()
    if(NOT IS_ABSOLUTE "${ARG_SOURCE}")
        set(ARG_SOURCE "${_SRC_DIR}/${ARG_SOURCE}")
    endif()
    if(NOT ARG_BINDIR)
        set(ARG_BINDIR "/usr/bin")
    endif()
    install(PROGRAMS ${ARG_SOURCE}
        DESTINATION ${ARG_BINDIR}
        RENAME ${NAME}
    )
endfunction()

# ============================================================================
# Commands (matching the SUBDIR list from commands/Makefile)
# ============================================================================

# Simple commands (one .c file, same name, no special flags)
_add_simple_command(at)
_add_simple_command(cawf)
_add_simple_command(cdprobe)
_add_simple_command(cleantmp)
_add_command(compress LIBS z)
_add_simple_command(crontab)
# devmand has main.c not devmand.c — use explicit add_minix_executable
add_minix_executable(devmand
    SOURCES ${CMAKE_CURRENT_SOURCE_DIR}/devmand/main.c
    LIBS asyn terminfo
    BINDIR /usr/bin
)
_add_simple_command(devsize)
_add_simple_command(dhrystone)
_add_simple_command(eject)
_add_simple_command(fix)
_add_simple_command(format)
_add_simple_command(ifdef)
_add_simple_command(intr)
_add_simple_command(isoread)
_add_simple_command(loadkeys)
_add_simple_command(look)
_add_simple_command(lp)
_add_simple_command(mail)
_add_simple_command(minix-service)
_add_simple_command(mt)
_add_simple_command(prep)
_add_simple_command(printroot)
_add_simple_command(progressbar)
_add_simple_command(rawspeed)
_add_simple_command(readclock)
_add_simple_command(remsync)
_add_script(rotate SOURCE rotate.sh)
_add_simple_command(slip)
_add_script(spell SOURCE spell.sh)
_add_script(srccrc SOURCE srccrc.sh)
_add_simple_command(svrctl)
# FIXME: swifi builds from systest.c with -lelf — needs explicit add_minix_executable
_add_simple_command(synctree)
_add_simple_command(sysenv)
_add_simple_command(term)
_add_simple_command(tget)
_add_simple_command(truncate)
_add_simple_command(umount)
_add_simple_command(update)
_add_script(version SOURCE version.sh)
_add_simple_command(vol)
# FIXME: zmodem builds rz and sz as separate programs — needs two add_minix_executable calls
_add_simple_command(atnormalize)
_add_simple_command(dosread)
_add_simple_command(fdisk)
_add_simple_command(loadfont)
_add_simple_command(part)
_add_simple_command(partition)
_add_simple_command(playwave)
_add_simple_command(recwave)

# Simple commands with .c and multiple sources
_add_command(backup SOURCES backup.c)
_add_command(crc SOURCES crc.c)
_add_command(cron SOURCES cron.c tab.c misc.c)
_add_script(DESCRIBE SOURCE DESCRIBE.sh)
_add_command(gcov-pull SOURCES gcov-pull.c)
_add_command(lpd SOURCES lpd.c)
_add_script(lspci SOURCE lspci.sh)
_add_script(netconf SOURCE netconf.sh)
_add_script(pkgin_all SOURCE pkgin_all.sh)
_add_script(pkgin_cd SOURCE pkgin_cd.sh)
_add_script(pkgin_sets SOURCE pkgin_sets.sh)
_add_command(profile SOURCES profile.c)
_add_command(ramdisk SOURCES ramdisk.c)
_add_command(termcap SOURCES termcap.c)
_add_script(worldstone SOURCE worldstone.sh)
_add_command(autopart SOURCES autopart.c)
_add_command(repartition SOURCES repartition.c)
_add_command(screendump SOURCES screendump.c)

# Commands with special libraries or flags
_add_command(fsck.mfs
    SOURCES fsck.c
    LIBS sys
)

_add_command(loadramdisk
    SOURCES loadramdisk.c
)

_add_command(mount
    SOURCES mount.c
    BINDIR /bin
)

# fetch needs libfetch + wolfSSL (when MKCRYPTO)
if(MKCRYPTO)
    _add_command(fetch
        SOURCES fetch.c
        LIBS fetch wolfssl
    )
    target_include_directories(fetch PRIVATE
        ${MINIX_SOURCE_DIR}/crypto/external/gpl2/wolfssl/dist
        ${MINIX_SOURCE_DIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
        ${MINIX_SOURCE_DIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl
    )
    target_compile_definitions(fetch PRIVATE
        HAVE_SYS_PARAM_H HAVE_SYS_IOCTL_H HAVE_SYS_SOCKET_H
        HAVE_SYS_STAT_H HAVE_SYS_TIME_H HAVE_ERR_H
        HAVE_STDINT_H HAVE_TERMIOS_H HAVE_SYSEXITS_H HAVE_UTIME_H
    )
else()
    _add_command(fetch SOURCES fetch.c LIBS fetch)
endif()

# Script-only commands
_add_script(setup SOURCE setup.sh BINDIR /bin)
_add_script(MAKEDEV SOURCE MAKEDEV.sh)

_add_script(updateboot SOURCE updateboot.sh BINDIR /usr/sbin)

# update_asr — for ASR support
if(USE_ASR)
    _add_command(update_asr SOURCES update_asr.c)
endif()

_add_script(update_bootcfg SOURCE update_bootcfg.sh BINDIR /usr/sbin)

# sprofalyze/spofdiff
_add_command(sprofalyze SOURCES sprofalyze.c)
_add_command(sprofdiff SOURCES sprofdiff.c)

# writeisofs
_add_command(writeisofs SOURCES writeisofs.c)
