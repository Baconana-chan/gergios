.include <bsd.own.mk>

SUBDIR+=	mfs
SUBDIR+=	pfs

.if ${MKIMAGEONLY} == "no"
SUBDIR+=	ext2
SUBDIR+=	isofs
SUBDIR+=	procfs
SUBDIR+=	ptyfs
# NOTE: i386-specific filesystems (hgfs, vbfs) removed in Phase 4 i386 deprecation
.endif # ${MKIMAGEONLY} == "no"

.include <bsd.subdir.mk>
