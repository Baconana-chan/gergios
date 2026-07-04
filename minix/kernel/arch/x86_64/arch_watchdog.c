/* x86_64 NMI watchdog using Performance Monitoring Counters.
 *
 * Ported from i386 with 64-bit register dump.
 *
 * Uses the Intel Architectural Performance Monitor (CPUID leaf 0xA)
 * or AMD performance counters (families 6, 15, 16, 17) to fire
 * periodic NMIs via LAPIC LVTPCR.
 *
 * The NMI handler (nmi_watchdog_handler in kernel/watchdog.c) checks
 * whether the timer is still ticking and calls arch_watchdog_lockup()
 * if the kernel has been stuck for ~10 timer ticks.
 */

#include "kernel/watchdog.h"
#include "kernel/kernel.h"
#include <minix/minlib.h>
#include <minix/u64.h>

#include "apic.h"

#define CPUID_UNHALTED_CORE_CYCLES_AVAILABLE	0

/*
 * Intel architecture performance counters watchdog
 */

static struct arch_watchdog intel_arch_watchdog;
static struct arch_watchdog amd_watchdog;

static void intel_arch_watchdog_init(const unsigned cpu)
{
	u64_t cpuf;
	u32_t val;

	ia32_msr_write(INTEL_MSR_PERFMON_CRT0, 0, 0);

	/* Int, OS, USR, Core cycles */
	val = 1 << 20 | 1 << 17 | 1 << 16 | 0x3c;
	ia32_msr_write(INTEL_MSR_PERFMON_SEL0, 0, val);

	/*
	 * Should give a tick approx. every 0.5-1s, the perf counter has only
	 * lowest 31 bits writable :(
	 */
	cpuf = cpu_get_freq(cpu);
	while (ex64hi(cpuf) || ex64lo(cpuf) > 0x7fffffffU)
		cpuf /= 2;
	cpuf = make64(-ex64lo(cpuf), ex64hi(cpuf));
	watchdog->resetval = watchdog->watchdog_resetval = cpuf;

	ia32_msr_write(INTEL_MSR_PERFMON_CRT0, 0, ex64lo(cpuf));

	ia32_msr_write(INTEL_MSR_PERFMON_SEL0, 0,
			val | INTEL_MSR_PERFMON_SEL0_ENABLE);

	/* Unmask the performance counter interrupt (NMI delivery) */
	lapic_write(LAPIC_LVTPCR, APIC_ICR_DM_NMI);
}

static void intel_arch_watchdog_reinit(const unsigned cpu)
{
	lapic_write(LAPIC_LVTPCR, APIC_ICR_DM_NMI);
	ia32_msr_write(INTEL_MSR_PERFMON_CRT0, 0, ex64lo(watchdog->resetval));
}

int arch_watchdog_init(void)
{
	u32_t eax, ebx, ecx, edx;
	unsigned cpu = cpuid;

	if (!lapic_addr) {
		printf("ERROR : Cannot use NMI watchdog if APIC is not enabled\n");
		return -1;
	}

	if (cpu_info[cpu].vendor == CPU_VENDOR_INTEL) {
		eax = 0xA;

		_cpuid(&eax, &ebx, &ecx, &edx);

		/*
		 * FIXME currently we support only watchdog based on the Intel
		 * architectural performance counters. Some Intel CPUs don't
		 * have this feature.
		 */
		if (ebx & (1 << CPUID_UNHALTED_CORE_CYCLES_AVAILABLE))
			return -1;
		if (!((((eax >> 8)) & 0xff) > 0))
			return -1;

		watchdog = &intel_arch_watchdog;
	} else if (cpu_info[cpu].vendor == CPU_VENDOR_AMD) {
		if (cpu_info[cpu].family != 6 &&
				cpu_info[cpu].family != 15 &&
				cpu_info[cpu].family != 16 &&
				cpu_info[cpu].family != 17)
			return -1;
		else
			watchdog = &amd_watchdog;
	} else
		return -1;

	/* Setup PC overflow as NMI for watchdog, masked for now */
	lapic_write(LAPIC_LVTPCR, APIC_ICR_INT_MASK | APIC_ICR_DM_NMI);
	(void) lapic_read(LAPIC_LVTPCR);

	/* double check if LAPIC is enabled */
	if (lapic_addr && watchdog->init) {
		watchdog->init(cpuid);
	}

	return 0;
}

void arch_watchdog_stop(void)
{
	/* LVTPCR will be masked — watchdog stops naturally */
}

void arch_watchdog_lockup(const struct nmi_frame * frame)
{
	/*
	 * On x86_64, reg_t is 64 bits wide. Use %lx for compatibility
	 * with both LP64 (Linux/BSD x86_64) and LLP64 (Windows x86_64).
	 */
	printf("KERNEL LOCK UP\n"
		"rax    0x%lx\n"
		"rcx    0x%lx\n"
		"rdx    0x%lx\n"
		"rbx    0x%lx\n"
		"rbp    0x%lx\n"
		"rsi    0x%lx\n"
		"rdi    0x%lx\n"
		"gs     0x%04x\n"
		"fs     0x%04x\n"
		"es     0x%04x\n"
		"ds     0x%04x\n"
		"pc     0x%lx\n"
		"cs     0x%lx\n"
		"eflags 0x%lx\n",
		(unsigned long)frame->eax,
		(unsigned long)frame->ecx,
		(unsigned long)frame->edx,
		(unsigned long)frame->ebx,
		(unsigned long)frame->ebp,
		(unsigned long)frame->esi,
		(unsigned long)frame->edi,
		frame->gs,
		frame->fs,
		frame->es,
		frame->ds,
		(unsigned long)frame->pc,
		(unsigned long)frame->cs,
		(unsigned long)frame->eflags
		);
	panic("Kernel lockup");
}

int x86_64_watchdog_start(void)
{
	if (arch_watchdog_init()) {
		printf("WARNING watchdog initialization "
				"failed! Disabled\n");
		watchdog_enabled = 0;
		return -1;
	}
	else
		BOOT_VERBOSE(printf("Watchdog enabled\n"););

	return 0;
}

static int intel_arch_watchdog_profile_init(const unsigned freq)
{
	u64_t cpuf;

	/* FIXME works only if all CPUs have the same freq */
	cpuf = cpu_get_freq(cpuid);
	cpuf /= freq;

	/*
	 * If freq is too low and the cpu freq too high we may get in a range
	 * of insane value which cannot be handled by the 31bit CPU perf counter
	 */
	if (ex64hi(cpuf) != 0 || ex64lo(cpuf) > 0x7fffffffU) {
		printf("ERROR : nmi watchdog ticks exceed 31bits, "
			"use higher frequency\n");
		return EINVAL;
	}

	cpuf = make64(-ex64lo(cpuf), ex64hi(cpuf));
	watchdog->profile_resetval = cpuf;

	return OK;
}

static struct arch_watchdog intel_arch_watchdog = {
	/*.init = */		intel_arch_watchdog_init,
	/*.reinit = */		intel_arch_watchdog_reinit,
	/*.profile_init = */	intel_arch_watchdog_profile_init
};

#define AMD_MSR_EVENT_SEL0		0xc0010000
#define AMD_MSR_EVENT_CTR0		0xc0010004
#define AMD_MSR_EVENT_SEL0_ENABLE	(1 << 22)

static void amd_watchdog_init(const unsigned cpu)
{
	u64_t cpuf;
	u32_t val;

	ia32_msr_write(AMD_MSR_EVENT_CTR0, 0, 0);

	/* Int, OS, USR, Cycles cpu is running */
	val = 1 << 20 | 1 << 17 | 1 << 16 | 0x76;
	ia32_msr_write(AMD_MSR_EVENT_SEL0, 0, val);

	cpuf = -cpu_get_freq(cpu);
	watchdog->resetval = watchdog->watchdog_resetval = cpuf;

	ia32_msr_write(AMD_MSR_EVENT_CTR0,
		       ex64hi(watchdog->resetval), ex64lo(watchdog->resetval));

	ia32_msr_write(AMD_MSR_EVENT_SEL0, 0,
			val | AMD_MSR_EVENT_SEL0_ENABLE);

	/* unmask the performance counter interrupt */
	lapic_write(LAPIC_LVTPCR, APIC_ICR_DM_NMI);
}

static void amd_watchdog_reinit(const unsigned cpu)
{
	lapic_write(LAPIC_LVTPCR, APIC_ICR_DM_NMI);
	ia32_msr_write(AMD_MSR_EVENT_CTR0,
		       ex64hi(watchdog->resetval), ex64lo(watchdog->resetval));
}

static int amd_watchdog_profile_init(const unsigned freq)
{
	u64_t cpuf;

	/* FIXME works only if all CPUs have the same freq */
	cpuf = cpu_get_freq(cpuid);
	cpuf = -cpuf / freq;

	watchdog->profile_resetval = cpuf;

	return OK;
}

static struct arch_watchdog amd_watchdog = {
	/*.init = */		amd_watchdog_init,
	/*.reinit = */		amd_watchdog_reinit,
	/*.profile_init = */	amd_watchdog_profile_init
};
