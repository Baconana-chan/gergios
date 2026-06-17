/* openssl_compat.h - OpenSSL compatibility wrappers for wolfSSL (DEPRECATED)
 * 
 * *** DEPRECATED ***
 * This file is no longer needed by migrated MINIX components.
 * All migrated code now uses direct wolfssl/openssl/*.h includes.
 *
 * This file is retained for reference but should not be included
 * by new code. Use individual wolfssl/openssl/*.h headers instead.
 *
 * Old components that have not yet been migrated still use this
 * file for backwards compatibility.
 */

#ifndef OPENSSL_COMPAT_H
#define OPENSSL_COMPAT_H

/* Include wolfSSL OpenSSL compatibility layer */
#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/crypto.h>
#include <wolfssl/openssl/x509.h>
#include <wolfssl/openssl/pem.h>
#include <wolfssl/openssl/err.h>
#include <wolfssl/openssl/dh.h>
#include <wolfssl/openssl/bn.h>
#include <wolfssl/openssl/rsa.h>
#include <wolfssl/openssl/dsa.h>
#include <wolfssl/openssl/evp.h>
#include <wolfssl/openssl/rand.h>
#include <wolfssl/openssl/sha.h>
#include <wolfssl/openssl/md5.h>
#include <wolfssl/openssl/asn1.h>
#include <wolfssl/openssl/bio.h>

/* Ensure compatibility macros are defined */
#ifndef OPENSSL_VERSION_NUMBER
#define OPENSSL_VERSION_NUMBER 0x00908000L /* OpenSSL 0.9.8 compatibility */
#endif

#ifndef OPENSSL_VERSION_TEXT
#define OPENSSL_VERSION_TEXT "OpenSSL 0.9.8-compatible (wolfSSL)"
#endif

/* OpenSSL 0.9.8 specific compatibility */
#ifndef SSL_OP_NO_SSLv2
#define SSL_OP_NO_SSLv2 0x01000000L
#endif

#ifndef SSL_OP_NO_SSLv3
#define SSL_OP_NO_SSLv3 0x02000000L
#endif

#ifndef SSL_OP_NO_TLSv1
#define SSL_OP_NO_TLSv1 0x04000000L
#endif

#ifndef SSL_OP_NO_TLSv1_1
#define SSL_OP_NO_TLSv1_1 0x10000000L
#endif

#ifndef SSL_OP_NO_TLSv1_2
#define SSL_OP_NO_TLSv1_2 0x08000000L
#endif

#ifndef SSL_OP_SINGLE_DH_USE
#define SSL_OP_SINGLE_DH_USE 0x00100000L
#endif

#ifndef SSL_OP_SINGLE_ECDH_USE
#define SSL_OP_SINGLE_ECDH_USE 0x00200000L
#endif

#ifndef SSL_OP_NO_COMPRESSION
#define SSL_OP_NO_COMPRESSION 0x00020000L
#endif

/* SSL mode flags */
#ifndef SSL_MODE_ENABLE_PARTIAL_WRITE
#define SSL_MODE_ENABLE_PARTIAL_WRITE 0x00000001L
#endif

#ifndef SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER
#define SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER 0x00000002L
#endif

#ifndef SSL_MODE_AUTO_RETRY
#define SSL_MODE_AUTO_RETRY 0x00000004L
#endif

/* SSL verification flags */
#ifndef SSL_VERIFY_NONE
#define SSL_VERIFY_NONE 0x00
#endif

#ifndef SSL_VERIFY_PEER
#define SSL_VERIFY_PEER 0x01
#endif

#ifndef SSL_VERIFY_FAIL_IF_NO_PEER_CERT
#define SSL_VERIFY_FAIL_IF_NO_PEER_CERT 0x02
#endif

#ifndef SSL_VERIFY_CLIENT_ONCE
#define SSL_VERIFY_CLIENT_ONCE 0x04
#endif

/* File type constants */
#ifndef X509_FILETYPE_PEM
#define X509_FILETYPE_PEM 1
#endif

#ifndef X509_FILETYPE_ASN1
#define X509_FILETYPE_ASN1 2
#endif

/* Error codes */
#ifndef SSL_ERROR_NONE
#define SSL_ERROR_NONE 0
#endif

#ifndef SSL_ERROR_SSL
#define SSL_ERROR_SSL 1
#endif

#ifndef SSL_ERROR_WANT_READ
#define SSL_ERROR_WANT_READ 2
#endif

#ifndef SSL_ERROR_WANT_WRITE
#define SSL_ERROR_WANT_WRITE 3
#endif

#ifndef SSL_ERROR_SYSCALL
#define SSL_ERROR_SYSCALL 5
#endif

#ifndef SSL_ERROR_ZERO_RETURN
#define SSL_ERROR_ZERO_RETURN 6
#endif

/* DH parameter functions compatibility */
#ifndef DH_new
#define DH_new wolfSSL_DH_new
#endif

#ifndef DH_free
#define DH_free wolfSSL_DH_free
#endif

#ifndef DH_generate_parameters
#define DH_generate_parameters wolfSSL_DH_generate_parameters
#endif

/* BN functions compatibility */
#ifndef BN_new
#define BN_new wolfSSL_BN_new
#endif

#ifndef BN_free
#define BN_free wolfSSL_BN_free
#endif

#ifndef BN_num_bits
#define BN_num_bits wolfSSL_BN_num_bits
#endif

#ifndef BN_bin2bn
#define BN_bin2bn wolfSSL_BN_bin2bn
#endif

#ifndef BN_bn2bin
#define BN_bn2bin wolfSSL_BN_bn2bin
#endif

/* EVP functions compatibility */
#ifndef EVP_md5
#define EVP_md5 wolfSSL_EVP_md5
#endif

#ifndef EVP_sha1
#define EVP_sha1 wolfSSL_EVP_sha1
#endif

#ifndef EVP_sha256
#define EVP_sha256 wolfSSL_EVP_sha256
#endif

#ifndef EVP_sha512
#define EVP_sha512 wolfSSL_EVP_sha512
#endif

#ifndef EVP_get_digestbyname
#define EVP_get_digestbyname wolfSSL_EVP_get_digestbyname
#endif

/* X509 functions compatibility */
#ifndef X509_free
#define X509_free wolfSSL_X509_free
#endif

#ifndef X509_get_subject_name
#define X509_get_subject_name wolfSSL_X509_get_subject_name
#endif

#ifndef X509_get_issuer_name
#define X509_get_issuer_name wolfSSL_X509_get_issuer_name
#endif

#ifndef X509_digest
#define X509_digest wolfSSL_X509_digest
#endif

/* RAND functions compatibility */
#ifndef RAND_bytes
#define RAND_bytes wolfSSL_RAND_bytes
#endif

#ifndef RAND_status
#define RAND_status wolfSSL_RAND_status
#endif

/* ERR functions compatibility */
#ifndef ERR_get_error
#define ERR_get_error wolfSSL_ERR_get_error
#endif

#ifndef ERR_error_string
#define ERR_error_string wolfSSL_ERR_error_string
#endif

#ifndef ERR_lib_error_string
#define ERR_lib_error_string wolfSSL_ERR_lib_error_string
#endif

#ifndef ERR_func_error_string
#define ERR_func_error_string wolfSSL_ERR_func_error_string
#endif

#ifndef ERR_reason_error_string
#define ERR_reason_error_string wolfSSL_ERR_reason_error_string
#endif

/* Memory functions compatibility */
#ifndef OPENSSL_free
#define OPENSSL_free wolfSSL_OPENSSL_free
#endif

#ifndef OPENSSL_malloc
#define OPENSSL_malloc wolfSSL_OPENSSL_malloc
#endif

/* Additional compatibility for Minix-specific usage */

/* DH parameter generation (used in syslogd) */
#ifdef NEED_DH_PARAMS
static inline DH* get_dh1024(void)
{
    /* wolfSSL provides built-in DH parameters */
    return wolfSSL_DH_get_2048_256();
}
#endif

/* X509_NAME functions compatibility */
#ifndef X509_NAME_get_index_by_NID
#define X509_NAME_get_index_by_NID wolfSSL_X509_NAME_get_index_by_NID
#endif

#ifndef X509_NAME_get_entry
#define X509_NAME_get_entry wolfSSL_X509_NAME_get_entry
#endif

#ifndef X509_NAME_ENTRY_get_data
#define X509_NAME_ENTRY_get_data wolfSSL_X509_NAME_ENTRY_get_data
#endif

#ifndef X509_NAME_oneline
#define X509_NAME_oneline wolfSSL_X509_NAME_oneline
#endif

/* ASN1_STRING functions compatibility */
#ifndef ASN1_STRING_to_UTF8
#define ASN1_STRING_to_UTF8 wolfSSL_ASN1_STRING_to_UTF8
#endif

#ifndef ASN1_OCTET_STRING_cmp
#define ASN1_OCTET_STRING_cmp wolfSSL_ASN1_OCTET_STRING_cmp
#endif

/* X509 extension functions */
#ifndef X509_get_ext_d2i
#define X509_get_ext_d2i wolfSSL_X509_get_ext_d2i
#endif

/* OBJ functions */
#ifndef OBJ_nid2sn
#define OBJ_nid2sn wolfSSL_OBJ_nid2sn
#endif

/* NID constants */
#ifndef NID_commonName
#define NID_commonName 13
#endif

#ifndef NID_subject_alt_name
#define NID_subject_alt_name 85
#endif

/* GENERAL_NAME and GENERAL_NAMES compatibility */
#ifndef GENERAL_NAME
typedef WOLFSSL_GENERAL_NAME GENERAL_NAME;
#endif

#ifndef GENERAL_NAMES
typedef WOLFSSL_GENERAL_NAMES GENERAL_NAMES;
#endif

#ifndef sk_GENERAL_NAME_num
#define sk_GENERAL_NAME_num wolfSSL_sk_GENERAL_NAME_num
#endif

#ifndef sk_GENERAL_NAME_value
#define sk_GENERAL_NAME_value wolfSSL_sk_GENERAL_NAME_value
#endif

/* a2i_IPADDRESS compatibility */
#ifndef a2i_IPADDRESS
#define a2i_IPADDRESS wolfSSL_a2i_IPADDRESS
#endif

/* EVP_MD_type compatibility */
#ifndef EVP_MD_type
#define EVP_MD_type wolfSSL_EVP_MD_type
#endif

/* EVP_MAX_MD_SIZE compatibility */
#ifndef EVP_MAX_MD_SIZE
#define EVP_MAX_MD_SIZE 64
#endif

/* SSL_FILETYPE constants */
#ifndef SSL_FILETYPE_PEM
#define SSL_FILETYPE_PEM 1
#endif

#ifndef SSL_FILETYPE_ASN1
#define SSL_FILETYPE_ASN1 2
#endif

/* SSL_set_tlsext_host_name compatibility */
#ifndef SSL_set_tlsext_host_name
#define SSL_set_tlsext_host_name wolfSSL_UseSNI
#endif

/* SSL_get_cipher compatibility */
#ifndef SSL_get_cipher
#define SSL_get_cipher wolfSSL_get_cipher
#endif

/* SSL_get_peer_certificate compatibility */
#ifndef SSL_get_peer_certificate
#define SSL_get_peer_certificate wolfSSL_get_peer_certificate
#endif

/* SSL_set_rfd and SSL_set_wfd compatibility (for bozohttpd) */
#ifndef SSL_set_rfd
#define SSL_set_rfd(ssl, fd) wolfSSL_set_fd(ssl, fd)
#endif

#ifndef SSL_set_wfd
#define SSL_set_wfd(ssl, fd) wolfSSL_set_fd(ssl, fd)
#endif

/* ERR_print_errors_fp compatibility */
#ifndef ERR_print_errors_fp
#define ERR_print_errors_fp(fp) wolfSSL_ERR_dump_errors_fp(fp)
#endif

/* Ensure all necessary types are available */
typedef WOLFSSL_DH DH;
typedef WOLFSSL_BIGNUM BIGNUM;
typedef WOLFSSL_X509 X509;
typedef WOLFSSL_X509_NAME X509_NAME;
typedef WOLFSSL_X509_NAME_ENTRY X509_NAME_ENTRY;
typedef WOLFSSL_EVP_MD EVP_MD;
typedef WOLFSSL_EVP_MD_CTX EVP_MD_CTX;
typedef WOLFSSL_EVP_CIPHER EVP_CIPHER;
typedef WOLFSSL_EVP_CIPHER_CTX EVP_CIPHER_CTX;
typedef WOLFSSL_SSL SSL;
typedef WOLFSSL_SSL_CTX SSL_CTX;
typedef WOLFSSL_SSL_METHOD SSL_METHOD;
typedef WOLFSSL_X509_STORE X509_STORE;
typedef WOLFSSL_X509_VERIFY_PARAM X509_VERIFY_PARAM;
typedef WOLFSSL_GENERAL_NAME GENERAL_NAME;
typedef WOLFSSL_GENERAL_NAMES GENERAL_NAMES;
typedef WOLFSSL_ASN1_STRING ASN1_STRING;
typedef WOLFSSL_ASN1_OCTET_STRING ASN1_OCTET_STRING;
typedef WOLFSSL_EVP_PKEY EVP_PKEY;

#endif /* OPENSSL_COMPAT_H */
