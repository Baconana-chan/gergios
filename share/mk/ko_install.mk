# $NetBSD: ko_install.mk,v 1.0 2026/07/06 gergios $
#
# GergiOS Kernel Module Install Target (ko_install.mk)
# =====================================================
#
# Installs pre-built .ko and .so modules into the /lib/modules/ hierarchy.
# Can be used standalone or included from ko.mk.
#
# Usage:
#   .include "ko_install.mk"
#
#   # Or standalone:
#   KERNEL_VER=5.15.0-gergios
#   KO_MODULES=ahci e1000 rtl8139
#   KO_SOURCE_DIR=/path/to/modules
#
# Installed files follow Linux convention:
#   /lib/modules/$(KERNEL_VER)/
#     modules.dep
#     modules.alias
#     modules.symbols
#     modules.softdep
#     kernel/drivers/{net,block,usb,audio,video,extra}/<module>.ko
#
# By default, modules without a subcategory go to kernel/drivers/extra/.
#
# ============================================================================

.if !defined(_KO_INSTALL_MK_)
_KO_INSTALL_MK_=1

# Kernel version (default: uname -r)
KERNEL_VER?=gergios
.if empty(KERNEL_VER)
KERNEL_VER!=sh -c 'uname -r 2>/dev/null || echo "gergios"'
.endif

# Module directory root
MODULES_DIR?=/lib/modules/${KERNEL_VER}

# Source directory for pre-built modules
KO_SOURCE_DIR?=/lib/modules/gergios

# Module subcategory mapping
# Format: module_name=subcategory
# e.g. e1000=net, ahci=block, xhci_hcd=usb
# Modules not listed go to "extra"
KO_MODULE_CATEGORIES?=

# ============================================================================
# Target: install all modules
# ============================================================================

modules_install: _ko_install_prepare _ko_install_copy _ko_install_run_depmod
	@echo "=== Module installation complete ==="

# Ensure hierarchy exists
_ko_install_prepare:
	@echo "=== Preparing /lib/modules/ hierarchy ==="
	@mkdir -p ${MODULES_DIR}/kernel/drivers/{net,block,usb,audio,video,extra}
	@mkdir -p ${MODULES_DIR}/kernel/drivers/{char,misc,input,hid,pci}
	@mkdir -p ${MODULES_DIR}/kernel/drivers/{scsi,ata,mmc,nvme}
	@mkdir -p ${MODULES_DIR}/kernel/drivers/{staging,media,firmware}

# Copy modules from source to their category directories
_ko_install_copy:
.if defined(KO_MODULES) && !empty(KO_MODULES)
	@echo "=== Installing modules ==="
.for _mod_ in ${KO_MODULES}
	@_cat_=""; \
	 for entry in ${KO_MODULE_CATEGORIES}; do \
	   mod=$${entry%%=*}; cat=$${entry#*=}; \
	   [ "$$mod" = "${_mod_}" ] && { _cat_=$$cat; break; }; \
	 done; \
	 _cat_=$${_cat_:-extra}; \
	 if [ -f ${KO_SOURCE_DIR}/${_mod_}.ko ]; then \
	   cp ${KO_SOURCE_DIR}/${_mod_}.ko ${MODULES_DIR}/kernel/drivers/$${_cat_}/; \
	   echo "  INSTALL ${MODULES_DIR}/kernel/drivers/$${_cat_}/${_mod_}.ko"; \
	 elif [ -f ${KO_SOURCE_DIR}/${_mod_}.so ]; then \
	   cp ${KO_SOURCE_DIR}/${_mod_}.so ${MODULES_DIR}/kernel/drivers/$${_cat_}/; \
	   echo "  INSTALL ${MODULES_DIR}/kernel/drivers/$${_cat_}/${_mod_}.so"; \
	 else \
	   echo "  WARNING: ${_mod_} not found in ${KO_SOURCE_DIR}"; \
	 fi
.endfor
.else
	@echo "  No modules specified (KO_MODULES is empty)"
	@echo "  Copying all .ko and .so files from ${KO_SOURCE_DIR}..."
	@for f in ${KO_SOURCE_DIR}/*.ko ${KO_SOURCE_DIR}/*.so; do \
	  [ -f "$$f" ] || continue; \
	  base=$$(basename $$f); \
	  name=$${base%.*}; \
	  ext=$${base##*.}; \
	  cp "$$f" ${MODULES_DIR}/kernel/drivers/extra/; \
	  echo "  INSTALL extra/$$base"; \
	done
.endif

# Run depmod to generate dependency and alias files
_ko_install_run_depmod:
	@echo "=== Running depmod ==="
	@if command -v depmod >/dev/null 2>&1; then \
	  depmod -a -o ${MODULES_DIR} && \
	  echo "  depmod: generated modules.dep, modules.alias, modules.symbols"; \
	else \
	  echo "  WARNING: depmod not found — run 'depmod -a -o ${MODULES_DIR}' manually"; \
	fi

# ============================================================================
# Target: install a single module
# ============================================================================

# Install a single module by name
# Usage: make install_module MODULE=ahci CATEGORY=block
install_module: _ko_install_prepare
.if defined(MODULE)
	@_cat_="${CATEGORY:-extra}"; \
	 if [ -f ${KO_SOURCE_DIR}/${MODULE}.ko ]; then \
	   cp ${KO_SOURCE_DIR}/${MODULE}.ko ${MODULES_DIR}/kernel/drivers/$${_cat_}/; \
	   echo "  INSTALL ${MODULES_DIR}/kernel/drivers/$${_cat_}/${MODULE}.ko"; \
	 elif [ -f ${KO_SOURCE_DIR}/${MODULE}.so ]; then \
	   cp ${KO_SOURCE_DIR}/${MODULE}.so ${MODULES_DIR}/kernel/drivers/$${_cat_}/; \
	   echo "  INSTALL ${MODULES_DIR}/kernel/drivers/$${_cat_}/${MODULE}.so"; \
	 else \
	   echo "  ERROR: ${MODULE} not found in ${KO_SOURCE_DIR}"; \
	   exit 1; \
	 fi
.else
	@echo "Usage: make install_module MODULE=module_name [CATEGORY=category]"
	@exit 1
.endif

# ============================================================================
# Target: uninstall (remove installed modules)
# ============================================================================

modules_uninstall:
	@echo "=== Removing installed modules ==="
	@rm -rf ${MODULES_DIR}/kernel
	@rm -f ${MODULES_DIR}/modules.dep
	@rm -f ${MODULES_DIR}/modules.alias
	@rm -f ${MODULES_DIR}/modules.symbols
	@echo "  Removed ${MODULES_DIR}/kernel/ and metadata files"

.endif  # !defined(_KO_INSTALL_MK_)
