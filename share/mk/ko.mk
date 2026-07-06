# $NetBSD: ko.mk,v 1.0 2026/07/06 gergios $
#
# GergiOS LKM Build System (ko.mk)
# =================================
#
# Linux Kbuild-compatible interface for building .ko kernel modules.
# Implements the standard obj-m / <module>-objs / ccflags-y syntax.
#
# Usage in a module Makefile:
#
#   .include "ko.mk"
#
#   obj-m += e1000.o
#   e1000-objs := main.o pci.o phy.o
#   ccflags-y := -DDEBUG -O2
#
#   # Optional:
#   EXTRA_SYMBOLS += /path/to/Module.symvers
#   hostprogs-y := gen_firmware
#
# Build invocation:
#
#   make -f /usr/share/mk/ko.mk M=/path/to/module
#
#   # Or from module directory:
#   make -C /lib/modules/$(uname -r)/build M=$(pwd) modules
#
# Supported variables:
#   obj-m          — List of .ko targets (e.g. obj-m += foo.o → builds foo.ko)
#   <module>-objs  — Object files for a multi-file module
#   ccflags-y      — Extra C compiler flags
#   asflags-y      — Extra assembler flags
#   ldflags-y      — Extra linker flags
#   EXTRA_CFLAGS   — Additional C flags (from Kbuild compat)
#   EXTRA_SYMBOLS  — Paths to external Module.symvers files
#
# GergiOS extensions:
#   ko_type        — "ko" (default) or "so" for shared object modules
#   ko_install_dir — Override install subdirectory (default: extra)
#   ko_subdir      — Kernel driver subcategory (net, block, usb, audio, video)
#
# ============================================================================

# Guard against multiple inclusion
.if !defined(_KO_MK_)
_KO_MK_=1

# ============================================================================
# Default settings
# ============================================================================

# Module type: "ko" for ELF relocatable, "so" for shared object
ko_type?=ko

# Kernel driver subcategory (affects install path)
ko_subdir?=extra

# Module install directory (under /lib/modules/$(KERNEL_VER)/kernel/drivers/)
ko_install_dir?=${ko_subdir}

# ============================================================================
# Kernel flags (matching Linux Kbuild conventions)
# ============================================================================

# These are the flags that Linux kernel modules are compiled with.
# GergiOS kernel_shim provides the Linux kernel API emulation.

KO_CFLAGS+=	-D__KERNEL__ -DMODULE
KO_CFLAGS+=	-D__LINUX_KERNEL_MODULE__
KO_CFLAGS+=	-fno-stack-protector -fomit-frame-pointer
KO_CFLAGS+=	-fno-strict-aliasing -fno-common
KO_CFLAGS+=	-fno-builtin -ffreestanding
KO_CFLAGS+=	-Wall -Werror -Wno-unused-variable
KO_CFLAGS+=	-Wno-pointer-sign -Wno-address-of-packed-member
KO_CFLAGS+=	-DKBUILD_MODNAME=\"${.TARGET:T:R}\"
KO_CFLAGS+=	-include ${NETBSDSRCDIR}/minix/include/linux/kernel_shim_compat.h

# Include kernel_shim headers
KO_CFLAGS+=	-I${NETBSDSRCDIR}/minix/lib/libgergios_driver
KO_CFLAGS+=	-I${NETBSDSRCDIR}/minix/include
KO_CFLAGS+=	-I${NETBSDSRCDIR}/sys

# ASM flags
KO_AFLAGS+=	-D__ASSEMBLY__ -D__KERNEL__

# Linker flags for relocatable object (ET_REL)
KO_LDFLAGS+=	-r -z noexecstack

# Optional debug info (strip by default, add -g for debugging)
.if !defined(KO_DEBUG)
KO_LDFLAGS+=	-S  # Strip debug from final .ko
.endif

# ============================================================================
# Target detection
# ============================================================================

# Determine kernel version for install path
KERNEL_VER?=gergios
.if empty(KERNEL_VER)
KERNEL_VER!=sh -c 'uname -r 2>/dev/null || echo "gergios"'
.endif

MODULES_DIR?=/lib/modules/${KERNEL_VER}
MODULES_INSTALL_DIR?=${MODULES_DIR}/kernel/drivers/${ko_install_dir}

# ============================================================================
# obj-m parsing
# ============================================================================

# Collect all object targets from obj-m
# obj-m += foo.o → builds foo.ko from foo.o (and potentially foo-objs)
_KO_OBJECTS?=	# List of .o files that turn into .ko

.for _mod_ in ${obj-m}
_mod_name_:=${_mod_:R}  # Strip .o suffix
.if !target(_ko_build_${_mod_name_})
_KO_OBJECTS+=${_mod_}

# Determine if multi-file: check if ${_mod_name_}-objs is defined
_objs:=${${_mod_name_}-objs}
.if !empty(_objs)
# Multi-file module: compile each source, then link together
${_mod_name_}.ko: ${_objs}
	${_MKTARGET_LINK}
	${LD} ${KO_LDFLAGS} -o ${.TARGET} ${.ALLSRC}
	@echo "  KOMOD ${.TARGET}"

# Compile each source file in the module
.if !target(_ko_srcs_defined_${_mod_name_})
.for _src_ in ${_objs:O:u}
_SRC_C_:=${_src_:R}.c
.if exists(${_SRC_C_})
${_src_}: ${_SRC_C_}
	${_MKTARGET_COMPILE}
	${CC} ${KO_CFLAGS} ${ccflags-y} ${EXTRA_CFLAGS} -c -o ${.TARGET} ${_SRC_C_}
.endif
_SRC_S_:=${_src_:R}.s
.if exists(${_SRC_S_})
${_src_}: ${_SRC_S_}
	${_MKTARGET_COMPILE}
	${CC} ${KO_AFLAGS} ${asflags-y} -c -o ${.TARGET} ${_SRC_S_}
.endif
.endfor
_ko_srcs_defined_${_mod_name_}:
.endif

.else
# Single-file module: foo.c → foo.o → foo.ko
${_mod_name_}.ko: ${_mod_name_}.o
	${_MKTARGET_LINK}
	${LD} ${KO_LDFLAGS} -o ${.TARGET} ${.ALLSRC}
	@echo "  KOMOD ${.TARGET}"

${_mod_name_}.o: ${_mod_name_}.c
	${_MKTARGET_COMPILE}
	${CC} ${KO_CFLAGS} ${ccflags-y} ${EXTRA_CFLAGS} -c -o ${.TARGET} ${_mod_name_}.c
.endif

.if !target(_ko_clean_${_mod_name_})
_ko_clean_${_mod_name_}:
	rm -f ${_mod_name_}.ko ${_objs} ${_mod_name_}.o
.endif
.endif
.endfor

# ============================================================================
# Top-level targets
# ============================================================================

.PHONY: all modules modules_install clean _ko_clean _ko_help

# Build all modules (Linux Kbuild compat)
all modules: ${_KO_OBJECTS:O:u:S,.o$,.ko,}
	@echo "  === GergiOS LKM build complete ==="
	@echo "  Modules: ${.ALLSRC}"

# Clean all module artifacts
clean:
	rm -f *.o *.ko *.mod.c .*.cmd Module.symvers modules.order
.for _mod_ in ${obj-m}
	${MAKE} _ko_clean_${_mod_:R}
.endfor

# ============================================================================
# Install targets (ko_install.mk is included separately)
# ============================================================================

# List modules that would be built
modules_install: modules
.if !empty(obj-m)
	@echo "  === Installing modules to ${MODULES_INSTALL_DIR} ==="
	@mkdir -p ${MODULES_INSTALL_DIR}
.for _mod_ in ${obj-m}
	@cp ${_mod_:R}.ko ${MODULES_INSTALL_DIR}/
	@echo "  INSTALL ${MODULES_INSTALL_DIR}/${_mod_:R}.ko"
.endfor
	@echo "  === Running depmod ==="
	@depmod -a -o ${MODULES_DIR} 2>/dev/null || true
.else
	@echo "  No modules to install (obj-m is empty)"
.endif

# Module.symvers generation (for external module dependencies)
modules_symvers:
	@echo "  === Generating Module.symvers ==="
	@rm -f Module.symvers
.for _mod_ in ${obj-m}
	@nm ${_mod_:R}.ko 2>/dev/null | \
	    awk '/ [TD] / {print $$1, $$3, "${_mod_:R}"}' >> Module.symvers || true
.endfor

# Help
_ko_help:
	@echo "GergiOS LKM Build System (ko.mk)"
	@echo ""
	@echo "Targets:"
	@echo "  all              Build all modules (default)"
	@echo "  modules          Alias for all"
	@echo "  clean            Remove build artifacts"
	@echo "  modules_install  Install built modules to ${MODULES_INSTALL_DIR}"
	@echo ""
	@echo "Example:"
	@echo "  obj-m += mydrv.o"
	@echo "  .include \"ko.mk\""
	@echo ""
	@echo "  make all"
	@echo "  make modules_install"

.endif  # !defined(_KO_MK_)
