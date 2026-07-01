# minix/drivers/storage/ahci/CMakeLists.txt
# AHCI SATA driver — Rust implementation (Phase 5 pilot)
#
# Builds a C shim that delegates to the minix-ahci Rust static library.
# The Rust crate provides the full AHCI driver implementation:
#   - HBA init/reset/probe
#   - Port state machine + DMA
#   - ATA commands (IDENTIFY, DMA R/W, FLUSH, SET FEATURES)
#   - Blockdriver table + SEF lifecycle
#   - Interrupt handler
#
# Dependencies:
#   - rust_minix-ahci (Rust staticlib via add_rust_library())
#   - libblockdriver (blockdriver_mt_*)
#   - libsys (pci_*, sys_irq*, vm_*, sef_*, env_*)
#   - libmthread (worker thread support)
#   - libtimers (timer management)
#   - libminc (MINIX libc)

# Only build if the Rust library target exists (requires cargo)
if(TARGET rust_minix-ahci)
    # Create the executable first (NO_DEFAULT_LIBS = no auto sys/minc linking)
    # We'll handle all linking explicitly to ensure correct static link order.
    add_minix_executable(ahci
        SOURCES ahci_rust_shim.c
        NO_DEFAULT_LIBS
        BINDIR /usr/sbin
    )

    # Explicit link order for static libraries:
    #   ahci_rust_shim.o → rust_minix-ahci → blockdriver → timers → sys → mthread → minc
    # The Rust staticlib (consumer) MUST come BEFORE system libs (providers)
    # because static linkers resolve symbols left-to-right.
    # Order: consumer references symbols from provider → provider comes last.
    target_link_libraries(ahci PRIVATE
        rust_minix-ahci       # provides ahci_rust_main, references PCI/IRQ/VM/blockdriver
        blockdriver           # provides blockdriver_mt_task et al.
        timers                # provides timer management (SPIN_UNTIL pattern)
        sys                   # provides pci_*, sys_irq*, vm_*, sef_*, env_*, sys_hz, micro_delay
        mthread               # provides threading support (needed by blockdriver_mt)
        minc                  # provides printf etc. (must be last)
    )

    # Ensure the Rust library is rebuilt when the C shim changes
    # The cargo custom target is created by add_rust_library(minix-ahci) in root CMakeLists.txt
    add_dependencies(ahci minix-ahci_cargo)
endif()
