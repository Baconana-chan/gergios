.include <bsd.own.mk>

# *** DEPRECATED: Use minix/servers/CMakeLists.txt instead ***

SUBDIR+=	ds input mib pm rs sched vfs vm

.if ${MKIMAGEONLY} == "no"
SUBDIR+=	ipc is devman
.endif

.include <bsd.subdir.mk>
