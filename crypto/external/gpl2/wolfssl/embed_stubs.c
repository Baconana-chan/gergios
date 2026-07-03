/*
 * embed_stubs.c - Stub implementations for wolfSSL Embed I/O functions.
 *
 * wolfSSL's EmbedReceive/EmbedSend (and variants) are declared in
 * wolfssl/wolfio.h but NOT defined in this stripped MINIX distribution.
 * They are expected to be provided by the user as custom I/O callbacks.
 *
 * These stubs are needed for bare-metal builds (x86_64-elf / aarch64-elf)
 * where actual network I/O is not available. They return WOLFSSL_CBIO_ERR
 * (-1) to indicate "I/O error" — which is correct for bare-metal since
 * there is no network stack available.
 *
 * When building for a full MINIX user-space target with networking,
 * these stubs should be replaced by real implementations from io.c
 * or custom user callbacks.
 *
 * NOTE: WOLFSSL_CBIO_ERR may be inside #if defined(USE_WOLFSSL_IO) in
 * wolfio.h, which can be inactive when WOLFSSL_USER_IO is auto-defined
 * by settings.h. We define a fallback here to be safe.
 */

/* Ensure WOLFSSL_CBIO_ERR is available regardless of USE_WOLFSSL_IO.
 * Value -1 is the standard wolfSSL definition. */
#ifndef WOLFSSL_CBIO_ERR
#define WOLFSSL_CBIO_ERR  (-1)
#endif

#include <wolfssl/wolfio.h>
#include <wolfssl/internal.h>

/* EmbedReceive - default callback for receiving data.
 * Bare-metal: no network available, return error.
 */
int wolfSSL_EmbedReceive(WOLFSSL* ssl, char* buf, int sz, void* ctx)
{
    (void)ssl;
    (void)buf;
    (void)sz;
    (void)ctx;
    return WOLFSSL_CBIO_ERR;
}

/* EmbedSend - default callback for sending data.
 * Bare-metal: no network available, return error.
 */
int wolfSSL_EmbedSend(WOLFSSL* ssl, char* buf, int sz, void* ctx)
{
    (void)ssl;
    (void)buf;
    (void)sz;
    (void)ctx;
    return WOLFSSL_CBIO_ERR;
}

/* EmbedReceiveFrom - default callback for receiving datagram data.
 * Bare-metal: no network available, return error.
 */
int wolfSSL_EmbedReceiveFrom(WOLFSSL* ssl, char* buf, int sz, void* ctx)
{
    (void)ssl;
    (void)buf;
    (void)sz;
    (void)ctx;
    return WOLFSSL_CBIO_ERR;
}

/* EmbedSendTo - default callback for sending datagram data.
 * Bare-metal: no network available, return error.
 */
int wolfSSL_EmbedSendTo(WOLFSSL* ssl, char* buf, int sz, void* ctx)
{
    (void)ssl;
    (void)buf;
    (void)sz;
    (void)ctx;
    return WOLFSSL_CBIO_ERR;
}
