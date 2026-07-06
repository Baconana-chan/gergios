/* kernel_shim.h — Linux Kernel API Shim for GergiOS LKM Compat Layer
 *
 * Translates ~50 Linux kernel API functions to GergiOS/MINIX equivalents
 * for use by LKM (.ko) drivers loaded via the ELF loader.  The shim
 * functions are compiled into libgergios_driver and registered as
 * EXPORT_SYMBOL entries in the host symbol table for the ELF loader.
 *
 * Thread safety: MINIX userspace drivers are single-threaded (message-
 * driven).  Synchronisation primitives (spinlocks, mutexes) are no-ops.
 * Workqueues execute callbacks synchronously.
 */

#ifndef _GERGIOS_KERNEL_SHIM_H
#define _GERGIOS_KERNEL_SHIM_H

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>

/*===========================================================================*
 *		Kernel types (subset of Linux <linux/types.h>)             *
 *===========================================================================*/

typedef uint8_t  u8;
typedef uint16_t u16;
typedef uint32_t u32;
typedef uint64_t u64;
typedef int8_t   s8;
typedef int16_t  s16;
typedef int32_t  s32;
typedef int64_t  s64;

typedef uint8_t  __u8;
typedef uint16_t __u16;
typedef uint32_t __u32;
typedef uint64_t __u64;

typedef unsigned long ulong;
typedef unsigned int  uint;
typedef unsigned long resource_size_t;
typedef unsigned long phys_addr_t;
typedef unsigned long dma_addr_t;

/* GFP flags (simplified — all memory is GFP_KERNEL) */
#define GFP_KERNEL     0x0000u
#define GFP_ATOMIC     0x0000u
#define GFP_DMA        0x0001u
#define GFP_KERNEL_ACCOUNT 0x0002u
#define __GFP_ZERO     0x8000u
#define GFP_ZERO       __GFP_ZERO

/*===========================================================================*
 *		Attribute macros                                            *
 *===========================================================================*/

#define __init
#define __exit
#define __initdata
#define __devinit
#define __devexit
#define __maybe_unused  __attribute__((__unused__))
#define __always_unused __attribute__((__unused__))
#define __must_check    __attribute__((__warn_unused_result__))
#define __user
#define __iomem
#define __force
#define __packed        __attribute__((__packed__))
#define __aligned(n)    __attribute__((__aligned__(n)))
#define __printf(a, b)  __attribute__((__format__(printf, a, b)))
#define likely(x)       __builtin_expect(!!(x), 1)
#define unlikely(x)     __builtin_expect(!!(x), 0)

/* Barrier macros */
#define barrier()       __asm__ __volatile__("": : :"memory")
#define mb()            __asm__ __volatile__("mfence" ::: "memory")
#define rmb()           __asm__ __volatile__("lfence" ::: "memory")
#define wmb()           __asm__ __volatile__("sfence" ::: "memory")

/*===========================================================================*
 *		Utility macros                                              *
 *===========================================================================*/

#define ARRAY_SIZE(arr)     (sizeof(arr) / sizeof((arr)[0]))
#define container_of(ptr, type, member) \
	((type *)((char *)(ptr) - offsetof(type, member)))
#define min(x, y)           ({ \
	typeof(x) _x = (x); typeof(y) _y = (y); _x < _y ? _x : _y; })
#define max(x, y)           ({ \
	typeof(x) _x = (x); typeof(y) _y = (y); _x > _y ? _x : _y; })
#define clamp(val, lo, hi)  min(max(val, lo), hi)
#define roundup(x, y)       ({ \
	typeof(y) _y = (y); ((x) + _y - 1) / _y * _y; })
#define rounddown(x, y)     ({ \
	typeof(x) _x = (x); typeof(y) _y = (y); _x / _y * _y; })
#define DIV_ROUND_UP(n, d)  (((n) + (d) - 1) / (d))
#define DIV_ROUND_CLOSEST(n, d) ({ \
	typeof(n) _n = (n); typeof(d) _d = (d); \
	((_n + _d / 2) / _d); })

/* IS_ERR / PTR_ERR / ERR_PTR */
#define IS_ERR_VALUE(x)     ((unsigned long)(void *)(x) >= (unsigned long)-4095)
#define IS_ERR(ptr)         IS_ERR_VALUE((unsigned long)(ptr))
#define PTR_ERR(ptr)        ((int)(long)(ptr))
#define ERR_PTR(err)        ((void *)(long)(err))
#define IS_ERR_OR_NULL(ptr) (!(ptr) || IS_ERR(ptr))

/*===========================================================================*
 *		PCI types (subset of Linux <linux/pci.h>)                  *
 *===========================================================================*/

#define PCI_ANY_ID       (~0U)
#define PCI_VENDOR_ID    (~0U)

struct pci_device_id {
	__u32 vendor, device;           /* Vendor/device ID or PCI_ANY_ID */
	__u32 subvendor, subdevice;     /* Subsystem ID or PCI_ANY_ID */
	__u32 class, class_mask;        /* Class code and mask */
	unsigned long driver_data;      /* Private driver data */
};

struct resource {
	resource_size_t start;
	resource_size_t end;
	const char *name;
	unsigned long flags;
};

#define IORESOURCE_IO       0x00000100
#define IORESOURCE_MEM      0x00000200
#define IORESOURCE_IRQ      0x00000400
#define IORESOURCE_PREFETCH 0x00001000

/* PCI BAR indices */
#define PCI_STD_RESOURCES       0
#define PCI_STD_RESOURCE_END    5

struct pci_dev {
	u32 vendor, device;
	u32 subsystem_vendor, subsystem_device;
	u32 class;
	u8  revision;
	u8  devfn;          /* bit 7:3 = device, bit 2:0 = function */
	unsigned int bus;
	unsigned int irq;
	struct resource resource[6];
	struct pci_driver *driver;
	void *driver_data;  /* per-device private data */
	int devind;         /* MINIX PCI device index */
	void *mmio_base;    /* Cached ioremap base for BAR 0 */
	void *mmio_base2;   /* Cached ioremap base for BAR 1 (if any) */
};

struct pci_driver {
	const char *name;
	const struct pci_device_id *id_table;
	int (*probe)(struct pci_dev *dev, const struct pci_device_id *id);
	void (*remove)(struct pci_dev *dev);
	int (*suspend)(struct pci_dev *dev, int state);
	int (*resume)(struct pci_dev *dev);
};

/* pm_message_t (simplified) */
typedef u32 pm_message_t;

/*===========================================================================*
 *		DMA types                                                   *
 *===========================================================================*/

enum dma_data_direction {
	DMA_BIDIRECTIONAL   = 0,
	DMA_TO_DEVICE       = 1,
	DMA_FROM_DEVICE     = 2,
	DMA_NONE            = 3,
};

struct device {
	void *driver_data;
};

/*===========================================================================*
 *		IRQ / interrupt types                                      *
 *===========================================================================*/

typedef int irqreturn_t;
#define IRQ_NONE        ((irqreturn_t)0)
#define IRQ_HANDLED     ((irqreturn_t)1)
#define IRQ_WAKE_THREAD ((irqreturn_t)2)

typedef irqreturn_t (*irq_handler_t)(int, void *);

#define IRQF_SHARED         0x00000001
#define IRQF_TRIGGER_NONE   0x00000000
#define IRQF_TRIGGER_RISING 0x00000002
#define IRQF_TRIGGER_FALLING 0x00000004
#define IRQF_TRIGGER_HIGH   0x00000008
#define IRQF_TRIGGER_LOW    0x00000010
#define IRQF_ONESHOT        0x00000020
#define IRQF_NO_SUSPEND     0x00000040
#define IRQF_NO_THREAD      0x00000080
#define IRQF_DISABLED       0x00000100

/*===========================================================================*
 *		Timer types (Linux <linux/timer.h> subset)                 *
 *===========================================================================*/

struct timer_list {
	void (*function)(unsigned long);
	unsigned long data;
	unsigned long expires;
	int active;         /* internal: 1 if timer is pending */
};

/*===========================================================================*
 *		Workqueue types                                             *
 *===========================================================================*/

typedef void (*work_func_t)(void *);

struct work_struct {
	work_func_t func;
	void *data;
	int pending;
};

struct delayed_work {
	struct work_struct work;
	unsigned long delay;
};

/*===========================================================================*
 *		Wait queue / completion types                               *
 *===========================================================================*/

struct completion {
	unsigned int done;
};

/*===========================================================================*
 *		Firmware types                                              *
 *===========================================================================*/

struct firmware {
	size_t size;
	const u8 *data;
};

/*===========================================================================*
 *		sk_buff (network buffer, simplified)                      *
 *===========================================================================*/

struct sk_buff {
	unsigned char *data;
	unsigned char *head;
	unsigned char *tail;
	unsigned char *end;
	unsigned int len;
	unsigned int truesize;
	unsigned int users;
};

/*===========================================================================*
 *		Module macros (for .modinfo parsing)                       *
 *===========================================================================*/

#define MODULE_LICENSE(license)     static const char *__module_license __attribute__((unused)) = license
#define MODULE_AUTHOR(author)       static const char *__module_author __attribute__((unused)) = author
#define MODULE_DESCRIPTION(desc)    static const char *__module_desc __attribute__((unused)) = desc
#define MODULE_VERSION(ver)         static const char *__module_ver __attribute__((unused)) = ver
#define MODULE_FIRMWARE(fw)         static const char *__module_fw __attribute__((unused)) = fw

#define module_init(fn)             int __module_init(void) { return fn(); }
#define module_exit(fn)             void __module_exit(void) { fn(); }

/*===========================================================================*
 *		Kernel API function declarations                          *
 *===========================================================================*/

/* --- Memory -------------------------------------------------------------- */
void *kmalloc(size_t size, int gfp_flags);
void *kzalloc(size_t size, int gfp_flags);
void  kfree(const void *ptr);
void *kcalloc(size_t n, size_t size, int gfp_flags);
void *krealloc(void *ptr, size_t size, int gfp_flags);
void *vzalloc(size_t size);
void  vfree(const void *addr);

/* --- PCI ----------------------------------------------------------------- */
int   __pci_register_driver(struct pci_driver *drv, const char *owner);
void  pci_unregister_driver(struct pci_driver *drv);
int   pci_enable_device(struct pci_dev *dev);
void  pci_disable_device(struct pci_dev *dev);
void  pci_set_master(struct pci_dev *dev);
void *pci_iomap(struct pci_dev *dev, int bar, unsigned long maxlen);
void  pci_iounmap(struct pci_dev *dev, void *addr);
int   pci_read_config_byte(const struct pci_dev *dev, int where, u8 *val);
int   pci_read_config_word(const struct pci_dev *dev, int where, u16 *val);
int   pci_read_config_dword(const struct pci_dev *dev, int where, u32 *val);
int   pci_write_config_byte(const struct pci_dev *dev, int where, u8 val);
int   pci_write_config_word(const struct pci_dev *dev, int where, u16 val);
int   pci_write_config_dword(const struct pci_dev *dev, int where, u32 val);
struct pci_dev *pci_get_device(unsigned int vendor, unsigned int device,
                               struct pci_dev *from);
void  pci_dev_put(struct pci_dev *dev);
int   pci_request_region(struct pci_dev *dev, int bar, const char *name);
void  pci_release_region(struct pci_dev *dev, int bar);
int   pci_set_dma_mask(struct pci_dev *dev, u64 mask);
int   pci_set_consistent_dma_mask(struct pci_dev *dev, u64 mask);
resource_size_t pci_resource_start(const struct pci_dev *dev, int bar);
resource_size_t pci_resource_end(const struct pci_dev *dev, int bar);
resource_size_t pci_resource_len(const struct pci_dev *dev, int bar);
unsigned int pci_irq_vector(const struct pci_dev *dev, unsigned int nr);

/* Helper for pci_register_driver */
#define pci_register_driver(drv) __pci_register_driver(drv, "gergios")
#define module_pci_driver(drv) \
	module_init(__initfn_##drv) \
	module_exit(__exitfn_##drv)

/* --- MMIO/IO ------------------------------------------------------------- */
void __iomem *ioremap(phys_addr_t phys_addr, unsigned long size);
void __iomem *ioremap_nocache(phys_addr_t phys_addr, unsigned long size);
void  iounmap(void __iomem *addr);

static inline u32 ioread32(const void __iomem *addr)
	{ return *(const volatile u32 *)addr; }
static inline u16 ioread16(const void __iomem *addr)
	{ return *(const volatile u16 *)addr; }
static inline u8  ioread8(const void __iomem *addr)
	{ return *(const volatile u8 *)addr; }
static inline void iowrite32(u32 val, void __iomem *addr)
	{ *(volatile u32 *)addr = val; }
static inline void iowrite16(u16 val, void __iomem *addr)
	{ *(volatile u16 *)addr = val; }
static inline void iowrite8(u8 val, void __iomem *addr)
	{ *(volatile u8 *)addr = val; }

#define readl(addr)     ioread32(addr)
#define readw(addr)     ioread16(addr)
#define readb(addr)     ioread8(addr)
#define writel(val, addr) iowrite32(val, addr)
#define writew(val, addr) iowrite16(val, addr)
#define writeb(val, addr) iowrite8(val, addr)

/* --- IRQ ----------------------------------------------------------------- */
int request_irq(unsigned int irq, irq_handler_t handler,
                unsigned long flags, const char *name, void *dev);
int request_threaded_irq(unsigned int irq, irq_handler_t handler,
                         irq_handler_t thread_fn, unsigned long flags,
                         const char *name, void *dev);
void free_irq(unsigned int irq, void *dev);
void disable_irq(unsigned int irq);
void disable_irq_nosync(unsigned int irq);
void enable_irq(unsigned int irq);
void synchronize_irq(unsigned int irq);

/* --- DMA ----------------------------------------------------------------- */
void *dma_alloc_coherent(struct device *dev, size_t size,
                         dma_addr_t *dma_handle, int gfp);
void  dma_free_coherent(struct device *dev, size_t size,
                        void *cpu_addr, dma_addr_t dma_handle);
dma_addr_t dma_map_single(struct device *dev, void *cpu_addr,
                          size_t size, enum dma_data_direction dir);
void  dma_unmap_single(struct device *dev, dma_addr_t dma_addr,
                       size_t size, enum dma_data_direction dir);
int   dma_set_mask(struct device *dev, u64 mask);
int   dma_set_coherent_mask(struct device *dev, u64 mask);
void  dma_sync_single_for_cpu(struct device *dev, dma_addr_t dma_addr,
                              size_t size, enum dma_data_direction dir);
void  dma_sync_single_for_device(struct device *dev, dma_addr_t dma_addr,
                                 size_t size, enum dma_data_direction dir);

/* --- Timers / delays ----------------------------------------------------- */
void mdelay(unsigned long msecs);
void udelay(unsigned long usecs);
void msleep(unsigned int msecs);
void ssleep(unsigned int seconds);
unsigned long msecs_to_jiffies(const unsigned int m);
unsigned int  jiffies_to_msecs(const unsigned long j);
unsigned long usecs_to_jiffies(const unsigned int u);
unsigned int  jiffies_to_usecs(const unsigned long j);
extern unsigned long volatile jiffies;
unsigned long get_jiffies_64(void);

/* Timer API */
void init_timer(struct timer_list *timer);
void timer_setup(struct timer_list *timer,
                 void (*callback)(unsigned long), unsigned long data);
int  mod_timer(struct timer_list *timer, unsigned long expires);
int  del_timer(struct timer_list *timer);
int  del_timer_sync(struct timer_list *timer);
int  timer_pending(const struct timer_list *timer);
void add_timer(struct timer_list *timer);

/* --- Workqueues ---------------------------------------------------------- */
int  schedule_work(struct work_struct *work);
int  schedule_delayed_work(struct delayed_work *dwork,
                           unsigned long delay);
void flush_work(struct work_struct *work);
int  cancel_work_sync(struct work_struct *work);
int  cancel_delayed_work(struct delayed_work *dwork);
int  cancel_delayed_work_sync(struct delayed_work *dwork);
void flush_scheduled_work(void);

#define INIT_WORK(_work, _func) do { \
	(_work)->func = (_func); \
	(_work)->data = NULL; \
	(_work)->pending = 0; \
} while (0)

#define INIT_DELAYED_WORK(_dwork, _func) do { \
	INIT_WORK(&(_dwork)->work, (_func)); \
	(_dwork)->delay = 0; \
} while (0)

/* --- Printk / logging ---------------------------------------------------- */
int __printf(1, 2) printk(const char *fmt, ...);
int __printf(2, 3) dev_printk(const char *level, const struct device *dev,
                              const char *fmt, ...);

/* KERN_* level prefixes (stripped by printk shim) */
#define KERN_SOH        "\001"
#define KERN_SOH_ASCII  '\001'
#define KERN_EMERG      KERN_SOH "0"
#define KERN_ALERT      KERN_SOH "1"
#define KERN_CRIT       KERN_SOH "2"
#define KERN_ERR        KERN_SOH "3"
#define KERN_WARNING    KERN_SOH "4"
#define KERN_NOTICE     KERN_SOH "5"
#define KERN_INFO       KERN_SOH "6"
#define KERN_DEBUG      KERN_SOH "7"
#define KERN_DEFAULT    ""
#define KERN_CONT       ""

#define pr_emerg(fmt, ...)    printk(KERN_EMERG fmt, ##__VA_ARGS__)
#define pr_alert(fmt, ...)    printk(KERN_ALERT fmt, ##__VA_ARGS__)
#define pr_crit(fmt, ...)     printk(KERN_CRIT fmt, ##__VA_ARGS__)
#define pr_err(fmt, ...)      printk(KERN_ERR fmt, ##__VA_ARGS__)
#define pr_warn(fmt, ...)     printk(KERN_WARNING fmt, ##__VA_ARGS__)
#define pr_notice(fmt, ...)   printk(KERN_NOTICE fmt, ##__VA_ARGS__)
#define pr_info(fmt, ...)     printk(KERN_INFO fmt, ##__VA_ARGS__)
#define pr_debug(fmt, ...)    printk(KERN_DEBUG fmt, ##__VA_ARGS__)

#define dev_emerg(dev, fmt, ...)   dev_printk(KERN_EMERG, dev, fmt, ##__VA_ARGS__)
#define dev_alert(dev, fmt, ...)   dev_printk(KERN_ALERT, dev, fmt, ##__VA_ARGS__)
#define dev_crit(dev, fmt, ...)    dev_printk(KERN_CRIT, dev, fmt, ##__VA_ARGS__)
#define dev_err(dev, fmt, ...)     dev_printk(KERN_ERR, dev, fmt, ##__VA_ARGS__)
#define dev_warn(dev, fmt, ...)    dev_printk(KERN_WARNING, dev, fmt, ##__VA_ARGS__)
#define dev_notice(dev, fmt, ...)  dev_printk(KERN_NOTICE, dev, fmt, ##__VA_ARGS__)
#define dev_info(dev, fmt, ...)    dev_printk(KERN_INFO, dev, fmt, ##__VA_ARGS__)
#define dev_dbg(dev, fmt, ...)     dev_printk(KERN_DEBUG, dev, fmt, ##__VA_ARGS__)

/* Firmware callback type for request_firmware_nowait */
typedef void (*firmware_cb_t)(const struct firmware *fw, void *context);

/* --- Firmware ------------------------------------------------------------ */
int request_firmware(const struct firmware **fw, const char *name,
                     struct device *device);
int request_firmware_nowait(const char *name, struct device *device,
                            firmware_cb_t callback, void *context);
void release_firmware(const struct firmware *fw);

/* --- Wait queues / completion -------------------------------------------- */
#define init_completion(x)      do { (x)->done = 0; } while (0)
#define reinit_completion(x)    do { (x)->done = 0; } while (0)
void wait_for_completion(struct completion *c);
unsigned long wait_for_completion_timeout(struct completion *c,
                                          unsigned long timeout);
void complete(struct completion *c);
void complete_all(struct completion *c);

/* --- Device model -------------------------------------------------------- */
static inline void dev_set_drvdata(struct device *dev, void *data)
	{ dev->driver_data = data; }
static inline void *dev_get_drvdata(const struct device *dev)
	{ return dev->driver_data; }

/* --- Misc ---------------------------------------------------------------- */
void get_random_bytes(void *buf, int len);
void dump_stack(void);

#define do_div(n, base)         ({ \
	unsigned long __base = (base); \
	unsigned long __rem = (n) % __base; \
	(n) /= __base; \
	__rem; })

/* BUG / WARN */
#define BUG()                   do { \
	fprintf(stderr, "BUG: at %s:%d\n", __FILE__, __LINE__); \
	abort(); \
} while (0)
#define BUG_ON(cond)            do { \
	if (unlikely(cond)) { \
		fprintf(stderr, "BUG_ON(%s) at %s:%d\n", \
			#cond, __FILE__, __LINE__); \
		abort(); \
	} \
} while (0)
#define WARN_ON(cond)           ({ \
	int __cond = !!(cond); \
	if (unlikely(__cond)) \
		fprintf(stderr, "WARN_ON(%s) at %s:%d\n", \
			#cond, __FILE__, __LINE__); \
	__cond; })
#define WARN(cond, fmt, ...)    ({ \
	int __cond = !!(cond); \
	if (unlikely(__cond)) \
		fprintf(stderr, "WARN: " fmt " at %s:%d\n", \
			##__VA_ARGS__, __FILE__, __LINE__); \
	__cond; })

/* Endianness (x86_64 is LE, so these are identity) */
#define cpu_to_le32(v)  ((u32)(v))
#define cpu_to_le16(v)  ((u16)(v))
#define le32_to_cpu(v)  ((u32)(v))
#define le16_to_cpu(v)  ((u16)(v))
#define cpu_to_be32(v)  (__builtin_bswap32(v))
#define cpu_to_be16(v)  (__builtin_bswap16(v))
#define be32_to_cpu(v)  (__builtin_bswap32(v))
#define be16_to_cpu(v)  (__builtin_bswap16(v))
#define cpu_to_le64(v)  ((u64)(v))
#define le64_to_cpu(v)  ((u64)(v))
#define cpu_to_be64(v)  (__builtin_bswap64(v))
#define be64_to_cpu(v)  (__builtin_bswap64(v))

/* put_unaligned / get_unaligned */
#define get_unaligned(ptr)      (*(ptr))
#define put_unaligned(val, ptr) (*(ptr) = (val))

/*===========================================================================*
 *		sk_buff API (simplified, for wireless drivers)            *
 *===========================================================================*/

struct sk_buff *dev_alloc_skb(unsigned int length);
void           dev_kfree_skb(struct sk_buff *skb);
unsigned char *skb_put(struct sk_buff *skb, unsigned int len);
unsigned char *skb_push(struct sk_buff *skb, unsigned int len);
unsigned char *skb_pull(struct sk_buff *skb, unsigned int len);
void           skb_reserve(struct sk_buff *skb, unsigned int len);
void           skb_trim(struct sk_buff *skb, unsigned int len);
struct sk_buff *skb_clone(struct sk_buff *skb, int gfp_mask);
struct sk_buff *skb_copy(struct sk_buff *skb, int gfp_mask);
void           kfree_skb(struct sk_buff *skb);

/*===========================================================================*
 *		LKM Shim integration — for main-loop dispatch           *
 *===========================================================================*/

/* Called by the driver's main loop when a HARDWARE notification arrives.
 * Dispatches to registered IRQ handlers. */
void klkm_irq_dispatch(unsigned int mask);

/* Called by the driver's main loop when a CLOCK notification arrives.
 * Fires expired timers. */
void klkm_timer_dispatch(clock_t stamp);

/* Initialise the kernel shim subsystem (called once at driver init). */
void klkm_init(void);

/* Clean up the kernel shim subsystem (called at driver exit). */
void klkm_exit(void);

#endif /* _GERGIOS_KERNEL_SHIM_H */
