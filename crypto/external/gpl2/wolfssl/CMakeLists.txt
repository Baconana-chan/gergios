# crypto/external/gpl2/wolfssl/CMakeLists.txt
# wolfSSL library build (PROTOTYPE)
#
# This file builds wolfSSL as a static library for MINIX.
# In the full migration, this would use the upstream wolfSSL CMake build
# system with MINIX-specific configuration overrides.
#
# For now, this is a simplified build that compiles the core wolfSSL
# sources and links them into libwolfssl.a.

# ============================================================================
# Option: use upstream CMake or custom build
# ============================================================================

option(USE_WOLFSSL_UPSTREAM_CMAKE "Use wolfSSL's upstream CMake build" OFF)

if(USE_WOLFSSL_UPSTREAM_CMAKE)
    # Use wolfSSL's own CMake build system
    add_subdirectory(upstream)
    # wolfSSL upstream CMake will create wolfssl::wolfssl target
    return()
endif()

# ============================================================================
# Custom wolfSSL build for MINIX
# ============================================================================

# Locate wolfSSL source directory
# Try upstream dist/ first, then the local source tree
if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/dist/src/ssl.c")
    set(WOLFSSL_SRC_DIR "${CMAKE_CURRENT_SOURCE_DIR}/dist")
elseif(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/src/ssl.c")
    set(WOLFSSL_SRC_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
else()
    message(FATAL_ERROR
        "wolfSSL sources not found at ${CMAKE_CURRENT_SOURCE_DIR}/dist/ "
        "or ${CMAKE_CURRENT_SOURCE_DIR}/. "
        "Please fetch wolfSSL sources first.")
endif()

message(STATUS "wolfSSL sources: ${WOLFSSL_SRC_DIR}")

# Core wolfSSL sources (minimal set needed for MINIX components)
set(WOLFSSL_CORE_SOURCES
    ${WOLFSSL_SRC_DIR}/src/ssl.c
    ${WOLFSSL_SRC_DIR}/src/tls.c
    ${WOLFSSL_SRC_DIR}/src/tls13.c
    ${WOLFSSL_SRC_DIR}/src/internal.c
    ${WOLFSSL_SRC_DIR}/src/keys.c
    ${WOLFSSL_SRC_DIR}/src/crl.c
    ${WOLFSSL_SRC_DIR}/src/ocsp.c
)


# Cryptographic algorithm support
set(WOLFSSL_CRYPTO_SOURCES
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/rsa.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/dh.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/dsa.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/aes.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/des3.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/sha.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/sha256.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/sha512.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/md5.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/hmac.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/random.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/asn.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/coding.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/ecc.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/pwdbased.c
)

# OpenSSL compatibility layer
set(WOLFSSL_COMPAT_SOURCES
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/evp.c
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/evp_pk.c
)

# Bare-metal Embed I/O stubs (wolfSSL_EmbedReceive/Send/SendTo/ReceiveFrom)
# These are normally defined in src/io.c in a full wolfSSL distribution,
# but that file is not present in this MINIX-stripped dist. We provide
# stubs that return WOLFSSL_CBIO_ERR since bare-metal has no network.
set(WOLFSSL_EMBED_STUBS
    ${CMAKE_CURRENT_SOURCE_DIR}/embed_stubs.c
)

# Create the wolfSSL static library
add_library(wolfssl STATIC
    ${WOLFSSL_CORE_SOURCES}
    ${WOLFSSL_CRYPTO_SOURCES}
    ${WOLFSSL_COMPAT_SOURCES}
    ${WOLFSSL_EMBED_STUBS}
)

# Verify all source files exist
foreach(FILE ${WOLFSSL_CORE_SOURCES} ${WOLFSSL_CRYPTO_SOURCES} ${WOLFSSL_COMPAT_SOURCES})
    if(NOT EXISTS "${FILE}")
        message(WARNING "wolfSSL source not found: ${FILE}")
    endif()
endforeach()

# Include paths
# Note: MINIX_SOURCE_DIR must be passed from the root CMakeLists.txt.
# We add it manually here since wolfSSL builds as a subdirectory and
# doesn't automatically inherit the parent target's include paths.
target_include_directories(wolfssl
    PUBLIC
        $<BUILD_INTERFACE:${WOLFSSL_SRC_DIR}>
        $<BUILD_INTERFACE:${WOLFSSL_SRC_DIR}/include>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}  # for config.h
        ${MINIX_SOURCE_DIR}/include  # for time.h and other MINIX libc stubs
        ${MINIX_SOURCE_DIR}/sys      # for sys/ headers
        ${MINIX_SOURCE_DIR}/sys/arch/x86_64/include  # for arch-specific headers (cdefs.h etc.)
)

# MINIX-specific compile definitions for wolfSSL.
# NOTE: config.h (force-included via -include) is the authoritative
# source for wolfSSL defines. Only put defines here that are NOT
# already in config.h, to avoid -Wmacro-redefined warnings.
target_compile_definitions(wolfssl PRIVATE
    HAVE_THREAD_LS
    NO_MAIN_DRIVER
    HAVE_EX_DATA
    WOLFSSL_SHA384
    WOLFSSL_SHA512
    HAVE_TLS_EXTENSIONS
    KEEP_OUR_CERT
    HAVE_SUPPORTED_CURVES
    NO_ERROR_STRINGS
)

# Compile flags
target_compile_options(wolfssl PRIVATE
    -fno-stack-protector
    -fno-builtin
    -Wno-unused-function
    # Force-include config.h BEFORE any source/header to ensure our
    # bare-metal defines (WOLFSSL_IP4, XINET_PTON, etc.) are set
    # before wolfSSL's own headers (wolfio.h) can override them.
    "-include${CMAKE_CURRENT_SOURCE_DIR}/config.h"
)

# Install
install(TARGETS wolfssl
    ARCHIVE DESTINATION /usr/lib
)

install(DIRECTORY ${WOLFSSL_SRC_DIR}/include/
    DESTINATION /usr/include
    FILES_MATCHING
        PATTERN "*.h"
)
