# minix/lib/libgergios_driver/CMakeLists.txt
# GergiOS Driver Core Library
#
# This library provides the unified driver model for GergiOS:
#   - Unified driver struct (gergios_driver) with type-specific operations
#   - Device abstraction (gergios_device) with state machine
#   - Device ID matching framework
#   - Compatibility wrappers for existing block/char/net drivers
#   - PCI bus scanning and automatic device discovery

cmake_minimum_required(VERSION 3.20)

add_minix_library(gergios_driver
    SOURCES
        core.c
        match.c
        compat.c
        device.c
        pci_scan.c
        dma.c
        iommu.c
        iommu_amd.c
        iommu_vtd.c
        pm.c
        hotplug.c
    INSTALL_DIR /usr/lib
)

# Make the source directory available as a public include path so that
# consumers can #include "gergios_driver.h" and "gergios_device.h".
target_include_directories(gergios_driver INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}")

# Install public headers so they are available system-wide.
install(FILES
    gergios_driver.h
    gergios_device.h
    pci_scan.h
    dma.h
    iommu.h
    pm.h
    hotplug.h
    DESTINATION /usr/include/minix
)
