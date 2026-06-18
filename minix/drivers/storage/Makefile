.include <bsd.own.mk>

# NOTE: i386-specific storage drivers (at_wini, floppy, ahci, fbd, filter, virtio_blk)
# removed in Phase 4 i386 deprecation.

.if ${MKIMAGEONLY} == "no"
.  if ${MACHINE_ARCH} == "earm"
SUBDIR+=	mmc
.  endif # ${MACHINE_ARCH} == "earm"

SUBDIR+=	vnd
.endif # ${MKIMAGEONLY} == "no"

# memory driver must be last for ramdisk image.
# Everything else must be done before ramdisk as it needs
# executables from other targets.
SUBDIR+=	.WAIT ramdisk .WAIT memory

.include <bsd.subdir.mk>
