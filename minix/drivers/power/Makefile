.include <bsd.own.mk>

# NOTE: i386-specific power driver (acpi) removed in Phase 4 i386 deprecation.

.if ${MACHINE_ARCH} == "earm"
SUBDIR+=	tps65217
SUBDIR+=	tps65950
.endif # ${MACHINE_ARCH} == "earm"

.include <bsd.subdir.mk>
