/* 8259 PIC interrupt controller driver (x86_64 — same as i386). */

#include "kernel/kernel.h"
#include "arch_proto.h"
#include "hw_intr.h"
#include <machine/cpu.h>

#define ICW1_AT         0x11
#define ICW1_PC         0x13
#define ICW1_PS         0x19
#define ICW4_AT_SLAVE   0x01
#define ICW4_AT_MASTER  0x05
#define ICW4_PC_SLAVE   0x09
#define ICW4_PC_MASTER  0x0D
#define ICW4_AT_AEOI_SLAVE   0x03
#define ICW4_AT_AEOI_MASTER  0x07
#define ICW4_PC_AEOI_SLAVE   0x0B
#define ICW4_PC_AEOI_MASTER  0x0F

int intr_init(const int auto_eoi)
{
      outb( INT_CTL, ICW1_AT);
      outb( INT_CTLMASK, IRQ0_VECTOR);
      outb( INT_CTLMASK, (1 << CASCADE_IRQ));
      if (auto_eoi)
          outb( INT_CTLMASK, ICW4_AT_AEOI_MASTER);
      else
          outb( INT_CTLMASK, ICW4_AT_MASTER);
      outb( INT_CTLMASK, ~(1 << CASCADE_IRQ));
      outb( INT2_CTL, ICW1_AT);
      outb( INT2_CTLMASK, IRQ8_VECTOR);
      outb( INT2_CTLMASK, CASCADE_IRQ);
      if (auto_eoi)
         outb( INT2_CTLMASK, ICW4_AT_AEOI_SLAVE);
      else
         outb( INT2_CTLMASK, ICW4_AT_SLAVE);
      outb( INT2_CTLMASK, ~0);

  return OK;
}

void irq_8259_unmask(const int irq)
{
	const unsigned ctl_mask = irq < 8 ? INT_CTLMASK : INT2_CTLMASK;
	outb(ctl_mask, inb(ctl_mask) & ~(1 << (irq & 0x7)));
}

void irq_8259_mask(const int irq)
{
	const unsigned ctl_mask = irq < 8 ? INT_CTLMASK : INT2_CTLMASK;
	outb(ctl_mask, inb(ctl_mask) | (1 << (irq & 0x7)));
}

void i8259_disable(void)
{
	outb(INT2_CTLMASK, 0xFF);
	outb(INT_CTLMASK, 0xFF);
	inb(INT_CTLMASK);
}

void irq_8259_eoi(int irq)
{
	if (irq < 8)
		eoi_8259_master();
	else
		eoi_8259_slave();
}

/*
 * hw_intr_* — Hardware interrupt wrappers called from kernel/interrupt.c.
 *
 * On x86_64 without APIC (legacy PIC mode), these delegate to the 8259
 * PIC driver functions. With APIC, they would delegate to I/O APIC
 * functions (not yet implemented for x86_64).
 *
 * hw_intr_used/hw_intr_not_used are no-ops in non-APIC mode (matching
 * the i386 non-APIC pattern).
 */

void hw_intr_mask(int irq)
{
	irq_8259_mask(irq);
}

void hw_intr_unmask(int irq)
{
	irq_8259_unmask(irq);
}

void hw_intr_ack(int irq)
{
	irq_8259_eoi(irq);
}

void hw_intr_used(int irq)
{
	/* No special action in PIC mode — hw_intr_unmask handles enable. */
}

void hw_intr_not_used(int irq)
{
	/* No special action — hw_intr_mask handles disable. */
}

void hw_intr_disable_all(void)
{
	/* No special action in PIC mode. */
}
