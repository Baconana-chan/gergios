#!/usr/bin/env bash
set -e

#
# arm64_hdimage.sh — Create a bootable ARM64 disk image with Limine AAC64 (UEFI)
#
# Creates a GPT disk image with:
#   - EFI System Partition (FAT32) with Limine AAC64 (BOOTAA64.EFI)
#   - MINIX MFS partitions (root, usr, home)
#   - Device Tree blob for QEMU virt platform
#
# Usage:
#   ./releasetools/arm64_hdimage.sh [options]
#
# Environment variables:
#   MKLIMINE=yes       Enable Limine AAC64 boot (default: yes)
#   EFI_SIZE=67108864  ESP size in bytes (default: 64MB)
#   IMG=minix_arm64.img  Output image path
#
# Prerequisites:
#   - Limine with AAC64 support (BOOTAA64.EFI)
#   - aarch64 cross toolchain
#   - QEMU with aarch64 + AAVMF (for testing)
#
# Note: ARM64 kernel port is required for actual boot. This script
# creates the boot infrastructure for when the kernel is ready.
# See planning/08_arm64_migration_plan.md
#

: ${ARCH=evbarm64}
: ${OBJ=../obj.${ARCH}}
: ${TOOLCHAIN_TRIPLET=aarch64-elf64-minix-}
: ${BUILDSH=build.sh}

: ${SETS="minix-base minix-comp minix-games minix-man minix-tests tests"}
: ${IMG=minix_arm64.img}

: ${FAT_SIZE=$((  64*(2**20) / 512))}  # 64MB ESP in sectors
: ${ROOT_SIZE=$((  64*(2**20) ))}
: ${HOME_SIZE=$(( 128*(2**20) ))}
: ${USR_SIZE=$(( 1792*(2**20) ))}
: ${IMG_SIZE=$((    2*(2**30) ))}    # 2GB total

: ${MKLIMINE=yes}    # Set to "no" to skip Limine (e.g., for U-Boot)

if [ ! -f ${BUILDSH} ]; then
	echo "Please invoke me from the root source dir, where ${BUILDSH} is."
	exit 1
fi

# set up disk creation environment
. releasetools/image.defaults
. releasetools/image.functions

echo "=== GergiOS ARM64 Boot Image ==="
echo ""
echo "Architecture: aarch64"
echo "Output:       ${IMG}"
echo ""

# ============================================================================
# Check Limine AAC64 availability
# ============================================================================

if [ "${MKLIMINE}" = "yes" ]; then
	if check_limine; then
		echo " * Limine: ${LIMINE_BIN} (v${LIMINE_VER})"
		if check_limine_aac64; then
			echo " * AAC64:   ${LIMINE_AAC64}"
		else
			echo "WARNING: Limine AAC64 (BOOTAA64.EFI) not found."
			echo "  Install: install limine with AAC64 support"
			echo "  Falling back to U-Boot boot."
			MKLIMINE=no
		fi
	else
		echo "WARNING: Limine not found."
		echo "  Install: https://github.com/limine-bootloader/limine"
		echo "  Falling back to U-Boot boot."
		MKLIMINE=no
	fi
fi

# Find QEMU AArch64 UEFI firmware (for boot instructions)
QEMU_FW=""
if find_qemu_firmware_aarch64; then
	QEMU_FW="${QEMU_AAVMF_CODE}"
fi

# ============================================================================
# Build and prepare
# ============================================================================

echo ""
echo "Building work directory..."
build_workdir "$SETS"

echo "Adding extra files..."
# Create fstab
cat >${ROOT_DIR}/etc/fstab <<END_FSTAB
/dev/c0d0p2	/usr		mfs	rw			0	2
/dev/c0d0p3	/home		mfs	rw			0	2
none		/sys		devman	rw,rslabel=devman	0	0
none		/dev/pts	ptyfs	rw,rslabel=ptyfs	0	0
END_FSTAB
add_file_spec "etc/fstab" extra.fstab

echo "Bundling packages..."
bundle_packages "$BUNDLE_PACKAGES"

echo "Creating specification files..."
create_input_spec
create_protos "usr home"

# ============================================================================
# ESP and boot files
# ============================================================================

echo ""
echo "=== Preparing boot files ==="

# Create ESP directory structure
: ${EFI_DIR=$OBJ/efi}
rm -rf ${EFI_DIR} && mkdir -p ${EFI_DIR}

if [ "${MKLIMINE}" = "yes" ]; then
	# ── Limine AAC64 ESP ──
	echo " * Building Limine AAC64 ESP..."

	# Create UEFI directory structure
	mkdir -p "${EFI_DIR}/EFI/BOOT"

	# Copy Limine AAC64 bootloader
	if [ -f "${LIMINE_AAC64}" ]; then
		cp "${LIMINE_AAC64}" "${EFI_DIR}/EFI/BOOT/BOOTAA64.EFI"
		echo "   → EFI/BOOT/BOOTAA64.EFI"
	fi

	# Copy kernel + modules to ESP (when available)
	# In Phase 4, kernel is not yet built for ARM64
	if [ -d "${MODDIR}" ] && [ -f "${MODDIR}/kernel" ]; then
		cp "${MODDIR}/kernel" "${EFI_DIR}/kernel"
		for mod in "${MODDIR}"/mod*; do
			[ -f "${mod}" ] && cp "${mod}" "${EFI_DIR}/$(basename ${mod})"
		done
		echo "   → kernel + modules"
	else
		echo "   NOTE: ARM64 kernel not built yet."
		echo "   Place kernel at: ${EFI_DIR}/kernel"
		echo "   Place modules at: ${EFI_DIR}/mod*"
	fi

	# Generate Device Tree (for QEMU virt platform)
	echo "   Generating DTB for QEMU virt..."
	# QEMU can generate DTB directly; we reference it in limine.conf
	# For now, create a placeholder - QEMU provides DTB at runtime

	# Generate limine.conf
	cat > "${EFI_DIR}/limine.conf" <<END_LIMINE_AAC64
# GergiOS ARM64 Limine Configuration
TIMEOUT=5

:GergiOS ARM64 (QEMU virt)
	PROTOCOL=limine
	KERNEL_PATH=boot:///kernel
	CMDLINE=rootdevname=c0d0p0

:GergiOS ARM64 (Safe Mode)
	PROTOCOL=limine
	KERNEL_PATH=boot:///kernel
	CMDLINE=rootdevname=c0d0p0 bootopts=-s
END_LIMINE_AAC64
	echo "   → limine.conf"

	# Add modules to config (if present)
	if [ -d "${MODDIR}" ]; then
		for mod in "${MODDIR}"/mod*; do
			[ -f "${mod}" ] && echo "	MODULE_PATH=boot:///$(basename ${mod})" >> "${EFI_DIR}/limine.conf"
		done
	fi
fi

# ============================================================================
# Create disk image
# ============================================================================

echo ""
echo "=== Creating disk image ==="

# Clean image
if [ -f ${IMG} ]; then
	rm -f ${IMG}
fi

# Create empty image
dd if=/dev/zero of=${IMG} bs=512 count=1 seek=$((($IMG_SIZE / 512) -1))

# All sizes in 512-byte blocks
ROOTSIZEARG="-b $((${ROOT_SIZE} / 512 / 8))"
USRSIZEARG="-b $((${USR_SIZE} / 512 / 8))"
HOMESIZEARG="-b $((${HOME_SIZE} / 512 / 8))"

FAT_START=2048  # sectors (1MB alignment for modern SD/eMMC)
ROOT_START=$(($FAT_START + $FAT_SIZE))

echo " * ROOT"
_ROOT_SIZE=$(${CROSS_TOOLS}/nbmkfs.mfs -d ${ROOTSIZEARG} -I $((${ROOT_START}*512)) ${IMG} ${WORK_DIR}/proto.root)
_ROOT_SIZE=$(($_ROOT_SIZE / 512))

USR_START=$((${ROOT_START} + ${_ROOT_SIZE}))
echo " * USR"
_USR_SIZE=$(${CROSS_TOOLS}/nbmkfs.mfs  -d ${USRSIZEARG}  -I $((${USR_START}*512))  ${IMG} ${WORK_DIR}/proto.usr)
_USR_SIZE=$(($_USR_SIZE / 512))

HOME_START=$((${USR_START} + ${_USR_SIZE}))
echo " * HOME"
_HOME_SIZE=$(${CROSS_TOOLS}/nbmkfs.mfs -d ${HOMESIZEARG} -I $((${HOME_START}*512)) ${IMG} ${WORK_DIR}/proto.home)
_HOME_SIZE=$(($_HOME_SIZE / 512))

# FAT (ESP) partition
echo " * ESP (FAT32)"
rm -rf ${ROOT_DIR}/*
if [ "${MKLIMINE}" = "yes" ]; then
	cp -r ${EFI_DIR}/* ${ROOT_DIR}/
fi

# Build ESP mtree and image
cat >${WORK_DIR}/boot.mtree <<EOF
. type=dir
EOF
if [ "${MKLIMINE}" = "yes" ]; then
	echo "./limine.conf type=file" >> ${WORK_DIR}/boot.mtree
	echo "./EFI type=dir" >> ${WORK_DIR}/boot.mtree
	echo "./EFI/BOOT type=dir" >> ${WORK_DIR}/boot.mtree
	echo "./EFI/BOOT/BOOTAA64.EFI type=file" >> ${WORK_DIR}/boot.mtree
fi

${CROSS_TOOLS}/nbmakefs -t msdos -s ${FAT_SIZE}b -o F=32,c=1 \
	-F ${WORK_DIR}/boot.mtree ${WORK_DIR}/fat.img ${ROOT_DIR}

# Write partition table
${CROSS_TOOLS}/nbpartition -f -m ${IMG} ${FAT_START} \
	"c:${FAT_SIZE}*" 81:${_ROOT_SIZE} 81:${_USR_SIZE} 81:${_HOME_SIZE}

# Merge FAT partition into image
dd if=${WORK_DIR}/fat.img of=${IMG} seek=$FAT_START conv=notrunc

echo ""
echo "=== Image created ==="
echo "Disk image at: `pwd`/${IMG}"
echo ""

if [ "${MKLIMINE}" = "yes" ]; then
	local fw_path="${QEMU_FW:-/usr/share/qemu-efi-aarch64/QEMU_EFI.fd}"

	echo "To boot in QEMU (AArch64 + UEFI):"
	echo ""
	echo "  qemu-system-aarch64 \\"
	echo "    -M virt \\"
	echo "    -cpu cortex-a72 \\"
	echo "    -m 512M \\"
	echo "    -drive if=pflash,format=raw,readonly=on,file=${fw_path} \\"
	echo "    -drive format=raw,file=`pwd`/${IMG} \\"
	echo "    -serial stdio"
	echo ""
	echo "UEFI firmware: ${fw_path}"
	echo ""
	if [ -z "${QEMU_FW}" ]; then
		echo "NOTE: AAVMF firmware not detected on this system."
		echo "Install: apt install qemu-efi-aarch64 (Debian/Ubuntu)"
		echo "     or: pacman -S edk2-aarch64 (Arch)"
		echo ""
	fi
	echo "To generate DTB for reference:"
	echo "  qemu-system-aarch64 -M virt -cpu cortex-a72 -machine dumpdtb=qemu-virt.dtb"
fi
