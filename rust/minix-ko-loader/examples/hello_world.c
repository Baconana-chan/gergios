/*
 * Minimal "Hello, World" Linux kernel module for GergiOS LKM compat.
 *
 * Compile on any x86_64 Linux host with kernel headers:
 *   make -C /lib/modules/$(uname -r)/build M=$(pwd) modules
 *
 * The resulting hello_world.ko can be loaded by the GergiOS Driver Manager
 * via the Rust `minix_ko_loader` crate.
 *
 * Architecture: x86_64
 * Format:       ELF64 relocatable (.ko)
 */

#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/init.h>

MODULE_LICENSE("GPL");
MODULE_AUTHOR("GergiOS Project");
MODULE_DESCRIPTION("Minimal kernel module for GergiOS LKM compat testing");
MODULE_VERSION("0.1");

/* Module parameters (parsable via depmod) */
static int debug = 0;
module_param(debug, int, 0644);
MODULE_PARM_DESC(debug, "Enable debug output (0=off, 1=on)");

static int __init hello_init(void)
{
    printk(KERN_INFO "hello_world: Hello from GergiOS LKM compat layer!\n");
    printk(KERN_INFO "hello_world: debug=%d\n", debug);
    return 0;   /* Success */
}

static void __exit hello_exit(void)
{
    printk(KERN_INFO "hello_world: Goodbye from GergiOS!\n");
}

module_init(hello_init);
module_exit(hello_exit);
