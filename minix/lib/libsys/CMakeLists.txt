# minix/lib/libsys/CMakeLists.txt
# System library for servers and drivers (libsys)
#
# Replicates minix/lib/libsys/Makefile

# Architecture-specific sources
if(MACHINE_ARCH STREQUAL "i386")
    set(LIBSYS_ARCH_SOURCES
        arch/i386/get_randomness.c
        arch/i386/getidle.c
        arch/i386/ser_putc.c
        arch/i386/spin.c
        arch/i386/sys_eniop.c
        arch/i386/sys_in.c
        arch/i386/sys_out.c
        arch/i386/sys_readbios.c
        arch/i386/sys_sdevio.c
        arch/i386/sys_umap_remote.c
        arch/i386/sys_vinb.c
        arch/i386/sys_vinl.c
        arch/i386/sys_vinw.c
        arch/i386/sys_voutb.c
        arch/i386/sys_voutl.c
        arch/i386/sys_voutw.c
        arch/i386/tsc_util.c
        arch/i386/vbox.c
    )
elseif(MACHINE_ARCH STREQUAL "earm")
    set(LIBSYS_ARCH_SOURCES
        arch/earm/frclock_util.c
        arch/earm/spin.c
        arch/earm/tsc_util.c
    )
endif()

set(LIBSYS_COMMON_SOURCES
    alloc_util.c
    assert.c
    asynsend.c
    clock_time.c
    closenb.c
    copyfd.c
    cpuavg.c
    ds.c
    env_get_prm.c
    env_panic.c
    env_parse.c
    fkey_ctl.c
    getepinfo.c
    getprocnr.c
    getticks.c
    getsysinfo.c
    getuptime.c
    kernel_call.c
    kprintf.c
    kputc.c
    kputs.c
    mapdriver.c
    optset.c
    panic.c
    proceventmask.c
    rmib.c
    safecopies.c
    sched_start.c
    sched_stop.c
    sef.c
    sef_fi.c
    sef_init.c
    sef_liveupdate.c
    sef_llvm.c
    sef_ping.c
    sef_signal.c
    sef_st.c
    socketpath.c
    sqrt_approx.c
    srv_fork.c
    srv_kill.c
    stacktrace.c
    sys_abort.c
    sys_clear.c
    sys_diagctl.c
    sys_endsig.c
    sys_exec.c
    sys_exit.c
    sys_fork.c
    sys_getinfo.c
    sys_getsig.c
    sys_hz.c
    sys_irqctl.c
    sys_kill.c
    sys_mcontext.c
    sys_memset.c
    sys_padconf.c
    sys_physcopy.c
    sys_privctl.c
    sys_runctl.c
    sys_safecopy.c
    sys_safememset.c
    sys_schedctl.c
    sys_schedule.c
    sys_setalarm.c
    sys_setgrant.c
    sys_settime.c
    sys_sigreturn.c
    sys_sigsend.c
    sys_sprof.c
    sys_statectl.c
    sys_stime.c
    sys_times.c
    sys_trace.c
    sys_umap.c
    sys_update.c
    sys_vircopy.c
    sys_vmctl.c
    sys_vsafecopy.c
    sys_vtimer.c
    sys_vumap.c
    taskcall.c
    tickdelay.c
    timers.c
    vm_cache.c
    vm_exit.c
    vm_fork.c
    vm_getrusage.c
    vm_info.c
    vm_map_phys.c
    vm_memctl.c
    vm_prepare.c
    vm_procctl.c
    vm_set_priv.c
    vm_update.c
)

# PCI sources (conditional)
if(MKPCI)
    set(LIBSYS_PCI_SOURCES
        pci_attr_r16.c
        pci_attr_r32.c
        pci_attr_r8.c
        pci_attr_w16.c
        pci_attr_w32.c
        pci_attr_w8.c
        pci_del_acl.c
        pci_dev_name.c
        pci_find_dev.c
        pci_first_dev.c
        pci_get_bar.c
        pci_ids.c
        pci_init.c
        pci_next_dev.c
        pci_rescan_bus.c
        pci_reserve.c
        pci_set_acl.c
        pci_slot_name.c
    )
endif()

# Coverage sources (conditional)
if(MKCOVERAGE)
    set(LIBSYS_COV_SOURCES
        gcov.c
        sef_gcov.c
        llvm_gcov.c
    )
endif()

add_minix_library(sys
    SOURCES
        ${LIBSYS_COMMON_SOURCES}
        ${LIBSYS_ARCH_SOURCES}
        ${LIBSYS_PCI_SOURCES}
        ${LIBSYS_COV_SOURCES}
    INSTALL_DIR /usr/lib
)

# Compile definitions
target_compile_definitions(sys PRIVATE
    _MINIX_SYSTEM
    _SYSTEM
)

target_compile_options(sys PRIVATE -fno-builtin)

# Per-file include paths
set_source_files_properties(sched_start.c sef_st.c PROPERTIES
    INCLUDE_DIRECTORIES "${MINIX_SOURCE_DIR}/minix"
)

# Conditional compile definitions
if(USE_LIVEUPDATE)
    target_compile_definitions(sys PRIVATE USE_LIVEUPDATE)
endif()
if(USE_SYSDEBUG)
    target_compile_definitions(sys PRIVATE USE_SYSDEBUG)
endif()
