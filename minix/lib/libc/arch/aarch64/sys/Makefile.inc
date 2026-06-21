# AArch64 MINIX libc syscall wrappers
HERE=${NETBSDSRCDIR}/minix/lib/libc/arch/${ARCHSUBDIR}/sys
.PATH: ${HERE}

TMP=ucontextoffsets.h.tmp
CF=${HERE}/ucontextoffsets.cf

INCS+=ucontextoffsets.h

ucontext.o: ucontextoffsets.h

SRCS+=   \
	__sigreturn.S \
	_do_kernel_call_intr.S \
	_ipc.S \
	brksize.S \
	ipc_minix_kerninfo.S \
	ucontext.S

ucontextoffsets.h: ${CF}
ucontextoffsets.h: ${NETBSDSRCDIR}/sys/sys/ucontext.h
ucontextoffsets.h: ${NETBSDSRCDIR}/sys/arch/aarch64/include/mcontext.h
ucontextoffsets.h:
	${_MKTARGET_CREATE}
	${TOOL_CAT} ${CF} | \
		${TOOL_GENASSYM} -- ${CC} ${CFLAGS:N-Wa,*} \
		${CPPFLAGS} ${PROF} ${GENASSYM_CPPFLAGS} >$TMP && \
	mv -f $TMP $@
