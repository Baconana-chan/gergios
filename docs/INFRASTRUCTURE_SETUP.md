# Infrastructure Setup Documentation

This document describes the CI/CD infrastructure and automated testing framework set up for Minix modernization.

## Overview

The infrastructure includes:
- **CI/CD Pipeline**: GitHub Actions workflows for automated builds and testing
- **Testing Framework**: Automated test runner with ATF/Kyua integration
- **Code Coverage**: Lcov-based coverage reporting with HTML/XML outputs
- **Static Analysis**: Clang-tidy and cppcheck integration
- **Security Scanning**: CodeQL, safety checks, and OSS-Fuzz integration

## Directory Structure

```
.github/
  workflows/
    ci.yml              # Main CI/CD pipeline
    coverage.yml        # Code coverage reporting
    security.yml        # Security scanning
scripts/
  run_tests.sh          # Automated test runner
  test_config.sh        # Test configuration
  generate_coverage.sh  # Coverage report generator
  run_static_analysis.sh # Static analysis runner
  run_security_scan.sh  # Security scanning runner
.clang-tidy            # Clang-tidy configuration
```

## CI/CD Pipeline

### Main CI Workflow (`.github/workflows/ci.yml`)

Triggers on:
- Push to main/develop branches
- Pull requests to main/develop branches
- Manual workflow dispatch

Jobs:
- **build**: Compiles the Minix system on Ubuntu
- **static-analysis**: Runs clang-tidy and cppcheck
- **security-scan**: Runs CodeQL analysis and dependency checks
- **tests**: Executes the test suite with coverage
- **code-quality**: Checks code formatting and common issues

### Coverage Workflow (`.github/workflows/coverage.yml`)

Generates code coverage reports using gcov/lcov and uploads to Codecov.

### Security Workflow (`.github/workflows/security.yml`)

Runs security scans including:
- CodeQL analysis
- Dependency vulnerability checks
- OSS-Fuzz integration (placeholder)

## Testing Framework

### Running Tests Locally

```bash
# Run all tests
./scripts/run_tests.sh

# Run with custom configuration
export TEST_RESULTS_DIR=./my-results
./scripts/run_tests.sh
```

### Test Configuration

Edit `scripts/test_config.sh` to customize:
- Test directories
- Parallel job count
- Coverage thresholds
- Test categories to run

## Code Coverage

### Generating Coverage Reports

```bash
# Generate coverage report
./scripts/generate_coverage.sh

# With custom build directory
export BUILD_DIR=./obj
./scripts/generate_coverage.sh
```

Coverage reports are generated in:
- HTML: `coverage/html/index.html`
- XML: `coverage/coverage.xml`
- Summary: `coverage/coverage-summary.txt`

## Static Analysis

### Running Static Analysis

```bash
# Run all static analysis tools
./scripts/run_static_analysis.sh

# Results are saved to static-analysis-results/
```

### Clang-tidy Configuration

The `.clang-tidy` file defines:
- Enabled checks (bugprone, cert, clang-analyzer, etc.)
- Naming conventions
- Code style options

### Cppcheck Configuration

Cppcheck runs with:
- All checks enabled
- C11 standard
- Unix64 platform
- XML output format

## Security Scanning

### Running Security Scans

```bash
# Run security scanning
./scripts/run_security_scan.sh

# Results are saved to security-results/
```

Security checks include:
- Clang static analyzer (scan-build)
- Python dependency safety checks
- Common security issue detection
- Hardcoded credential detection

## wolfSSL Integration

wolfSSL has replaced OpenSSL 0.9.8 for the following components:
- syslogd (TLS + syslog-sign)
- ftp (SSL/TLS)
- httpd/bozohttpd (HTTPS)
- telnet/telnetd (SRA encryption)
- passwd (Kerberos UI)
- factor (BN factorization)
- BIND/named (DNSSEC)

### Build Configuration

wolfSSL is configured via `crypto/Makefile.wolfssl` and
`crypto/external/gpl2/wolfssl/config.h`. The OpenSSL compatibility layer
(`OPENSSL_EXTRA`) enables most OpenSSL API calls to work unchanged.

### Running wolfSSL Tests

```bash
# Unit tests for wolfSSL migration
cd tests/crypto/libcrypto
atf-run t_wolfssl        # API migration tests
atf-run t_security        # Security tests
atf-run t_perf            # Performance benchmarks
atf-run t_compat          # Compatibility tests

# Integration tests
cd tests/integration
atf-run t_syslogd_tls
atf-run t_ftp_ssl
atf-run t_httpd_ssl
atf-run t_telnet_encrypt
atf-run t_bind_dnssec
atf-run t_cross_component
```

For more details, see:
- [Build Instructions](BUILDING.md)
- [wolfSSL Usage Guide](wolfssl-usage-guide.md)
- [Migration Plan](../planning/06_openssl_to_wolfssl_migration.md)

## Local Development Setup

### Prerequisites

Install required tools:

```bash
# Ubuntu/Debian
sudo apt-get install -y \
  build-essential gcc g++ make \
  clang clang-tidy cppcheck \
  lcov gcov python3 python3-pip

# Python tools
pip3 install safety bandit
```

### Running All Checks Locally

```bash
# Make scripts executable
chmod +x scripts/*.sh

# Run tests
./scripts/run_tests.sh

# Generate coverage
./scripts/generate_coverage.sh

# Run static analysis
./scripts/run_static_analysis.sh

# Run security scan
./scripts/run_security_scan.sh
```

## CI/CD Best Practices

1. **Branch Protection**: Enable branch protection rules for main/develop
2. **Required Checks**: Require CI checks to pass before merging
3. **Automated Merging**: Consider using Dependabot for dependency updates
4. **Artifact Retention**: Configure appropriate retention periods for build artifacts
5. **Secrets Management**: Store sensitive data in GitHub Secrets

## Coverage Goals

Target coverage thresholds:
- **Overall**: 70%
- **Critical Components**: 80%
- **New Code**: 85%

## Security Scanning Schedule

Security scans run:
- On every push and pull request
- Weekly on Sundays (scheduled)
- On manual trigger

## OSS-Fuzz Integration

OSS-Fuzz integration is prepared but requires:
1. Project registration at https://github.com/google/oss-fuzz
2. Adding fuzz targets to the codebase
3. Configuring the OSS-Fuzz build system

## Troubleshooting

### Build Failures

Check the build logs in GitHub Actions for specific error messages.

### Coverage Not Generated

Ensure:
- Build was done with `--coverage` flags
- gcov and lcov are installed
- Source files match build directory

### Static Analysis Errors

Some checks may be disabled for legacy code. Review `.clang-tidy` configuration.

## Future Enhancements

- [ ] Add more comprehensive fuzzing targets
- [ ] Integrate Coverity Scan
- [ ] Add performance benchmarking
- [ ] Implement mutation testing
- [ ] Add container-based builds
- [ ] Set up multi-platform CI (macOS, Windows)

## References

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Lcov Documentation](http://ltp.sourceforge.net/coverage/lcov.php)
- [Clang-tidy Documentation](https://clang.llvm.org/extra/clang-tidy/)
- [Cppcheck Documentation](http://cppcheck.sourceforge.net/)
- [CodeQL Documentation](https://codeql.github.com/docs/)
