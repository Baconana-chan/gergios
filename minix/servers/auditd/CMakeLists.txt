# minix/servers/auditd/CMakeLists.txt
# auditd — Audit Daemon

cmake_minimum_required(VERSION 3.20)

# Source files
set(AUDITD_SOURCES
    auditd.c
)

# Library target
add_executable(auditd
    ${AUDITD_SOURCES}
)

# Link dependencies
target_link_libraries(auditd PRIVATE sys)

# Install to /service
install(TARGETS auditd DESTINATION /service)
