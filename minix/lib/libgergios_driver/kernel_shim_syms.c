/* kernel_shim_syms.c — Kernel API Shim: Host Symbol Table for ELF Loader
 *
 * This file defines the host symbol table that the ELF .ko loader uses to
 * resolve external symbols (kmalloc, printk, pci_register_driver, ioremap,
 * etc.) when loading a Linux kernel module (.ko) into the GergiOS LKM
 * compatibility layer.
 *
 * Each entry maps a Linux kernel API function name to its address in the
 * GergiOS/MINIX shim library (kernel_shim.c).  The table is modelled after
 * Linux's EXPORT_SYMBOL/EXPORT_SYMBOL_GPL mechanism: GPL-only symbols
 * are marked with gpl_only=1.
 *
 * Usage:
 *   In the Driver Manager LKM loader:
 *     extern const struct elf_host_symbol gergios_kernel_syms[];
 *     extern const size_t gergios_kernel_nsyms;
 *
 *     elf_load_buffer(ko_data, ko_size,
 *         gergios_kernel_syms, gergios_kernel_nsyms, &mod);
 */

#include "elf_loader.h"
#include "kernel_shim.h"

/*===========================================================================*
 *		Helper macro: declare an exported symbol                  *
 *===========================================================================*/

/* GPL-only symbol access: only modules with GPL-compatible license
 * can resolve these.  Unrestricted symbols are available to any module.
 * The access is enforced by the ELF loader's GPL gate. */
#define EXPORT_SYMBOL(name)       { #name, (void *)(name), 0, 0 }
#define EXPORT_SYMBOL_GPL(name)   { #name, (void *)(name), 0, 1 }

/*===========================================================================*
 *		Host symbol table                                        *
 *===========================================================================*/

const struct elf_host_symbol gergios_kernel_syms[] = {

	/*--- Memory allocation ---*/
	EXPORT_SYMBOL(kmalloc),
	EXPORT_SYMBOL(kzalloc),
	EXPORT_SYMBOL(kfree),
	EXPORT_SYMBOL(kcalloc),
	EXPORT_SYMBOL(krealloc),
	EXPORT_SYMBOL(vzalloc),
	EXPORT_SYMBOL(vfree),

	/*--- PCI subsystem ---*/
	EXPORT_SYMBOL(__pci_register_driver),
	EXPORT_SYMBOL(pci_unregister_driver),
	EXPORT_SYMBOL_GPL(pci_enable_device),
	EXPORT_SYMBOL_GPL(pci_disable_device),
	EXPORT_SYMBOL_GPL(pci_set_master),
	EXPORT_SYMBOL_GPL(pci_iomap),
	EXPORT_SYMBOL_GPL(pci_iounmap),
	EXPORT_SYMBOL_GPL(pci_read_config_byte),
	EXPORT_SYMBOL_GPL(pci_read_config_word),
	EXPORT_SYMBOL_GPL(pci_read_config_dword),
	EXPORT_SYMBOL_GPL(pci_write_config_byte),
	EXPORT_SYMBOL_GPL(pci_write_config_word),
	EXPORT_SYMBOL_GPL(pci_write_config_dword),
	EXPORT_SYMBOL_GPL(pci_get_device),
	EXPORT_SYMBOL_GPL(pci_dev_put),
	EXPORT_SYMBOL_GPL(pci_request_region),
	EXPORT_SYMBOL_GPL(pci_release_region),
	EXPORT_SYMBOL_GPL(pci_set_dma_mask),
	EXPORT_SYMBOL_GPL(pci_set_consistent_dma_mask),
	EXPORT_SYMBOL_GPL(pci_resource_start),
	EXPORT_SYMBOL_GPL(pci_resource_end),
	EXPORT_SYMBOL_GPL(pci_resource_len),
	EXPORT_SYMBOL_GPL(pci_irq_vector),

	/*--- MMIO / IO access ---*/
	EXPORT_SYMBOL_GPL(ioremap),
	EXPORT_SYMBOL_GPL(ioremap_nocache),
	EXPORT_SYMBOL_GPL(iounmap),

	/*--- Interrupts ---*/
	EXPORT_SYMBOL_GPL(request_irq),
	EXPORT_SYMBOL_GPL(request_threaded_irq),
	EXPORT_SYMBOL_GPL(free_irq),
	EXPORT_SYMBOL_GPL(disable_irq),
	EXPORT_SYMBOL_GPL(disable_irq_nosync),
	EXPORT_SYMBOL_GPL(enable_irq),
	EXPORT_SYMBOL_GPL(synchronize_irq),

	/*--- DMA API ---*/
	EXPORT_SYMBOL_GPL(dma_alloc_coherent),
	EXPORT_SYMBOL_GPL(dma_free_coherent),
	EXPORT_SYMBOL_GPL(dma_map_single),
	EXPORT_SYMBOL_GPL(dma_unmap_single),
	EXPORT_SYMBOL_GPL(dma_set_mask),
	EXPORT_SYMBOL_GPL(dma_set_coherent_mask),
	EXPORT_SYMBOL_GPL(dma_sync_single_for_cpu),
	EXPORT_SYMBOL_GPL(dma_sync_single_for_device),

	/*--- Timers and delays ---*/
	EXPORT_SYMBOL(mdelay),
	EXPORT_SYMBOL(udelay),
	EXPORT_SYMBOL(msleep),
	EXPORT_SYMBOL(ssleep),
	EXPORT_SYMBOL(msecs_to_jiffies),
	EXPORT_SYMBOL(jiffies_to_msecs),
	EXPORT_SYMBOL(usecs_to_jiffies),
	EXPORT_SYMBOL(jiffies_to_usecs),
	EXPORT_SYMBOL(get_jiffies_64),
	EXPORT_SYMBOL_GPL(jiffies),

	/*--- Timer API ---*/
	EXPORT_SYMBOL(init_timer),
	EXPORT_SYMBOL(timer_setup),
	EXPORT_SYMBOL(mod_timer),
	EXPORT_SYMBOL(del_timer),
	EXPORT_SYMBOL(del_timer_sync),
	EXPORT_SYMBOL(timer_pending),
	EXPORT_SYMBOL(add_timer),

	/*--- Workqueues ---*/
	EXPORT_SYMBOL(schedule_work),
	EXPORT_SYMBOL(schedule_delayed_work),
	EXPORT_SYMBOL(flush_work),
	EXPORT_SYMBOL(cancel_work_sync),
	EXPORT_SYMBOL(cancel_delayed_work),
	EXPORT_SYMBOL(cancel_delayed_work_sync),
	EXPORT_SYMBOL(flush_scheduled_work),

	/*--- Printk / logging ---*/
	EXPORT_SYMBOL(printk),
	EXPORT_SYMBOL(dev_printk),

	/*--- Firmware ---*/
	EXPORT_SYMBOL_GPL(request_firmware),
	EXPORT_SYMBOL_GPL(request_firmware_nowait),
	EXPORT_SYMBOL_GPL(release_firmware),

	/*--- Wait queues / completion ---*/
	EXPORT_SYMBOL(wait_for_completion),
	EXPORT_SYMBOL(wait_for_completion_timeout),
	EXPORT_SYMBOL(complete),
	EXPORT_SYMBOL(complete_all),

	/*--- Misc ---*/
	EXPORT_SYMBOL(get_random_bytes),
	EXPORT_SYMBOL(dump_stack),

	/*--- Network / sk_buff ---*/
	EXPORT_SYMBOL(dev_alloc_skb),
	EXPORT_SYMBOL(dev_kfree_skb),
	EXPORT_SYMBOL(kfree_skb),
	EXPORT_SYMBOL(skb_put),
	EXPORT_SYMBOL(skb_push),
	EXPORT_SYMBOL(skb_pull),
	EXPORT_SYMBOL(skb_reserve),
	EXPORT_SYMBOL(skb_trim),
	EXPORT_SYMBOL(skb_clone),
	EXPORT_SYMBOL(skb_copy),

	/*--- LKM integration ---*/
	EXPORT_SYMBOL(klkm_init),
	EXPORT_SYMBOL(klkm_exit),
	EXPORT_SYMBOL(klkm_irq_dispatch),
	EXPORT_SYMBOL(klkm_timer_dispatch),

	/* Sentinel (end-of-table marker: name == NULL) */
	{ NULL, NULL, 0, 0 },
};

const size_t gergios_kernel_nsyms =
    (sizeof(gergios_kernel_syms) / sizeof(gergios_kernel_syms[0])) - 1;
