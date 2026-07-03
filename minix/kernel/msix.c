/* MSI-X IRQ allocator implementation.
 *
 * Maintains a bitmap of MSI-X IRQ slots (NR_MSIX_IRQS = 32). Each bit
 * corresponds to an IRQ in the range [MSIX_IRQ_BASE, MSIX_IRQ_BASE+32).
 *
 * Allocation is performed under kernel lock (BKL), so no additional
 * synchronization is needed.
 */

#include "msix.h"

/* The MSI-X IRQ allocation bitmap. */
static unsigned msix_bitmap[MSIX_BITMAP_WORDS];

/*===========================================================================*
 *				msix_alloc_irq				     *
 *===========================================================================*/
int msix_alloc_irq(void)
{
	int i;

	for (i = 0; i < NR_MSIX_IRQS; i++) {
		int word = i / 32;
		int bit  = i % 32;

		if (!(msix_bitmap[word] & (1u << bit))) {
			msix_bitmap[word] |= (1u << bit);
			return MSIX_IRQ(i);
		}
	}

	return -1;  /* no free MSI-X IRQs */
}

/*===========================================================================*
 *				msix_free_irq				     *
 *===========================================================================*/
void msix_free_irq(int irq)
{
	int idx = irq - MSIX_IRQ_BASE;

	if (idx < 0 || idx >= NR_MSIX_IRQS)
		return;

	msix_bitmap[idx / 32] &= ~(1u << (idx % 32));
}

/*===========================================================================*
 *				msix_is_allocated			     *
 *===========================================================================*/
int msix_is_allocated(int irq)
{
	int idx = irq - MSIX_IRQ_BASE;

	if (idx < 0 || idx >= NR_MSIX_IRQS)
		return 0;

	return (msix_bitmap[idx / 32] >> (idx % 32)) & 1;
}
