C_HERE=${NETBSDSRCDIR}/minix/lib/libc/arch/${ARCHSUBDIR}
.PATH: ${C_HERE}

SRCS+=\t_cpuid.S\t\\\n\tget_bp.S \t\\\n\tgetprocessor.S\t\\\n\tread_tsc.S\t\\\n\t_cpufeature.c\n
