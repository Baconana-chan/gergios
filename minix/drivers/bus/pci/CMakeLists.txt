# minix/drivers/bus/pci/CMakeLists.txt
# PCI driver
#
# Replicates minix/drivers/bus/pci/Makefile

add_minix_service(pci
    SOURCES
        main.c
        pci.c
        pci_table.c
        ${MINIX_SOURCE_DIR}/sys/dev/pci/dev_verbose.c
        ${MINIX_SOURCE_DIR}/sys/dev/pci/pci_subr.c
    LIBS chardriver acpi
)

# Additional include paths for files from sys/dev/pci
target_include_directories(pci PRIVATE
    ${MINIX_SOURCE_DIR}/sys
    ${MINIX_SOURCE_DIR}/sys/dev
    ${MINIX_SOURCE_DIR}/sys/dev/pci
)

set_source_files_properties(pci_subr.c PROPERTIES
    COMPILE_DEFINITIONS "_PCI_SERVER"
)
