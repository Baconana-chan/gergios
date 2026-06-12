# OpenSSL Compatibility Layer Documentation

## Overview

The wolfSSL OpenSSL compatibility layer provides a comprehensive set of macros and wrappers to enable smooth migration from OpenSSL 0.9.8 to wolfSSL v5.9.1-stable in the Minix codebase.

## Compatibility Layer Components

### 1. Configuration File (config.h)

The wolfSSL configuration is defined in `crypto/external/gpl2/wolfssl/config.h` and includes:

- **OpenSSL Compatibility Layer**: `OPENSSL_EXTRA`, `OPENSSL_EXTRA_X509_SMALL`, `OPENSSL_ALL`
- **Cryptographic Algorithms**: AES-GCM, ChaCha20-Poly1305, ECC, Curve25519, Ed25519, DH, RSA, DSA, SHA, SHA256, SHA512, MD5, HMAC, PKCS7, ASN, EVP, PKCS12
- **TLS/DTLS Support**: TLS 1.3, DTLS 1.3
- **Certificate Handling**: X509, OCSP, CRL, certificate generation and parsing
- **Additional Features**: BIO, CONF, PKCS8, PKCS12, ASN.1, session handling, certificate compression, KDF, HKDF, HPKE

### 2. Compatibility Wrapper (openssl_compat.h)

The compatibility wrapper is defined in `crypto/external/gpl2/wolfssl/openssl_compat.h` and provides:

#### SSL/TLS Functions
- `SSL_library_init()` → `wolfSSL_library_init()`
- `SSL_load_error_strings()` → `wolfSSL_load_error_strings()`
- `SSL_CTX_new()` → `wolfSSL_CTX_new()`
- `SSLv23_method()` → `wolfSSLv23_method()`
- `SSLv23_client_method()` → `wolfSSLv23_client_method()`
- `SSLv23_server_method()` → `wolfSSLv23_server_method()`
- `SSL_new()` → `wolfSSL_new()`
- `SSL_set_fd()` → `wolfSSL_set_fd()`
- `SSL_set_tlsext_host_name()` → `wolfSSL_UseSNI()`
- `SSL_connect()` → `wolfSSL_connect()`
- `SSL_accept()` → `wolfSSL_accept()`
- `SSL_read()` → `wolfSSL_read()`
- `SSL_write()` → `wolfSSL_write()`
- `SSL_free()` → `wolfSSL_free()`
- `SSL_get_error()` → `wolfSSL_get_error()`
- `SSL_get_cipher()` → `wolfSSL_get_cipher()`
- `SSL_get_peer_certificate()` → `wolfSSL_get_peer_certificate()`
- `SSL_CTX_use_PrivateKey()` → `wolfSSL_CTX_use_PrivateKey()`
- `SSL_CTX_use_certificate()` → `wolfSSL_CTX_use_certificate()`
- `SSL_CTX_use_PrivateKey_file()` → `wolfSSL_CTX_use_PrivateKey_file()`
- `SSL_CTX_use_certificate_chain_file()` → `wolfSSL_CTX_use_certificate_chain_file()`
- `SSL_CTX_check_private_key()` → `wolfSSL_CTX_check_private_key()`
- `SSL_CTX_load_verify_locations()` → `wolfSSL_CTX_load_verify_locations()`
- `SSL_CTX_set_options()` → `wolfSSL_CTX_set_options()`
- `SSL_CTX_set_mode()` → `wolfSSL_CTX_set_mode()`
- `SSL_CTX_set_verify()` → `wolfSSL_CTX_set_verify()`
- `SSL_CTX_set_tmp_dh()` → `wolfSSL_CTX_set_tmp_dh()`

#### X509 Certificate Functions
- `X509_free()` → `wolfSSL_X509_free()`
- `X509_get_subject_name()` → `wolfSSL_X509_get_subject_name()`
- `X509_get_issuer_name()` → `wolfSSL_X509_get_issuer_name()`
- `X509_digest()` → `wolfSSL_X509_digest()`
- `X509_NAME_get_index_by_NID()` → `wolfSSL_X509_NAME_get_index_by_NID()`
- `X509_NAME_get_entry()` → `wolfSSL_X509_NAME_get_entry()`
- `X509_NAME_oneline()` → `wolfSSL_X509_NAME_oneline()`
- `X509_NAME_ENTRY_get_data()` → `wolfSSL_X509_NAME_ENTRY_get_data()`
- `X509_get_ext_d2i()` → `wolfSSL_X509_get_ext_d2i()`

#### Cryptographic Functions
- `EVP_get_digestbyname()` → `wolfSSL_EVP_get_digestbyname()`
- `EVP_md5()` → `wolfSSL_EVP_md5()`
- `EVP_sha1()` → `wolfSSL_EVP_sha1()`
- `EVP_sha256()` → `wolfSSL_EVP_sha256()`
- `EVP_sha512()` → `wolfSSL_EVP_sha512()`
- `EVP_MD_type()` → `wolfSSL_EVP_MD_type()`

#### DH Functions
- `DH_new()` → `wolfSSL_DH_new()`
- `DH_free()` → `wolfSSL_DH_free()`
- `DH_generate_parameters()` → `wolfSSL_DH_generate_parameters()`

#### BN Functions
- `BN_new()` → `wolfSSL_BN_new()`
- `BN_free()` → `wolfSSL_BN_free()`
- `BN_num_bits()` → `wolfSSL_BN_num_bits()`
- `BN_bin2bn()` → `wolfSSL_BN_bin2bn()`
- `BN_bn2bin()` → `wolfSSL_BN_bn2bin()`

#### Random Number Functions
- `RAND_bytes()` → `wolfSSL_RAND_bytes()`
- `RAND_status()` → `wolfSSL_RAND_status()`

#### Error Handling Functions
- `ERR_get_error()` → `wolfSSL_ERR_get_error()`
- `ERR_error_string()` → `wolfSSL_ERR_error_string()`
- `ERR_lib_error_string()` → `wolfSSL_ERR_lib_error_string()`
- `ERR_func_error_string()` → `wolfSSL_ERR_func_error_string()`
- `ERR_reason_error_string()` → `wolfSSL_ERR_reason_error_string()`
- `ERR_print_errors_fp()` → `wolfSSL_ERR_dump_errors_fp()`

#### Memory Functions
- `OPENSSL_free()` → `wolfSSL_OPENSSL_free()`
- `OPENSSL_malloc()` → `wolfSSL_OPENSSL_malloc()`

#### ASN.1 Functions
- `ASN1_STRING_to_UTF8()` → `wolfSSL_ASN1_STRING_to_UTF8()`
- `ASN1_OCTET_STRING_cmp()` → `wolfSSL_ASN1_OCTET_STRING_cmp()`
- `OBJ_nid2sn()` → `wolfSSL_OBJ_nid2sn()`
- `a2i_IPADDRESS()` → `wolfSSL_a2i_IPADDRESS()`

## Minix Code Coverage

The compatibility layer covers all OpenSSL usage in the following Minix components:

### 1. usr.sbin/syslogd/tls.c
- ✅ SSL_CTX initialization and configuration
- ✅ Certificate and key loading
- ✅ DH parameter generation
- ✅ X509 certificate handling
- ✅ Certificate fingerprint calculation
- ✅ Common name extraction
- ✅ Subject alternative name verification
- ✅ Error handling

### 2. usr.bin/ftp/ssl.c
- ✅ SSL library initialization
- ✅ SSL context creation
- ✅ SSL connection establishment
- ✅ SSL read/write operations
- ✅ Certificate information display
- ✅ Error handling

### 3. libexec/httpd/ssl-bozo.c
- ✅ SSL library initialization
- ✅ SSL context creation
- ✅ Certificate and key loading
- ✅ SSL accept/read/write operations
- ✅ Error queue handling
- ✅ Error reporting

## Known Limitations

### 1. API Differences
- **SSL_set_rfd/SSL_set_wfd**: wolfSSL uses `SSL_set_fd()` for both read and write file descriptors. The compatibility wrapper maps both to `wolfSSL_set_fd()`.
- **SSL_set_tlsext_host_name**: wolfSSL uses `wolfSSL_UseSNI()` for Server Name Indication. The compatibility wrapper provides the mapping.
- **DH parameter generation**: wolfSSL provides built-in DH parameters via `wolfSSL_DH_get_2048_256()`. The custom `get_dh1024()` function uses this instead of manual parameter generation.

### 2. Disabled Features
The following OpenSSL features are disabled in the wolfSSL configuration for size reduction:
- MD4, RC4, PSK, HC128, Rabbit
- DES3 (can be enabled if needed)
- DSA, DH (can be enabled if needed)
- Old TLS versions (SSLv2, SSLv3, TLSv1, TLSv1.1)
- File system operations (WOLFSSL_NO_FILESYSTEM)

### 3. Threading Model
- wolfSSL is configured for single-threaded operation (`SINGLE_THREADED`)
- OpenSSL's multi-threaded support is not available
- This is appropriate for Minix's typical use cases

### 4. Memory Management
- wolfSSL uses its own memory management functions
- The compatibility wrapper maps `OPENSSL_free()` to `wolfSSL_OPENSSL_free()`
- Applications should not mix OpenSSL and wolfSSL memory functions

### 5. Error Codes
- wolfSSL error codes are compatible with OpenSSL error codes
- The compatibility layer ensures error code compatibility
- Some wolfSSL-specific error codes may not have exact OpenSSL equivalents

### 6. Certificate Extensions
- Most common X509 extensions are supported
- Some obscure or rarely used extensions may not be available
- Subject alternative name extension is fully supported

### 7. Post-Quantum Cryptography
- Post-quantum algorithms (Kyber, Dilithium) are optional
- Can be disabled to reduce library size
- Not required for basic OpenSSL compatibility

## Migration Strategy

### Phase 1: Header Replacement
Replace OpenSSL headers with wolfSSL compatibility headers:
```c
// Before
#include <openssl/ssl.h>
#include <openssl/x509.h>
#include <openssl/err.h>

// After
#include <wolfssl/openssl/ssl.h>
#include <wolfssl/openssl/x509.h>
#include <wolfssl/openssl/err.h>
```

Or use the compatibility wrapper:
```c
#include <wolfssl/openssl_compat.h>
```

### Phase 2: Library Linking
Update Makefiles to link against wolfSSL instead of OpenSSL:
```makefile
# Before
LDADD+= -lssl -lcrypto

# After
LDADD+= -lwolfssl
```

### Phase 3: Testing
Test each component individually:
1. Compile with wolfSSL headers
2. Link against wolfSSL library
3. Test functionality
4. Verify error handling
5. Check performance

### Phase 4: Cleanup
Remove OpenSSL dependencies:
1. Remove OpenSSL-specific code
2. Clean up compatibility wrappers
3. Update documentation
4. Remove OpenSSL from build system

## Performance Considerations

### Advantages
- **Smaller footprint**: wolfSSL is 2-20 times smaller than OpenSSL
- **Better performance**: Optimized for embedded systems
- **Modern algorithms**: Supports TLS 1.3, ChaCha20-Poly1305, Curve25519
- **Lower memory usage**: Designed for resource-constrained environments

### Considerations
- **Compatibility overhead**: Compatibility layer adds minimal overhead
- **Feature differences**: Some OpenSSL features may not be available
- **Learning curve**: Developers may need to learn wolfSSL-specific APIs

## Security Improvements

### Advantages
- **Active maintenance**: wolfSSL is actively maintained with regular security updates
- **Modern cryptography**: Supports post-quantum cryptography
- **FIPS validation**: FIPS 140-2 and FIPS 140-3 validated versions available
- **No known vulnerabilities**: Recent versions fix multiple CVEs

### Considerations
- **Different threat model**: wolfSSL's threat model may differ from OpenSSL
- **Audit history**: OpenSSL has been audited more extensively over time

## Conclusion

The wolfSSL OpenSSL compatibility layer provides comprehensive coverage of all OpenSSL usage in the Minix codebase. The compatibility wrapper ensures smooth migration with minimal code changes. Known limitations are well-documented and can be addressed through configuration adjustments or additional wrappers.

The migration is expected to provide significant security and performance benefits while maintaining compatibility with existing Minix functionality.
