.include <bsd.own.mk>

# NOTE: i386-specific network drivers (e1000, rtl8139, virtio_net, etc.)
# removed in Phase 4 i386 deprecation.

.if ${MACHINE_ARCH} == "earm"
SUBDIR+=	lan8710a
.endif # ${MACHINE_ARCH} == "earm"

.include <bsd.subdir.mk>
