/* MAC (Mandatory Access Control) Framework
 *
 * This header defines the MAC API for MINIX. It provides:
 *   - Hook types for securable operations (IPC, file access, etc.)
 *   - Context structures passed between hooks and policy daemon
 *   - Kernel hook registration and invocation
 *   - Userspace libmac API for server-side checks
 *
 * Architecture:
 *   Kernel hooks   → mac_kernel_check() calls registered hook(s)
 *   Userspace hooks → mac_request() sends IPC to macd daemon
 *
 * Multiple MAC modules can be composed via hook chains.
 * By default, all operations are ALLOWed (compatible mode).
 * The macd daemon implements the system-wide security policy.
 */

#ifndef _MINIX_MAC_H
#define _MINIX_MAC_H

#include <sys/types.h>
#include <minix/endpoint.h>
#include <stdint.h>

/*===========================================================================*
 *                    MAC Hook Types (what is being checked)                *
 *===========================================================================*/

#define MAC_IPC_SEND		1	/* process sending IPC message */
#define MAC_IPC_RECEIVE		2	/* process receiving IPC message */
#define MAC_FILE_ACCESS		3	/* file open/read/write/execute */
#define MAC_PRIVCTL_SET_SYS	4	/* setting system privileges */
#define MAC_DEVICE_BIND		5	/* device driver binding */
#define MAC_PROC_FORK		6	/* process fork */
#define MAC_PROC_EXEC		7	/* process exec */
#define MAC_PROC_KILL		8	/* signal delivery */
#define MAC_RAWIO		9	/* raw I/O access */

/*===========================================================================*
 *                    MAC Result Codes                                       *
 *===========================================================================*/

#define MAC_ALLOW	0	/* operation permitted */
#define MAC_DENY	(-1)	/* operation denied */

/*===========================================================================*
 *                    MAC Context Structures                                 *
 *===========================================================================*/

/* Context for IPC send/receive check */
struct mac_context_ipc {
	endpoint_t mac_src;		/* sender endpoint */
	endpoint_t mac_dst;		/* receiver endpoint */
	int mac_call_nr;		/* IPC call (SEND, SENDREC, NOTIFY, etc.) */
};

/* Context for file access check (used by VFS) */
struct mac_context_file {
	endpoint_t mac_proc;		/* requesting process */
	endpoint_t mac_fs_ep;		/* filesystem serving the file */
	uid_t mac_uid;			/* effective uid of caller */
	gid_t mac_gid;			/* effective gid of caller */
	mode_t mac_access_desired;	/* requested access (R_BIT, W_BIT, X_BIT) */
	mode_t mac_file_mode;		/* file permissions */
	uid_t mac_file_uid;		/* file owner */
	gid_t mac_file_gid;		/* file group */
};

/* Context for privilege control check */
struct mac_context_privctl {
	endpoint_t mac_caller;		/* process making the request */
	endpoint_t mac_target;		/* target process */
	int mac_request;		/* SYS_PRIV_* request type */
};

/* Context for device binding check */
struct mac_context_device {
	endpoint_t mac_driver;		/* driver endpoint */
	endpoint_t mac_devman;		/* devman endpoint */
	int mac_device_id;		/* device identifier */
	int mac_bind;			/* 1 = bind, 0 = unbind */
};

/* Context for process management checks */
struct mac_context_proc {
	endpoint_t mac_caller;		/* process performing the action */
	endpoint_t mac_target;		/* target process */
	int mac_signal;			/* signal number (for kill) */
};

/* Unified context — passed to mac_request() / mac_kernel_check() */
typedef union {
	struct mac_context_ipc	ipc;
	struct mac_context_file	file;
	struct mac_context_privctl	privctl;
	struct mac_context_device	device;
	struct mac_context_proc	proc;
} mac_context_t;

/*===========================================================================*
 *                    Kernel Hook API                                        *
 *===========================================================================*/

/* Type for a kernel-level MAC hook function.
 * Returns MAC_ALLOW or MAC_DENY.
 * what: one of the MAC_* hook types above.
 * ctx: pointer to the appropriate mac_context_* struct.
 */
typedef int (*mac_hook_fn_t)(int what, mac_context_t *ctx);

/* Register a kernel MAC hook. Multiple hooks form a chain.
 * All hooks in the chain must allow the operation for it to proceed.
 * If hook is NULL, the chain is cleared.
 */
void mac_register_kernel_hook(mac_hook_fn_t hook);

/* Invoke the MAC hook chain for a given operation.
 * Returns MAC_ALLOW if all hooks allow, MAC_DENY if any denies.
 * If no hook is registered, returns MAC_ALLOW (compatible default).
 */
int mac_kernel_check(int what, mac_context_t *ctx);

/* Initialize the MAC hook subsystem (called during kernel boot). */
void mac_hook_init(void);

/*===========================================================================*
 *                    Userspace libmac API                                   *
 *===========================================================================*/

/* Send a MAC check request to the macd policy daemon.
 * Returns MAC_ALLOW if allowed, MAC_DENY if denied,
 * or a negative errno on communication error.
 * what: one of the MAC_* hook types above.
 * ctx: pointer to the appropriate mac_context_* struct.
 */
int mac_request(int what, mac_context_t *ctx);

#endif /* _MINIX_MAC_H */
