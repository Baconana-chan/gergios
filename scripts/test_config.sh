#!/bin/sh
# Test Configuration for Minix Automated Testing Framework
# This file contains configuration variables for the test suite

# Test directories
TEST_BASE_DIR="$(pwd)/tests"
TEST_RESULTS_DIR="$(pwd)/test-results"
TEST_COVERAGE_DIR="$(pwd)/coverage"

# Test execution settings
PARALLEL_JOBS=$(nproc)
TEST_TIMEOUT=300  # seconds per test
TEST_RETRY_COUNT=2

# Coverage settings
COVERAGE_ENABLED=true
COVERAGE_THRESHOLD=70  # minimum coverage percentage
COVERAGE_FORMATS="xml html"

# Static analysis settings
STATIC_ANALYSIS_ENABLED=true
CLANG_TIDY_ENABLED=true
CPPCHECK_ENABLED=true

# Test categories to run
TEST_CATEGORIES="unit integration regression"

# Component-specific test settings
TEST_BIN=true
TEST_LIB=true
TEST_KERNEL=false  # Kernel tests require special environment
TEST_FS=false      # Filesystem tests require special setup

# Reporting settings
GENERATE_HTML_REPORT=true
GENERATE_XML_REPORT=true
EMAIL_REPORT=false
REPORT_RECIPIENTS=""

# CI/CD integration
CI_INTEGRATION=true
CI_PLATFORM="github"  # github, gitlab, jenkins

# Logging
LOG_LEVEL="INFO"  # DEBUG, INFO, WARNING, ERROR
LOG_FILE="${TEST_RESULTS_DIR}/test-run.log"

# Export variables
export TEST_BASE_DIR
export TEST_RESULTS_DIR
export TEST_COVERAGE_DIR
export PARALLEL_JOBS
export TEST_TIMEOUT
export TEST_RETRY_COUNT
export COVERAGE_ENABLED
export COVERAGE_THRESHOLD
export COVERAGE_FORMATS
export STATIC_ANALYSIS_ENABLED
export CLANG_TIDY_ENABLED
export CPPCHECK_ENABLED
export TEST_CATEGORIES
export TEST_BIN
export TEST_LIB
export TEST_KERNEL
export TEST_FS
export GENERATE_HTML_REPORT
export GENERATE_XML_REPORT
export EMAIL_REPORT
export REPORT_RECIPIENTS
export CI_INTEGRATION
export CI_PLATFORM
export LOG_LEVEL
export LOG_FILE
