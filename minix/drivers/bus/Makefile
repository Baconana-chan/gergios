.include <bsd.own.mk>

# NOTE: i386-specific bus drivers (pci, ti1225) removed in Phase 4 i386 deprecation.

.if ${MKIMAGEONLY} == "no"
.  if ${MACHINE_ARCH} == "earm"
SUBDIR+=	i2c
.  endif # ${MACHINE_ARCH} == "earm"
.endif # ${MKIMAGEONLY} == "no"

.include <bsd.subdir.mk>
