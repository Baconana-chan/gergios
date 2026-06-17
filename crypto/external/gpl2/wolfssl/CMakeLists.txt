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
    ${WOLFSSL_SRC_DIR}/src/ocsp.asn
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
    ${WOLFSSL_SRC_DIR}/wolfcrypt/src/openssl.c
)

# Create the wolfSSL static library
add_library(wolfssl STATIC
    ${WOLFSSL_CORE_SOURCES}
    ${WOLFSSL_CRYPTO_SOURCES}
    ${WOLFSSL_COMPAT_SOURCES}
)

# Include paths
target_include_directories(wolfssl
    PUBLIC
        $<BUILD_INTERFACE:${WOLFSSL_SRC_DIR}>
        $<BUILD_INTERFACE:${WOLFSSL_SRC_DIR}/include>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}  # for config.h
)

# MINIX-specific compile definitions for wolfSSL
target_compile_definitions(wolfssl PRIVATE
    WOLFSSL_MINIX
    WOLFSSL_OPENSSL_COMPATIBLE
    OPENSSL_EXTRA
    HAVE_THREAD_LS
    NO_FILESYSTEM      # MINIX kernel doesn't have full filesystem
    NO_MAIN_DRIVER
    NO_ERROR_STRINGS   # Reduce size
    HAVE_EX_DATA
    WOLFSSL_SHA384
    WOLFSSL_SHA512
    NO_DES3
    NO_RC4
    NO_MD4
    HAVE_TLS_EXTENSIONS
    KEEP_OUR_CERT
    HAVE_SUPPORTED_CURVES
)

# Compile flags
target_compile_options(wolfssl PRIVATE
    -fno-stack-protector
    -fno-builtin
    -Wno-unused-function
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
