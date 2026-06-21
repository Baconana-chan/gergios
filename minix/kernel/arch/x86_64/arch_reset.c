#include "kernel/kernel.h"

#include <ctype.h>
#include <string.h>
#include <machine/cmos.h>
#include <machine/bios.h>
#include <machine/cpu.h>
#include <minix/cpufeature.h>
#include <sys/reboot.h>
#include <assert.h>
#include <signal.h>

#include <minix/u64.h>

#include "arch_proto.h"
#include "oxpcie.h"
#include "direct_utils.h"

#ifdef USE_ACPI
#include "acpi.h"
#endif

#define     KBCMDP          4
#define      KBC_PULSE0     0xfe
#define      IO_KBD          0x060

int cpu_has_tsc;

void reset(void)
{
        uint8_t b;
        outb(IO_KBD + KBCMDP, KBC_PULSE0);
        busy_delay_ms(100);
        outb(IO_KBD + KBCMDP, KBC_PULSE0);
        busy_delay_ms(100);

        outb(0xcf9, 0x2);
        outb(0xcf9, 0x6);
        busy_delay_ms(500);

        b = inb(0x92);
        if (b != 0xff) {
                if ((b & 0x1) != 0)
                        outb(0x92, b & 0xfe);
                outb(0x92, b | 0x1);
                busy_delay_ms(500);
        }

	x86_triplefault();

	while(1) {
		;
	}
}

static _Noreturn void halt(void)
{
	for ( ; ; )
		halt_cpu();
}

static _Noreturn void poweroff(void)
{
	const char *shutdown_str;

#ifdef USE_ACPI
	acpi_poweroff();
#endif
	shutdown_str = "Shutdown";
        while (*shutdown_str) outb(0x8900, *(shutdown_str++));

	poweroff_vmware_clihlt();

	halt();
}

_Noreturn void arch_shutdown(int how)
{
	unsigned char unused_ch;
	outb( INT_CTLMASK, ~0);

	while(direct_read_char(&unused_ch))
		;

	if(kinfo.minix_panicing) {
		if (kinfo.do_serial_debug)
			reset();

		direct_cls();
		direct_print("Minix panic. System diagnostics buffer:\n\n");
		direct_print(kmess.kmess_buf);
		direct_print("\nSystem has panicked, press any key to reboot");
		while (!direct_read_char(&unused_ch))
			;
		reset();
	}

	if((how & RB_POWERDOWN) == RB_POWERDOWN) {
		poweroff();
		NOT_REACHABLE;
	}

	if(how & RB_HALT) {
		for (; ; ) halt_cpu();
		NOT_REACHABLE;
	}

	reset();
	NOT_REACHABLE;
}

#ifdef DEBUG_SERIAL
void ser_putc(char c)
{
        int i;
        int lsr, thr;

#if CONFIG_OXPCIE
        oxpcie_putc(c);
#else
        lsr= COM1_LSR;
        thr= COM1_THR;
        for (i= 0; i<100000; i++)
        {
                if (inb( lsr) & LSR_THRE)
                        break;
        }
        outb( thr, c);
#endif
}

#endif
