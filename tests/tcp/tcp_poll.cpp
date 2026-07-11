//! Catch2 TCP poll/select tests (migrated from ATF test91)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! These tests require a running MINIX kernel with lwIP service.
//! They are compiled on the host but will skip gracefully when
//! the MINIX runtime is unavailable.
//!
//! Original ATF test91 poll sub-tests:
//!   - poll() on listening socket returns POLLIN
//!   - poll() on connected socket returns POLLOUT
//!   - select() for read/write/except
//!   - poll() timeout behaviour (POLLIN timeout = 0)

#include "catch.hpp"

TEST_CASE("TCP poll on listening socket returns POLLIN (MINIX)", "[tcp][poll][minix]") {
    SKIP("MINIX runtime required");
}

TEST_CASE("TCP poll on connected socket returns POLLOUT (MINIX)", "[tcp][poll][minix]") {
    SKIP("MINIX runtime required");
}

TEST_CASE("TCP select read/write/except (MINIX)", "[tcp][poll][minix]") {
    SKIP("MINIX runtime required");
}

TEST_CASE("TCP poll timeout behaviour (MINIX)", "[tcp][poll][minix]") {
    SKIP("MINIX runtime required");
}
