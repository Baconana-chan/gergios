#!/usr/bin/env bash
set -e

#
# This script creates a bootable image and should at some point in the future
# be replaced by the proper NetBSD infrastructure.
#

: ${ARCH=i386}
: ${OBJ=../obj.${ARCH}}
: ${TOOLCHAIN_TRIPLET=i586-elf32-minix-}
: ${BUILDSH=build.sh}

: ${SETS="minix-base minix-comp minix-games minix-man minix-tests tests"}
: ${IMG=minix_x86.img}

if [ ! -f ${BUILDSH} ]
then
	echo "Please invoke me from the root source dir, where ${BUILDSH} is."
	exit 1
fi

# we create a disk image of about 2 gig's
# for alignment reasons, prefer sizes which are multiples of 4096 bytes
: ${BOOTXX_SECS=32}
: ${ROOT_SIZE=$((  128*(2**20) - ${BOOTXX_SECS} * 512 ))}
: ${HOME_SIZE=$((  128*(2**20) ))}
: ${USR_SIZE=$((  1792*(2**20) ))}
: ${EFI_SIZE=$((  0  ))}  # Size of EFI System Partition (0 = no ESP)
: ${MKLIMINE=no}         # Set to "yes" to use Limine bootloader instead of GRUB
: ${SB_KEY_DIR=}           # Path to Secure Boot signing keys (auto-detect if empty)

# set up disk creation environment
. releasetools/image.defaults
. releasetools/image.functions

echo "Building work directory..."
build_workdir "$SETS"

echo "Adding extra files..."
workdir_add_hdd_files

# add kernels
add_link_spec "boot/minix_latest" "minix_default" extra.kernel
workdir_add_kernel minix_default
workdir_add_kernel minix/$RELEASE_VERSION

# add boot.cfg (GRUB/Multiboot)
cat >${ROOT_DIR}/boot.cfg <<END_BOOT_CFG
menu=Start GergiOS:load_mods /boot/minix_default/mod*; multiboot /boot/minix_default/kernel rootdevname=c0d0p0
menu=Start latest GergiOS:load_mods /boot/minix_latest/mod*; multiboot /boot/minix_latest/kernel rootdevname=c0d0p0
menu=Start latest GergiOS (safe mode):load_mods /boot/minix_latest/mod*; multiboot /boot/minix_latest/kernel rootdevname=c0d0p0 bootopts=-s
menu=Edit menu option:edit
menu=Drop to boot prompt:prompt
clear=1
timeout=5
default=2
menu=Start GergiOS ($RELEASE_VERSION):load_mods /boot/minix/$RELEASE_VERSION/mod*; multiboot /boot/minix/$RELEASE_VERSION/kernel rootdevname=c0d0p0
END_BOOT_CFG
add_file_spec "boot.cfg" extra.boot

# If MKLIMINE=yes, check Limine availability early
if [ "${MKLIMINE:-no}" = "yes" ] && [ ${EFI_SIZE} -ge 512 ]; then
	if check_limine; then
		echo " * Limine: ${LIMINE_BIN} (v${LIMINE_VER}) at ${LIMINE_DATA}"
	else
		echo "WARNING: MKLIMINE=yes but Limine not found."
		echo "  Install: https://github.com/limine-bootloader/limine"
		echo "  Falling back to GRUB EFI boot."
		MKLIMINE=no
	fi
fi

# Check Secure Boot signing infrastructure when MKLIMINE=yes
SB_SIGN=0
if [ "${MKLIMINE:-no}" = "yes" ] && [ ${EFI_SIZE} -ge 512 ]; then
	if find_signing_tools; then
		# Locate signing keys
		if find_sb_keys; then
			echo " * Secure Boot keys: ${SB_KEY}"
			SB_SIGN=1
		else
			echo " * Secure Boot: signing keys not found"
			echo "   Set SB_KEY_DIR or run: ./releasetools/gen_secure_boot_keys.sh"
			echo "   Images will be unsigned (Secure Boot testing requires signed EFI)"
		fi
	else
		echo " * Secure Boot: sbsign/sbverify not found"
		echo "   Install: apt install sbsigntool (Debian/Ubuntu) or pacman -S sbsigntools (Arch)"
		echo "   Images will be unsigned"
	fi
fi

echo "Bundling packages..."
bundle_packages "$BUNDLE_PACKAGES"

echo "Creating specification files..."
create_input_spec
create_protos "usr home"

# Clean image
if [ -f ${IMG} ]	# IMG might be a block device
then
	rm -f ${IMG}
fi

#
# Generate /root, /usr and /home partition images.
#
echo "Writing disk image..."

# all sizes are written in 512 byte blocks
ROOTSIZEARG="-b $((${ROOT_SIZE} / 512 / 8))"
USRSIZEARG="-b $((${USR_SIZE} / 512 / 8))"
HOMESIZEARG="-b $((${HOME_SIZE} / 512 / 8))"

if [ ${EFI_SIZE} -ge 512 ]
then
	: ${EFI_DIR=$OBJ/efi}
	rm -rf ${EFI_DIR} && mkdir -p ${EFI_DIR}

	if [ "${MKLIMINE:-no}" = "yes" ]; then
		# ── Limine UEFI + BIOS ESP ──
		echo " * Building Limine ESP..."

		# Copy kernel + modules to ESP root (Limine reads from FAT32)
		cp ${MODDIR}/kernel ${EFI_DIR}/kernel
		for mod in ${MODDIR}/mod*; do
			cp "${mod}" "${EFI_DIR}/$(basename ${mod})"
		done

		# Copy Limine bootloader files
		mkdir -p "${EFI_DIR}/EFI/BOOT"
		if [ -f "${LIMINE_DATA}/BOOTX64.EFI" ]; then
			cp "${LIMINE_DATA}/BOOTX64.EFI" "${EFI_DIR}/EFI/BOOT/BOOTX64.EFI"
		fi
		if [ -f "${LIMINE_DATA}/limine.sys" ]; then
			cp "${LIMINE_DATA}/limine.sys" "${EFI_DIR}/limine.sys"
		fi

		# Generate limine.conf
		create_limine_cfg "${EFI_DIR}/limine.conf" \
			"/kernel" "${EFI_DIR}" "rootdevname=c0d0p0"

		# Sign EFI bootloader if keys are available
		if [ "${SB_SIGN}" = "1" ] && [ -f "${EFI_DIR}/EFI/BOOT/BOOTX64.EFI" ]; then
			sign_efi "${EFI_DIR}/EFI/BOOT/BOOTX64.EFI" || {
				echo "   WARNING: Failed to sign BOOTX64.EFI, continuing unsigned"
			}

			# Copy signing.der to ESP for MOK enrollment reference
			if [ -n "${SB_DER}" ] && [ -f "${SB_DER}" ]; then
				cp "${SB_DER}" "${EFI_DIR}/gergios-sb.der"
			fi
		fi

		# Record that we need to install Limine stage 1 after image creation
		LIMINE_INSTALL_REQUIRED=1
	else
		# ── GRUB EFI ESP (legacy) ──
		echo " * Building GRUB ESP..."
		fetch_and_build_grub

		mkdir -p ${EFI_DIR}/boot/minix_default ${EFI_DIR}/boot/efi
		create_grub_cfg
		cp ${MODDIR}/* ${EFI_DIR}/boot/minix_default/
		cp ${RELEASETOOLSDIR}/grub/grub-core/booti386.efi ${EFI_DIR}/boot/efi
		cp ${RELEASETOOLSDIR}/grub/grub-core/*.mod ${EFI_DIR}/boot/efi
	fi
fi

ROOT_START=${BOOTXX_SECS}
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

#
# Write the partition table using the natively compiled
# minix partition utility
#
if [ ${EFI_SIZE} -ge 512 ]
then
       dd if=/dev/zero bs=${EFI_SIZE} count=1 > ${OBJ}/efi.img
       EFI_START=$((${HOME_START} + ${_HOME_SIZE}))
       echo " * EFI"
       ${CROSS_TOOLS}/nbmakefs -t msdos -s ${EFI_SIZE} -o "F=32,c=1" ${OBJ}/efi.img ${EFI_DIR}
       dd if=${OBJ}/efi.img >> ${IMG}
       ${CROSS_TOOLS}/nbpartition -m ${IMG} ${BOOTXX_SECS} 81:${_ROOT_SIZE}* 81:${_USR_SIZE} 81:${_HOME_SIZE} EF:1+
else
       ${CROSS_TOOLS}/nbpartition -m ${IMG} ${BOOTXX_SECS} 81:${_ROOT_SIZE}* 81:${_USR_SIZE} 81:${_HOME_SIZE}
fi

${CROSS_TOOLS}/nbinstallboot -f -m ${ARCH} ${IMG} ${DESTDIR}/usr/mdec/bootxx_minixfs3

# If we built a Limine ESP, install Limine stage 1 into MBR/GPT
if [ "${LIMINE_INSTALL_REQUIRED:-0}" = "1" ] && [ -n "${LIMINE_BIN}" ]; then
	echo " * Installing Limine stage 1..."
	# For BIOS boot: limine bios-install writes stage 1 into the MBR
	# For UEFI boot: the ESP already has EFI/BOOT/BOOTX64.EFI
	"${LIMINE_BIN}" bios-install "${IMG}" 2>/dev/null || {
		echo "   WARNING: limine bios-install failed (expected on non-Linux host)"
		echo "   On Linux, run manually: ${LIMINE_BIN} bios-install ${IMG}"
	}
fi

echo ""
echo "Disk image at `pwd`/${IMG}"
echo ""
if [ "${MKLIMINE}" = "yes" ]; then
	echo "To boot in QEMU (BIOS):"
	echo "qemu-system-x86_64 --enable-kvm -m 256M -drive file=${IMG},if=ide,format=raw -serial stdio"
	echo ""
	echo "To boot in QEMU (UEFI):"
	echo "qemu-system-x86_64 --enable-kvm -m 256M -bios /usr/share/ovmf/OVMF_CODE.fd -drive file=${IMG},if=ide,format=raw -serial stdio"
	echo ""
else
	echo "To boot this image on kvm using the bootloader:"
	echo "qemu-system-i386 --enable-kvm -m 256 -hda `pwd`/${IMG}"
	echo ""
	echo "To boot this image on kvm:"
	echo "cd ${MODDIR} && qemu-system-i386 --enable-kvm -m 256M -kernel kernel -append \"rootdevname=c0d0p0\" -initrd \"${mods}\" -hda `pwd`/${IMG}"
	echo "To boot this image on kvm with EFI (tianocore OVMF):"
	echo "qemu-system-i386 -L . -bios OVMF-i32.fd -m 256M -drive file=minix_x86.img,if=ide,format=raw"
fi
