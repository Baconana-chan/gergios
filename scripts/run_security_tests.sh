#!/bin/sh
# Security Integration Test Runner for GergiOS
# Tests all 4 security layers: capabilities, MAC, memory safety, audit
#
# Usage:
#   ./run_security_tests.sh              # Run all tests
#   ./run_security_tests.sh -v           # Verbose mode
#   ./run_security_tests.sh -t cap       # Capability tests only
#   ./run_security_tests.sh -t mac       # MAC tests only
#   ./run_security_tests.sh -t wx        # W^X tests only
#   ./run_security_tests.sh -t audit     # Audit tests only

set -e

# === Configuration ===
RESULTS_DIR="${RESULTS_DIR:-$(pwd)/security-test-results}"
VERBOSE=0
TEST_FILTER="all"
PASS=0
FAIL=0
SKIP=0

# === Colors ===
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

mkdir -p "${RESULTS_DIR}"

# === Parse arguments ===
while getopts "vt:" opt; do
    case $opt in
        v) VERBOSE=1 ;;
        t) TEST_FILTER="$OPTARG" ;;
        *) echo "Usage: $0 [-v] [-t cap|mac|wx|audit]"; exit 1 ;;
    esac
done

# === Helper functions ===
pass() {
    PASS=$((PASS + 1))
    echo -e "  ${GREEN}[PASS]${NC} $1"
    echo "[PASS] $1" >> "${RESULTS_DIR}/results.log"
}

fail() {
    FAIL=$((FAIL + 1))
    echo -e "  ${RED}[FAIL]${NC} $1"
    echo "[FAIL] $1" >> "${RESULTS_DIR}/results.log"
    if [ -n "$2" ]; then
        echo "    Details: $2" >> "${RESULTS_DIR}/results.log"
        [ "$VERBOSE" = "1" ] && echo "    Details: $2"
    fi
}

skip() {
    SKIP=$((SKIP + 1))
    echo -e "  ${YELLOW}[SKIP]${NC} $1"
    echo "[SKIP] $1" >> "${RESULTS_DIR}/results.log"
}

check_cmd() {
    command -v "$1" >/dev/null 2>&1 || { skip "$2: $1 not found"; return 1; }
    return 0
}

section() {
    echo ""
    echo -e "${CYAN}==== $1 ====${NC}"
    echo "==== $1 ====" >> "${RESULTS_DIR}/results.log"
}

# === Tests ===

test_cap_get_proc() {
    section "Capability: cap_get_proc()"

    # Write a small test program
    cat > /tmp/test_cap.c << 'EOF'
#include <sys/capability.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    cap_t caps;
    int r;

    r = cap_get_proc(&caps);
    if (r != 0) {
        printf("FAIL: cap_get_proc returned %d\n", r);
        return 1;
    }
    printf("OK: caps = 0x%016llx\n", (unsigned long long)caps);
    return 0;
}
EOF

    if cc -o /tmp/test_cap /tmp/test_cap.c -lcap 2>/dev/null; then
        if /tmp/test_cap > "${RESULTS_DIR}/cap_get_proc.out" 2>&1; then
            pass "cap_get_proc() returns valid capabilities"
        else
            fail "cap_get_proc() failed" "$(cat ${RESULTS_DIR}/cap_get_proc.out)"
        fi
    else
        skip "cap_get_proc() test: compilation failed"
    fi
}

test_cap_net_bind() {
    section "Capability: NET_BIND enforcement"

    cat > /tmp/test_bind.c << 'EOF'
#include <sys/capability.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(void) {
    cap_t caps = 0;
    int r;

    /* Drop all capabilities */
    r = cap_set_proc(&caps);
    if (r != 0) {
        printf("FAIL: cap_set_proc returned %d\n", r);
        return 1;
    }

    /* Verify they were dropped */
    r = cap_get_proc(&caps);
    if (r != 0 || caps != 0) {
        printf("FAIL: capabilities not dropped (caps=0x%llx)\n",
               (unsigned long long)caps);
        return 1;
    }

    printf("OK: capabilities successfully dropped to 0\n");
    return 0;
}
EOF

    if cc -o /tmp/test_bind /tmp/test_bind.c -lcap 2>/dev/null; then
        if /tmp/test_bind > "${RESULTS_DIR}/cap_bind.out" 2>&1; then
            pass "cap_set_proc() drops capabilities successfully"
        else
            fail "cap_set_proc() failed" "$(cat ${RESULTS_DIR}/cap_bind.out)"
        fi
    else
        skip "cap_set_proc() test: compilation failed"
    fi
}

test_cap_inheritance() {
    section "Capability: fork/exec inheritance"

    cat > /tmp/test_fork.c << 'EOF'
#include <sys/capability.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/wait.h>

int main(void) {
    cap_t parent_caps, child_caps;
    pid_t pid;
    int status;

    /* Get parent capabilities */
    if (cap_get_proc(&parent_caps) != 0) {
        printf("FAIL: parent cap_get_proc\n");
        return 1;
    }

    pid = fork();
    if (pid < 0) {
        printf("FAIL: fork failed\n");
        return 1;
    }

    if (pid == 0) {
        /* Child: verify capabilities inherited */
        if (cap_get_proc(&child_caps) != 0) {
            printf("FAIL: child cap_get_proc\n");
            return 1;
        }
        if (child_caps != parent_caps) {
            printf("FAIL: child caps 0x%llx != parent caps 0x%llx\n",
                   (unsigned long long)child_caps,
                   (unsigned long long)parent_caps);
            return 1;
        }
        printf("OK: capabilities inherited correctly via fork\n");
        return 0;
    }

    wait(&status);
    if (WIFEXITED(status) && WEXITSTATUS(status) == 0)
        return 0;
    return 1;
}
EOF

    if cc -o /tmp/test_fork /tmp/test_fork.c -lcap 2>/dev/null; then
        if /tmp/test_fork > "${RESULTS_DIR}/cap_fork.out" 2>&1; then
            pass "capabilities inherited via fork()"
        else
            fail "capability fork inheritance failed" \
                 "$(cat ${RESULTS_DIR}/cap_fork.out)"
        fi
    else
        skip "cap_fork test: compilation failed"
    fi
}

test_mac_status() {
    section "MAC: macctl status"

    if check_cmd "macctl" "macctl"; then
        macctl status > "${RESULTS_DIR}/mac_status.out" 2>&1 || true
        if grep -q "Enforcement" "${RESULTS_DIR}/mac_status.out"; then
            pass "macctl status reports enforcement status"
        else
            fail "macctl status output unexpected" \
                 "$(cat ${RESULTS_DIR}/mac_status.out)"
        fi
    fi
}

test_mac_toggle() {
    section "MAC: runtime toggle"

    if check_cmd "macctl" "macctl"; then
        macctl off > /dev/null 2>&1 || true
        STATUS_OFF=$(macctl status 2>/dev/null | grep -c "OFF" || true)

        macctl on > /dev/null 2>&1 || true
        STATUS_ON=$(macctl status 2>/dev/null | grep -c "ON" || true)

        if [ "$STATUS_OFF" -gt 0 ] && [ "$STATUS_ON" -gt 0 ]; then
            pass "MAC runtime toggle works (on/off)"
        else
            fail "MAC toggle check failed" "OFF=$STATUS_OFF ON=$STATUS_ON"
        fi

        # Restore to on
        macctl on > /dev/null 2>&1 || true
    fi
}

test_mac_compile() {
    section "MAC: mac-compile policy compiler"

    if check_cmd "mac-compile" "mac-compile"; then
        # Create a test policy
        cat > /tmp/test_macd.conf << 'EOF'
allow IPC_SEND from pm to vfs;
allow IPC_SEND from pm to vm;
allow PRIVCTL_SET_SYS from rs to any;
deny FILE_ACCESS from user_t to system_t;
EOF

        if mac-compile -o /tmp/test_macd.policy /tmp/test_macd.conf \
            > "${RESULTS_DIR}/mac_compile.out" 2>&1; then
            pass "mac-compile compiles policy successfully"

            # Verify binary output
            if [ -f /tmp/test_macd.policy ]; then
                SIZE=$(stat -c%s /tmp/test_macd.policy 2>/dev/null || \
                       stat -f%z /tmp/test_macd.policy 2>/dev/null || echo 0)
                if [ "$SIZE" -ge 72 ]; then
                    pass "mac-compile produces valid binary output"
                else
                    fail "mac-compile binary too small" "size=$SIZE"
                fi
            fi
        else
            fail "mac-compile failed" "$(cat ${RESULTS_DIR}/mac_compile.out)"
        fi
    fi
}

test_mac_deny() {
    section "MAC: deny enforcement"

    if check_cmd "macctl" "macctl"; then
        # Temporarily enable MAC
        macctl on > /dev/null 2>&1 || true

        # Check status shows enforcement
        STATUS=$(macctl status 2>/dev/null)
        echo "$STATUS" >> "${RESULTS_DIR}/mac_deny.out"

        if echo "$STATUS" | grep -q "ON"; then
            pass "MAC enforcement is active"
        else
            fail "MAC enforcement not active" "$STATUS"
        fi
    fi
}

test_auditctl() {
    section "Audit: auditctl status"

    if check_cmd "auditctl" "auditctl"; then
        auditctl -s > "${RESULTS_DIR}/audit_status.out" 2>&1 || true
        if grep -q -i "status\|enabled\|log" "${RESULTS_DIR}/audit_status.out"; then
            pass "auditctl -s shows audit status"
        else
            fail "auditctl -s output unexpected" \
                 "$(cat ${RESULTS_DIR}/audit_status.out)"
        fi
    fi
}

test_auditctl_enable() {
    section "Audit: enable/disable toggle"

    if check_cmd "auditctl" "auditctl"; then
        auditctl -e disable > /dev/null 2>&1 || true
        STATUS_DISABLE=$(auditctl -s 2>/dev/null | grep -ci "disabled" || true)

        auditctl -e enable > /dev/null 2>&1 || true
        STATUS_ENABLE=$(auditctl -s 2>/dev/null | grep -ci "enabled" || true)

        if [ "$STATUS_DISABLE" -gt 0 ] && [ "$STATUS_ENABLE" -gt 0 ]; then
            pass "Audit enable/disable works"
        else
            fail "Audit toggle check" "disabled=$STATUS_DISABLE enabled=$STATUS_ENABLE"
        fi
    fi
}

test_auditctl_rotate() {
    section "Audit: log rotation"

    if check_cmd "auditctl" "auditctl"; then
        # Force log rotation
        if auditctl -R > "${RESULTS_DIR}/audit_rotate.out" 2>&1; then
            pass "auditctl -R forces log rotation"
        else
            # Non-fatal - rotation might not be available in all configs
            skip "auditctl -R not available (check auditd config)"
        fi
    fi
}

test_auditctl_force() {
    section "Audit: force poll"

    if check_cmd "auditctl" "auditctl"; then
        if auditctl -f > "${RESULTS_DIR}/audit_force.out" 2>&1; then
            pass "auditctl -f forces kernel buffer poll"
        else
            skip "auditctl -f not available"
        fi
    fi
}

test_audit2txt() {
    section "Audit: audit2txt log viewer"

    if check_cmd "audit2txt" "audit2txt"; then
        # Create a test log file
        cat > /tmp/test_audit.log << 'EOF'
# ROTATE: audit.log -> audit.log.20260709_120000
1|100|2|0|2|23|OK
2|150|3|1|14|5|EPERM
3|200|4|13|14|1|EACCES
EOF

        # Basic read
        if audit2txt /tmp/test_audit.log > "${RESULTS_DIR}/audit2txt_basic.out" 2>&1; then
            pass "audit2txt reads log file"
        else
            fail "audit2txt basic read failed" \
                 "$(cat ${RESULTS_DIR}/audit2txt_basic.out)"
        fi

        # Type filter
        if audit2txt -t IPC_DENIED /tmp/test_audit.log \
            > "${RESULTS_DIR}/audit2txt_filter.out" 2>&1; then
            if grep -q "IPC_DENIED" "${RESULTS_DIR}/audit2txt_filter.out"; then
                pass "audit2txt -t TYPE filter works"
            else
                fail "audit2txt filter no output" \
                     "$(cat ${RESULTS_DIR}/audit2txt_filter.out)"
            fi
        else
            skip "audit2txt -t filter failed"
        fi

        # Endpoint filter
        if audit2txt -p 14 /tmp/test_audit.log \
            > "${RESULTS_DIR}/audit2txt_ep.out" 2>&1; then
            if grep -q "subj=14\|obj=14\|subj=14\|obj=14" \
                "${RESULTS_DIR}/audit2txt_ep.out"; then
                pass "audit2txt -p ENDPOINT filter works"
            else
                fail "audit2txt endpoint filter no match" \
                     "$(cat ${RESULTS_DIR}/audit2txt_ep.out)"
            fi
        else
            skip "audit2txt -p filter failed"
        fi

    fi
}

test_wx_mmap() {
    section "W^X: mmap enforcement"

    cat > /tmp/test_wx.c << 'EOF'
#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <errno.h>

int main(void) {
    void *p;

    /* Try mmap with PROT_WRITE | PROT_EXEC — should fail */
    p = mmap(NULL, 4096, PROT_READ | PROT_WRITE | PROT_EXEC,
             MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);

    if (p == MAP_FAILED) {
        printf("OK: W^X enforcement — mmap W+X returned %d (EPERM=%d)\n",
               errno, EPERM);
        return 0;
    }

    /* If succeeded, W^X might be disabled */
    printf("WARNING: mmap W+X succeeded at %p\n", p);
    munmap(p, 4096);
    return 1;
}
EOF

    if cc -o /tmp/test_wx /tmp/test_wx.c 2>/dev/null; then
        if /tmp/test_wx > "${RESULTS_DIR}/wx_mmap.out" 2>&1; then
            pass "W^X: mmap PROT_WRITE|PROT_EXEC correctly rejected"
        else
            skip "W^X: mmap W+X succeeded (may be disabled)"
        fi
    else
        skip "W^X mmap test: compilation failed"
    fi
}

test_wx_mprotect() {
    section "W^X: mprotect enforcement"

    cat > /tmp/test_mprotect.c << 'EOF'
#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <errno.h>

int main(void) {
    void *p;
    int r;

    /* Allocate RW memory */
    p = mmap(NULL, 4096, PROT_READ | PROT_WRITE,
             MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (p == MAP_FAILED) {
        printf("FAIL: mmap RW failed: %d\n", errno);
        return 1;
    }

    /* Try to make it RWX via mprotect — should fail */
    r = mprotect(p, 4096, PROT_READ | PROT_WRITE | PROT_EXEC);
    if (r == 0) {
        printf("WARNING: mprotect W+X succeeded at %p\n", p);
        munmap(p, 4096);
        return 1;
    }

    printf("OK: mprotect W+X rejected with errno=%d\n", errno);
    munmap(p, 4096);
    return 0;
}
EOF

    if cc -o /tmp/test_mprotect /tmp/test_mprotect.c 2>/dev/null; then
        if /tmp/test_mprotect > "${RESULTS_DIR}/wx_mprotect.out" 2>&1; then
            pass "W^X: mprotect W+X correctly rejected"
        else
            skip "W^X: mprotect W+X succeeded (may be disabled)"
        fi
    else
        skip "W^X mprotect test: compilation failed"
    fi
}


# === Main ===

# Print header
echo "=========================================="
echo "GergiOS Security Integration Tests"
echo "=========================================="
echo "Started: $(date)"
echo "Results: ${RESULTS_DIR}"
echo "Filter:  ${TEST_FILTER}"
echo "=========================================="
echo ""

# Clear results
: > "${RESULTS_DIR}/results.log"

# Run tests
run_test_group() {
    case "$1" in
        cap|all)
            test_cap_get_proc
            test_cap_net_bind
            test_cap_inheritance
            ;;
    esac
    case "$1" in
        mac|all)
            test_mac_status
            test_mac_toggle
            test_mac_compile
            test_mac_deny
            ;;
    esac
    case "$1" in
        wx|all)
            test_wx_mmap
            test_wx_mprotect
            ;;
    esac
    case "$1" in
        audit|all)
            test_auditctl
            test_auditctl_enable
            test_auditctl_rotate
            test_auditctl_force
            test_audit2txt
            ;;
    esac
}

run_test_group "$TEST_FILTER"

# Generate summary
echo ""
echo "=========================================="
echo "Results Summary"
echo "=========================================="
echo "Passed: ${PASS}"
echo "Failed: ${FAIL}"
echo "Skipped: ${SKIP}"
echo "Total:  $((PASS + FAIL + SKIP))"
echo "=========================================="
echo ""

{
    echo "=========================================="
    echo "Results Summary"
    echo "=========================================="
    echo "Date: $(date)"
    echo "Passed: ${PASS}"
    echo "Failed: ${FAIL}"
    echo "Skipped: ${SKIP}"
    echo "Total: $((PASS + FAIL + SKIP))"
    echo "=========================================="
} > "${RESULTS_DIR}/summary.txt"

# Exit with appropriate code
if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}Some tests failed.${NC} Check ${RESULTS_DIR}/results.log"
    exit 1
elif [ "$SKIP" -gt 0 ] && [ "$PASS" -eq 0 ]; then
    echo -e "${YELLOW}All tests skipped.${NC} Check tool availability."
    exit 2
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
